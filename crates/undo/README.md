# craft-undo

The shared undo/redo history for the Craft apps.

- **Commands.** Each app implements `Command<Doc>` for its own edits: `apply`, `undo`, a menu
  `label`, and optionally `merge`. Use an enum of edits, or `Box<dyn Command<Doc, Error = E>>`
  for open-ended command types (boxed commands do not merge).
- **Coalescing.** A new edit is folded into the previous step when the command's `merge` agrees,
  the run was not ended with `seal()` (call it on mouse-up, caret jumps and focus changes), and,
  if a window is set with `with_coalesce_window`, it arrived within that time of the last edit
  (times are passed in by the caller with `execute_at`, so the crate stays wasm-safe).
- **Undo and redo** return the step's label for "Undo Typing" style menus. A command that fails
  to apply is not recorded; one that fails to undo or redo stays where it was.
- **Clean state.** `mark_clean()` after a save, `is_clean()` for the unsaved-changes indicator.
  The saved state is tracked through undo, redo, the depth limit and discarded redo branches,
  and an edit right after a save never merges into the saved step.
- **`revision()`** changes on every edit, undo and redo, for autosave.

No panics, no `unsafe`, no dependencies.
