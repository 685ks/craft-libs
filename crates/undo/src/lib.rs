#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

//! A shared undo/redo stack for the Craft apps.
//!
//! Each app keeps its own document type and its own commands. This crate keeps the history:
//! which commands are done and which can be redone, when a new edit may be folded into the
//! previous one ("coalescing", so typing a word is one undo step), and which point in the
//! history matches the file on disk (the clean-state marker that drives the unsaved-changes
//! dot and the save prompt).
//!
//! ```
//! use craft_undo::{Command, UndoStack};
//!
//! struct Insert { at: usize, text: String }
//!
//! impl Command<String> for Insert {
//!     type Error = std::convert::Infallible;
//!     fn apply(&mut self, doc: &mut String) -> Result<(), Self::Error> {
//!         doc.insert_str(self.at, &self.text);
//!         Ok(())
//!     }
//!     fn undo(&mut self, doc: &mut String) -> Result<(), Self::Error> {
//!         doc.replace_range(self.at..self.at + self.text.len(), "");
//!         Ok(())
//!     }
//!     fn label(&self) -> &str { "Typing" }
//!     // Typing right after the previous insert extends it, so one undo removes the word.
//!     fn merge(&mut self, next: Self) -> Result<(), Self> {
//!         if next.at == self.at + self.text.len() {
//!             self.text.push_str(&next.text);
//!             Ok(())
//!         } else {
//!             Err(next)
//!         }
//!     }
//! }
//!
//! let mut doc = String::new();
//! let mut history = UndoStack::new();
//! history.execute(Insert { at: 0, text: "Hi".into() }, &mut doc).ok();
//! history.execute(Insert { at: 2, text: "!".into() }, &mut doc).ok();
//! assert_eq!(history.undo_len(), 1); // the two inserts coalesced
//! history.undo(&mut doc).ok();
//! assert_eq!(doc, "");
//! assert!(history.is_clean());
//! ```
//!
//! Nothing here panics. A command that fails to apply is not recorded, and a command that fails
//! to undo or redo stays where it was, so the history always describes the document as long as
//! each command leaves the document unchanged when it returns an error.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

/// One reversible edit to a target of type `T` (usually the app's document).
pub trait Command<T: ?Sized> {
    /// What `apply` and `undo` return when they cannot do their work.
    type Error;

    /// Makes the change (the first time, and again on redo).
    ///
    /// On error, leave `target` as it was: the stack treats the change as not made.
    fn apply(&mut self, target: &mut T) -> Result<(), Self::Error>;

    /// Reverses the change made by [`Command::apply`].
    ///
    /// On error, leave `target` as it was: the stack treats the change as still made.
    fn undo(&mut self, target: &mut T) -> Result<(), Self::Error>;

    /// The name shown in the Edit menu, such as `Typing` for "Undo Typing".
    fn label(&self) -> &str;

    /// Folds `next`, which has already been applied, into `self`.
    ///
    /// Return `Ok(())` when `self` now covers both changes (one undo reverses both), or give
    /// `next` back with `Err(next)` to record it as its own step. The default never merges.
    fn merge(&mut self, next: Self) -> Result<(), Self>
    where
        Self: Sized,
    {
        Err(next)
    }
}

/// A boxed command runs the boxed one, so `UndoStack<Box<dyn Command<T, Error = E>>>` holds
/// commands of different types. Boxed commands do not merge.
impl<T: ?Sized, C: Command<T> + ?Sized> Command<T> for Box<C> {
    type Error = C::Error;

    fn apply(&mut self, target: &mut T) -> Result<(), Self::Error> {
        (**self).apply(target)
    }

    fn undo(&mut self, target: &mut T) -> Result<(), Self::Error> {
        (**self).undo(target)
    }

    fn label(&self) -> &str {
        (**self).label()
    }
}

/// What happened to a command passed to [`UndoStack::execute`] or [`UndoStack::record`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recorded {
    /// It became a new undo step.
    NewStep,
    /// It was folded into the previous step.
    Merged,
}

/// The undo/redo history.
#[derive(Debug)]
pub struct UndoStack<C> {
    done: VecDeque<C>,
    undone: Vec<C>,
    /// Number of steps in `done` that matches the saved file, or `None` when that state can no
    /// longer be reached (it was dropped by the depth limit or replaced by a new branch).
    clean: Option<usize>,
    limit: Option<usize>,
    coalesce_window: Option<Duration>,
    last_change: Option<Instant>,
    sealed: bool,
    revision: u64,
}

impl<C> Default for UndoStack<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C> UndoStack<C> {
    /// An empty history with no depth limit and no coalescing time window. It starts clean.
    pub fn new() -> Self {
        Self {
            done: VecDeque::new(),
            undone: Vec::new(),
            clean: Some(0),
            limit: None,
            coalesce_window: None,
            last_change: None,
            sealed: false,
            revision: 0,
        }
    }

    /// Keeps at most `steps` undo steps, dropping the oldest. `0` keeps no history at all.
    pub fn with_limit(mut self, steps: usize) -> Self {
        self.limit = Some(steps);
        self.enforce_limit();
        self
    }

    /// Only coalesces edits that arrive within `window` of the previous one, when the caller
    /// gives times with [`UndoStack::execute_at`] or [`UndoStack::record_at`]. Edits without a
    /// time are coalesced whenever the commands agree to merge.
    pub fn with_coalesce_window(mut self, window: Duration) -> Self {
        self.coalesce_window = Some(window);
        self
    }

    /// Applies `command` to `target` and records it.
    ///
    /// If applying fails, nothing is recorded and the error is returned.
    pub fn execute<T: ?Sized>(&mut self, mut command: C, target: &mut T) -> Result<Recorded, C::Error>
    where
        C: Command<T>,
    {
        command.apply(target)?;
        Ok(self.push(command, None))
    }

    /// [`UndoStack::execute`] with the time of the edit, for the coalescing window.
    pub fn execute_at<T: ?Sized>(&mut self, mut command: C, target: &mut T, at: Instant) -> Result<Recorded, C::Error>
    where
        C: Command<T>,
    {
        command.apply(target)?;
        Ok(self.push(command, Some(at)))
    }

    /// Records a command whose change the caller has already made (for example a drag that
    /// moved the selection live and is committed on mouse-up).
    pub fn record<T: ?Sized>(&mut self, command: C) -> Recorded
    where
        C: Command<T>,
    {
        self.push(command, None)
    }

    /// [`UndoStack::record`] with the time of the edit, for the coalescing window.
    pub fn record_at<T: ?Sized>(&mut self, command: C, at: Instant) -> Recorded
    where
        C: Command<T>,
    {
        self.push(command, Some(at))
    }

    /// Ends coalescing: the next edit becomes a new step even if it could merge. Call it when
    /// the user's gesture ends, such as on mouse-up, a caret jump or a focus change.
    pub fn seal(&mut self) {
        self.sealed = true;
    }

    /// Undoes the latest step and returns its label, or `Ok(None)` when there is nothing to undo.
    ///
    /// If the command fails to undo, it stays the latest step and the error is returned.
    pub fn undo<T: ?Sized>(&mut self, target: &mut T) -> Result<Option<String>, C::Error>
    where
        C: Command<T>,
    {
        let Some(mut command) = self.done.pop_back() else {
            return Ok(None);
        };
        if let Err(error) = command.undo(target) {
            self.done.push_back(command);
            return Err(error);
        }
        let label = command.label().to_owned();
        self.undone.push(command);
        self.changed();
        Ok(Some(label))
    }

    /// Redoes the latest undone step and returns its label, or `Ok(None)` when there is nothing
    /// to redo.
    ///
    /// If the command fails to apply, it stays redoable and the error is returned.
    pub fn redo<T: ?Sized>(&mut self, target: &mut T) -> Result<Option<String>, C::Error>
    where
        C: Command<T>,
    {
        let Some(mut command) = self.undone.pop() else {
            return Ok(None);
        };
        if let Err(error) = command.apply(target) {
            self.undone.push(command);
            return Err(error);
        }
        let label = command.label().to_owned();
        self.done.push_back(command);
        self.changed();
        self.enforce_limit();
        Ok(Some(label))
    }

    /// Whether there is a step to undo.
    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    /// Whether there is a step to redo.
    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    /// The label of the step [`UndoStack::undo`] would reverse.
    pub fn undo_label<T: ?Sized>(&self) -> Option<&str>
    where
        C: Command<T>,
    {
        self.done.back().map(Command::label)
    }

    /// The label of the step [`UndoStack::redo`] would apply again.
    pub fn redo_label<T: ?Sized>(&self) -> Option<&str>
    where
        C: Command<T>,
    {
        self.undone.last().map(Command::label)
    }

    /// Number of steps that can be undone.
    pub fn undo_len(&self) -> usize {
        self.done.len()
    }

    /// Number of steps that can be redone.
    pub fn redo_len(&self) -> usize {
        self.undone.len()
    }

    /// Marks the current state as the one on disk. Call it after a successful save.
    pub fn mark_clean(&mut self) {
        self.clean = Some(self.done.len());
        self.sealed = true;
    }

    /// Forgets the saved state, so the document stays dirty until the next
    /// [`UndoStack::mark_clean`] (for example after a save failed half-way or the file changed
    /// on disk).
    pub fn mark_dirty(&mut self) {
        self.clean = None;
    }

    /// Whether the document matches the saved state.
    pub fn is_clean(&self) -> bool {
        self.clean == Some(self.done.len())
    }

    /// A counter that changes on every edit, undo and redo, including coalesced edits.
    ///
    /// Autosave can compare it with the value from its last snapshot to see whether anything
    /// changed since, independently of the clean marker.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Drops the whole history. The current state counts as clean when `clean` is true.
    pub fn clear(&mut self, clean: bool) {
        self.done.clear();
        self.undone.clear();
        self.clean = clean.then_some(0);
        self.sealed = false;
        self.last_change = None;
        self.changed_without_time();
    }

    fn push<T: ?Sized>(&mut self, command: C, at: Option<Instant>) -> Recorded
    where
        C: Command<T>,
    {
        // A new edit replaces the redo branch. If the saved state lived on that branch, it can
        // no longer be reached by undo or redo.
        if !self.undone.is_empty() {
            self.undone.clear();
            if self.clean.is_some_and(|clean| clean > self.done.len()) {
                self.clean = None;
            }
        }

        let in_window = match (self.coalesce_window, at, self.last_change) {
            (Some(window), Some(now), Some(last)) => now.checked_duration_since(last).is_some_and(|gap| gap <= window),
            _ => true,
        };
        // Merging into the step that matches the saved file would make a changed document look
        // clean, so the step after a save always starts fresh.
        let may_merge = !self.sealed && in_window && self.clean != Some(self.done.len());

        let outcome = match (may_merge, self.done.back_mut()) {
            (true, Some(last)) => match last.merge(command) {
                Ok(()) => Recorded::Merged,
                Err(command) => {
                    self.done.push_back(command);
                    Recorded::NewStep
                }
            },
            _ => {
                self.done.push_back(command);
                Recorded::NewStep
            }
        };

        self.sealed = false;
        if at.is_some() {
            self.last_change = at;
        }
        self.changed_without_time();
        self.enforce_limit();
        outcome
    }

    fn changed(&mut self) {
        // Undo and redo end any run of coalescing edits.
        self.sealed = true;
        self.changed_without_time();
    }

    fn changed_without_time(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    fn enforce_limit(&mut self) {
        let Some(limit) = self.limit else { return };
        while self.done.len() > limit {
            if self.done.pop_front().is_none() {
                break;
            }
            self.clean = match self.clean {
                Some(0) | None => None,
                Some(clean) => Some(clean - 1),
            };
        }
        // Redo steps beyond the limit are dropped too, keeping the newest ones.
        if self.undone.len() > limit {
            let excess = self.undone.len() - limit;
            self.undone.drain(..excess);
            if self.clean.is_some_and(|clean| clean > self.done.len() + self.undone.len()) {
                self.clean = None;
            }
        }
    }
}
