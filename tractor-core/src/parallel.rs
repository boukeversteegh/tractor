//! Parallel file processing using Rayon

use rayon::prelude::*;
use std::path::Path;

use crate::parser::{parse_file, ParseResult, ParseError};

/// Process multiple files in parallel
pub fn process_files_parallel<P: AsRef<Path> + Sync>(
    files: &[P],
    lang_override: Option<&str>,
    raw_mode: bool,
    concurrency: Option<usize>,
) -> Vec<Result<ParseResult, ParseError>> {
    // Configure thread pool
    if let Some(num_threads) = concurrency {
        rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build_global()
            .ok(); // Ignore error if pool already initialized
    }

    files
        .par_iter()
        .map(|path| parse_file(path.as_ref(), lang_override, raw_mode))
        .collect()
}

/// Expand glob patterns to file paths
pub fn expand_globs(patterns: &[String]) -> Vec<String> {
    let mut files = Vec::new();

    for pattern in patterns {
        if pattern.contains('*') || pattern.contains('?') {
            // Use glob crate for pattern expansion
            match glob::glob(pattern) {
                Ok(paths) => {
                    for entry in paths.flatten() {
                        if entry.is_file() {
                            if let Some(path) = entry.to_str() {
                                files.push(path.to_string());
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Invalid glob pattern '{}': {}", pattern, e);
                }
            }
        } else {
            // Not a glob, use as-is
            files.push(pattern.clone());
        }
    }

    files
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
        assert_eq!(filtered, vec!["test.cs", "test.rs"]);
    }
}
