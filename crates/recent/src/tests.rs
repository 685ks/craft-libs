use super::*;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[test]
fn recent_files_dedupes_and_keeps_newest_first() {
    let mut recent = RecentFiles::with_limit(3);
    recent.add("/a");
    recent.add("/b");
    recent.add("/a");
    assert_eq!(recent.iter().collect::<Vec<_>>(), [Path::new("/a"), Path::new("/b")]);
    recent.add("/c");
    recent.add("/d");
    assert_eq!(recent.len(), 3);
    assert_eq!(recent.latest(), Some(Path::new("/d")));
    assert!(!recent.contains(Path::new("/b")), "the oldest falls off");
    recent.add("");
    assert_eq!(recent.len(), 3, "empty paths are ignored");
}

#[test]
fn recent_files_remove_rename_retain_clear() {
    let mut recent = RecentFiles::default();
    for p in ["/a", "/b", "/c"] {
        recent.add(p);
    }
    assert!(recent.remove(Path::new("/b")));
    assert!(!recent.remove(Path::new("/b")));
    recent.rename(Path::new("/a"), "/renamed");
    assert_eq!(recent.iter().collect::<Vec<_>>(), [Path::new("/c"), Path::new("/renamed")]);
    // Renaming onto another listed file keeps one entry, at the renamed one's place.
    recent.rename(Path::new("/renamed"), "/c");
    assert_eq!(recent.iter().collect::<Vec<_>>(), [Path::new("/c")]);
    recent.rename(Path::new("/missing"), "/x");
    assert_eq!(recent.len(), 1);
    recent.add("/gone");
    assert_eq!(recent.retain(|p| p != Path::new("/gone")), 1);
    recent.clear();
    assert!(recent.is_empty());
}

#[test]
fn recent_files_limit_changes() {
    let mut recent = RecentFiles::with_limit(0);
    assert_eq!(recent.limit(), 1);
    recent.set_limit(5);
    for p in ["/1", "/2", "/3", "/4"] {
        recent.add(p);
    }
    recent.set_limit(2);
    assert_eq!(recent.iter().collect::<Vec<_>>(), [Path::new("/4"), Path::new("/3")]);
}

#[test]
fn recent_files_round_trip_through_json() {
    let mut recent = RecentFiles::with_limit(4);
    recent.add("/x");
    recent.add("/y");
    let text = serde_json::to_string(&recent).unwrap();
    let back: RecentFiles = serde_json::from_str(&text).unwrap();
    assert_eq!(back, recent);
}

#[test]
fn recent_files_load_forgivingly() {
    let back: RecentFiles = serde_json::from_str(r#"{"limit":"lots","files":["/a",3,"/b","/a",""],"extra":1}"#).unwrap();
    assert_eq!(back.limit(), DEFAULT_RECENT_LIMIT);
    assert_eq!(back.iter().collect::<Vec<_>>(), [Path::new("/a"), Path::new("/b")]);

    let short: RecentFiles = serde_json::from_str(r#"{"limit":1,"files":["/a","/b"]}"#).unwrap();
    assert_eq!(short.iter().collect::<Vec<_>>(), [Path::new("/a")]);

    let empty: RecentFiles = serde_json::from_str("{}").unwrap();
    assert_eq!(empty, RecentFiles::default());
    let zero: RecentFiles = serde_json::from_str(r#"{"limit":0}"#).unwrap();
    assert_eq!(zero.limit(), DEFAULT_RECENT_LIMIT);
}

#[test]
fn same_path_follows_the_platform_case_rule() {
    assert!(same_path(Path::new("/a/b"), Path::new("/a/b")));
    let differs_in_case = same_path(Path::new("/A/b.TXT"), Path::new("/a/B.txt"));
    assert_eq!(differs_in_case, cfg!(any(windows, target_os = "macos")));
    assert!(!same_path(Path::new("/a/b"), Path::new("/a/c")));
    assert!(!same_path(Path::new("/a"), Path::new("/a/b")));
    let _ = PathBuf::new();
}

#[test]
fn autosave_waits_an_interval_after_the_first_change() {
    let t0 = Instant::now();
    let s = |secs| t0 + Duration::from_secs(secs);
    let mut autosave = Autosave::new(Duration::from_secs(10));
    autosave.reset(0);
    assert!(!autosave.tick(s(0), 0), "nothing changed");
    assert_eq!(autosave.next_due(s(0), 0), None);

    assert!(!autosave.tick(s(1), 1), "first change starts the wait");
    assert!(!autosave.tick(s(5), 2), "more changes do not restart it");
    assert_eq!(autosave.next_due(s(5), 2), Some(Duration::from_secs(6)));
    assert!(autosave.tick(s(11), 3));
    autosave.saved(3);
    assert!(!autosave.tick(s(12), 3));
    assert!(!autosave.tick(s(13), 4));
    assert!(autosave.tick(s(23), 4));
}

#[test]
fn autosave_backs_off_after_failures_and_recovers() {
    let t0 = Instant::now();
    let s = |secs| t0 + Duration::from_secs(secs);
    let mut autosave = Autosave::new(Duration::from_secs(10));
    autosave.reset(0);
    autosave.tick(s(0), 1);
    assert!(autosave.tick(s(10), 1));
    autosave.failed(s(10));
    assert_eq!(autosave.failures(), 1);
    assert!(!autosave.tick(s(25), 1), "waits two intervals after one failure");
    assert!(autosave.tick(s(30), 1));
    for _ in 0..10 {
        autosave.failed(s(30));
    }
    assert!(!autosave.tick(s(100), 1), "capped at eight intervals");
    assert!(autosave.tick(s(110), 1));
    autosave.saved(1);
    assert_eq!(autosave.failures(), 0);
}

#[test]
fn autosave_can_be_disabled_and_retuned() {
    let t0 = Instant::now();
    let mut autosave = Autosave::new(Duration::from_secs(60));
    autosave.reset(0);
    autosave.set_enabled(false);
    assert!(!autosave.tick(t0 + Duration::from_secs(600), 1));
    assert!(!autosave.is_enabled());
    autosave.set_enabled(true);
    autosave.tick(t0, 1);
    autosave.set_interval(Duration::from_secs(1));
    assert_eq!(autosave.interval(), Duration::from_secs(1));
    assert!(autosave.tick(t0 + Duration::from_secs(1), 1));
}

#[test]
fn autosave_without_reset_treats_the_first_revision_as_changed() {
    let t0 = Instant::now();
    let mut autosave = Autosave::new(Duration::ZERO);
    assert!(autosave.tick(t0, 0));
}
