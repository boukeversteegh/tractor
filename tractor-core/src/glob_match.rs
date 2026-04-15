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

    /// Extract the literal (wildcard-free) path prefix of a glob pattern.
    ///
    /// * `backend/**/tests/*.cs` → `backend/`
    /// * `/abs/backend/foo.cs`   → `/abs/backend/foo.cs` (no wildcards → full path)
    /// * `**/*.cs`               → `` (pattern starts with a wildcard)
    /// * `foo*bar/baz.cs`        → `` (wildcard inside the first segment)
    ///
    /// The returned prefix is always a path-segment boundary (ends at `/`
    /// or is the full pattern). Used to build [`FilePrune`] constraints.
    pub fn pattern_literal_prefix(pattern: &str) -> String {
        let norm = normalize_path(pattern);
        match norm.find(|c: char| c == '*' || c == '?' || c == '[') {
            None => norm,
            Some(0) => String::new(),
            Some(idx) => {
                let before = &norm[..idx];
                match before.rfind('/') {
                    Some(slash) => norm[..=slash].to_string(),
                    None => String::new(),
                }
            }
        }
    }

    /// Subtree pruning filter for [`expand_canonical`].
    ///
    /// Holds an AND of OR-groups: each inner group is a list of absolute
    /// literal path prefixes; a path passes if, for every group, at least
    /// one prefix is *compatible* with it. "Compatible" means the two
    /// paths lie on the same path branch — either is a path-segment
    /// prefix of the other. This lets the walker descend *toward* a
    /// constraint before reaching it, and keep walking *below* it once
    /// reached.
    ///
    /// Typical construction:
    ///
    /// ```ignore
    /// // CLI passed `backend/**/tests/*.cs`; config-root has its own patterns.
    /// // When expanding config-root globs, prune by CLI prefixes:
    /// let prune = FilePrune::new()
    ///     .with_group([NormalizedPath::absolute("backend")]);
    /// ```
    ///
    /// Prefixes MUST be absolute and case-canonical (i.e. produced by
    /// [`NormalizedPath::absolute`]) so they match the walker's current
    /// path, which is built from `read_dir` entries.
    #[derive(Debug, Default, Clone)]
    pub struct FilePrune {
        groups: Vec<Vec<NormalizedPath>>,
    }

    impl FilePrune {
        pub fn new() -> Self { Self::default() }

        /// Add an OR-group. An empty group (no constraint) is dropped so
        /// it doesn't reject all paths vacuously.
        pub fn with_group(mut self, prefixes: impl IntoIterator<Item = NormalizedPath>) -> Self {
            let g: Vec<NormalizedPath> = prefixes.into_iter()
                .filter(|p| !p.as_str().is_empty())
                .collect();
            if !g.is_empty() {
                self.groups.push(g);
            }
            self
        }

        /// True if `path` passes every OR-group.
        pub fn allows(&self, path: &NormalizedPath) -> bool {
            self.groups.iter().all(|group| {
                group.iter().any(|prefix| paths_compatible(path.as_str(), prefix.as_str()))
            })
        }

        /// True if this predicate has no constraints (accepts everything).
        pub fn is_empty(&self) -> bool { self.groups.is_empty() }
    }

    /// Two paths are compatible if one is a path-segment-prefix of the
    /// other. This allows a walker at `/proj` to descend toward a
    /// constraint `/proj/backend/foo.cs`, and a walker that has already
    /// reached `/proj/backend/` to emit `/proj/backend/foo.cs`.
    fn paths_compatible(a: &str, b: &str) -> bool {
        is_path_prefix(a, b) || is_path_prefix(b, a)
    }

    fn is_path_prefix(prefix: &str, path: &str) -> bool {
        if prefix.is_empty() { return true; }
        if path.len() < prefix.len() { return false; }
        if !path.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes()) {
            // Use case-insensitive comparison because on Windows the walker's
            // canonical casing may differ from a user-typed pattern prefix
            // even after `NormalizedPath::absolute`. On case-sensitive Unix
            // this is a no-op since both sides already agree.
            return false;
        }
        if prefix.ends_with('/') { return true; }
        let rest = &path[prefix.len()..];
        rest.is_empty() || rest.starts_with('/')
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
    ///
    /// If `prune` is `Some`, subtrees that can't contain any path compatible
    /// with *every* prune group are skipped. This is how sibling patterns
    /// (e.g. CLI ∩ operation) narrow each other's walk — see [`FilePrune`].
    pub fn expand_canonical(
        pattern: &str,
        limit: usize,
        prune: Option<&FilePrune>,
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
        let canonical_root = NormalizedPath::absolute(&root);

        // If a prune predicate is set and doesn't even permit the root, we
        // can't produce any results — short-circuit before the walk.
        if let Some(p) = prune {
            if !p.allows(&canonical_root) {
                return Ok(vec![]);
            }
        }

        // Compile the wildcard suffix for matching
        let compiled = CompiledPattern::new(&suffix_pattern).map_err(|e| GlobExpandError {
            pattern: pattern.to_string(),
            message: e.message,
            kind: GlobExpandErrorKind::InvalidPattern,
        })?;

        let mut results = Vec::new();
        walk_dir(&canonical_root, &canonical_root, "", &compiled, limit, prune, &mut results, 0)
            .map_err(|_| GlobExpandError {
                pattern: pattern.to_string(),
                message: format!("exceeded {} file limit", limit),
                kind: GlobExpandErrorKind::LimitExceeded,
            })?;

        Ok(results)
    }

    /// Marker error type returned when the walker exceeds the expansion limit.
    struct WalkLimitExceeded;

    /// Recursively walk a directory, matching relative paths against the
    /// compiled pattern.
    ///
    /// `root` is the normalized base of the walk (used for the warning message
    /// only). `current` is the normalized absolute path of the directory being
    /// scanned — each child is built by `current.join_segment(entry_name)`,
    /// reusing the validated parent string without re-normalization.
    /// `relative` is the path relative to `root` used for pattern matching.
    #[allow(clippy::too_many_arguments)] // recursion state — flattening further would be worse
    fn walk_dir(
        root: &NormalizedPath,
        current: &NormalizedPath,
        relative: &str,
        pattern: &CompiledPattern,
        limit: usize,
        prune: Option<&FilePrune>,
        results: &mut Vec<NormalizedPath>,
        depth: usize,
    ) -> Result<(), WalkLimitExceeded> {
        if depth > MAX_DEPTH {
            eprintln!(
                "warning: glob walker reached max depth ({}) at '{}' — \
                 skipping deeper entries (may indicate a symlink cycle or unusually deep tree)",
                MAX_DEPTH,
                current,
            );
            return Ok(());
        }

        let entries = match std::fs::read_dir(current.as_str()) {
            Ok(e) => e,
            Err(_) => return Ok(()), // permission denied, etc. — skip silently
        };

        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Extend the already-normalized parent with the raw entry name.
            // `read_dir` yields single path components (no separators), so
            // this preserves the NormalizedPath invariant by construction.
            let child_path = current.join_segment(&name_str);

            // Prune subtrees / files that no prune group admits. For a file,
            // this is an exact-allows check; for a directory, `allows` still
            // returns true when the directory is only a path-*prefix* of the
            // constraint — so walking continues toward the constraint and
            // resumes unrestricted below it.
            if let Some(p) = prune {
                if !p.allows(&child_path) {
                    continue;
                }
            }

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
                    results.push(child_path.clone());
                    if results.len() > limit {
                        return Err(WalkLimitExceeded);
                    }
                }
            }

            if file_type.is_dir() || (file_type.is_symlink() && entry.path().is_dir()) {
                walk_dir(root, &child_path, &child_relative, pattern, limit, prune, results, depth + 1)?;
            }
        }

        Ok(())
    }
}

#[cfg(feature = "native")]
pub use walk::{expand_canonical, pattern_literal_prefix, FilePrune, GlobExpandError, GlobExpandErrorKind};

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
        use crate::NormalizedPath;

        #[test]
        fn expand_non_glob_returns_as_is() {
            let result = expand_canonical("some/literal/path.rs", 100, None).unwrap();
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].as_str(), "some/literal/path.rs");
        }

        #[test]
        fn expand_nonexistent_root_returns_empty() {
            let result = expand_canonical("/nonexistent/path/**/*.rs", 100, None).unwrap();
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

            let result = expand_canonical(&pattern, 10000, None).unwrap();
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

            let result = expand_canonical(&pattern, 1, None);
            let err = result.expect_err("should fail when exceeding limit of 1");
            assert!(matches!(err.kind, GlobExpandErrorKind::LimitExceeded),
                "limit overflow must be classified as LimitExceeded, got {:?}", err.kind);
        }

        /// `expand_canonical` must classify a malformed wildcard suffix as
        /// `InvalidPattern` so callers (e.g. `expand_globs_checked`) don't
        /// confuse it with a limit overflow. The pattern below puts a `[...]`
        /// character class after the first `*`, so it lands in the
        /// `CompiledPattern::new()` failure path. Uses `./*` so the literal
        /// root is always the cwd — otherwise on Windows `/tmp` doesn't exist
        /// and expansion short-circuits before the pattern is compiled.
        #[test]
        fn expand_invalid_pattern_kind() {
            let result = expand_canonical("./*/[abc]/*.rs", 100, None);
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
            let result = expand_canonical(&pattern, 10_000, None)
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

        // -- pattern_literal_prefix tests --

        #[test]
        fn literal_prefix_for_glob_with_dir_prefix() {
            assert_eq!(pattern_literal_prefix("backend/**/tests/*.cs"), "backend/");
        }

        #[test]
        fn literal_prefix_for_deep_dir_prefix() {
            assert_eq!(
                pattern_literal_prefix("src/app/modules/*.ts"),
                "src/app/modules/"
            );
        }

        #[test]
        fn literal_prefix_for_non_glob_returns_full_path() {
            // Non-glob path — the whole thing is literal (the caller can
            // choose to treat it as a file-scope constraint).
            assert_eq!(pattern_literal_prefix("backend/foo.cs"), "backend/foo.cs");
        }

        #[test]
        fn literal_prefix_for_leading_wildcard_is_empty() {
            assert_eq!(pattern_literal_prefix("**/*.cs"), "");
            assert_eq!(pattern_literal_prefix("*.cs"), "");
        }

        #[test]
        fn literal_prefix_stops_at_segment_boundary() {
            // A wildcard *inside* the first segment yields no literal prefix —
            // we can't prune to `foo` because `foobar/` also matches.
            assert_eq!(pattern_literal_prefix("foo*/bar.cs"), "");
        }

        #[test]
        fn literal_prefix_normalizes_separators() {
            // Windows-style separators get normalized to forward slashes
            // before prefix extraction.
            assert_eq!(
                pattern_literal_prefix("backend\\**\\*.cs"),
                "backend/"
            );
        }

        // -- FilePrune tests --

        #[test]
        fn file_prune_empty_accepts_everything() {
            let p = FilePrune::new();
            assert!(p.is_empty());
            assert!(p.allows(&NormalizedPath::new("/anything/goes/here.cs")));
            assert!(p.allows(&NormalizedPath::new("")));
        }

        #[test]
        fn file_prune_empty_group_is_dropped() {
            // An empty constraint group would otherwise reject everything
            // (the any() over zero prefixes is false) — guard against that.
            let p = FilePrune::new().with_group(std::iter::empty());
            assert!(p.is_empty(), "empty group must be dropped");
            assert!(p.allows(&NormalizedPath::new("/foo.cs")));
        }

        #[test]
        fn file_prune_accepts_descendants_of_prefix() {
            let p = FilePrune::new().with_group([NormalizedPath::new("/proj/backend")]);
            assert!(p.allows(&NormalizedPath::new("/proj/backend")));
            assert!(p.allows(&NormalizedPath::new("/proj/backend/a.cs")));
            assert!(p.allows(&NormalizedPath::new("/proj/backend/sub/b.cs")));
        }

        #[test]
        fn file_prune_accepts_ancestors_of_prefix() {
            // Walking *toward* a constraint must be allowed — otherwise
            // we'd never reach `/proj/backend/` from `/proj/`.
            let p = FilePrune::new().with_group([NormalizedPath::new("/proj/backend")]);
            assert!(p.allows(&NormalizedPath::new("/proj")));
            assert!(p.allows(&NormalizedPath::new("/")));
        }

        #[test]
        fn file_prune_rejects_sibling_directories() {
            let p = FilePrune::new().with_group([NormalizedPath::new("/proj/backend")]);
            assert!(!p.allows(&NormalizedPath::new("/proj/frontend")));
            assert!(!p.allows(&NormalizedPath::new("/proj/frontend/a.cs")));
        }

        #[test]
        fn file_prune_rejects_segment_boundary_lookalikes() {
            // `/proj/back` must not accept `/proj/backend` — the prefix
            // check must split at path segment boundaries.
            let p = FilePrune::new().with_group([NormalizedPath::new("/proj/back")]);
            assert!(!p.allows(&NormalizedPath::new("/proj/backend")));
            assert!(p.allows(&NormalizedPath::new("/proj/back")));
            assert!(p.allows(&NormalizedPath::new("/proj/back/a.cs")));
        }

        #[test]
        fn file_prune_or_within_group() {
            // Single group with two prefixes — either one satisfies the group.
            let p = FilePrune::new().with_group([
                NormalizedPath::new("/proj/backend"),
                NormalizedPath::new("/proj/frontend"),
            ]);
            assert!(p.allows(&NormalizedPath::new("/proj/backend/a.cs")));
            assert!(p.allows(&NormalizedPath::new("/proj/frontend/b.cs")));
            assert!(!p.allows(&NormalizedPath::new("/proj/docs/c.md")));
        }

        #[test]
        fn file_prune_and_across_groups() {
            // Two groups — path must satisfy both. Group 1 restricts to
            // `/proj/backend` OR `/proj/frontend`; group 2 restricts to
            // `/proj/backend` OR `/proj/docs`. Only `/proj/backend` is
            // compatible with both groups.
            let p = FilePrune::new()
                .with_group([
                    NormalizedPath::new("/proj/backend"),
                    NormalizedPath::new("/proj/frontend"),
                ])
                .with_group([
                    NormalizedPath::new("/proj/backend"),
                    NormalizedPath::new("/proj/docs"),
                ]);
            assert!(p.allows(&NormalizedPath::new("/proj/backend/a.cs")));
            assert!(!p.allows(&NormalizedPath::new("/proj/frontend/a.cs")));
            assert!(!p.allows(&NormalizedPath::new("/proj/docs/a.cs")));
            // Ancestor — compatible with both groups.
            assert!(p.allows(&NormalizedPath::new("/proj")));
        }

        // -- pruning walker integration tests --

        /// The user's primary use case 1: scope to a directory. A CLI
        /// constraint of `backend/**/*.cs` should prevent the walker from
        /// ever descending into `frontend/`, even when expanding a broad
        /// rule-side pattern like `**/*.cs`.
        #[test]
        fn walk_with_prune_skips_non_compatible_subtrees() {
            let dir = std::env::temp_dir().join("tractor_prune_dir_scope");
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("backend")).unwrap();
            std::fs::create_dir_all(dir.join("frontend")).unwrap();
            std::fs::write(dir.join("backend/a.cs"), "").unwrap();
            std::fs::write(dir.join("frontend/b.cs"), "").unwrap();

            let backend_prefix = NormalizedPath::absolute(
                dir.join("backend").to_string_lossy().as_ref(),
            );
            let prune = FilePrune::new().with_group([backend_prefix]);

            let pattern = format!(
                "{}/**/*.cs",
                normalize_path(&dir.to_string_lossy())
            );
            let result = expand_canonical(&pattern, 100, Some(&prune)).unwrap();

            let names: Vec<&str> = result.iter()
                .map(|p| p.as_str().rsplit('/').next().unwrap())
                .collect();
            assert!(names.contains(&"a.cs"), "backend/a.cs must be found: {:?}", names);
            assert!(!names.contains(&"b.cs"), "frontend/b.cs must be pruned: {:?}", names);

            std::fs::remove_dir_all(&dir).ok();
        }

        /// The user's primary use case 2: scope to a single file. A CLI
        /// constraint pointing at one specific file should narrow a broad
        /// rule-side `**/*.cs` pattern to just that file.
        #[test]
        fn walk_with_prune_scopes_to_single_file() {
            let dir = std::env::temp_dir().join("tractor_prune_file_scope");
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("src")).unwrap();
            std::fs::write(dir.join("src/a.cs"), "").unwrap();
            std::fs::write(dir.join("src/b.cs"), "").unwrap();
            std::fs::write(dir.join("src/c.cs"), "").unwrap();

            let single_file = NormalizedPath::absolute(
                dir.join("src/b.cs").to_string_lossy().as_ref(),
            );
            let prune = FilePrune::new().with_group([single_file]);

            let pattern = format!(
                "{}/**/*.cs",
                normalize_path(&dir.to_string_lossy())
            );
            let result = expand_canonical(&pattern, 100, Some(&prune)).unwrap();

            assert_eq!(result.len(), 1, "only the scoped file should be returned: {:?}",
                result.iter().map(|p| p.as_str()).collect::<Vec<_>>());
            assert!(result[0].as_str().ends_with("/src/b.cs"),
                "expected src/b.cs, got {}", result[0].as_str());

            std::fs::remove_dir_all(&dir).ok();
        }

        /// When the prune prefix points outside the expansion root entirely,
        /// the walker must short-circuit (return empty) rather than produce
        /// stray matches — otherwise the intersection semantics would leak.
        #[test]
        fn walk_with_prune_outside_root_returns_empty() {
            let dir = std::env::temp_dir().join("tractor_prune_disjoint");
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("backend")).unwrap();
            std::fs::write(dir.join("backend/a.cs"), "").unwrap();

            // Prune prefix points at a sibling directory that doesn't even
            // exist in the walk — no path the walker produces can satisfy it.
            let disjoint = NormalizedPath::absolute(
                std::env::temp_dir().join("some_other_unrelated_tree").to_string_lossy().as_ref(),
            );
            let prune = FilePrune::new().with_group([disjoint]);

            let pattern = format!(
                "{}/**/*.cs",
                normalize_path(&dir.to_string_lossy())
            );
            let result = expand_canonical(&pattern, 100, Some(&prune)).unwrap();
            assert!(result.is_empty(),
                "disjoint prune must short-circuit the walk: {:?}",
                result.iter().map(|p| p.as_str()).collect::<Vec<_>>());

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

            let result = expand_canonical(&pattern, 10000, None).unwrap();
            // Verify paths match what canonicalize would return
            for p in result.iter().take(3) {
                #[allow(clippy::disallowed_methods)] // test verifies canonical walker output
                let canonical = std::fs::canonicalize(p.as_str())
                    .map(|c| normalize_path(&c.to_string_lossy()))
                    .unwrap_or_else(|_| p.as_str().to_string());
                assert_eq!(p.as_str(), canonical,
                    "expanded path should match canonical form");
            }
        }
    }
}
