//! File discovery: glob expansion, language filtering, and safety limits.

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
    pub files: Vec<String>,
    pub empty_patterns: Vec<String>,
}

/// Expand glob patterns to file paths, with a per-pattern expansion limit.
///
/// Uses a custom canonical filesystem walker that produces correctly-cased
/// paths on Windows by building paths from `read_dir` entry names.
///
/// Returns the expanded file list and a list of patterns that matched 0 files.
/// Returns `Err` if any single pattern exceeds `expansion_limit` matches.
pub fn expand_globs_checked(
    patterns: &[String],
    expansion_limit: usize,
) -> Result<GlobExpansion, GlobExpansionError> {
    let mut files = Vec::new();
    let mut empty_patterns = Vec::new();

    for pattern in patterns {
        if pattern.contains('*') {
            let remaining = expansion_limit.saturating_sub(files.len());
            match crate::glob_match::expand_canonical(pattern, remaining) {
                Ok(paths) => {
                    if paths.is_empty() {
                        empty_patterns.push(pattern.clone());
                    }
                    for path in paths {
                        files.push(path.into_string());
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
            // Not a glob, use as-is
            files.push(pattern.clone());
        }
    }

    Ok(GlobExpansion { files, empty_patterns })
}

/// Expand glob patterns to file paths (convenience wrapper with no limit).
pub fn expand_globs(patterns: &[String]) -> Vec<String> {
    match expand_globs_checked(patterns, usize::MAX) {
        Ok(result) => result.files,
        Err(_) => unreachable!("usize::MAX limit cannot be exceeded"),
    }
}

/// Filter files by supported languages
pub fn filter_supported_files(files: Vec<String>) -> Vec<String> {
    use crate::parser::detect_language;

    files
        .into_iter()
        .filter(|f| detect_language(f) != "unknown")
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_globs_non_glob() {
        let patterns = vec!["test.cs".to_string()];
        let files = expand_globs(&patterns);
        assert_eq!(files, vec!["test.cs"]);
    }

    #[test]
    fn test_filter_supported_files() {
        let files = vec![
            "test.cs".to_string(),
            "test.rs".to_string(),
            "test.unknown".to_string(),
            "readme.md".to_string(),
        ];
        let filtered = filter_supported_files(files);
        assert_eq!(filtered, vec!["test.cs", "test.rs", "readme.md"]);
    }

}
