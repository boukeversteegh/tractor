//! XPath-based upsert: insert or update values in data files.
//!
//! Given a source string (JSON), an XPath expression, and a value, this module
//! either updates the existing match or inserts the minimal structure needed to
//! make the XPath match with the given value — without re-rendering the entire
//! file.
//!
//! # Design
//!
//! The upsert works by:
//! 1. Parsing the source into a data tree (with source locations preserved)
//! 2. Parsing the XPath into location steps
//! 3. Walking the data tree alongside the XPath steps to find the deepest
//!    existing ancestor
//! 4. Rendering only the missing subtree as a source fragment
//! 5. Splicing the fragment into the original source at the right byte offset
//!
//! This approach preserves all original formatting, comments, and ordering —
//! only the minimal insertion is made.
//!
//! # Example
//!
//! ```
//! use tractor_core::xpath_upsert::upsert;
//!
//! let source = r#"{"name": "Alice"}"#;
//! let result = upsert(source, "json", "//age", "30").unwrap();
//! assert!(result.source.contains("\"age\": 30"));
//! assert!(result.source.contains("\"name\": \"Alice\""));
//! ```

use crate::xpath_xml_builder::{self, XPathBuildError};
use crate::xot_transform::helpers;

/// Result of an upsert operation.
#[derive(Debug)]
pub struct UpsertResult {
    /// The modified source string.
    pub source: String,
    /// Whether an insertion was made (vs. an update of existing value).
    pub inserted: bool,
    /// Human-readable description of what was done.
    pub description: String,
}

/// Errors during upsert.
#[derive(Debug, thiserror::Error)]
pub enum UpsertError {
    #[error("xpath error: {0}")]
    XPath(#[from] XPathBuildError),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("no insertion point found: {0}")]
    NoInsertionPoint(String),

    #[error("unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("xot error: {0}")]
    Xot(#[from] xot::Error),

    #[error("xpath query error: {0}")]
    Query(String),
}

/// Upsert a value into a source string at the path given by an XPath expression.
///
/// If the XPath already matches an element, its text content is replaced with
/// `value`. If the XPath does not match, the minimal structure is created and
/// inserted.
///
/// `lang` must be a data-aware language (`"json"` or `"yaml"`). Currently only
/// `"json"` is implemented.
///
/// `value` is a raw literal: `"hello"` for a JSON string (with quotes), `42`
/// for a number, `true`/`false` for booleans.
pub fn upsert(
    source: &str,
    lang: &str,
    xpath: &str,
    value: &str,
) -> Result<UpsertResult, UpsertError> {
    match lang {
        "json" => upsert_json(source, xpath, value),
        _ => Err(UpsertError::UnsupportedLanguage(lang.to_string())),
    }
}

// ---------------------------------------------------------------------------
// JSON upsert implementation
// ---------------------------------------------------------------------------

/// Parse XPath into simple path steps (element names from root to leaf).
///
/// For the data tree, XPath steps map directly to JSON keys. We strip `//`
/// since the data tree flattens to direct children.
fn xpath_to_key_path(xpath: &str) -> Result<Vec<String>, UpsertError> {
    // Use the xpath_xml_builder parser to get steps, then extract names
    let xml = xpath_xml_builder::build_xml_from_xpath(xpath)?;

    // Parse the XML to extract the element chain
    let mut names = Vec::new();
    extract_element_chain(&xml, &mut names);
    Ok(names)
}

/// Extract the chain of element names from a linear XML structure.
/// `<a><b><c/></b></a>` → `["a", "b", "c"]`
fn extract_element_chain(xml: &str, names: &mut Vec<String>) {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                names.push(String::from_utf8_lossy(e.name().as_ref()).to_string());
            }
            Ok(Event::Empty(e)) => {
                names.push(String::from_utf8_lossy(e.name().as_ref()).to_string());
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
    }
}

fn upsert_json(
    source: &str,
    xpath: &str,
    value: &str,
) -> Result<UpsertResult, UpsertError> {
    use crate::parser::parse_string_to_documents;
    use crate::tree_mode::TreeMode;
    use crate::xpath::XPathEngine;
    use std::sync::Arc;

    let key_path = xpath_to_key_path(xpath)?;
    if key_path.is_empty() {
        return Err(UpsertError::NoInsertionPoint(
            "XPath resolves to empty path".into(),
        ));
    }

    // Parse source into data tree
    let mut result = parse_string_to_documents(
        source,
        "json",
        "<upsert>".to_string(),
        Some(TreeMode::Data),
        false,
    )
    .map_err(|e| UpsertError::Parse(e.to_string()))?;

    // First, check if the XPath already matches
    let engine = XPathEngine::new();
    let existing = engine
        .query_documents(
            &mut result.documents,
            result.doc_handle,
            xpath,
            Arc::new(vec![]),
            "<upsert>",
        )
        .map_err(|e| UpsertError::Query(e.to_string()))?;

    if !existing.is_empty() {
        // Update: replace existing value
        return upsert_json_update(source, &existing[0], value);
    }

    // Insert: walk the data tree to find what exists
    let doc_node = result
        .documents
        .document_node(result.doc_handle)
        .ok_or_else(|| UpsertError::Parse("no document node".into()))?;

    let xot = result.documents.xot();

    // Walk the data tree to find the deepest existing ancestor
    let (existing_depth, parent_node) =
        find_deepest_ancestor(xot, doc_node, &key_path);

    // The remaining key path steps need to be created
    let missing_keys = &key_path[existing_depth..];

    if missing_keys.is_empty() {
        return Err(UpsertError::NoInsertionPoint(
            "all path elements exist but XPath didn't match — predicate mismatch?".into(),
        ));
    }

    // Find insertion point in the source: we need the byte offset just before
    // the closing `}` of the parent object.
    let insert_info = find_json_insert_point(source, xot, parent_node)?;

    // Render the new JSON fragment
    let fragment = render_json_fragment(missing_keys, value, &insert_info);

    // Splice into source
    let mut new_source = String::with_capacity(source.len() + fragment.len());
    new_source.push_str(&source[..insert_info.offset]);
    new_source.push_str(&fragment);
    new_source.push_str(&source[insert_info.offset..]);

    let description = format!(
        "inserted {} at {}",
        missing_keys.join("/"),
        key_path[..existing_depth].join("/"),
    );

    Ok(UpsertResult {
        source: new_source,
        inserted: true,
        description,
    })
}

/// Update an existing match's value in the source.
fn upsert_json_update(
    source: &str,
    matched: &crate::xpath::Match,
    value: &str,
) -> Result<UpsertResult, UpsertError> {
    // The match has source location — replace the text content range
    let start = line_col_to_byte_offset(source, matched.line, matched.column)
        .ok_or_else(|| UpsertError::NoInsertionPoint("start position out of bounds".into()))?;
    let end = line_col_to_byte_offset(source, matched.end_line, matched.end_column)
        .ok_or_else(|| UpsertError::NoInsertionPoint("end position out of bounds".into()))?;

    // The matched range covers the element (e.g. `"old_value"`). We need to
    // find the actual value portion in the source. For a JSON pair like
    // `"key": "old"`, the match span covers from the key through the value.
    // We need to find just the value part after the colon.
    let matched_text = &source[start..end];

    // Find the colon separator — everything after `: ` is the value
    let value_start = if let Some(colon_pos) = matched_text.find(':') {
        let after_colon = &matched_text[colon_pos + 1..];
        let trimmed_len = after_colon.len() - after_colon.trim_start().len();
        start + colon_pos + 1 + trimmed_len
    } else {
        // No colon — this is a leaf text node, replace entire match
        start
    };

    let mut new_source = String::with_capacity(source.len());
    new_source.push_str(&source[..value_start]);
    new_source.push_str(value);
    new_source.push_str(&source[end..]);

    Ok(UpsertResult {
        source: new_source,
        inserted: false,
        description: "updated existing value".into(),
    })
}

/// Information about where to insert a new JSON property.
struct InsertPoint {
    /// Byte offset in the source where the new content should be spliced.
    offset: usize,
    /// Whether the parent already has children (needs a leading comma).
    has_siblings: bool,
    /// Indentation string for the new property.
    indent: String,
    /// Newline string detected from the source.
    newline: String,
    /// Whether the object is on a single line (compact style).
    is_compact: bool,
}

/// Walk the data tree to find the deepest existing element matching the key path.
///
/// Returns `(depth, node)` where `depth` is how many keys were matched
/// and `node` is the xot node of the deepest match. When depth == 0,
/// `node` is the container where new children should be inserted.
fn find_deepest_ancestor(
    xot: &xot::Xot,
    root: xot::Node,
    key_path: &[String],
) -> (usize, xot::Node) {
    // Navigate through Document → Files → File to reach the container.
    // In the JSON data tree, the `object` is flattened so File's element
    // children ARE the top-level properties directly. File acts as the
    // root object container — we must NOT skip past it.
    let container = find_container(xot, root);
    let mut current = container;
    let mut depth = 0;

    for key in key_path {
        let found = xot.children(current).find(|&child| {
            helpers::get_element_name(xot, child)
                .map(|n| n == *key)
                .unwrap_or(false)
        });

        match found {
            Some(child) => {
                current = child;
                depth += 1;
            }
            None => break,
        }
    }

    (depth, current)
}

/// Navigate from the document root down to the container element.
/// For data trees this is the File element (which holds flattened properties).
fn find_container(xot: &xot::Xot, node: xot::Node) -> xot::Node {
    let mut current = node;
    if xot.is_document(current) {
        if let Ok(doc_el) = xot.document_element(current) {
            current = doc_el;
        }
    }
    // Descend through Files → File but stop AT File (it's the container)
    if let Some(name) = helpers::get_element_name(xot, current) {
        if name == "Files" {
            if let Some(file_el) = xot.children(current).find(|&c| {
                helpers::get_element_name(xot, c)
                    .map(|n| n == "File")
                    .unwrap_or(false)
            }) {
                current = file_el;
            }
        }
    }
    current
}

/// Find the byte offset where a new property should be inserted in JSON source.
///
/// This looks at the parent node's source location to find the closing `}`
/// of the object, then positions the insert just before it.
fn find_json_insert_point(
    source: &str,
    xot: &xot::Xot,
    parent_node: xot::Node,
) -> Result<InsertPoint, UpsertError> {
    let newline = detect_newline(source);

    // Check if parent has element children (existing properties)
    let has_siblings = xot
        .children(parent_node)
        .any(|c| xot.element(c).is_some());

    // Try to find the closing `}` of the object containing our insertion point.
    //
    // Strategy:
    // 1. If the parent has `end` attribute (nested object), search backwards from there
    // 2. Otherwise (root object / File container), use the last `}` in source
    let brace_pos = if let Some(end_pos) = helpers::get_attr(xot, parent_node, "end") {
        if let Some((end_line, end_col)) = parse_position(&end_pos) {
            if let Some(end_offset) = line_col_to_byte_offset(source, end_line, end_col) {
                // Search backwards from end position for `}`
                source[..end_offset].rfind('}')
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Fallback to last `}` in source (root-level insertion)
    let brace_pos = brace_pos.or_else(|| source.rfind('}'));

    let brace_pos = brace_pos.ok_or_else(|| {
        UpsertError::NoInsertionPoint("could not find closing brace in JSON".into())
    })?;

    // Detect indentation from existing siblings or the brace itself
    let indent = if has_siblings {
        detect_indent_from_siblings(source, xot, parent_node)
            .unwrap_or_else(|| detect_indent_at(source, brace_pos))
    } else {
        detect_indent_at(source, brace_pos)
    };

    let is_compact = !source[..brace_pos].contains('\n');

    Ok(InsertPoint {
        offset: brace_pos,
        has_siblings,
        indent,
        newline: newline.to_string(),
        is_compact,
    })
}

/// Render a JSON fragment for the missing key path with a value.
///
/// `insert.indent` is the indentation of sibling properties (i.e., the indent
/// level where the first missing key should appear). Nested keys get additional
/// indentation.
///
/// Given keys `["db", "host"]`, value `"localhost"`, sibling indent `"  "`:
/// ```text
/// ,\n  "db": {\n    "host": "localhost"\n  }
/// ```
fn render_json_fragment(
    keys: &[String],
    value: &str,
    insert: &InsertPoint,
) -> String {
    if insert.is_compact {
        return render_json_fragment_compact(keys, value, insert);
    }

    let sibling_indent = &insert.indent;
    let nl = &insert.newline;

    // Detect the indent unit (single level of indentation).
    // If sibling_indent is e.g. "    " (4 spaces) and the brace is at "" (0),
    // then the unit is "    ". We just use sibling_indent as the unit.
    let indent_unit = if sibling_indent.is_empty() {
        "  ".to_string()
    } else {
        sibling_indent.clone()
    };

    let mut result = String::new();

    // Add comma after last existing property if needed
    if insert.has_siblings {
        result.push(',');
    }

    let total_keys = keys.len();
    for (i, key) in keys.iter().enumerate() {
        // First key is at sibling_indent level, deeper keys get extra indent
        let current_indent = if i == 0 {
            sibling_indent.clone()
        } else {
            format!("{}{}", sibling_indent, indent_unit.repeat(i))
        };

        result.push_str(nl);
        result.push_str(&current_indent);
        result.push_str(&format!("\"{}\"", escape_json_string(key)));

        if i == total_keys - 1 {
            result.push_str(": ");
            result.push_str(value);
        } else {
            result.push_str(": {");
        }
    }

    // Close intermediate objects (in reverse)
    for i in (0..total_keys - 1).rev() {
        let current_indent = if i == 0 {
            sibling_indent.clone()
        } else {
            format!("{}{}", sibling_indent, indent_unit.repeat(i))
        };
        result.push_str(nl);
        result.push_str(&current_indent);
        result.push('}');
    }

    // Final newline + brace indent (one level less than sibling)
    // The closing `}` of the parent should be at one level less than sibling_indent.
    // We just need a newline so the `}` stays where it was.
    result.push_str(nl);

    // Restore the indent before the parent's closing `}`.
    // This is the brace indent = sibling_indent minus one unit.
    let brace_indent = if sibling_indent.len() >= indent_unit.len() {
        &sibling_indent[..sibling_indent.len() - indent_unit.len()]
    } else {
        ""
    };
    result.push_str(brace_indent);

    result
}

/// Render a compact (single-line) JSON fragment.
fn render_json_fragment_compact(
    keys: &[String],
    value: &str,
    insert: &InsertPoint,
) -> String {
    let mut result = String::new();

    if insert.has_siblings {
        result.push_str(", ");
    } else {
        result.push(' ');
    }

    let total = keys.len();
    for (i, key) in keys.iter().enumerate() {
        result.push_str(&format!("\"{}\"", escape_json_string(key)));
        if i == total - 1 {
            result.push_str(": ");
            result.push_str(value);
        } else {
            result.push_str(": { ");
        }
    }

    // Close intermediate objects
    for _ in 0..total - 1 {
        result.push_str(" }");
    }

    result.push(' ');
    result
}

/// Escape a string for use in a JSON key.
fn escape_json_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Detect the newline style used in the source.
fn detect_newline(source: &str) -> &str {
    if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// Detect indentation by looking at existing sibling elements' source positions.
fn detect_indent_from_siblings(
    source: &str,
    xot: &xot::Xot,
    parent_node: xot::Node,
) -> Option<String> {
    // Find the first element child with a `start` attribute
    for child in xot.children(parent_node) {
        if let Some(start_pos) = helpers::get_attr(xot, child, "start") {
            if let Some((line, _col)) = parse_position(&start_pos) {
                // Find the byte offset of this line start
                if let Some(line_offset) = line_col_to_byte_offset(source, line, 1) {
                    let line_text = &source[line_offset..];
                    let line_end = line_text.find('\n').unwrap_or(line_text.len());
                    let line_text = &line_text[..line_end];
                    let indent_len = line_text.len() - line_text.trim_start().len();
                    return Some(line_text[..indent_len].to_string());
                }
            }
        }
    }
    None
}

/// Detect the indentation at a given byte offset by looking at the line start.
fn detect_indent_at(source: &str, offset: usize) -> String {
    // Find the start of the line containing `offset`
    let line_start = source[..offset]
        .rfind('\n')
        .map(|p| p + 1)
        .unwrap_or(0);

    let line = &source[line_start..offset];
    let indent_len = line.len() - line.trim_start().len();
    line[..indent_len].to_string()
}

/// Parse a "line:col" position string.
fn parse_position(pos: &str) -> Option<(u32, u32)> {
    let mut parts = pos.split(':');
    let line = parts.next()?.parse().ok()?;
    let col = parts.next()?.parse().ok()?;
    Some((line, col))
}

/// Convert 1-based line:column to byte offset.
fn line_col_to_byte_offset(content: &str, line: u32, col: u32) -> Option<usize> {
    let col_offset = (col as usize).saturating_sub(1);

    if line == 0 {
        return None;
    }

    let mut current_line = 1u32;

    if current_line == line {
        return if col_offset <= content.len() {
            Some(col_offset)
        } else {
            None
        };
    }

    for (i, byte) in content.bytes().enumerate() {
        if byte == b'\n' {
            current_line += 1;
            if current_line == line {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Insert into empty-ish objects ------------------------------------

    #[test]
    fn insert_simple_property_compact() {
        let source = r#"{"name": "Alice"}"#;
        let result = upsert(source, "json", "//age", "30").unwrap();
        assert!(result.inserted);
        // Must preserve existing content
        assert!(result.source.contains("\"name\": \"Alice\""));
        // Must contain new property
        assert!(result.source.contains("\"age\": 30"));
        // Must be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&result.source)
            .expect("result should be valid JSON");
        assert_eq!(parsed["name"], "Alice");
        assert_eq!(parsed["age"], 30);
    }

    #[test]
    fn insert_simple_property_multiline() {
        let source = r#"{
  "name": "Alice"
}"#;
        let result = upsert(source, "json", "//age", "30").unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source)
            .expect("result should be valid JSON");
        assert_eq!(parsed["name"], "Alice");
        assert_eq!(parsed["age"], 30);
    }

    #[test]
    fn insert_string_value() {
        let source = r#"{"a": 1}"#;
        let result = upsert(source, "json", "//b", "\"hello\"").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result.source)
            .expect("result should be valid JSON");
        assert_eq!(parsed["b"], "hello");
    }

    #[test]
    fn insert_boolean_value() {
        let source = r#"{"a": 1}"#;
        let result = upsert(source, "json", "//enabled", "true").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result.source)
            .expect("result should be valid JSON");
        assert_eq!(parsed["enabled"], true);
    }

    // -- Nested insertion -------------------------------------------------

    #[test]
    fn insert_nested_property() {
        let source = r#"{
  "name": "myapp"
}"#;
        let result = upsert(source, "json", "//database/host", "\"localhost\"").unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source)
            .expect("result should be valid JSON");
        assert_eq!(parsed["database"]["host"], "localhost");
        assert_eq!(parsed["name"], "myapp");
    }

    #[test]
    fn insert_into_existing_parent() {
        let source = r#"{
  "database": {
    "port": 5432
  }
}"#;
        let result = upsert(source, "json", "//database/host", "\"localhost\"").unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source)
            .expect("result should be valid JSON");
        assert_eq!(parsed["database"]["host"], "localhost");
        assert_eq!(parsed["database"]["port"], 5432);
    }

    // -- Update existing values -------------------------------------------

    #[test]
    fn update_existing_string() {
        let source = r#"{"name": "Alice"}"#;
        let result = upsert(source, "json", "//name", "\"Bob\"").unwrap();
        assert!(!result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source)
            .expect("result should be valid JSON");
        assert_eq!(parsed["name"], "Bob");
    }

    #[test]
    fn update_existing_number() {
        let source = r#"{"count": 5}"#;
        let result = upsert(source, "json", "//count", "10").unwrap();
        assert!(!result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source)
            .expect("result should be valid JSON");
        assert_eq!(parsed["count"], 10);
    }

    // -- Formatting preservation ------------------------------------------

    #[test]
    fn preserves_indentation_style() {
        // 4-space indentation
        let source = "{\n    \"name\": \"Alice\"\n}";
        let result = upsert(source, "json", "//age", "30").unwrap();
        // The new property should use the same 4-space indent
        assert!(
            result.source.contains("    \"age\""),
            "should use 4-space indent, got:\n{}",
            result.source
        );
    }

    #[test]
    fn preserves_tab_indentation() {
        let source = "{\n\t\"name\": \"Alice\"\n}";
        let result = upsert(source, "json", "//age", "30").unwrap();
        assert!(
            result.source.contains("\t\"age\""),
            "should use tab indent, got:\n{}",
            result.source
        );
    }

    // -- Edge cases -------------------------------------------------------

    #[test]
    fn insert_deeply_nested() {
        let source = r#"{"a": 1}"#;
        let result = upsert(source, "json", "//x/y/z", "\"deep\"").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result.source)
            .expect("result should be valid JSON");
        assert_eq!(parsed["x"]["y"]["z"], "deep");
    }

    #[test]
    fn empty_object() {
        let source = "{}";
        let result = upsert(source, "json", "//key", "\"val\"").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result.source)
            .expect("result should be valid JSON");
        assert_eq!(parsed["key"], "val");
    }

    #[test]
    fn unsupported_language_error() {
        let result = upsert("{}", "python", "//x", "1");
        assert!(result.is_err());
    }
}
