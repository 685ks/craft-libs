#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

//! Recent files, autosave timing and crash-recovery snapshots for the Craft apps.
//!
//! - [`RecentFiles`] is the File > Open Recent list. It is a plain serde type: put it in a
//!   section of the app's [`craft_settings::Settings`] type and it is saved with the rest of
//!   the settings, atomically, with backup and recovery.
//! - [`Autosave`] decides *when* to write a recovery snapshot: some time after the first
//!   unsaved change, never while nothing changed, and with back-off after a failure. It takes
//!   the document's revision (for example `craft_undo::UndoStack::revision`) and the current
//!   time from the caller.
//! - [`RecoveryStore`] (native only) keeps the snapshots in the app's data folder, written with
//!   `craft-atomic`, and finds the ones a crashed or killed instance left behind. Each running
//!   instance holds a lock on its session file, so a second instance never mistakes the first
//!   one's snapshots for a crash.
//!
//! ```no_run
//! # fn main() -> std::io::Result<()> {
//! use craft_recent::{Autosave, RecoveryStore, SnapshotInfo};
//! use std::time::{Duration, Instant};
//!
//! let store = RecoveryStore::for_app("photocraft").ok_or(std::io::ErrorKind::NotFound)?;
//! for orphan in store.orphans()? {
//!     // Offer to restore orphan.info().title; then read it or discard it.
//!     let _bytes = store.read(&orphan)?;
//!     store.discard(&orphan)?;
//! }
//!
//! let mut session = store.start_session()?;
//! let doc = session.new_document();
//! let mut autosave = Autosave::new(Duration::from_secs(60));
//! # let revision = 1; let bytes = b"...";
//! // Every frame:
//! if autosave.tick(Instant::now(), revision) {
//!     match session.save(doc, &SnapshotInfo::new("Untitled 1"), bytes) {
//!         Ok(()) => autosave.saved(revision),
//!         Err(_) => autosave.failed(Instant::now()),
//!     }
//! }
//! // After the user saves or closes the document: session.discard(doc)?;
//! // On a clean quit:
//! session.end()?;
//! # Ok(()) }
//! ```

mod autosave;
mod recent_files;
#[cfg(not(target_arch = "wasm32"))]
mod recovery;
#[cfg(test)]
mod tests;

pub use autosave::Autosave;
pub use recent_files::{DEFAULT_RECENT_LIMIT, RecentFiles, same_path};
#[cfg(not(target_arch = "wasm32"))]
pub use recovery::{DocumentKey, RecoveryStore, Session, Snapshot, SnapshotInfo};
