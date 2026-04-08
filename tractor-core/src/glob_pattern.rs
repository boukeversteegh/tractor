//! A typed wrapper for glob patterns that guarantees path normalization.
//!
//! Glob patterns use forward slashes so they match against [`NormalizedPath`]
//! values. On Windows, `PathBuf::join` produces backslashes which glob
//! interprets as escape characters — this type prevents that by normalizing
//! on construction.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::de::{self, Deserialize, Deserializer};
use serde::{Serialize, Serializer};

use crate::output::normalize_path as normalize_path_str;

/// A glob pattern with normalized path separators (forward slashes).
///
/// Constructed via [`GlobPattern::new`], [`From<String>`], or serde
/// [`Deserialize`]. All patterns go through normalization so matching
/// against [`NormalizedPath`] works correctly on all platforms.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GlobPattern(String);

impl GlobPattern {
    /// Normalize a raw glob pattern string.
    pub fn new(raw: &str) -> Self {
        Self(normalize_path_str(raw))
    }

    /// Return the normalized pattern as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume and return the inner String.
    pub fn into_string(self) -> String {
        self.0
    }

    /// Make a relative pattern absolute by prepending `base_dir`.
    /// Absolute patterns are normalized but otherwise unchanged.
    pub fn resolve(raw: &str, base_dir: &Path) -> Self {
        if Path::new(raw).is_absolute() {
            Self::new(raw)
        } else {
            Self::new(&base_dir.join(raw).to_string_lossy())
        }
    }

    /// Resolve a batch of raw pattern strings against a base directory.
    /// If `base_dir` is None, patterns are normalized but not resolved.
    pub fn resolve_all(patterns: &[String], base_dir: &Option<PathBuf>) -> Vec<Self> {
        if let Some(base) = base_dir {
            patterns.iter().map(|p| Self::resolve(p, base)).collect()
        } else {
            patterns.iter().map(|p| Self::new(p)).collect()
        }
    }
}

// ---- Display / conversions ----

impl fmt::Display for GlobPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for GlobPattern {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for GlobPattern {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for GlobPattern {
    fn from(s: String) -> Self {
        Self::new(&s)
    }
}

// ---- Serde ----

impl Serialize for GlobPattern {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GlobPattern {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer).map_err(de::Error::custom)?;
        Ok(Self::new(&raw))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_backslashes() {
        assert_eq!(GlobPattern::new("src\\**\\*.rs").as_str(), "src/**/*.rs");
    }

    #[test]
    fn resolve_makes_absolute() {
        let p = GlobPattern::resolve("src/**/*.rs", Path::new("/home/user/project"));
        assert_eq!(p.as_str(), "/home/user/project/src/**/*.rs");
    }

    #[test]
    fn resolve_preserves_absolute() {
        let p = GlobPattern::resolve("/abs/src/**/*.rs", Path::new("/home/user/project"));
        assert_eq!(p.as_str(), "/abs/src/**/*.rs");
    }

    #[test]
    fn resolve_all_with_base() {
        let patterns = vec!["src/**/*.rs".into(), "lib/**/*.rs".into()];
        let base = Some(PathBuf::from("/project"));
        let resolved = GlobPattern::resolve_all(&patterns, &base);
        assert_eq!(resolved[0].as_str(), "/project/src/**/*.rs");
        assert_eq!(resolved[1].as_str(), "/project/lib/**/*.rs");
    }

    #[test]
    fn resolve_all_without_base() {
        let patterns = vec!["src\\**\\*.rs".into()];
        let resolved = GlobPattern::resolve_all(&patterns, &None);
        assert_eq!(resolved[0].as_str(), "src/**/*.rs");
    }

    #[test]
    fn serde_roundtrip_normalizes() {
        let p: GlobPattern = serde_json::from_str("\"src\\\\**\\\\*.rs\"").unwrap();
        assert_eq!(p.as_str(), "src/**/*.rs");
    }
}
