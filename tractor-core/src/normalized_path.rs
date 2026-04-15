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

/// Normalize casing and 8.3 short-name aliases of an existing path
/// against the true filesystem form. No-op on non-Windows platforms.
///
/// On Windows we use `GetLongPathNameW`, which:
/// - Resolves 8.3 short-name aliases (`RUNNER~1` → `runneradmin`) that
///   CMD and legacy tools sometimes pass in.
/// - Returns the path using the true filesystem casing.
/// - Does NOT follow symlinks — unlike `std::fs::canonicalize`.
///
/// If the full path doesn't exist, we walk up to the nearest existing
/// ancestor, resolve it, then re-append the missing components as-is.
/// The drive letter is uppercased for consistency regardless.
#[cfg(not(windows))]
fn case_correct_existing(path: &Path) -> PathBuf {
    path.to_path_buf()
}

#[cfg(windows)]
fn case_correct_existing(path: &Path) -> PathBuf {
    // Find the longest existing ancestor to hand to GetLongPathName.
    // `ancestors()` yields `path` itself first, then shorter prefixes;
    // the drive root (e.g. `C:\`) always exists so this eventually
    // finds something.
    let anchor = path.ancestors().find(|p| p.exists());

    let resolved = if let Some(anchor) = anchor {
        let long = get_long_path_name(anchor).unwrap_or_else(|| anchor.to_path_buf());
        // Re-append any trailing components that didn't exist yet.
        let anchor_comps = anchor.components().count();
        let mut result = long;
        for comp in path.components().skip(anchor_comps) {
            result.push(comp.as_os_str());
        }
        result
    } else {
        path.to_path_buf()
    };

    uppercase_drive_letter(resolved)
}

/// Uppercase the ASCII drive letter prefix of a Windows path, if any.
/// `GetLongPathName` preserves whatever casing the caller passed in for
/// the drive letter, so we normalize it here for consistent intersection.
#[cfg(windows)]
fn uppercase_drive_letter(path: PathBuf) -> PathBuf {
    use std::path::Prefix;
    let mut comps = path.components();
    let first = match comps.next() {
        Some(c) => c,
        None => return path,
    };
    let prefix_str: std::ffi::OsString = match first {
        Component::Prefix(p) => match p.kind() {
            Prefix::Disk(letter) => {
                format!("{}:", (letter as char).to_ascii_uppercase()).into()
            }
            Prefix::VerbatimDisk(letter) => {
                format!(r"\\?\{}:", (letter as char).to_ascii_uppercase()).into()
            }
            _ => return path, // unsupported prefix shape — leave untouched
        },
        _ => return path, // no drive prefix — nothing to uppercase
    };
    let mut result = PathBuf::from(prefix_str);
    for c in comps {
        result.push(c.as_os_str());
    }
    result
}

/// Call the Windows `GetLongPathNameW` API to resolve 8.3 short names
/// and get the true filesystem casing, without following symlinks.
/// Returns `None` if the call fails (e.g. the path doesn't exist).
#[cfg(windows)]
fn get_long_path_name(path: &Path) -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    #[link(name = "kernel32")]
    extern "system" {
        fn GetLongPathNameW(
            lpsz_short_path: *const u16,
            lpsz_long_path: *mut u16,
            cch_buffer: u32,
        ) -> u32;
    }

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // First call with a null buffer returns the required size *including*
    // the terminating null; 0 means the call failed (path missing, etc).
    let needed = unsafe { GetLongPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0) };
    if needed == 0 {
        return None;
    }

    let mut buf = vec![0u16; needed as usize];
    // Second call returns the length *excluding* the terminating null.
    let written = unsafe { GetLongPathNameW(wide.as_ptr(), buf.as_mut_ptr(), needed) };
    if written == 0 || written >= needed {
        return None;
    }

    buf.truncate(written as usize);
    Some(PathBuf::from(OsString::from_wide(&buf)))
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
    /// produce the same NormalizedPath after absolute(). This also
    /// covers 8.3 short-name aliases (e.g. `RUNNER~1` passed from CMD
    /// or legacy tools) — both are handled by `GetLongPathNameW`.
    #[cfg(windows)]
    #[test]
    fn absolute_case_insensitive_on_windows() {
        // Use the raw tempdir directly. On GitHub Actions runners this
        // contains the 8.3 short-name alias `RUNNER~1`, which is exactly
        // the CMD-prompt scenario we want to cover.
        let tmp = std::env::temp_dir().join("tractor_test_CaseCheck.txt");
        std::fs::write(&tmp, "").unwrap();

        let path_str = tmp.to_string_lossy();
        let lower = path_str.to_lowercase();
        let upper = path_str.to_uppercase();

        let from_lower = NormalizedPath::absolute(&lower);
        let from_upper = NormalizedPath::absolute(&upper);
        assert_eq!(
            from_lower, from_upper,
            "absolute() must produce the same path regardless of input casing on Windows"
        );

        std::fs::remove_file(&tmp).ok();
    }

    /// Windows: drive letters are always uppercased to give consistent
    /// casing across inputs, even when the filesystem root `read_dir`
    /// lookup can't help (the drive letter is not a `read_dir` entry).
    #[cfg(windows)]
    #[test]
    fn absolute_uppercases_drive_letter_on_windows() {
        let tmp = std::env::temp_dir().join("tractor_test_DriveCase.txt");
        std::fs::write(&tmp, "").unwrap();

        let path_str = tmp.to_string_lossy().into_owned();
        // Force a lowercase drive letter on the input.
        assert!(path_str.len() >= 2 && path_str.as_bytes()[1] == b':');
        let mut lowered = path_str.clone();
        lowered.replace_range(0..1, &path_str[0..1].to_ascii_lowercase());

        let result = NormalizedPath::absolute(&lowered);
        // First character of the result must be an uppercase drive letter.
        let first = result.as_str().chars().next().unwrap();
        assert!(first.is_ascii_uppercase(), "drive letter should be uppercased, got: {}", result);

        std::fs::remove_file(&tmp).ok();
    }

    /// Windows: 8.3 short-name aliases (`RUNNER~1`, `PROGRA~1`, etc.)
    /// must resolve to the long form so that short-name input from CMD
    /// intersects with glob-expanded long-form paths.
    #[cfg(windows)]
    #[test]
    fn absolute_resolves_83_short_names_on_windows() {
        let tmp = std::env::temp_dir().join("tractor_test_ShortName.txt");
        std::fs::write(&tmp, "").unwrap();

        // Resolve to the long form by handing `absolute()` the raw tempdir
        // path (may contain 8.3 on CI) and comparing to the long form
        // obtained via canonicalize (strip `\\?\` via NormalizedPath::new).
        let resolved = NormalizedPath::absolute(&tmp.to_string_lossy());
        let canonical = std::fs::canonicalize(&tmp).unwrap();
        let long_form = NormalizedPath::new(&canonical.to_string_lossy());
        assert_eq!(
            resolved, long_form,
            "absolute() must resolve 8.3 short names to the long form"
        );

        std::fs::remove_file(&tmp).ok();
    }
}
