use super::*;

fn store() -> (tempfile::TempDir, RecoveryStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = RecoveryStore::at(dir.path().join("recovery"));
    (dir, store)
}

#[test]
fn missing_folder_has_no_orphans() {
    let (_dir, store) = store();
    assert_eq!(store.orphans().unwrap(), Vec::new());
}

#[test]
fn a_running_session_is_not_an_orphan() {
    let (_dir, store) = store();
    let mut session = store.start_session().unwrap();
    let doc = session.new_document();
    session.save(doc, &SnapshotInfo::new("Untitled 1"), b"bytes").unwrap();

    // Another instance (here: the same process asking) must not see it while the lock is held.
    assert!(store.orphans().unwrap().is_empty());
    assert!(store.orphans_excluding(&session).unwrap().is_empty());
}

#[test]
fn a_crashed_session_leaves_orphans_that_can_be_restored_and_discarded() {
    let (_dir, store) = store();
    let mut crashed = store.start_session().unwrap();
    let first = crashed.new_document();
    let second = crashed.new_document();
    assert_ne!(first, second);
    let info = SnapshotInfo::new("Poster").with_original_path("/home/me/poster.pc").with_format("photocraft-doc-3");
    crashed.save(first, &info, b"first").unwrap();
    crashed.save(second, &SnapshotInfo { saved_at_unix: 1, ..SnapshotInfo::new("Old") }, b"second").unwrap();
    let crashed_id = crashed.id().to_owned();
    drop(crashed); // the process died: the lock is released, the files stay

    let next = store.start_session().unwrap();
    let orphans = store.orphans_excluding(&next).unwrap();
    assert_eq!(orphans.len(), 2);
    let newest = &orphans[0];
    assert_eq!(newest.info().title, "Poster");
    assert_eq!(newest.info().format, "photocraft-doc-3");
    assert_eq!(newest.info().original_path.as_deref(), Some(Path::new("/home/me/poster.pc")));
    assert!(newest.info().saved_at_unix > 1, "save fills in the time");
    assert_eq!(newest.session_id(), crashed_id);
    assert_eq!(store.read(newest).unwrap(), b"first");

    for orphan in &orphans {
        store.discard(orphan).unwrap();
    }
    assert!(store.orphans().unwrap().is_empty());
    assert!(!store.dir().join(lock_name(&crashed_id)).exists(), "the dead session's lock file is cleaned up");
    next.end().unwrap();
}

#[test]
fn saving_again_replaces_the_snapshot() {
    let (_dir, store) = store();
    let mut session = store.start_session().unwrap();
    let doc = session.new_document();
    session.save(doc, &SnapshotInfo::new("A"), b"one").unwrap();
    session.save(doc, &SnapshotInfo::new("B"), b"two").unwrap();
    drop(session);
    let orphans = store.orphans().unwrap();
    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0].info().title, "B");
    assert_eq!(store.read(&orphans[0]).unwrap(), b"two");
}

#[test]
fn discard_and_end_remove_this_sessions_files() {
    let (_dir, store) = store();
    let mut session = store.start_session().unwrap();
    let kept = session.new_document();
    let closed = session.new_document();
    session.save(kept, &SnapshotInfo::new("Kept"), b"k").unwrap();
    session.save(closed, &SnapshotInfo::new("Closed"), b"c").unwrap();
    session.discard(closed).unwrap();
    session.discard(closed).unwrap(); // twice is fine
    session.end().unwrap();
    let left: Vec<_> = fs::read_dir(store.dir()).unwrap().flatten().map(|e| e.file_name()).collect();
    assert!(left.is_empty(), "left behind: {left:?}");
}

#[test]
fn damaged_and_foreign_files_are_ignored() {
    let (_dir, store) = store();
    fs::create_dir_all(store.dir()).unwrap();
    // Info without data, damaged info, and names this module never writes.
    fs::write(store.dir().join("s1-a-0.d0.json"), "{}").unwrap();
    fs::write(store.dir().join("s1-a-0.d1.json"), "not json").unwrap();
    fs::write(store.dir().join("s1-a-0.d1.data"), "x").unwrap();
    fs::write(store.dir().join("../escape.d0.json"), "{}").unwrap();
    fs::write(store.dir().join("notes.txt"), "hello").unwrap();
    fs::write(store.dir().join("a.b.c.json"), "{}").unwrap();
    assert!(store.orphans().unwrap().is_empty());
}

#[test]
fn unknown_info_fields_and_missing_fields_still_load() {
    let (_dir, store) = store();
    fs::create_dir_all(store.dir()).unwrap();
    fs::write(store.dir().join("s9-x-0.d0.data"), "x").unwrap();
    fs::write(store.dir().join("s9-x-0.d0.json"), r#"{"title":"Kept","from_a_newer_version":true}"#).unwrap();
    let orphans = store.orphans().unwrap();
    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0].info().title, "Kept");
    assert_eq!(orphans[0].info().original_path, None);
}

#[test]
fn names_are_parsed_strictly() {
    assert_eq!(parse_info_name("s1-2-3.d0.json"), Some(("s1-2-3", "d0")));
    assert_eq!(parse_info_name("s1.d0.data"), None);
    assert_eq!(parse_snapshot_name("s1.d0.data"), Some(("s1", "d0")));
    assert_eq!(parse_info_name(".d0.json"), None);
    assert_eq!(parse_info_name("s1..json"), None);
    assert_eq!(parse_info_name("s 1.d0.json"), None);
    assert_eq!(parse_info_name(&format!("{}.d0.json", "s".repeat(MAX_ID_LEN + 1))), None);
}

#[test]
fn two_sessions_get_different_ids() {
    let (_dir, store) = store();
    let a = store.start_session().unwrap();
    let b = store.start_session().unwrap();
    assert_ne!(a.id(), b.id());
    a.end().unwrap();
    b.end().unwrap();
}
