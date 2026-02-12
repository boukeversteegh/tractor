//! File replacement based on XPath match positions.
//!
//! Given a set of matches from an XPath query, replace the matched source text
//! in the original files with a new value. The replacement value is used as-is
//! (literal text splice — the caller is responsible for escaping/formatting).

use crate::xpath::Match;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io;

/// Summary of replacements applied to files.
#[derive(Debug)]
pub struct ReplaceSummary {
    /// Number of files that were modified.
    pub files_modified: usize,
    /// Total number of replacements made.
    pub replacements_made: usize,
}

/// Errors that can occur during replacement.
#[derive(Debug)]
pub enum ReplaceError {
    /// I/O error reading or writing a file.
    Io { path: String, source: io::Error },
    /// Two matches overlap in the same file, making replacement ambiguous.
    OverlappingMatches {
        file: String,
        first: (u32, u32),
        second: (u32, u32),
    },
    /// No file path available (e.g. stdin input).
    NoFilePath { description: String },
}

impl fmt::Display for ReplaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplaceError::Io { path, source } => write!(f, "{}: {}", path, source),
            ReplaceError::OverlappingMatches { file, first, second } => {
                write!(
                    f,
                    "overlapping matches in {} at {}:{} and {}:{}, replacement is ambiguous",
                    file, first.0, first.1, second.0, second.1
                )
            }
            ReplaceError::NoFilePath { description } => {
                write!(f, "cannot replace in {}: no file path", description)
            }
        }
    }
}

impl std::error::Error for ReplaceError {}

/// Convert 1-based line:column to a byte offset within the content string.
///
/// Returns `None` if the position is out of bounds.
/// Column values are byte offsets within the line (1-based, matching Tree-sitter + 1).
fn line_col_to_byte_offset(content: &str, line: u32, col: u32) -> Option<usize> {
    let target_line = line;
    let col_offset = (col as usize).saturating_sub(1);

    if target_line == 0 {
        return None;
    }

    let mut current_line = 1u32;

    if current_line == target_line {
        let offset = col_offset;
        return if offset <= content.len() {
            Some(offset)
        } else {
            None
        };
    }

    for (i, byte) in content.bytes().enumerate() {
        if byte == b'\n' {
            current_line += 1;
            if current_line == target_line {
                let offset = i + 1 + col_offset;
                return if offset <= content.len() {
                    Some(offset)
                } else {
                    None
                };
            }
        }
    }

    None
}

/// Apply replacements to files based on XPath match positions.
///
/// Each match's source range `[line:column, end_line:end_column)` is replaced
/// with `new_value` in the original file. The range is exclusive on the end
/// (same semantics as Tree-sitter positions).
///
/// Matches are grouped by file and applied in a single pass (ascending order).
/// The replacement value is used literally — no escaping or formatting is applied.
///
/// # Errors
///
/// Returns an error if:
/// - Two matches overlap in the same file
/// - A file cannot be read or written
/// - A match has no valid file path (e.g. from stdin)
pub fn apply_replacements(matches: &[Match], new_value: &str) -> Result<ReplaceSummary, ReplaceError> {
    if matches.is_empty() {
        return Ok(ReplaceSummary {
            files_modified: 0,
            replacements_made: 0,
        });
    }

    // Reject stdin matches
    for m in matches {
        if m.file == "<stdin>" {
            return Err(ReplaceError::NoFilePath {
                description: "<stdin>".to_string(),
            });
        }
    }

    // Group matches by file
    let mut by_file: HashMap<&str, Vec<&Match>> = HashMap::new();
    for m in matches {
        by_file.entry(&m.file).or_default().push(m);
    }

    let mut files_modified = 0;
    let mut replacements_made = 0;

    for (file_path, mut file_matches) in by_file {
        // Sort by position ascending for single-pass replacement
        file_matches.sort_by(|a, b| (a.line, a.column).cmp(&(b.line, b.column)));

        // Deduplicate matches at identical positions
        file_matches.dedup_by(|a, b| {
            a.line == b.line
                && a.column == b.column
                && a.end_line == b.end_line
                && a.end_column == b.end_column
        });

        // Check for overlapping matches
        for i in 0..file_matches.len().saturating_sub(1) {
            let current = file_matches[i];
            let next = file_matches[i + 1];
            if (current.end_line, current.end_column) > (next.line, next.column) {
                return Err(ReplaceError::OverlappingMatches {
                    file: file_path.to_string(),
                    first: (current.line, current.column),
                    second: (next.line, next.column),
                });
            }
        }

        // Read the original file content
        let content = fs::read_to_string(file_path).map_err(|e| ReplaceError::Io {
            path: file_path.to_string(),
            source: e,
        })?;

        // Pre-compute byte ranges on the original content
        let mut byte_ranges: Vec<(usize, usize)> = Vec::new();
        for m in &file_matches {
            let start = line_col_to_byte_offset(&content, m.line, m.column);
            let end = line_col_to_byte_offset(&content, m.end_line, m.end_column);
            match (start, end) {
                (Some(s), Some(e)) if s <= e && e <= content.len() => {
                    byte_ranges.push((s, e));
                }
                _ => {
                    eprintln!(
                        "warning: {}: position {}:{}-{}:{} out of bounds, skipping",
                        file_path, m.line, m.column, m.end_line, m.end_column
                    );
                }
            }
        }

        if byte_ranges.is_empty() {
            continue;
        }

        // Build the result string in a single pass
        let mut result = String::with_capacity(content.len());
        let mut last_end = 0;

        for &(start, end) in &byte_ranges {
            result.push_str(&content[last_end..start]);
            result.push_str(new_value);
            last_end = end;
        }
        result.push_str(&content[last_end..]);

        replacements_made += byte_ranges.len();

        // Write back only if content actually changed
        if result != content {
            fs::write(file_path, &result).map_err(|e| ReplaceError::Io {
                path: file_path.to_string(),
                source: e,
            })?;
            files_modified += 1;
        }
    }

    Ok(ReplaceSummary {
        files_modified,
        replacements_made,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_line_col_to_byte_offset_first_line() {
        let content = "hello world";
        assert_eq!(line_col_to_byte_offset(content, 1, 1), Some(0));
        assert_eq!(line_col_to_byte_offset(content, 1, 6), Some(5));
        assert_eq!(line_col_to_byte_offset(content, 1, 12), Some(11));
    }

    #[test]
    fn test_line_col_to_byte_offset_multiline() {
        let content = "line1\nline2\nline3";
        // line1 is bytes 0-4, \n at 5
        // line2 is bytes 6-10, \n at 11
        // line3 is bytes 12-16
        assert_eq!(line_col_to_byte_offset(content, 1, 1), Some(0));
        assert_eq!(line_col_to_byte_offset(content, 2, 1), Some(6));
        assert_eq!(line_col_to_byte_offset(content, 3, 1), Some(12));
        assert_eq!(line_col_to_byte_offset(content, 2, 5), Some(10));
    }

    #[test]
    fn test_line_col_to_byte_offset_crlf() {
        let content = "line1\r\nline2\r\nline3";
        // line1 is bytes 0-4, \r at 5, \n at 6
        // line2 is bytes 7-11, \r at 12, \n at 13
        // line3 is bytes 14-18
        assert_eq!(line_col_to_byte_offset(content, 1, 1), Some(0));
        assert_eq!(line_col_to_byte_offset(content, 2, 1), Some(7));
        assert_eq!(line_col_to_byte_offset(content, 3, 1), Some(14));
    }

    #[test]
    fn test_line_col_to_byte_offset_out_of_bounds() {
        let content = "ab";
        assert_eq!(line_col_to_byte_offset(content, 0, 1), None);
        assert_eq!(line_col_to_byte_offset(content, 2, 1), None);
        assert_eq!(line_col_to_byte_offset(content, 1, 4), None);
    }

    #[test]
    fn test_apply_replacements_single() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.json");
        fs::write(&file, r#"{"key": "old"}"#).unwrap();

        let matches = vec![Match::with_location(
            file.to_str().unwrap().to_string(),
            1,
            9, // start of "old" (the quote)
            1,
            14, // end of "old" (after closing quote)
            "\"old\"".to_string(),
            Arc::new(vec![r#"{"key": "old"}"#.to_string()]),
        )];

        let result = apply_replacements(&matches, "\"new\"").unwrap();
        assert_eq!(result.replacements_made, 1);
        assert_eq!(result.files_modified, 1);
        assert_eq!(fs::read_to_string(&file).unwrap(), r#"{"key": "new"}"#);
    }

    #[test]
    fn test_apply_replacements_multiple_same_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "aaa bbb aaa").unwrap();

        let file_str = file.to_str().unwrap().to_string();
        let source = Arc::new(vec!["aaa bbb aaa".to_string()]);

        let matches = vec![
            Match::with_location(file_str.clone(), 1, 1, 1, 4, "aaa".to_string(), Arc::clone(&source)),
            Match::with_location(file_str.clone(), 1, 9, 1, 12, "aaa".to_string(), Arc::clone(&source)),
        ];

        let result = apply_replacements(&matches, "xxx").unwrap();
        assert_eq!(result.replacements_made, 2);
        assert_eq!(fs::read_to_string(&file).unwrap(), "xxx bbb xxx");
    }

    #[test]
    fn test_apply_replacements_multiline() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "line1\nOLD\nline3").unwrap();

        let matches = vec![Match::with_location(
            file.to_str().unwrap().to_string(),
            2, 1, 2, 4,
            "OLD".to_string(),
            Arc::new(vec!["line1".to_string(), "OLD".to_string(), "line3".to_string()]),
        )];

        let result = apply_replacements(&matches, "NEW").unwrap();
        assert_eq!(result.replacements_made, 1);
        assert_eq!(fs::read_to_string(&file).unwrap(), "line1\nNEW\nline3");
    }

    #[test]
    fn test_apply_replacements_overlapping_error() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "abcdefgh").unwrap();

        let file_str = file.to_str().unwrap().to_string();
        let source = Arc::new(vec!["abcdefgh".to_string()]);

        let matches = vec![
            Match::with_location(file_str.clone(), 1, 1, 1, 5, "abcd".to_string(), Arc::clone(&source)),
            Match::with_location(file_str.clone(), 1, 3, 1, 7, "cdef".to_string(), Arc::clone(&source)),
        ];

        let result = apply_replacements(&matches, "x");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ReplaceError::OverlappingMatches { .. }));
    }

    #[test]
    fn test_apply_replacements_dedup() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "hello").unwrap();

        let file_str = file.to_str().unwrap().to_string();
        let source = Arc::new(vec!["hello".to_string()]);

        // Duplicate match at same position
        let matches = vec![
            Match::with_location(file_str.clone(), 1, 1, 1, 6, "hello".to_string(), Arc::clone(&source)),
            Match::with_location(file_str.clone(), 1, 1, 1, 6, "hello".to_string(), Arc::clone(&source)),
        ];

        let result = apply_replacements(&matches, "world").unwrap();
        assert_eq!(result.replacements_made, 1);
        assert_eq!(fs::read_to_string(&file).unwrap(), "world");
    }

    #[test]
    fn test_apply_replacements_stdin_rejected() {
        let matches = vec![Match::new("<stdin>".to_string(), "value".to_string())];
        let result = apply_replacements(&matches, "x");
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_replacements_empty() {
        let result = apply_replacements(&[], "x").unwrap();
        assert_eq!(result.replacements_made, 0);
        assert_eq!(result.files_modified, 0);
    }

    #[test]
    fn test_apply_replacements_different_length() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "ab").unwrap();

        let matches = vec![Match::with_location(
            file.to_str().unwrap().to_string(),
            1, 1, 1, 3,
            "ab".to_string(),
            Arc::new(vec!["ab".to_string()]),
        )];

        // Replace with longer string
        let result = apply_replacements(&matches, "xyz123").unwrap();
        assert_eq!(result.replacements_made, 1);
        assert_eq!(fs::read_to_string(&file).unwrap(), "xyz123");
    }

    #[test]
    fn test_apply_replacements_to_empty() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "remove_me").unwrap();

        let matches = vec![Match::with_location(
            file.to_str().unwrap().to_string(),
            1, 1, 1, 10,
            "remove_me".to_string(),
            Arc::new(vec!["remove_me".to_string()]),
        )];

        let result = apply_replacements(&matches, "").unwrap();
        assert_eq!(result.replacements_made, 1);
        assert_eq!(fs::read_to_string(&file).unwrap(), "");
    }
}
