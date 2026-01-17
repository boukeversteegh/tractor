//! Source text extraction utilities
//!
//! Functions for converting line:column positions to offsets and extracting
//! source snippets. These are shared between CLI and WASM interfaces.

/// Convert line and column (1-based) to byte offset in source
pub fn position_to_offset(source: &str, line: u32, column: u32) -> usize {
    let mut current_line = 1u32;
    let mut current_column = 1u32;

    for (i, ch) in source.char_indices() {
        if current_line == line && current_column == column {
            return i;
        }

        if ch == '\n' {
            current_line += 1;
            current_column = 1;
        } else {
            current_column += 1;
        }
    }

    // If we reached the end and we're at the target line, return end position
    source.len()
}

/// Parse a "line:col" position string into (line, column) tuple
pub fn parse_position(pos: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = pos.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let line = parts[0].parse::<u32>().ok()?;
    let column = parts[1].parse::<u32>().ok()?;
    Some((line, column))
}

/// Extract source snippet given start/end positions in "line:col" format
pub fn extract_snippet(source: &str, start: &str, end: &str) -> Result<String, String> {
    let (start_line, start_col) = parse_position(start)
        .ok_or_else(|| format!("Invalid start position: {}", start))?;
    let (end_line, end_col) = parse_position(end)
        .ok_or_else(|| format!("Invalid end position: {}", end))?;

    let start_offset = position_to_offset(source, start_line, start_col);
    let end_offset = position_to_offset(source, end_line, end_col);

    if end_offset <= start_offset {
        return Ok(String::new());
    }

    Ok(source[start_offset..end_offset].to_string())
}

/// Get full source lines for a range (inclusive of start and end lines)
pub fn get_source_lines(source: &str, start_line: u32, end_line: u32) -> Vec<String> {
    if start_line == 0 || end_line == 0 || start_line > end_line {
        return Vec::new();
    }

    let lines: Vec<&str> = source.lines().collect();
    let start_idx = (start_line as usize).saturating_sub(1);
    let end_idx = (end_line as usize).min(lines.len());

    if start_idx >= lines.len() {
        return Vec::new();
    }

    lines[start_idx..end_idx]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// Get source lines for a range given "line:col" format positions
/// Returns the full lines from start position's line to end position's line
pub fn get_source_lines_for_range(source: &str, start: &str, end: &str) -> Result<Vec<String>, String> {
    let (start_line, _) = parse_position(start)
        .ok_or_else(|| format!("Invalid start position: {}", start))?;
    let (end_line, _) = parse_position(end)
        .ok_or_else(|| format!("Invalid end position: {}", end))?;

    Ok(get_source_lines(source, start_line, end_line))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_position() {
        assert_eq!(parse_position("1:5"), Some((1, 5)));
        assert_eq!(parse_position("10:20"), Some((10, 20)));
        assert_eq!(parse_position("invalid"), None);
        assert_eq!(parse_position("1:"), None);
        assert_eq!(parse_position(":5"), None);
    }

    #[test]
    fn test_position_to_offset() {
        let source = "hello\nworld\ntest";
        // Line 1, col 1 = offset 0
        assert_eq!(position_to_offset(source, 1, 1), 0);
        // Line 1, col 6 = offset 5 (the 'o' in hello)
        assert_eq!(position_to_offset(source, 1, 6), 5);
        // Line 2, col 1 = offset 6 (start of 'world')
        assert_eq!(position_to_offset(source, 2, 1), 6);
        // Line 2, col 6 = offset 11 (end of 'world')
        assert_eq!(position_to_offset(source, 2, 6), 11);
        // Line 3, col 1 = offset 12 (start of 'test')
        assert_eq!(position_to_offset(source, 3, 1), 12);
    }

    #[test]
    fn test_extract_snippet() {
        let source = "hello\nworld\ntest";
        // Extract "world"
        assert_eq!(extract_snippet(source, "2:1", "2:6").unwrap(), "world");
        // Extract across lines
        assert_eq!(extract_snippet(source, "1:1", "2:6").unwrap(), "hello\nworld");
        // Invalid positions
        assert!(extract_snippet(source, "invalid", "2:6").is_err());
    }

    #[test]
    fn test_get_source_lines() {
        let source = "line1\nline2\nline3\nline4";
        assert_eq!(get_source_lines(source, 1, 2), vec!["line1", "line2"]);
        assert_eq!(get_source_lines(source, 2, 4), vec!["line2", "line3", "line4"]);
        assert_eq!(get_source_lines(source, 1, 1), vec!["line1"]);
        assert_eq!(get_source_lines(source, 0, 2), Vec::<String>::new());
    }

    #[test]
    fn test_get_source_lines_for_range() {
        let source = "line1\nline2\nline3";
        assert_eq!(
            get_source_lines_for_range(source, "1:3", "2:5").unwrap(),
            vec!["line1", "line2"]
        );
    }
}
