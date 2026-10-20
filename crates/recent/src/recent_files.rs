//! The File > Open Recent list.

use serde::{Deserialize, Deserializer, Serialize};
use std::path::{Path, PathBuf};

/// How many entries [`RecentFiles::default`] keeps.
pub const DEFAULT_RECENT_LIMIT: usize = 10;

/// The most recently opened documents, newest first, without duplicates.
///
/// Serializes as `{ limit = 10, files = ["…", …] }`. Loading is forgiving: a missing or invalid
/// `limit` falls back to the default, duplicates and empty paths are dropped, and a list longer
/// than the limit is cut, so a hand-edited settings file never fails to load because of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecentFiles {
    limit: usize,
    files: Vec<PathBuf>,
}

impl Default for RecentFiles {
    fn default() -> Self {
        Self::with_limit(DEFAULT_RECENT_LIMIT)
    }
}

impl RecentFiles {
    /// An empty list that keeps at most `limit` entries (at least one).
    pub fn with_limit(limit: usize) -> Self {
        Self { limit: limit.max(1), files: Vec::new() }
    }

    /// Puts `path` first, removing an older entry for the same file. Empty paths are ignored.
    pub fn add(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();
        if path.as_os_str().is_empty() {
            return;
        }
        self.files.retain(|known| !same_path(known, &path));
        self.files.insert(0, path);
        self.files.truncate(self.limit);
    }

    /// Removes `path` (for example after it failed to open). Returns whether it was listed.
    pub fn remove(&mut self, path: &Path) -> bool {
        let before = self.files.len();
        self.files.retain(|known| !same_path(known, path));
        self.files.len() != before
    }

    /// Records that a listed file was moved or renamed, keeping its place in the list.
    pub fn rename(&mut self, from: &Path, to: impl Into<PathBuf>) {
        let to = to.into();
        if to.as_os_str().is_empty() {
            return;
        }
        let Some(index) = self.files.iter().position(|known| same_path(known, from)) else {
            return;
        };
        self.files.retain(|known| !same_path(known, &to) || same_path(known, from));
        if let Some(slot) = self.files.iter_mut().find(|known| same_path(known, from)) {
            *slot = to;
        } else {
            self.files.insert(index.min(self.files.len()), to);
        }
    }

    /// Drops every entry for which `keep` returns false, such as files that no longer exist:
    /// `recent.retain(|p| p.exists())`. Returns how many were dropped.
    pub fn retain(&mut self, mut keep: impl FnMut(&Path) -> bool) -> usize {
        let before = self.files.len();
        self.files.retain(|p| keep(p));
        before - self.files.len()
    }

    /// Empties the list (File > Open Recent > Clear Menu).
    pub fn clear(&mut self) {
        self.files.clear();
    }

    /// Changes the limit (at least one), dropping the oldest entries beyond it.
    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit.max(1);
        self.files.truncate(self.limit);
    }

    /// The maximum number of entries.
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// The entries, newest first.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Path> {
        self.files.iter().map(PathBuf::as_path)
    }

    /// The newest entry.
    pub fn latest(&self) -> Option<&Path> {
        self.files.first().map(PathBuf::as_path)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Whether `path` is listed.
    pub fn contains(&self, path: &Path) -> bool {
        self.files.iter().any(|known| same_path(known, path))
    }
}

impl<'de> Deserialize<'de> for RecentFiles {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Lenient {
            Path(PathBuf),
            Other(serde::de::IgnoredAny),
        }

        /// Accepts anything for each field and keeps what makes sense.
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct Shape {
            limit: LenientLimit,
            files: Vec<Lenient>,
        }

        #[derive(Default)]
        struct LenientLimit(Option<usize>);

        impl<'de> Deserialize<'de> for LenientLimit {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                #[derive(Deserialize)]
                #[serde(untagged)]
                enum Any {
                    Number(u64),
                    Other(serde::de::IgnoredAny),
                }
                Ok(match Any::deserialize(d)? {
                    Any::Number(n) => LenientLimit(usize::try_from(n).ok().filter(|n| *n > 0)),
                    Any::Other(_) => LenientLimit(None),
                })
            }
        }

        let shape = Shape::deserialize(deserializer)?;
        let mut list = RecentFiles::with_limit(shape.limit.0.unwrap_or(DEFAULT_RECENT_LIMIT));
        // Re-add oldest first so the newest keeps its place and duplicates collapse.
        for entry in shape.files.into_iter().rev() {
            if let Lenient::Path(path) = entry {
                list.add(path);
            }
        }
        Ok(list)
    }
}

/// Whether two paths name the same recent-files entry.
///
/// Compares the paths as written, ignoring case on Windows and macOS, whose default file systems
/// are case-insensitive. It does not touch the disk, so it works for files that are offline.
pub fn same_path(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    if cfg!(any(windows, target_os = "macos")) {
        let mut left = a.components();
        let mut right = b.components();
        loop {
            match (left.next(), right.next()) {
                (None, None) => return true,
                (Some(x), Some(y)) => {
                    if x.as_os_str().to_string_lossy().to_lowercase() != y.as_os_str().to_string_lossy().to_lowercase() {
                        return false;
                    }
                }
                _ => return false,
            }
        }
    }
    false
}
