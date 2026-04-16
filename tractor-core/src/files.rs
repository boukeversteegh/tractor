//! File discovery: glob expansion, language filtering, and safety limits.

use crate::glob_match::FilePrune;
use crate::NormalizedPath;

/// Error returned when glob expansion exceeds the path limit.
#[derive(Debug, Clone)]
pub struct GlobExpansionError {
    /// The pattern that caused the limit to be exceeded.
    pub pattern: String,
    /// The limit that was exceeded.
    pub limit: usize,
}

impl std::fmt::Display for GlobExpansionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "pattern \"{}\" expanded to over {} paths — this likely indicates a misconfigured glob",
            self.pattern, self.limit
        )
    }
}

/// Result of expanding glob patterns: files and any patterns that matched nothing.
pub struct GlobExpansion {
    pub files: Vec<NormalizedPath>,
    pub empty_patterns: Vec<String>,
}

/// Expand glob patterns to file paths, with a per-pattern expansion limit.
///
/// Uses a custom canonical filesystem walker that produces correctly-cased
/// paths on Windows by building paths from `read_dir` entry names.
///
/// `prune`, if provided, narrows the walk to subtrees compatible with the
/// given prefix groups — typically the literal prefixes of *sibling*
/// pattern sets that will be intersected with this one (e.g. CLI ∩ rule).
/// See [`FilePrune`] for the compatibility model.
///
/// Returns the expanded file list and a list of patterns that matched 0 files.
/// Returns `Err` if any single pattern exceeds `expansion_limit` matches.
pub fn expand_globs_checked(
    patterns: &[String],
    expansion_limit: usize,
    prune: Option<&FilePrune>,
) -> Result<GlobExpansion, GlobExpansionError> {
    let mut files: Vec<NormalizedPath> = Vec::new();
    let mut empty_patterns = Vec::new();

    for pattern in patterns {
        if pattern.contains('*') {
            let remaining = expansion_limit.saturating_sub(files.len());
            match crate::glob_match::expand_canonical(pattern, remaining, prune) {
                Ok(paths) => {
                    if paths.is_empty() {
                        empty_patterns.push(pattern.clone());
                    }
                    for path in paths {
                        files.push(path);
                        if files.len() > expansion_limit {
                            return Err(GlobExpansionError {
                                pattern: pattern.clone(),
                                limit: expansion_limit,
                            });
                        }
                    }
                }
                Err(e) => match e.kind {
                    crate::glob_match::GlobExpandErrorKind::LimitExceeded => {
                        return Err(GlobExpansionError {
                            pattern: pattern.clone(),
                            limit: expansion_limit,
                        });
                    }
                    crate::glob_match::GlobExpandErrorKind::InvalidPattern => {
                        eprintln!("Invalid glob pattern '{}': {}", pattern, e);
                    }
                },
            }
        } else {
            // Not a glob, use as-is (normalized). Non-glob patterns bypass
            // the walker entirely, so prune doesn't apply here — sibling-set
            // intersection still runs downstream.
            //
            // Brackets and question marks are valid in literal filenames
            // (at least on Unix), so we don't reject them — but they are
            // much more likely to be a glob-syntax mistake (e.g. `[abc]`
            // character class or `?` single-char wildcard) than an actual
            // filename, so warn.
            if pattern.contains('[') || pattern.contains('?') {
                eprintln!(
                    "warning: pattern '{}' contains '?' or '[' but no '*' — \
                     treating as a literal file path; if you intended glob syntax, \
                     add a '*' or '**/' prefix",
                    pattern
                );
            }
            files.push(NormalizedPath::new(pattern));
        }
    }

    Ok(GlobExpansion { files, empty_patterns })
}

/// Expand glob patterns to file paths (convenience wrapper with no limit).
pub fn expand_globs(patterns: &[String]) -> Vec<NormalizedPath> {
    match expand_globs_checked(patterns, usize::MAX, None) {
        Ok(result) => result.files,
        Err(_) => unreachable!("usize::MAX limit cannot be exceeded"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_globs_non_glob() {
        let patterns = vec!["test.cs".to_string()];
        let files = expand_globs(&patterns);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], "test.cs");
    }
}
