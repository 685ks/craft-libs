use super::*;

/// Appends text at the end of the document; adjacent typing merges.
#[derive(Debug)]
struct Type(String);

/// A command that can be told to fail.
#[derive(Debug)]
enum Edit {
    Type(Type),
    FailApply,
    FailUndo(String),
}

#[derive(Debug, PartialEq, Eq)]
struct Refused;

impl Command<String> for Edit {
    type Error = Refused;

    fn apply(&mut self, doc: &mut String) -> Result<(), Refused> {
        match self {
            Edit::Type(Type(text)) | Edit::FailUndo(text) => {
                doc.push_str(text);
                Ok(())
            }
            Edit::FailApply => Err(Refused),
        }
    }

    fn undo(&mut self, doc: &mut String) -> Result<(), Refused> {
        match self {
            Edit::Type(Type(text)) => {
                let keep = doc.len().saturating_sub(text.len());
                doc.truncate(keep);
                Ok(())
            }
            Edit::FailUndo(_) | Edit::FailApply => Err(Refused),
        }
    }

    fn label(&self) -> &str {
        match self {
            Edit::Type(_) => "Typing",
            Edit::FailApply => "Broken",
            Edit::FailUndo(_) => "Sticky",
        }
    }

    fn merge(&mut self, next: Self) -> Result<(), Self> {
        match (self, next) {
            (Edit::Type(Type(mine)), Edit::Type(Type(theirs))) if !theirs.starts_with(' ') => {
                mine.push_str(&theirs);
                Ok(())
            }
            (_, next) => Err(next),
        }
    }
}

fn typed(text: &str) -> Edit {
    Edit::Type(Type(text.to_owned()))
}

#[test]
fn execute_undo_redo_round_trip() {
    let mut doc = String::new();
    let mut stack = UndoStack::new();
    assert_eq!(stack.execute(typed("a"), &mut doc), Ok(Recorded::NewStep));
    stack.seal();
    assert_eq!(stack.execute(typed("b"), &mut doc), Ok(Recorded::NewStep));
    assert_eq!(doc, "ab");
    assert_eq!(stack.undo_label(), Some("Typing"));

    assert_eq!(stack.undo(&mut doc), Ok(Some("Typing".to_owned())));
    assert_eq!(doc, "a");
    assert!(stack.can_redo());
    assert_eq!(stack.redo(&mut doc), Ok(Some("Typing".to_owned())));
    assert_eq!(doc, "ab");
    assert_eq!(stack.undo(&mut doc), Ok(Some("Typing".to_owned())));
    assert_eq!(stack.undo(&mut doc), Ok(Some("Typing".to_owned())));
    assert_eq!(doc, "");
    assert_eq!(stack.undo(&mut doc), Ok(None));
    assert!(!stack.can_undo());
    assert_eq!(stack.redo_len(), 2);
}

#[test]
fn adjacent_edits_coalesce_until_sealed() {
    let mut doc = String::new();
    let mut stack = UndoStack::new();
    stack.execute(typed("he"), &mut doc).ok();
    assert_eq!(stack.execute(typed("llo"), &mut doc), Ok(Recorded::Merged));
    // The command refuses to merge a word break.
    assert_eq!(stack.execute(typed(" w"), &mut doc), Ok(Recorded::NewStep));
    stack.seal();
    assert_eq!(stack.execute(typed("orld"), &mut doc), Ok(Recorded::NewStep));
    assert_eq!(doc, "hello world");
    assert_eq!(stack.undo_len(), 3);

    stack.undo(&mut doc).ok();
    assert_eq!(doc, "hello w");
    stack.undo(&mut doc).ok();
    assert_eq!(doc, "hello");
}

#[test]
fn undo_ends_a_coalescing_run() {
    let mut doc = String::new();
    let mut stack = UndoStack::new();
    stack.execute(typed("a"), &mut doc).ok();
    stack.execute(typed("b"), &mut doc).ok();
    stack.undo(&mut doc).ok();
    stack.redo(&mut doc).ok();
    assert_eq!(stack.execute(typed("c"), &mut doc), Ok(Recorded::NewStep));
}

#[test]
fn coalescing_window_uses_caller_times() {
    let start = Instant::now();
    let mut doc = String::new();
    let mut stack = UndoStack::new().with_coalesce_window(Duration::from_millis(500));
    stack.execute_at(typed("a"), &mut doc, start).ok();
    let merged = stack.execute_at(typed("b"), &mut doc, start + Duration::from_millis(400));
    assert_eq!(merged, Ok(Recorded::Merged));
    let late = stack.execute_at(typed("c"), &mut doc, start + Duration::from_millis(1000));
    assert_eq!(late, Ok(Recorded::NewStep));
    // A time earlier than the last edit (clock went backwards) never merges.
    let earlier = stack.execute_at(typed("d"), &mut doc, start);
    assert_eq!(earlier, Ok(Recorded::NewStep));
}

#[test]
fn new_edit_discards_the_redo_branch() {
    let mut doc = String::new();
    let mut stack = UndoStack::new();
    stack.execute(typed("a"), &mut doc).ok();
    stack.seal();
    stack.execute(typed("b"), &mut doc).ok();
    stack.undo(&mut doc).ok();
    stack.execute(typed("c"), &mut doc).ok();
    assert_eq!(doc, "ac");
    assert!(!stack.can_redo());
    assert_eq!(stack.redo(&mut doc), Ok(None));
}

#[test]
fn clean_marker_follows_undo_and_redo() {
    let mut doc = String::new();
    let mut stack = UndoStack::new();
    assert!(stack.is_clean());
    stack.execute(typed("a"), &mut doc).ok();
    assert!(!stack.is_clean());
    stack.mark_clean();
    assert!(stack.is_clean());
    stack.undo(&mut doc).ok();
    assert!(!stack.is_clean());
    stack.redo(&mut doc).ok();
    assert!(stack.is_clean());
}

#[test]
fn edit_after_save_never_merges_into_the_saved_step() {
    let mut doc = String::new();
    let mut stack = UndoStack::new();
    stack.execute(typed("a"), &mut doc).ok();
    stack.mark_clean();
    assert_eq!(stack.execute(typed("b"), &mut doc), Ok(Recorded::NewStep));
    assert!(!stack.is_clean());
    stack.undo(&mut doc).ok();
    assert!(stack.is_clean());
    assert_eq!(doc, "a");
}

#[test]
fn saved_state_on_a_discarded_branch_is_unreachable() {
    let mut doc = String::new();
    let mut stack = UndoStack::new();
    stack.execute(typed("a"), &mut doc).ok();
    stack.mark_clean();
    stack.undo(&mut doc).ok();
    stack.execute(typed("b"), &mut doc).ok();
    assert!(!stack.is_clean());
    stack.undo(&mut doc).ok();
    assert!(!stack.is_clean(), "the empty document was never saved");
}

#[test]
fn mark_dirty_and_clear() {
    let mut doc = String::new();
    let mut stack = UndoStack::new();
    stack.mark_dirty();
    assert!(!stack.is_clean());
    stack.execute(typed("a"), &mut doc).ok();
    stack.clear(true);
    assert!(stack.is_clean());
    assert!(!stack.can_undo());
    stack.clear(false);
    assert!(!stack.is_clean());
}

#[test]
fn failed_apply_is_not_recorded() {
    let mut doc = String::new();
    let mut stack = UndoStack::new();
    let before = stack.revision();
    assert_eq!(stack.execute(Edit::FailApply, &mut doc), Err(Refused));
    assert!(!stack.can_undo());
    assert!(stack.is_clean());
    assert_eq!(stack.revision(), before);
}

#[test]
fn failed_undo_keeps_the_step() {
    let mut doc = String::new();
    let mut stack = UndoStack::new();
    stack.execute(Edit::FailUndo("x".into()), &mut doc).ok();
    assert_eq!(stack.undo(&mut doc), Err(Refused));
    assert_eq!(stack.undo_len(), 1);
    assert_eq!(stack.undo_label(), Some("Sticky"));
    assert_eq!(doc, "x");
}

#[test]
fn failed_redo_keeps_the_step_redoable() {
    struct Flaky {
        fail_next_apply: bool,
    }
    impl Command<u32> for Flaky {
        type Error = Refused;
        fn apply(&mut self, n: &mut u32) -> Result<(), Refused> {
            if std::mem::take(&mut self.fail_next_apply) {
                return Err(Refused);
            }
            *n += 1;
            Ok(())
        }
        fn undo(&mut self, n: &mut u32) -> Result<(), Refused> {
            *n -= 1;
            self.fail_next_apply = true;
            Ok(())
        }
        fn label(&self) -> &str {
            "Count"
        }
    }

    let mut n = 0;
    let mut stack = UndoStack::new();
    stack.execute(Flaky { fail_next_apply: false }, &mut n).ok();
    stack.undo(&mut n).ok();
    assert_eq!(stack.redo(&mut n), Err(Refused));
    assert_eq!(stack.redo_len(), 1);
    assert_eq!(stack.redo(&mut n), Ok(Some("Count".to_owned())));
    assert_eq!(n, 1);
}

#[test]
fn record_keeps_changes_already_made() {
    let mut doc = String::from("moved");
    let mut stack = UndoStack::new();
    assert_eq!(stack.record::<String>(typed("moved")), Recorded::NewStep);
    stack.undo(&mut doc).ok();
    assert_eq!(doc, "");
}

#[test]
fn limit_drops_the_oldest_steps_and_the_unreachable_clean_state() {
    let mut doc = String::new();
    let mut stack = UndoStack::new().with_limit(2);
    for text in ["a", "b", "c"] {
        stack.seal();
        stack.execute(typed(text), &mut doc).ok();
    }
    assert_eq!(stack.undo_len(), 2);
    assert!(!stack.is_clean());
    stack.undo(&mut doc).ok();
    stack.undo(&mut doc).ok();
    assert_eq!(doc, "a");
    assert_eq!(stack.undo(&mut doc), Ok(None));
    assert!(!stack.is_clean(), "the empty start was dropped by the limit");

    let mut zero = UndoStack::new().with_limit(0);
    let mut doc = String::new();
    zero.execute(typed("a"), &mut doc).ok();
    assert!(!zero.can_undo());
    assert_eq!(doc, "a");
}

#[test]
fn clean_state_shifts_when_old_steps_are_dropped() {
    let mut doc = String::new();
    let mut stack = UndoStack::new().with_limit(2);
    stack.execute(typed("a"), &mut doc).ok();
    stack.mark_clean();
    stack.execute(typed("b"), &mut doc).ok();
    stack.seal();
    stack.execute(typed("c"), &mut doc).ok();
    stack.undo(&mut doc).ok();
    stack.undo(&mut doc).ok();
    assert_eq!(doc, "a");
    assert!(stack.is_clean());
}

#[test]
fn revision_changes_on_every_edit() {
    let mut doc = String::new();
    let mut stack = UndoStack::new();
    let r0 = stack.revision();
    stack.execute(typed("a"), &mut doc).ok();
    let r1 = stack.revision();
    stack.execute(typed("b"), &mut doc).ok();
    let r2 = stack.revision();
    stack.undo(&mut doc).ok();
    let r3 = stack.revision();
    assert!(r0 != r1 && r1 != r2 && r2 != r3);
    assert_eq!(stack.undo(&mut doc), Ok(None));
    assert_eq!(stack.revision(), r3, "nothing to undo is not a change");
}

#[test]
fn boxed_commands_work_without_merging() {
    struct Add(i32);
    impl Command<i32> for Add {
        type Error = Refused;
        fn apply(&mut self, n: &mut i32) -> Result<(), Refused> {
            *n += self.0;
            Ok(())
        }
        fn undo(&mut self, n: &mut i32) -> Result<(), Refused> {
            *n -= self.0;
            Ok(())
        }
        fn label(&self) -> &str {
            "Add"
        }
    }
    let mut n = 0;
    let mut stack: UndoStack<Box<dyn Command<i32, Error = Refused>>> = UndoStack::new();
    stack.execute(Box::new(Add(2)), &mut n).ok();
    stack.execute(Box::new(Add(3)), &mut n).ok();
    assert_eq!(stack.undo_len(), 2);
    stack.undo(&mut n).ok();
    assert_eq!(n, 2);
}
