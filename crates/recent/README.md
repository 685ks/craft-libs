# craft-recent

Recent files, autosave timing and crash-recovery snapshots for the Craft apps.

- **`RecentFiles`**: the File > Open Recent list, newest first, without duplicates (case-blind
  on Windows and macOS), with a limit, rename and prune. It is a serde type meant to live in a
  section of the app's `craft_settings::Settings`, so it is saved with the settings; loading
  never fails on a hand-edited list.
- **`Autosave`**: decides when to write a recovery snapshot: one interval after the first
  change not yet covered, never while the revision is unchanged (pair it with
  `craft_undo::UndoStack::revision`), with back-off up to eight intervals after failures. The
  caller passes the time in.
- **`RecoveryStore` / `Session`** (native only): snapshots in `<data dir>/<app>/recovery`
  (`craft_settings::paths::data_dir`), written with `craft-atomic` and private permissions,
  data before info so a listed snapshot is always complete. Each running instance holds an OS
  file lock on its session; `orphans()` lists only snapshots whose session lock is gone or free,
  so a second running instance is never mistaken for a crash. `Session::end()` on a clean quit
  removes the session's files; a crash leaves them for the next start.

Nothing here panics; damaged or foreign files in the recovery folder are skipped.
