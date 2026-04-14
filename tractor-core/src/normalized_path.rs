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
    ///
    /// Uses `std::fs::canonicalize` when the file exists to resolve symlinks
    /// and — on Windows — get the true filesystem casing. This prevents
    /// intersection failures caused by `current_dir()` returning a different
    /// casing than `canonicalize()` used on config paths (fix #127).
    pub fn absolute(raw: &str) -> Self {
        let p = Path::new(raw);
        let full = if p.is_absolute() {
            p.to_path_buf()
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd.join(raw)
        } else {
            return Self::new(raw);
        };
        // Prefer canonicalize for consistent casing (Windows) and symlink resolution.
        // Fall back to lexical normalization when the path doesn't exist yet.
        let resolved = std::fs::canonicalize(&full)
            .unwrap_or_else(|_| normalize_lexically(&full));
        Self::new(&resolved.to_string_lossy())
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

    /// Fix #127 bug 1: absolute() must canonicalize existing files so that
    /// CLI paths match config-derived paths regardless of cwd casing.
    #[test]
    fn absolute_canonicalizes_existing_file() {
        // Create a temp file and resolve it via absolute() — the result
        // must match std::fs::canonicalize (true casing on Windows).
        let tmp = std::env::temp_dir().join("tractor_test_canon.txt");
        std::fs::write(&tmp, "").unwrap();

        let canonical = std::fs::canonicalize(&tmp).unwrap();
        let expected = NormalizedPath::new(&canonical.to_string_lossy());
        let actual = NormalizedPath::absolute(&tmp.to_string_lossy());
        assert_eq!(actual, expected, "absolute() should canonicalize existing files");

        std::fs::remove_file(&tmp).ok();
    }

    /// Fix #127 bug 1: absolute() with a relative path to an existing file
    /// must produce the same NormalizedPath as expanding a glob that matches
    /// the same file (both should use canonical casing).
    #[test]
    fn absolute_relative_matches_glob_expanded() {
        // Use Cargo.toml (always present in the repo root) as a known file.
        // The worktree cwd is the repo root, so "Cargo.toml" is resolvable.
        let cwd = std::env::current_dir().unwrap();
        let cargo_toml = cwd.join("Cargo.toml");
        if !cargo_toml.exists() {
            return; // skip if not in repo root
        }

        let from_absolute = NormalizedPath::absolute("Cargo.toml");
        let canonical = std::fs::canonicalize(&cargo_toml).unwrap();
        let from_canonical = NormalizedPath::new(&canonical.to_string_lossy());
        assert_eq!(
            from_absolute, from_canonical,
            "absolute('Cargo.toml') must match canonical form"
        );
    }

    /// Fix #127 bug 1: on Windows, two paths differing only in case must
    /// produce the same NormalizedPath after absolute().
    #[cfg(target_os = "windows")]
    #[test]
    fn absolute_case_insensitive_on_windows() {
        let tmp = std::env::temp_dir().join("tractor_test_CaseCheck.txt");
        std::fs::write(&tmp, "").unwrap();

        let lower = tmp.to_string_lossy().to_lowercase();
        let upper = tmp.to_string_lossy().to_uppercase();

        let from_lower = NormalizedPath::absolute(&lower);
        let from_upper = NormalizedPath::absolute(&upper);
        assert_eq!(
            from_lower, from_upper,
            "absolute() must produce the same path regardless of input casing on Windows"
        );

        std::fs::remove_file(&tmp).ok();
    }
}
