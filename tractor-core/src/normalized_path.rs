//! A typed wrapper for file paths that guarantees normalization.
//!
//! All paths use forward slashes and have the Windows `\\?\` prefix stripped.
//! This makes `HashSet` intersection and string comparison work correctly
//! across platforms — the type system enforces it.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};

use crate::output::normalize_path as normalize_path_str;

/// A file path that has been normalized (forward slashes, no `\\?\` prefix).
///
/// Constructed via [`NormalizedPath::new`] or [`From<String>`].
/// All paths go through normalization so the invariant holds by construction.
#[derive(Debug, Clone, Eq)]
pub struct NormalizedPath(String);

impl NormalizedPath {
    /// Normalize a raw path string.
    pub fn new(raw: &str) -> Self {
        Self(normalize_path_str(raw))
    }

    /// Return the normalized path as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume and return the inner String.
    pub fn into_string(self) -> String {
        self.0
    }

    /// Make a relative path absolute by joining with `cwd`.
    /// If already absolute, just normalizes.
    pub fn absolute(raw: &str) -> Self {
        let p = Path::new(raw);
        if p.is_absolute() {
            Self::new(&normalize_lexically(p).to_string_lossy())
        } else if let Ok(cwd) = std::env::current_dir() {
            Self::new(&normalize_lexically(&cwd.join(raw)).to_string_lossy())
        } else {
            Self::new(raw)
        }
    }
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let last = normalized.components().next_back();
                if matches!(last, Some(Component::Normal(_))) {
                    normalized.pop();
                } else if !path.is_absolute() && !matches!(last, Some(Component::RootDir | Component::Prefix(_))) {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }

    normalized
}

// ---- Display / conversions ----

impl fmt::Display for NormalizedPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl PartialEq for NormalizedPath {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl PartialEq<str> for NormalizedPath {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for NormalizedPath {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl Hash for NormalizedPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl AsRef<str> for NormalizedPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<Path> for NormalizedPath {
    fn as_ref(&self) -> &Path {
        Path::new(&self.0)
    }
}

impl AsRef<std::ffi::OsStr> for NormalizedPath {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.0.as_ref()
    }
}

impl std::borrow::Borrow<str> for NormalizedPath {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl From<&str> for NormalizedPath {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for NormalizedPath {
    fn from(s: String) -> Self {
        Self::new(&s)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn normalizes_backslashes() {
        assert_eq!(NormalizedPath::new("src\\foo.rs"), "src/foo.rs");
    }

    #[test]
    fn strips_windows_prefix() {
        assert_eq!(
            NormalizedPath::new("//?/C:/project/src/foo.rs"),
            "C:/project/src/foo.rs"
        );
    }

    #[test]
    fn hashset_intersection_works_across_separators() {
        let mut set: HashSet<NormalizedPath> = HashSet::new();
        set.insert(NormalizedPath::new("src\\foo.rs"));
        assert!(set.contains("src/foo.rs"));
    }

    #[test]
    fn forward_slashes_unchanged() {
        assert_eq!(NormalizedPath::new("src/foo.rs"), "src/foo.rs");
    }

    #[test]
    fn display_shows_normalized() {
        let p = NormalizedPath::new("src\\main.rs");
        assert_eq!(format!("{}", p), "src/main.rs");
    }

    #[test]
    fn absolute_collapses_current_dir_segments() {
        let cwd = std::env::current_dir().unwrap();
        let expected = NormalizedPath::new(&cwd.join("src/foo.rs").to_string_lossy());
        assert_eq!(NormalizedPath::absolute("./src/./foo.rs"), expected);
    }

    #[test]
    fn absolute_collapses_parent_segments() {
        let cwd = std::env::current_dir().unwrap();
        let expected = NormalizedPath::new(&cwd.join("src/foo.rs").to_string_lossy());
        assert_eq!(NormalizedPath::absolute("nested/../src/foo.rs"), expected);
    }
}
