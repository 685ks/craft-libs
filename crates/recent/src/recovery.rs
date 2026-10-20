//! Crash-recovery snapshots kept in the app's data folder.
//!
//! Layout of the recovery folder (`<data dir>/<app id>/recovery` by default):
//!
//! - `<session>.lock`: held locked by the running instance for as long as it runs.
//! - `<session>.<doc>.data`: the snapshot bytes, in whatever format the app chose.
//! - `<session>.<doc>.json`: a [`SnapshotInfo`] describing it, written after the data, so a
//!   snapshot is listed only once its data is complete.
//!
//! Both snapshot files are written with `craft-atomic` and private permissions. A snapshot is
//! an *orphan* (left by an instance that crashed or was killed) when the lock of its session is
//! missing or can be taken; the operating system releases the lock when a process dies.

use serde::{Deserialize, Serialize};
use std::fs::{self, File, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use craft_atomic::{NewFilePermissions, WriteOptions};

const LOCK_EXT: &str = "lock";
const DATA_EXT: &str = "data";
const INFO_EXT: &str = "json";
/// Longest id accepted when reading names back from the folder.
const MAX_ID_LEN: usize = 64;

/// What the app wants to remember about a snapshot, shown when offering to restore it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SnapshotInfo {
    /// The name shown to the user, such as the file name or "Untitled 2".
    pub title: String,
    /// Where the document was saved, if it ever was.
    pub original_path: Option<PathBuf>,
    /// The app's own format tag for the bytes, such as `photocraft-doc-3`, so a newer version
    /// can tell what it is reading.
    pub format: String,
    /// When the snapshot was written, in seconds since the Unix epoch. Filled in by
    /// [`Session::save`] when left at zero.
    pub saved_at_unix: u64,
}

impl SnapshotInfo {
    /// Info with just a title.
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), ..Self::default() }
    }

    /// Sets where the document was saved.
    pub fn with_original_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.original_path = Some(path.into());
        self
    }

    /// Sets the app's format tag.
    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        self.format = format.into();
        self
    }
}

/// Identifies one document's snapshot within a [`Session`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentKey(u64);

/// A snapshot found in the recovery folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    session: String,
    doc: String,
    info: SnapshotInfo,
}

impl Snapshot {
    /// The info written with the snapshot.
    pub fn info(&self) -> &SnapshotInfo {
        &self.info
    }

    /// The id of the session (app instance) that wrote it.
    pub fn session_id(&self) -> &str {
        &self.session
    }
}

/// The recovery folder of one app.
#[derive(Debug, Clone)]
pub struct RecoveryStore {
    dir: PathBuf,
}

impl RecoveryStore {
    /// The default folder, `recovery` inside the app's platform data folder
    /// ([`craft_settings::paths::data_dir`]). `None` when the platform has no data folder.
    pub fn for_app(app_id: &str) -> Option<Self> {
        craft_settings::paths::data_dir(app_id).map(|dir| Self::at(dir.join("recovery")))
    }

    /// A store in `dir` (for portable installs and tests). The folder is created on first write.
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Starts this instance's session and takes its lock. Call once at startup.
    pub fn start_session(&self) -> io::Result<Session> {
        fs::create_dir_all(&self.dir)?;
        let id = new_session_id();
        let lock_path = self.dir.join(format!("{id}.{LOCK_EXT}"));
        let lock = File::options().write(true).create_new(true).open(&lock_path)?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(io::Error::new(io::ErrorKind::WouldBlock, "the new recovery session lock is already held"));
            }
            // Some file systems (certain network shares) do not support locks. The session
            // still works; another instance may then treat its snapshots as orphans.
            Err(TryLockError::Error(e)) if e.kind() == io::ErrorKind::Unsupported => {}
            Err(TryLockError::Error(e)) => return Err(e),
        }
        Ok(Session { dir: self.dir.clone(), id, lock: Some(lock), next_doc: 0 })
    }

    /// Snapshots left by instances that are no longer running, newest first.
    ///
    /// Unreadable or damaged info files are skipped. A missing folder means no orphans.
    pub fn orphans(&self) -> io::Result<Vec<Snapshot>> {
        self.orphans_except(None)
    }

    /// [`RecoveryStore::orphans`], also skipping `session`'s own snapshots.
    pub fn orphans_excluding(&self, session: &Session) -> io::Result<Vec<Snapshot>> {
        self.orphans_except(Some(&session.id))
    }

    fn orphans_except(&self, own: Option<&str>) -> io::Result<Vec<Snapshot>> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut found = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some((session, doc)) = name.to_str().and_then(parse_info_name) else { continue };
            if own == Some(session) || self.session_alive(session) {
                continue;
            }
            if !self.dir.join(data_name(session, doc)).is_file() {
                continue;
            }
            let Ok(text) = fs::read_to_string(entry.path()) else { continue };
            let Ok(info) = serde_json::from_str::<SnapshotInfo>(&text) else { continue };
            found.push(Snapshot { session: session.to_owned(), doc: doc.to_owned(), info });
        }
        found.sort_by(|a, b| b.info.saved_at_unix.cmp(&a.info.saved_at_unix).then_with(|| a.doc.cmp(&b.doc)));
        Ok(found)
    }

    /// Reads a snapshot's bytes.
    pub fn read(&self, snapshot: &Snapshot) -> io::Result<Vec<u8>> {
        fs::read(self.dir.join(data_name(&snapshot.session, &snapshot.doc)))
    }

    /// Deletes a snapshot (after it was restored or the user declined it), and the lock file
    /// of its session once that session has no snapshots left.
    pub fn discard(&self, snapshot: &Snapshot) -> io::Result<()> {
        remove_snapshot_files(&self.dir, &snapshot.session, &snapshot.doc)?;
        if !self.session_has_snapshots(&snapshot.session) && !self.session_alive(&snapshot.session) {
            remove_if_present(&self.dir.join(lock_name(&snapshot.session)))?;
        }
        Ok(())
    }

    /// Whether the session's lock is held by a running instance.
    fn session_alive(&self, session: &str) -> bool {
        let Ok(file) = File::options().write(true).open(self.dir.join(lock_name(session))) else {
            return false;
        };
        // The lock is released when `file` drops at the end of this function.
        matches!(file.try_lock(), Err(TryLockError::WouldBlock))
    }

    fn session_has_snapshots(&self, session: &str) -> bool {
        let Ok(entries) = fs::read_dir(&self.dir) else { return false };
        entries.flatten().any(|entry| entry.file_name().to_str().and_then(parse_snapshot_name).is_some_and(|(owner, _)| owner == session))
    }
}

/// This instance's recovery session. Holding it keeps the session lock.
///
/// Call [`Session::end`] on a clean quit to remove this session's snapshots. If the session is
/// dropped without `end` (a crash, or a quit that skipped it), its snapshots stay for the next
/// start to offer.
#[derive(Debug)]
pub struct Session {
    dir: PathBuf,
    id: String,
    lock: Option<File>,
    next_doc: u64,
}

impl Session {
    /// The session id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// A key for a newly opened document. Keys are never reused within a session.
    pub fn new_document(&mut self) -> DocumentKey {
        let key = DocumentKey(self.next_doc);
        self.next_doc = self.next_doc.wrapping_add(1);
        key
    }

    /// Writes (or replaces) the snapshot of `doc`.
    pub fn save(&self, doc: DocumentKey, info: &SnapshotInfo, bytes: &[u8]) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let mut info = info.clone();
        if info.saved_at_unix == 0 {
            info.saved_at_unix = unix_now();
        }
        let json = serde_json::to_vec_pretty(&info).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let options = WriteOptions::default().with_new_file_permissions(NewFilePermissions::Private);
        let doc = doc_id(doc);
        // Data first: the info file is what makes a snapshot visible.
        options.atomic_write(&self.dir.join(data_name(&self.id, &doc)), bytes)?;
        options.atomic_write(&self.dir.join(info_name(&self.id, &doc)), &json)
    }

    /// Deletes the snapshot of `doc`, after the user saved or closed it. Missing is fine.
    pub fn discard(&self, doc: DocumentKey) -> io::Result<()> {
        remove_snapshot_files(&self.dir, &self.id, &doc_id(doc))
    }

    /// Ends the session on a clean quit: removes its snapshots and its lock file.
    pub fn end(mut self) -> io::Result<()> {
        let mut first_error = None;
        if let Ok(entries) = fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let ours = name.to_str().and_then(parse_snapshot_name).is_some_and(|(session, _)| session == self.id);
                if ours && let Err(e) = remove_if_present(&entry.path()) {
                    first_error.get_or_insert(e);
                }
            }
        }
        // Close (and so unlock) the file before deleting it: Windows refuses to delete open files.
        drop(self.lock.take());
        if let Err(e) = remove_if_present(&self.dir.join(lock_name(&self.id))) {
            first_error.get_or_insert(e);
        }
        first_error.map_or(Ok(()), Err)
    }
}

fn remove_snapshot_files(dir: &Path, session: &str, doc: &str) -> io::Result<()> {
    // Info first, so a half-removed snapshot is never listed.
    remove_if_present(&dir.join(info_name(session, doc)))?;
    remove_if_present(&dir.join(data_name(session, doc)))
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

fn doc_id(doc: DocumentKey) -> String {
    format!("d{}", doc.0)
}

fn lock_name(session: &str) -> String {
    format!("{session}.{LOCK_EXT}")
}

fn data_name(session: &str, doc: &str) -> String {
    format!("{session}.{doc}.{DATA_EXT}")
}

fn info_name(session: &str, doc: &str) -> String {
    format!("{session}.{doc}.{INFO_EXT}")
}

/// Splits `<session>.<doc>.json`, accepting only names this module writes.
fn parse_info_name(name: &str) -> Option<(&str, &str)> {
    parse_name(name, INFO_EXT)
}

/// Splits `<session>.<doc>.json` or `<session>.<doc>.data`.
fn parse_snapshot_name(name: &str) -> Option<(&str, &str)> {
    parse_name(name, INFO_EXT).or_else(|| parse_name(name, DATA_EXT))
}

fn parse_name<'a>(name: &'a str, ext: &str) -> Option<(&'a str, &'a str)> {
    let stem = name.strip_suffix(ext)?.strip_suffix('.')?;
    let (session, doc) = stem.split_once('.')?;
    (valid_id(session) && valid_id(doc)).then_some((session, doc))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_ID_LEN && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn new_session_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    format!("s{}-{nanos:x}-{n}", std::process::id())
}

#[cfg(test)]
mod tests;
