//! When to write a recovery snapshot.

use std::time::{Duration, Instant};

/// The longest wait after repeated failures, as a multiple of the interval.
const MAX_BACKOFF: u32 = 8;

/// Decides when to write a recovery snapshot of one document.
///
/// A snapshot is due `interval` after the first change that is not in a snapshot yet, so a
/// burst of typing produces one write, not one per keystroke. Nothing is written while the
/// revision matches the last snapshot (or the last real save, see [`Autosave::reset`]). After a
/// failed write the wait doubles, up to eight intervals, until a write succeeds.
///
/// The caller passes the time in, so this works on every platform (including the web, where
/// `Instant::now` is not available on every target) and in tests.
#[derive(Debug, Clone)]
pub struct Autosave {
    interval: Duration,
    covered_revision: Option<u64>,
    dirty_since: Option<Instant>,
    retry_at: Option<Instant>,
    failures: u32,
    enabled: bool,
}

impl Autosave {
    /// A timer that snapshots `interval` after the first unsaved change.
    ///
    /// The first revision passed to [`Autosave::tick`] counts as changed; call
    /// [`Autosave::reset`] with the revision of a freshly opened or saved document first.
    pub fn new(interval: Duration) -> Self {
        Self { interval, covered_revision: None, dirty_since: None, retry_at: None, failures: 0, enabled: true }
    }

    /// The configured interval.
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Changes the interval (for example from a preference). The current wait restarts from
    /// the first unsaved change, so a shorter interval can make a snapshot due at once.
    pub fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    /// Turns autosave on or off. While off, [`Autosave::tick`] is never due.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Whether autosave is on.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Call every frame (or from a timer). Returns true when a snapshot should be written now.
    pub fn tick(&mut self, now: Instant, revision: u64) -> bool {
        if !self.enabled || self.covered_revision == Some(revision) {
            self.dirty_since = None;
            return false;
        }
        let since = *self.dirty_since.get_or_insert(now);
        let waited_enough = now.checked_duration_since(since).is_some_and(|waited| waited >= self.interval);
        let retry_allowed = self.retry_at.is_none_or(|at| now >= at);
        waited_enough && retry_allowed
    }

    /// How long until [`Autosave::tick`] could next be due, for scheduling a repaint, or `None`
    /// when nothing is waiting to be snapshotted.
    pub fn next_due(&self, now: Instant, revision: u64) -> Option<Duration> {
        if !self.enabled || self.covered_revision == Some(revision) {
            return None;
        }
        let Some(since) = self.dirty_since else {
            return Some(self.interval);
        };
        let mut due = since.checked_add(self.interval).unwrap_or(now);
        if let Some(retry) = self.retry_at {
            due = due.max(retry);
        }
        Some(due.saturating_duration_since(now))
    }

    /// Records a successful snapshot of `revision`.
    pub fn saved(&mut self, revision: u64) {
        self.covered_revision = Some(revision);
        self.dirty_since = None;
        self.retry_at = None;
        self.failures = 0;
    }

    /// Records a failed snapshot. The next attempt waits longer, up to eight intervals.
    pub fn failed(&mut self, now: Instant) {
        self.failures = self.failures.saturating_add(1);
        let factor = 1u32.checked_shl(self.failures.min(4)).unwrap_or(MAX_BACKOFF).min(MAX_BACKOFF);
        let wait = self.interval.saturating_mul(factor);
        self.retry_at = now.checked_add(wait);
    }

    /// How many snapshots in a row have failed.
    pub fn failures(&self) -> u32 {
        self.failures
    }

    /// Records that `revision` is safe on disk without a snapshot: the document was just
    /// opened, saved by the user, or reverted. Nothing is due until the revision changes.
    pub fn reset(&mut self, revision: u64) {
        self.saved(revision);
    }
}
