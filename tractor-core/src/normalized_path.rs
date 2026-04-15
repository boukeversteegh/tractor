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

    /// Make a relative path absolute by joining with `cwd`, normalizing
    /// lexically (collapsing `.`/`..`) and — on Windows — case-correcting
    /// each existing path component against the true filesystem casing.
    ///
    /// This deliberately does **not** resolve symlinks, on any platform:
    /// a symlink passed on the CLI stays at the symlink path, so it can
    /// still intersect with glob-expanded paths that also use the link
    /// name. This matches the glob walker's behavior (which builds paths
    /// from `read_dir` entry names).
    ///
    /// Missing components are kept as-is without case correction.
    pub fn absolute(raw: &str) -> Self {
        let p = Path::new(raw);
        let full = if p.is_absolute() {
            p.to_path_buf()
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd.join(raw)
        } else {
            return Self::new(raw);
        };
        let lexical = normalize_lexically(&full);
        let resolved = case_correct_existing(&lexical);
        Self::new(&resolved.to_string_lossy())
    }
}

/// Case-correct each existing component of an absolute path against the
/// true filesystem casing. No-op on non-Windows platforms.
///
/// On Windows, walks the path component by component, calling `read_dir`
/// on each parent to find the entry whose name matches case-insensitively,
/// then appends that entry's name (which has true filesystem casing).
/// Does not resolve symlinks — unlike `std::fs::canonicalize`.
///
/// Stops correcting at the first missing component (pushes the remaining
/// components as-is). Paths where the parent can't be read (permissions,
/// missing) are returned unchanged from that point.
#[cfg(not(windows))]
fn case_correct_existing(path: &Path) -> PathBuf {
    path.to_path_buf()
}

#[cfg(windows)]
fn case_correct_existing(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    let mut components = path.components();

    // Copy the prefix (drive letter) and root directory verbatim.
    // Case of a drive letter is not something `read_dir` can tell us;
    // upstream `normalize_path` / the rest of the pipeline handles that.
    while let Some(comp) = components.clone().next() {
        match comp {
            Component::Prefix(_) | Component::RootDir => {
                result.push(comp.as_os_str());
                components.next();
            }
            _ => break,
        }
    }

    // Case-correct remaining components via read_dir.
    loop {
        let comp = match components.next() {
            Some(c) => c,
            None => return result,
        };
        let name = comp.as_os_str();
        let entries = match std::fs::read_dir(&result) {
            Ok(e) => e,
            Err(_) => {
                // Parent can't be read — stop correcting, push rest as-is.
                result.push(name);
                for rest in components {
                    result.push(rest.as_os_str());
                }
                return result;
            }
        };

        let mut found: Option<std::ffi::OsString> = None;
        for entry in entries.flatten() {
            let entry_name = entry.file_name();
            if entry_name.to_string_lossy()
                .eq_ignore_ascii_case(&name.to_string_lossy())
            {
                found = Some(entry_name);
                break;
            }
        }

        match found {
            Some(n) => result.push(n),
            None => {
                // Missing component — push as-is and stop correcting
                // (we can't read_dir into something that doesn't exist).
                result.push(name);
                for rest in components {
                    result.push(rest.as_os_str());
                }
                return result;
            }
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

    /// absolute() must produce an absolute, normalized path for an existing
    /// file. Symlinks are NOT resolved, so the result keeps the input path's
    /// identity (this is what lets CLI paths intersect with glob-expanded
    /// paths that both reference the same symlink).
    #[test]
    fn absolute_on_existing_file_is_absolute() {
        let tmp = std::env::temp_dir().join("tractor_test_abs.txt");
        std::fs::write(&tmp, "").unwrap();

        let actual = NormalizedPath::absolute(&tmp.to_string_lossy());
        assert!(
            std::path::Path::new(actual.as_str()).is_absolute(),
            "absolute() must return an absolute path: {}",
            actual
        );

        std::fs::remove_file(&tmp).ok();
    }

    /// A relative path and its absolute counterpart must produce the same
    /// NormalizedPath — this is what lets CLI-supplied relative paths
    /// intersect with config-derived absolute paths (fix #127).
    #[test]
    fn absolute_relative_and_absolute_agree() {
        let cwd = std::env::current_dir().unwrap();
        let cargo_toml = cwd.join("Cargo.toml");
        if !cargo_toml.exists() {
            return; // skip if not in repo root
        }

        let from_relative = NormalizedPath::absolute("Cargo.toml");
        let from_absolute = NormalizedPath::absolute(&cargo_toml.to_string_lossy());
        assert_eq!(
            from_relative, from_absolute,
            "absolute('Cargo.toml') and absolute('/abs/.../Cargo.toml') must agree"
        );
    }

    /// Unix-only regression test for the Copilot #128 review: `absolute()`
    /// must NOT resolve symlinks. A CLI arg of `link.txt` pointing at
    /// `target.txt` must stay `link.txt`, so it can intersect with
    /// glob-expanded paths that also use the link name.
    #[cfg(unix)]
    #[test]
    fn absolute_does_not_resolve_unix_symlinks() {
        use std::os::unix::fs::symlink;
        let dir = std::env::temp_dir().join("tractor_test_symlink");
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("target.txt");
        let link = dir.join("link.txt");
        std::fs::write(&target, "").unwrap();
        // Remove stale link from a previous run, if any.
        let _ = std::fs::remove_file(&link);
        symlink(&target, &link).unwrap();

        let absolute = NormalizedPath::absolute(&link.to_string_lossy());
        assert!(
            absolute.as_str().ends_with("/link.txt"),
            "symlink should be preserved, not resolved to target; got: {}",
            absolute
        );
        assert!(
            !absolute.as_str().ends_with("/target.txt"),
            "symlink was resolved to target — this is the #128 review bug: {}",
            absolute
        );

        // Also: absolute(link) should intersect with a HashSet built from
        // the link path (what a glob walker would produce).
        let mut set: HashSet<NormalizedPath> = HashSet::new();
        set.insert(NormalizedPath::new(&link.to_string_lossy()));
        assert!(
            set.contains(&absolute),
            "CLI-resolved symlink path must intersect with glob-walker path"
        );

        std::fs::remove_file(&link).ok();
        std::fs::remove_file(&target).ok();
        std::fs::remove_dir(&dir).ok();
    }

    /// Fix #127 bug 1: on Windows, two paths differing only in case must
    /// produce the same NormalizedPath after absolute(). Handled by the
    /// Windows-only `case_correct_existing` helper, which walks each
    /// component via `read_dir` to find the true filesystem casing
    /// (without following symlinks, unlike `std::fs::canonicalize`).
    #[cfg(windows)]
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
