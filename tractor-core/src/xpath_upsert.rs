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
/// The update path (replacing existing values) is language-agnostic: it works
/// for any language that has a parser and data tree with source spans. The
/// insert path (creating new structure) currently only supports JSON.
///
/// `value` is a raw literal in the target language's syntax: `"hello"` for a
/// JSON string (with quotes), `42` for a number, `true`/`false` for booleans.
pub fn upsert(
    source: &str,
    lang: &str,
    xpath: &str,
    value: &str,
) -> Result<UpsertResult, UpsertError> {
    upsert_impl(source, lang, xpath, value)
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

fn upsert_impl(
    source: &str,
    lang: &str,
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
        lang,
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
        // Update: surgical replacement (language-agnostic)
        return surgical_update(source, lang, &existing[0], value);
    }

    // Insert: walk the data tree to find what exists
    // Currently only JSON supports insertion.
    if lang != "json" {
        return Err(UpsertError::NoInsertionPoint(format!(
            "XPath '{}' did not match any node — insertion is only supported for JSON",
            xpath,
        )));
    }

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

/// Update an existing node's value in the source by direct span splicing.
///
/// Language-agnostic: the matched node's source span identifies the byte range
/// to replace, and the value is spliced in literally. Works for any language
/// that produces data tree nodes with source spans.
fn surgical_update(
    source: &str,
    _lang: &str,
    matched: &crate::xpath::Match,
    value: &str,
) -> Result<UpsertResult, UpsertError> {
    let orig_start = line_col_to_byte_offset(source, matched.line, matched.column)
        .ok_or_else(|| UpsertError::NoInsertionPoint("start position out of bounds".into()))?;
    let orig_end = line_col_to_byte_offset(source, matched.end_line, matched.end_column)
        .ok_or_else(|| UpsertError::NoInsertionPoint("end position out of bounds".into()))?;

    let mut new_source = String::with_capacity(source.len());
    new_source.push_str(&source[..orig_start]);
    new_source.push_str(value);
    new_source.push_str(&source[orig_end..]);

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
    #[allow(dead_code)]
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

/// Build an XmlNode tree for the missing key path and value, then render it
/// using the JSON renderer.
///
/// Returns just the property fragment to splice (without outer `{}`).
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
    use crate::render::{self, RenderOptions};
    use crate::xpath::XmlNode;

    // Build the XmlNode tree: nest keys as property elements with the value
    // at the deepest level.
    //
    // The `value` parameter is a raw JSON literal: `"hello"` (with quotes) for strings,
    // `30` for numbers, `true`/`false` for booleans. We need to unwrap string quotes
    // since the renderer will add them back.
    let text_value = if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        // Unwrap JSON string quotes — the renderer's render_scalar will re-add them
        let inner = &value[1..value.len() - 1];
        // Unescape JSON escapes so the renderer can re-escape properly
        inner
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
            .replace("\\n", "\n")
            .replace("\\r", "\r")
            .replace("\\t", "\t")
    } else {
        value.to_string()
    };
    let leaf_value = XmlNode::Text(text_value);
    let last_key = keys.last().unwrap().clone();
    let mut node = XmlNode::Element {
        name: last_key.clone(),
        attributes: vec![("field".to_string(), last_key)],
        children: vec![leaf_value],
    };

    // Wrap in intermediate objects (from inside out)
    for key in keys[..keys.len() - 1].iter().rev() {
        node = XmlNode::Element {
            name: key.clone(),
            attributes: vec![("field".to_string(), key.clone())],
            children: vec![node],
        };
    }

    // Determine indent unit from the insertion context
    let indent_unit = if insert.indent.is_empty() {
        "  ".to_string()
    } else {
        insert.indent.clone()
    };

    // Calculate the indent level for the outermost new property.
    // insert.indent is the sibling indent (e.g., "    " for 4-space at level 1).
    // The indent level = sibling_indent / indent_unit.
    let indent_level = if !indent_unit.is_empty() && !insert.indent.is_empty() {
        insert.indent.len() / indent_unit.len()
    } else {
        1
    };

    // Render using the JSON renderer — wraps in a container to get proper object
    let container = XmlNode::Element {
        name: "_container_".to_string(),
        attributes: vec![],
        children: vec![node],
    };

    let opts = RenderOptions {
        indent: indent_unit.clone(),
        indent_level: indent_level.saturating_sub(1),
        newline: insert.newline.clone(),
    };

    let rendered = render::json::render_node(&container, &opts)
        .unwrap_or_else(|_| "{}".to_string());

    // Extract just the inner content (between the outer `{` and `}`)
    // The rendered output looks like:
    //   {\n  "key": value\n}
    // We want:
    //   \n  "key": value\n
    let inner = extract_object_inner(&rendered);

    // Build the fragment with comma prefix if needed
    let mut result = String::new();
    if insert.has_siblings {
        result.push(',');
    }
    result.push_str(inner);

    result
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

/// Extract the inner content of a rendered JSON object (between `{` and `}`).
///
/// Given `"{\n  \"key\": value\n}\n"`, returns `"\n  \"key\": value\n"`.
fn extract_object_inner(rendered: &str) -> &str {
    let trimmed = rendered.trim();
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            return &trimmed[start + 1..end];
        }
    }
    rendered
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
