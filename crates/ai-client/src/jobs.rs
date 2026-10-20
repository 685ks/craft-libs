//! Jobs running on worker threads, observed by `poll`.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use craft_ai_core::{AiError, ErrorKind, Event, JobId, JobSnapshot, JobState, Usage};

/// What a running job can report. Handed to the worker.
pub(crate) struct JobRecord {
    inner: Mutex<Inner>,
    cancel: AtomicBool,
}

struct Inner {
    state: JobState,
    events: Vec<Event>,
    usage: Option<Usage>,
}

impl JobRecord {
    fn new() -> Self {
        Self { inner: Mutex::new(Inner { state: JobState::Queued, events: Vec::new(), usage: None }), cancel: AtomicBool::new(false) }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // Never-crash: a panic elsewhere must not stop polls from answering.
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The cancel flag, for loops and sleeps to check.
    pub fn cancel_flag(&self) -> &AtomicBool {
        &self.cancel
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Reports progress. Progress never goes backwards and a finished job stays finished.
    pub fn running(&self, percent: Option<u8>) {
        let mut inner = self.lock();
        let next = JobState::Running { percent: percent.map(|p| p.min(99)) };
        // An illegal move (backwards, or to "unknown" after a percent) keeps the known state.
        if inner.state.can_become(&next) {
            inner.state = next;
        }
    }

    pub fn push(&self, event: Event) {
        let mut inner = self.lock();
        if !inner.state.is_terminal() {
            if matches!(inner.state, JobState::Queued) {
                inner.state = JobState::Running { percent: None };
            }
            inner.events.push(event);
        }
    }

    pub fn usage(&self, usage: Usage) {
        self.lock().usage = Some(usage);
    }

    fn finish(&self, result: Result<(), AiError>) {
        let mut inner = self.lock();
        if inner.state.is_terminal() {
            return;
        }
        inner.state = match result {
            _ if self.is_cancelled() => JobState::Cancelled,
            Ok(()) => JobState::Succeeded,
            Err(e) if e.kind == ErrorKind::Cancelled => JobState::Cancelled,
            Err(error) => JobState::Failed { error },
        };
        if inner.state == JobState::Cancelled {
            inner.events.clear();
        }
    }

    fn snapshot(&self) -> JobSnapshot {
        let mut inner = self.lock();
        if self.is_cancelled() && !inner.state.is_terminal() {
            // Cancel takes effect at once; whatever the worker still produces is dropped.
            inner.state = JobState::Cancelled;
            inner.events.clear();
        }
        let events = std::mem::take(&mut inner.events);
        let usage = if inner.state.is_terminal() { inner.usage.take() } else { inner.usage.clone() };
        JobSnapshot { state: inner.state.clone(), events, usage }
    }
}

/// The jobs one adapter started.
pub(crate) struct JobTable {
    prefix: &'static str,
    next: AtomicU64,
    jobs: Mutex<HashMap<JobId, Arc<JobRecord>>>,
}

impl JobTable {
    pub fn new(prefix: &'static str) -> Self {
        Self { prefix, next: AtomicU64::new(1), jobs: Mutex::new(HashMap::new()) }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<JobId, Arc<JobRecord>>> {
        self.jobs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Starts `work` on a new thread and returns the job's id at once.
    pub fn spawn(&self, work: impl FnOnce(&JobRecord) -> Result<(), AiError> + Send + 'static) -> Result<JobId, AiError> {
        let n = self.next.fetch_add(1, Ordering::Relaxed);
        let id = JobId::new(format!("{}-{n}", self.prefix)).map_err(|e| AiError::new(ErrorKind::Internal, e.to_string()))?;
        let record = Arc::new(JobRecord::new());
        self.lock().insert(id.clone(), Arc::clone(&record));
        let worker = Arc::clone(&record);
        let spawned = std::thread::Builder::new().name(format!("craft-ai-{}", self.prefix)).spawn(move || {
            let result = catch_unwind(AssertUnwindSafe(|| work(&worker)))
                .unwrap_or_else(|_| Err(AiError::new(ErrorKind::Internal, "the adapter stopped unexpectedly")));
            worker.finish(result);
        });
        if let Err(e) = spawned {
            self.lock().remove(&id);
            return Err(AiError::new(ErrorKind::Internal, format!("could not start the job: {e}")));
        }
        Ok(id)
    }

    pub fn poll(&self, job: &JobId) -> Result<JobSnapshot, AiError> {
        let record = self.lock().get(job).cloned().ok_or_else(|| AiError::invalid(format!("unknown job {job}")))?;
        Ok(record.snapshot())
    }

    pub fn cancel(&self, job: &JobId) -> Result<(), AiError> {
        let record = self.lock().get(job).cloned().ok_or_else(|| AiError::invalid(format!("unknown job {job}")))?;
        let finished = record.lock().state.is_terminal();
        if !finished {
            record.cancel.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn wait_terminal(table: &JobTable, id: &JobId) -> (JobState, Vec<Event>) {
        let start = Instant::now();
        let mut events = Vec::new();
        loop {
            let snap = table.poll(id).unwrap();
            events.extend(snap.events);
            if snap.state.is_terminal() || start.elapsed() > Duration::from_secs(5) {
                return (snap.state, events);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn success_failure_and_panics_are_reported() {
        let table = JobTable::new("t");
        let ok = table
            .spawn(|r| {
                r.push(Event::TextDelta { text: "hi".into() });
                Ok(())
            })
            .unwrap();
        let (state, events) = wait_terminal(&table, &ok);
        assert_eq!(state, JobState::Succeeded);
        assert_eq!(events.len(), 1);

        let bad = table.spawn(|_| Err(AiError::new(ErrorKind::Provider, "boom"))).unwrap();
        assert!(matches!(wait_terminal(&table, &bad).0, JobState::Failed { .. }));

        #[allow(clippy::panic)]
        let panicky = table.spawn(|_| panic!("adapter bug")).unwrap();
        let (state, _) = wait_terminal(&table, &panicky);
        assert!(matches!(state, JobState::Failed { error } if error.kind == ErrorKind::Internal));
        assert!(table.poll(&JobId::new("t-999").unwrap()).is_err());
    }

    #[test]
    fn cancel_is_immediate_and_final() {
        let table = JobTable::new("t");
        let id = table
            .spawn(|r| {
                while !r.is_cancelled() {
                    std::thread::sleep(Duration::from_millis(2));
                }
                r.push(Event::TextDelta { text: "late".into() });
                Ok(())
            })
            .unwrap();
        table.cancel(&id).unwrap();
        let snap = table.poll(&id).unwrap();
        assert_eq!(snap.state, JobState::Cancelled);
        std::thread::sleep(Duration::from_millis(30));
        let again = table.poll(&id).unwrap();
        assert_eq!(again.state, JobState::Cancelled);
        assert!(again.events.is_empty());
        table.cancel(&id).unwrap();
    }

    #[test]
    fn progress_never_goes_backwards() {
        let r = JobRecord::new();
        r.running(Some(50));
        r.running(Some(20));
        r.running(None);
        assert_eq!(r.snapshot().state, JobState::Running { percent: Some(50) });
        r.running(Some(100));
        assert_eq!(r.snapshot().state, JobState::Running { percent: Some(99) }, "only success reaches 100");
    }
}
