//! Custom glob matching and canonical filesystem walking.
//!
//! Replaces the `glob` crate with a faster implementation that produces
//! canonically-cased paths on Windows by building paths from `read_dir`
//! entry names (which have true filesystem casing).
//!
//! Two capabilities:
//! - [`CompiledPattern`]: pattern matching against path strings (always available, WASM-safe)
//! - [`expand_canonical`]: filesystem walk with pattern matching (native only)

use std::fmt;

use crate::output::normalize_path;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Error compiling a glob pattern.
#[derive(Debug, Clone)]
pub struct GlobCompileError {
    pub pattern: String,
    pub message: String,
}

impl fmt::Display for GlobCompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid glob pattern '{}': {}", self.pattern, self.message)
    }
}

impl std::error::Error for GlobCompileError {}

// ---------------------------------------------------------------------------
// Pattern segments
// ---------------------------------------------------------------------------

/// A single segment of a compiled glob pattern (one path component).
#[derive(Debug, Clone)]
enum Segment {
    /// Exact literal match (e.g. "src", "lib").
    Literal(String),
    /// Wildcard match within a single path component (e.g. "*.rs", "test_*").
    Wild {
        /// Text before the first `*` (empty if pattern starts with `*`).
        prefix: String,
        /// Text after the last `*` (empty if pattern ends with `*`).
        suffix: String,
        /// Fragments between consecutive `*`s (for patterns like "a*b*c").
        inner: Vec<String>,
    },
    /// Matches zero or more path segments.
    DoubleStar,
}

// ---------------------------------------------------------------------------
// CompiledPattern
// ---------------------------------------------------------------------------

/// A compiled glob pattern for matching against normalized path strings.
///
/// Supports `*` (single-segment wildcard) and `**` (recursive).
/// Case sensitivity is determined at compile time based on the target OS.
#[derive(Debug, Clone)]
pub struct CompiledPattern {
    segments: Vec<Segment>,
    case_insensitive: bool,
}

impl CompiledPattern {
    /// Compile a glob pattern string.
    ///
    /// The pattern uses `/` as separator (normalized). Supported wildcards:
    /// - `*` matches any characters within a single path component
    /// - `**` matches zero or more path components
    ///
    /// Returns an error for unsupported syntax: character classes (`[...]`)
    /// and single-character wildcards (`?`).
    pub fn new(pattern: &str) -> Result<Self, GlobCompileError> {
        let normalized = normalize_path(pattern);

        if normalized.contains('[') {
            return Err(GlobCompileError {
                pattern: normalized,
                message: "character classes [...] are not supported".into(),
            });
        }

        if normalized.contains('?') {
            return Err(GlobCompileError {
                pattern: normalized,
                message: "`?` single-character wildcard is not supported; use `*` instead".into(),
            });
        }

        let case_insensitive = cfg!(target_os = "windows");

        let parts: Vec<&str> = normalized.split('/').collect();
        let mut segments = Vec::with_capacity(parts.len());

        for part in &parts {
            if part.is_empty() && segments.is_empty() {
                // Leading empty part from absolute path like "/home/..." or "C:/..."
                // Skip — the literal root is handled by the full pattern prefix.
                continue;
            }

            if *part == "**" {
                // Collapse consecutive ** segments
                if !matches!(segments.last(), Some(Segment::DoubleStar)) {
                    segments.push(Segment::DoubleStar);
                }
            } else if part.contains('*') {
                segments.push(compile_wild_segment(part, case_insensitive));
            } else {
                let lit = if case_insensitive {
                    part.to_ascii_lowercase()
                } else {
                    part.to_string()
                };
                segments.push(Segment::Literal(lit));
            }
        }

        // Handle absolute path prefix: store as a single literal segment
        // e.g. "C:/Work/Repo/src/**/*.rs" → Literal("C:"), Literal("Work"), ...
        // We need to re-parse to capture the drive letter / root
        let segments = if normalized.starts_with('/') || (normalized.len() >= 2 && normalized.as_bytes()[1] == b':') {
            // Absolute path — rebuild segments including the root parts
            let mut abs_segments = Vec::new();
            for (i, part) in parts.iter().enumerate() {
                if part.is_empty() && i == 0 {
                    continue; // skip leading empty from "/"
                }
                if *part == "**" {
                    if !matches!(abs_segments.last(), Some(Segment::DoubleStar)) {
                        abs_segments.push(Segment::DoubleStar);
                    }
                } else if part.contains('*') {
                    abs_segments.push(compile_wild_segment(part, case_insensitive));
                } else {
                    let lit = if case_insensitive {
                        part.to_ascii_lowercase()
                    } else {
                        part.to_string()
                    };
                    abs_segments.push(Segment::Literal(lit));
                }
            }
            abs_segments
        } else {
            segments
        };

        Ok(CompiledPattern { segments, case_insensitive })
    }

    /// Does `path` match this pattern?
    ///
    /// `path` should be a normalized path string (forward slashes).
    pub fn matches(&self, path: &str) -> bool {
        let path_parts = split_path(path);
        match_segments(&self.segments, &path_parts, self.case_insensitive)
    }
}

/// Compile a single wildcard segment (contains `*`).
fn compile_wild_segment(part: &str, case_insensitive: bool) -> Segment {
    let part = if case_insensitive {
        part.to_ascii_lowercase()
    } else {
        part.to_string()
    };

    // Split on `*` to get prefix, inner fragments, and suffix.
    let chunks: Vec<&str> = part.split('*').collect();
    let prefix = chunks[0].to_string();
    let suffix = chunks[chunks.len() - 1].to_string();
    let inner: Vec<String> = if chunks.len() > 2 {
        chunks[1..chunks.len() - 1].iter().map(|s| s.to_string()).collect()
    } else {
        vec![]
    };

    Segment::Wild { prefix, suffix, inner }
}

/// Split a path into components, handling drive letters and absolute paths.
fn split_path(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty()).collect()
}

/// Recursive segment-level matching.
fn match_segments(segments: &[Segment], path_parts: &[&str], case_insensitive: bool) -> bool {
    let mut si = 0; // segment index
    let mut pi = 0; // path part index

    // Track DoubleStar backtrack points: (segment_index_after_star, min_path_index)
    let mut star_stack: Vec<(usize, usize)> = Vec::new();

    loop {
        if si == segments.len() && pi == path_parts.len() {
            return true; // both exhausted — match
        }

        if si < segments.len() {
            match &segments[si] {
                Segment::DoubleStar => {
                    si += 1;
                    // ** at end of pattern matches everything remaining
                    if si == segments.len() {
                        return true;
                    }
                    // Push backtrack point: try consuming 0 parts first
                    star_stack.push((si, pi));
                    continue;
                }
                segment if pi < path_parts.len() => {
                    let part = path_parts[pi];
                    if match_one_segment(segment, part, case_insensitive) {
                        si += 1;
                        pi += 1;
                        continue;
                    }
                    // Fall through to backtrack
                }
                _ => {
                    // Segment exists but no path parts left — fall through to backtrack
                }
            }
        }

        // Backtrack: try consuming one more path part at the most recent **
        if let Some(last) = star_stack.last_mut() {
            last.1 += 1; // consume one more path part
            if last.1 <= path_parts.len() {
                si = last.0;
                pi = last.1;
                continue;
            } else {
                star_stack.pop();
            }
        }

        // Try popping further back in the stack
        while let Some(last) = star_stack.last_mut() {
            last.1 += 1;
            if last.1 <= path_parts.len() {
                si = last.0;
                pi = last.1;
                break;
            } else {
                star_stack.pop();
            }
        }

        if star_stack.is_empty() {
            return false; // no more backtrack options
        }
    }
}

/// Match a single non-DoubleStar segment against a single path component.
fn match_one_segment(segment: &Segment, part: &str, case_insensitive: bool) -> bool {
    match segment {
        Segment::Literal(lit) => {
            if case_insensitive {
                part.eq_ignore_ascii_case(lit)
            } else {
                part == lit
            }
        }
        Segment::Wild { prefix, suffix, inner } => {
            let p = if case_insensitive {
                part.to_ascii_lowercase()
            } else {
                part.to_string()
            };

            // Check prefix
            if !p.starts_with(prefix.as_str()) {
                return false;
            }
            // Check suffix
            if !p.ends_with(suffix.as_str()) {
                return false;
            }

            // Check that prefix + suffix don't overlap
            if prefix.len() + suffix.len() > p.len() {
                return false;
            }

            // Check inner fragments appear in order between prefix and suffix
            if inner.is_empty() {
                return true;
            }

            let mut search_start = prefix.len();
            let search_end = p.len() - suffix.len();
            for frag in inner {
                if frag.is_empty() {
                    continue; // consecutive ** collapsed
                }
                if let Some(pos) = p[search_start..search_end].find(frag.as_str()) {
                    search_start = search_start + pos + frag.len();
                } else {
                    return false;
                }
            }
            true
        }
        Segment::DoubleStar => unreachable!("DoubleStar handled in match_segments"),
    }
}

// ---------------------------------------------------------------------------
// Filesystem walking (native only)
// ---------------------------------------------------------------------------

#[cfg(feature = "native")]
mod walk {
    use super::*;
    use crate::NormalizedPath;

    /// Maximum directory depth to prevent symlink cycle hangs.
    const MAX_DEPTH: usize = 100;

    /// Classifies why a glob expansion failed, so callers can react
    /// structurally instead of string-matching the error message.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum GlobExpandErrorKind {
        /// The expansion produced more paths than the caller's limit allowed.
        LimitExceeded,
        /// The pattern itself was invalid (e.g. unsupported syntax).
        InvalidPattern,
    }

    /// Error from canonical glob expansion.
    #[derive(Debug, Clone)]
    pub struct GlobExpandError {
        pub pattern: String,
        pub message: String,
        pub kind: GlobExpandErrorKind,
    }

    impl fmt::Display for GlobExpandError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "glob expansion failed for '{}': {}", self.pattern, self.message)
        }
    }

    impl std::error::Error for GlobExpandError {}

    /// Expand a glob pattern into canonically-cased file paths.
    ///
    /// The root (non-wildcard prefix) is canonicalized once. File and directory
    /// names come from `read_dir` entries which have true filesystem casing.
    /// Symlinks are traversed but use the link name, not the target path.
    ///
    /// Returns at most `limit` files. Returns an error if the limit is exceeded.
    pub fn expand_canonical(
        pattern: &str,
        limit: usize,
    ) -> Result<Vec<NormalizedPath>, GlobExpandError> {
        let normalized = normalize_path(pattern);

        // Non-glob pattern: return as-is
        if !normalized.contains('*') {
            return Ok(vec![NormalizedPath::new(&normalized)]);
        }

        // Split into root prefix (literal) and wildcard suffix
        let parts: Vec<&str> = normalized.split('/').collect();
        let first_wild = parts.iter().position(|p| p.contains('*'))
            .unwrap(); // safe: we checked for wildcards above

        let root = if first_wild == 0 {
            // Pattern starts with wildcard (e.g. "**/*.rs") — root is cwd
            ".".to_string()
        } else {
            parts[..first_wild].join("/")
        };

        let suffix_pattern = parts[first_wild..].join("/");

        // Resolve the root: absolute + lexically normalized, with true
        // filesystem casing on Windows. Does NOT resolve symlinks — paths
        // built from `read_dir` further down keep the symlink name, so
        // glob-expanded paths match CLI args that also use the link name.
        if !std::path::Path::new(&root).exists() {
            return Ok(vec![]); // root doesn't exist → empty
        }
        let canonical_root = NormalizedPath::absolute(&root).into_string();

        // Compile the wildcard suffix for matching
        let compiled = CompiledPattern::new(&suffix_pattern).map_err(|e| GlobExpandError {
            pattern: pattern.to_string(),
            message: e.message,
            kind: GlobExpandErrorKind::InvalidPattern,
        })?;

        let mut results = Vec::new();
        walk_dir(&canonical_root, "", &compiled, limit, &mut results, 0)
            .map_err(|_| GlobExpandError {
                pattern: pattern.to_string(),
                message: format!("exceeded {} file limit", limit),
                kind: GlobExpandErrorKind::LimitExceeded,
            })?;

        Ok(results)
    }

    /// Marker error type returned when the walker exceeds the expansion limit.
    struct WalkLimitExceeded;

    /// Recursively walk a directory, matching relative paths against the compiled pattern.
    fn walk_dir(
        root: &str,
        relative: &str,
        pattern: &CompiledPattern,
        limit: usize,
        results: &mut Vec<NormalizedPath>,
        depth: usize,
    ) -> Result<(), WalkLimitExceeded> {
        if depth > MAX_DEPTH {
            eprintln!(
                "warning: glob walker reached max depth ({}) at '{}{}{}' — \
                 skipping deeper entries (may indicate a symlink cycle or unusually deep tree)",
                MAX_DEPTH,
                root,
                if relative.is_empty() { "" } else { "/" },
                relative,
            );
            return Ok(());
        }

        let full_dir = if relative.is_empty() {
            root.to_string()
        } else {
            format!("{}/{}", root, relative)
        };

        let entries = match std::fs::read_dir(&full_dir) {
            Ok(e) => e,
            Err(_) => return Ok(()), // permission denied, etc. — skip silently
        };

        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            let child_relative = if relative.is_empty() {
                name_str.to_string()
            } else {
                format!("{}/{}", relative, name_str)
            };

            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            if file_type.is_file() || (file_type.is_symlink() && entry.path().is_file()) {
                if pattern.matches(&child_relative) {
                    results.push(NormalizedPath::new(&format!("{}/{}", root, child_relative)));
                    if results.len() > limit {
                        return Err(WalkLimitExceeded);
                    }
                }
            }

            if file_type.is_dir() || (file_type.is_symlink() && entry.path().is_dir()) {
                walk_dir(root, &child_relative, pattern, limit, results, depth + 1)?;
            }
        }

        Ok(())
    }
}

#[cfg(feature = "native")]
pub use walk::{expand_canonical, GlobExpandError, GlobExpandErrorKind};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- CompiledPattern matching tests --

    fn matches(pattern: &str, path: &str) -> bool {
        CompiledPattern::new(pattern).unwrap().matches(path)
    }

    #[test]
    fn literal_exact_match() {
        assert!(matches("src/main.rs", "src/main.rs"));
        assert!(!matches("src/main.rs", "src/lib.rs"));
        assert!(!matches("src/main.rs", "src/main.rs/extra"));
    }

    #[test]
    fn star_matches_within_segment() {
        assert!(matches("*.rs", "main.rs"));
        assert!(matches("*.rs", "lib.rs"));
        assert!(!matches("*.rs", "main.ts"));
        assert!(!matches("*.rs", "src/main.rs")); // * doesn't cross /
    }

    #[test]
    fn star_prefix_and_suffix() {
        assert!(matches("test_*", "test_foo"));
        assert!(matches("test_*", "test_"));
        assert!(!matches("test_*", "other_foo"));

        assert!(matches("*_test.rs", "foo_test.rs"));
        assert!(!matches("*_test.rs", "foo_test.ts"));
    }

    #[test]
    fn star_in_middle() {
        assert!(matches("src/*.rs", "src/main.rs"));
        assert!(matches("src/*.rs", "src/lib.rs"));
        assert!(!matches("src/*.rs", "test/main.rs"));
        assert!(!matches("src/*.rs", "src/sub/main.rs")); // * doesn't cross /
    }

    #[test]
    fn double_star_matches_zero_segments() {
        assert!(matches("**/*.rs", "main.rs"));
        assert!(matches("src/**/*.rs", "src/main.rs"));
    }

    #[test]
    fn double_star_matches_multiple_segments() {
        assert!(matches("**/*.rs", "src/main.rs"));
        assert!(matches("**/*.rs", "src/deep/nested/lib.rs"));
        assert!(matches("src/**/*.rs", "src/a/b/c/d.rs"));
    }

    #[test]
    fn double_star_at_end_matches_all_files() {
        assert!(matches("src/**", "src/main.rs"));
        assert!(matches("src/**", "src/a/b/c.rs"));
        assert!(!matches("src/**", "test/main.rs"));
    }

    #[test]
    fn double_star_in_middle() {
        assert!(matches("src/**/test/*.rs", "src/test/foo.rs"));
        assert!(matches("src/**/test/*.rs", "src/a/b/test/foo.rs"));
        assert!(!matches("src/**/test/*.rs", "src/a/b/foo.rs"));
    }

    #[test]
    fn standalone_double_star() {
        assert!(matches("**", "anything"));
        assert!(matches("**", "a/b/c/d.rs"));
    }

    #[test]
    fn multiple_stars_in_segment() {
        assert!(matches("*_test_*.rs", "foo_test_bar.rs"));
        assert!(!matches("*_test_*.rs", "foo_bar.rs"));
    }

    #[test]
    fn absolute_path_patterns() {
        assert!(matches("C:/Work/Repo/src/**/*.rs", "C:/Work/Repo/src/main.rs"));
        assert!(matches("C:/Work/Repo/src/**/*.rs", "C:/Work/Repo/src/a/b.rs"));
        assert!(!matches("C:/Work/Repo/src/**/*.rs", "D:/Other/src/main.rs"));
    }

    #[test]
    fn pattern_no_match_too_short() {
        assert!(!matches("src/a/b/*.rs", "src/a/main.rs"));
    }

    #[test]
    fn pattern_no_match_too_long() {
        assert!(!matches("src/*.rs", "src/a/b/main.rs"));
    }

    #[test]
    fn empty_pattern_matches_empty_path() {
        // Edge case: both empty
        assert!(CompiledPattern::new("").unwrap().matches(""));
    }

    #[test]
    fn rejects_character_classes() {
        assert!(CompiledPattern::new("src/[abc]/*.rs").is_err());
    }

    #[test]
    fn rejects_question_mark_wildcard() {
        // `?` was previously silently accepted but never actually matched —
        // it's now rejected at compile time with a clear error.
        let err = CompiledPattern::new("file?.rs").unwrap_err();
        assert!(err.message.contains("`?`"), "error should mention `?`: {}", err.message);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn case_insensitive_on_windows() {
        assert!(matches("SRC/**/*.RS", "src/main.rs"));
        assert!(matches("src/**/*.rs", "SRC/MAIN.RS"));
        assert!(matches("C:/Work/Repo/**", "c:/work/repo/file.rs"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn case_sensitive_on_unix() {
        assert!(!matches("SRC/**/*.RS", "src/main.rs"));
        assert!(matches("src/**/*.rs", "src/main.rs"));
    }

    #[test]
    fn backslash_normalized() {
        // Backslashes in pattern are normalized to forward slashes
        assert!(matches("src\\**\\*.rs", "src/main.rs"));
    }

    // -- Filesystem walk tests (native only) --

    #[cfg(feature = "native")]
    mod walk_tests {
        use super::super::*;

        #[test]
        fn expand_non_glob_returns_as_is() {
            let result = expand_canonical("some/literal/path.rs", 100).unwrap();
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].as_str(), "some/literal/path.rs");
        }

        #[test]
        fn expand_nonexistent_root_returns_empty() {
            let result = expand_canonical("/nonexistent/path/**/*.rs", 100).unwrap();
            assert!(result.is_empty());
        }

        #[test]
        fn expand_finds_real_files() {
            // Use the tractor-core src directory which we know exists
            let cwd = std::env::current_dir().unwrap();
            let pattern = format!("{}/**/*.rs",
                normalize_path(&cwd.join("tractor-core/src").to_string_lossy()));

            // Only run if we're in the repo root
            if !cwd.join("tractor-core/src").exists() {
                return;
            }

            let result = expand_canonical(&pattern, 10000).unwrap();
            assert!(!result.is_empty(), "should find .rs files");

            // All paths should be normalized (forward slashes, no \\?\)
            for p in &result {
                assert!(!p.as_str().contains('\\'), "path should use forward slashes: {}", p);
                assert!(!p.as_str().contains("\\\\?\\"), "path should not have \\\\?\\ prefix: {}", p);
            }

            // Should find files we know exist
            let names: Vec<&str> = result.iter()
                .map(|p| p.as_str().rsplit('/').next().unwrap())
                .collect();
            assert!(names.contains(&"lib.rs"), "should find lib.rs");
        }

        #[test]
        fn expand_respects_limit() {
            let cwd = std::env::current_dir().unwrap();
            let pattern = format!("{}/**/*.rs",
                normalize_path(&cwd.join("tractor-core/src").to_string_lossy()));

            if !cwd.join("tractor-core/src").exists() {
                return;
            }

            let result = expand_canonical(&pattern, 1);
            let err = result.expect_err("should fail when exceeding limit of 1");
            assert!(matches!(err.kind, GlobExpandErrorKind::LimitExceeded),
                "limit overflow must be classified as LimitExceeded, got {:?}", err.kind);
        }

        /// `expand_canonical` must classify a malformed wildcard suffix as
        /// `InvalidPattern` so callers (e.g. `expand_globs_checked`) don't
        /// confuse it with a limit overflow. Pattern below puts a `[...]`
        /// character class after the first `*`, so it lands in the
        /// `CompiledPattern::new()` failure path.
        #[test]
        fn expand_invalid_pattern_kind() {
            let result = expand_canonical("/tmp/*/[abc]/*.rs", 100);
            let err = result.expect_err("invalid pattern must surface as Err");
            assert!(matches!(err.kind, GlobExpandErrorKind::InvalidPattern),
                "expected InvalidPattern, got {:?}", err.kind);
        }

        /// A symlink cycle (`a/loop → a`) must not hang the walker. The
        /// MAX_DEPTH guard logs a warning and returns `Ok`, so expansion
        /// completes and non-cyclic files inside the cycle are still found.
        #[cfg(unix)]
        #[test]
        fn walk_terminates_on_symlink_cycle() {
            use std::os::unix::fs::symlink;

            let dir = std::env::temp_dir().join("tractor_test_cycle");
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();

            let a = dir.join("a");
            std::fs::create_dir(&a).unwrap();
            std::fs::write(a.join("real.rs"), "").unwrap();
            // a/loop → a creates the cycle.
            symlink(&a, a.join("loop")).unwrap();

            let pattern = format!("{}/**/*.rs", dir.to_string_lossy());
            let result = expand_canonical(&pattern, 10_000)
                .expect("walker must terminate without returning an error");

            // MAX_DEPTH stops the recursion; the real file (at every depth
            // up to the cutoff) shows up in the results.
            assert!(
                result.iter().any(|p| p.as_str().ends_with("/real.rs")),
                "walker should still find real.rs before hitting MAX_DEPTH; got: {:?}",
                result.iter().map(|p| p.as_str()).collect::<Vec<_>>()
            );

            std::fs::remove_dir_all(&dir).ok();
        }

        #[cfg(target_os = "windows")]
        #[test]
        fn expand_returns_canonical_casing() {
            // On Windows, paths should have true filesystem casing
            let cwd = std::env::current_dir().unwrap();
            let pattern = format!("{}/**/*.rs",
                normalize_path(&cwd.join("tractor-core/src").to_string_lossy()));

            if !cwd.join("tractor-core/src").exists() {
                return;
            }

            let result = expand_canonical(&pattern, 10000).unwrap();
            // Verify paths match what canonicalize would return
            for p in result.iter().take(3) {
                let canonical = std::fs::canonicalize(p.as_str())
                    .map(|c| normalize_path(&c.to_string_lossy()))
                    .unwrap_or_else(|_| p.as_str().to_string());
                assert_eq!(p.as_str(), canonical,
                    "expanded path should match canonical form");
            }
        }
    }
}
