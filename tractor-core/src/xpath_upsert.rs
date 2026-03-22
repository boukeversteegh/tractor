//! XPath-based upsert: insert or update values in data files.
//!
//! Implements the language-agnostic patching architecture described in
//! `specs/patching.md`. The algorithm:
//!
//! 1. Parse the source into a data tree with source spans
//! 2. Query with XPath to determine update vs insert
//! 3. Identify the splice node and record its original source span
//! 4. Mutate the data tree (update value or insert new children)
//! 5. Re-render the full modified tree with span tracking
//! 6. Look up the splice node's new span from the renderer's span map
//! 7. Splice the new span into the original source
//!
//! The renderer annotates each node with its byte span in the output,
//! keyed by the node's original source position (`start` attribute).
//! This avoids re-parsing and re-querying entirely.
//!
//! All language-specific knowledge lives in the parser and renderer.
//! The upsert algorithm itself is language-agnostic.

use crate::parser::parse_string_to_documents;
use crate::render::{self, RenderOptions};
use crate::tree_mode::TreeMode;
use crate::xpath::xot_node_to_xml_node;
use crate::xpath::{XPathEngine, Match};
use crate::xot_transform::helpers::*;
use std::sync::Arc;
use xot::Xot;

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
    #[error("parse error: {0}")]
    Parse(String),

    #[error("render error: {0}")]
    Render(String),

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
/// `value` is a raw literal in the target language's syntax: `"hello"` for a
/// JSON string (with quotes), `42` for a number, `true`/`false` for booleans.
pub fn upsert(
    source: &str,
    lang: &str,
    xpath: &str,
    value: &str,
) -> Result<UpsertResult, UpsertError> {
    // Verify the language has a renderer
    let test_render = render::render(
        &crate::xpath::XmlNode::Element {
            name: "test".to_string(),
            attributes: vec![],
            children: vec![],
        },
        lang,
        &RenderOptions::default(),
    );
    if let Err(render::RenderError::UnsupportedLanguage(_)) = test_render {
        return Err(UpsertError::UnsupportedLanguage(lang.to_string()));
    }

    // Step 1: Parse source into data tree
    let mut result = parse_string_to_documents(
        source,
        lang,
        "<upsert>".to_string(),
        Some(TreeMode::Data),
        false,
    )
    .map_err(|e| UpsertError::Parse(e.to_string()))?;

    // Query with XPath to determine update vs insert
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
        // Update path
        update_existing(source, lang, xpath, value, &existing[0])
    } else {
        // Insert path
        insert_new(source, lang, xpath, value)
    }
}

/// Update an existing node's value using render-with-spans-splice.
///
/// Instead of re-parsing and re-querying the rendered output, we use the
/// renderer's span map to locate the modified node directly. The node is
/// identified by its original source position (`start` attribute), which
/// survives the tree mutation (only text children change, not the element).
fn update_existing(
    source: &str,
    lang: &str,
    _xpath: &str,
    value: &str,
    matched: &Match,
) -> Result<UpsertResult, UpsertError> {
    // Step 1: Record the original source span of the matched node
    let orig_start = line_col_to_byte_offset(source, matched.line, matched.column)
        .ok_or_else(|| UpsertError::NoInsertionPoint("start position out of bounds".into()))?;
    let orig_end = line_col_to_byte_offset(source, matched.end_line, matched.end_column)
        .ok_or_else(|| UpsertError::NoInsertionPoint("end position out of bounds".into()))?;

    // Step 2: Mutate the data tree — re-parse, find the node, update its text
    let mut result = parse_string_to_documents(
        source, lang, "<upsert>".to_string(), Some(TreeMode::Data), false,
    ).map_err(|e| UpsertError::Parse(e.to_string()))?;

    let doc_node = result.documents.document_node(result.doc_handle)
        .ok_or_else(|| UpsertError::Parse("no document node".into()))?;

    let file_node = find_file_node(result.documents.xot(), doc_node)
        .ok_or_else(|| UpsertError::NoInsertionPoint("no File node found".into()))?;

    let target = find_node_by_span(result.documents.xot(), file_node, matched.line, matched.column)
        .ok_or_else(|| UpsertError::NoInsertionPoint("could not locate matched node in tree".into()))?;

    replace_text_content(result.documents.xot_mut(), target, value)?;

    // Step 3: Re-render with span tracking
    let xml_node = xot_node_to_xml_node(result.documents.xot(), file_node);
    let render_opts = detect_render_options(source);
    let (rendered, span_map) = render::render_with_spans(&xml_node, lang, &render_opts)
        .map_err(|e| UpsertError::Render(e.to_string()))?;

    // Step 4: Look up the modified node's new span from the renderer's span map
    let span_key = format!("{}:{}", matched.line, matched.column);
    let (new_start, new_end) = span_map.get(&span_key)
        .ok_or_else(|| UpsertError::NoInsertionPoint(
            format!("node at {} not found in rendered output span map", span_key),
        ))?;

    // Step 5: Splice
    let mut new_source = String::with_capacity(source.len());
    new_source.push_str(&source[..orig_start]);
    new_source.push_str(&rendered[*new_start..*new_end]);
    new_source.push_str(&source[orig_end..]);

    Ok(UpsertResult {
        source: new_source,
        inserted: false,
        description: "updated existing value".into(),
    })
}

/// Insert new structure using the render-reparse-splice approach.
fn insert_new(
    source: &str,
    lang: &str,
    xpath: &str,
    value: &str,
) -> Result<UpsertResult, UpsertError> {
    // Parse the XPath into key steps
    let key_path = xpath_to_key_path(xpath)?;
    if key_path.is_empty() {
        return Err(UpsertError::NoInsertionPoint(
            "XPath resolves to empty path".into(),
        ));
    }

    // Step 1: Parse and walk the tree to find the deepest existing ancestor
    let mut result = parse_string_to_documents(
        source, lang, "<upsert>".to_string(), Some(TreeMode::Data), false,
    ).map_err(|e| UpsertError::Parse(e.to_string()))?;

    let doc_node = result.documents.document_node(result.doc_handle)
        .ok_or_else(|| UpsertError::Parse("no document node".into()))?;

    let file_node = find_file_node(result.documents.xot(), doc_node)
        .ok_or_else(|| UpsertError::NoInsertionPoint("no File node found".into()))?;

    // Step 2: Find the deepest existing ancestor and record its span
    let (existing_depth, ancestor_node) =
        find_deepest_ancestor(result.documents.xot(), file_node, &key_path);

    let missing_keys = &key_path[existing_depth..];
    if missing_keys.is_empty() {
        return Err(UpsertError::NoInsertionPoint(
            "all path elements exist but XPath didn't match — predicate mismatch?".into(),
        ));
    }

    // Record the splice node's original span and build the re-query XPath.
    // When existing_depth == 0, the splice node is the File container (the whole
    // document), so the entire source is replaced with the full re-render.
    let is_root_splice = existing_depth == 0;

    let (orig_start, orig_end) = if is_root_splice {
        (0, source.len())
    } else {
        get_node_byte_span(result.documents.xot(), ancestor_node, source)
            .ok_or_else(|| UpsertError::NoInsertionPoint(
                "splice node has no source span".into(),
            ))?
    };

    let ancestor_xpath = if is_root_splice {
        String::new() // not needed — we use the full rendered output
    } else {
        let mut xp = String::new();
        for key in &key_path[..existing_depth] {
            xp.push_str("//");
            xp.push_str(key);
        }
        xp
    };

    // Step 3: Mutate the tree — add missing children
    let xot = result.documents.xot_mut();
    add_nested_children(xot, ancestor_node, missing_keys, value)?;

    // Step 4: Re-render the full modified tree
    let xml_node = xot_node_to_xml_node(result.documents.xot(), file_node);
    let render_opts = detect_render_options(source);
    let rendered = render::render(&xml_node, lang, &render_opts)
        .map_err(|e| UpsertError::Render(e.to_string()))?;

    // Step 5: Determine the new splice content
    let new_content = if is_root_splice {
        // Full re-render replaces the entire source
        rendered.trim_end().to_string()
    } else {
        // Re-parse and re-query to find the splice node's new span
        let mut new_result = parse_string_to_documents(
            &rendered, lang, "<upsert>".to_string(), Some(TreeMode::Data), false,
        ).map_err(|e| UpsertError::Parse(format!("re-parse failed: {}", e)))?;

        let new_matches = XPathEngine::new()
            .query_documents(
                &mut new_result.documents,
                new_result.doc_handle,
                &ancestor_xpath,
                Arc::new(vec![]),
                "<upsert>",
            )
            .map_err(|e| UpsertError::Query(format!("re-query failed: {}", e)))?;

        if new_matches.is_empty() {
            return Err(UpsertError::NoInsertionPoint(
                "splice node not found after re-render".into(),
            ));
        }

        let new_match = &new_matches[0];
        let new_start = line_col_to_byte_offset(&rendered, new_match.line, new_match.column)
            .ok_or_else(|| UpsertError::NoInsertionPoint("new start out of bounds".into()))?;
        let new_end = line_col_to_byte_offset(&rendered, new_match.end_line, new_match.end_column)
            .ok_or_else(|| UpsertError::NoInsertionPoint("new end out of bounds".into()))?;

        rendered[new_start..new_end].to_string()
    };

    // Step 6: Splice
    let mut new_source = String::with_capacity(source.len());
    new_source.push_str(&source[..orig_start]);
    new_source.push_str(&new_content);
    new_source.push_str(&source[orig_end..]);

    let description = format!(
        "inserted {}",
        missing_keys.join("/"),
    );

    Ok(UpsertResult {
        source: new_source,
        inserted: true,
        description,
    })
}

// ---------------------------------------------------------------------------
// Tree helpers
// ---------------------------------------------------------------------------

/// Navigate from document root to the File element.
fn find_file_node(xot: &Xot, doc_node: xot::Node) -> Option<xot::Node> {
    let doc_el = xot.document_element(doc_node).ok()?;
    // doc_el is typically <Files>, find <File> inside
    if get_element_name(xot, doc_el).as_deref() == Some("Files") {
        xot.children(doc_el).find(|&c| {
            get_element_name(xot, c).as_deref() == Some("File")
        })
    } else {
        Some(doc_el)
    }
}

/// Find a node in the xot tree by its start position (line:col).
fn find_node_by_span(xot: &Xot, root: xot::Node, target_line: u32, target_col: u32) -> Option<xot::Node> {
    // Check if this node matches
    if let Some(start) = get_attr(xot, root, "start") {
        if let Some((line, col)) = parse_position(&start) {
            if line == target_line && col == target_col {
                return Some(root);
            }
        }
    }

    // Recurse into children
    for child in xot.children(root) {
        if xot.element(child).is_some() {
            if let Some(found) = find_node_by_span(xot, child, target_line, target_col) {
                return Some(found);
            }
        }
    }
    None
}

/// Get start/end span of a node as (line, col, end_line, end_col).
fn get_node_span(xot: &Xot, node: xot::Node) -> Option<(u32, u32, u32, u32)> {
    let start = get_attr(xot, node, "start")?;
    let end = get_attr(xot, node, "end")?;
    let (sl, sc) = parse_position(&start)?;
    let (el, ec) = parse_position(&end)?;
    Some((sl, sc, el, ec))
}

/// Get the byte span of a node in the source string.
fn get_node_byte_span(xot: &Xot, node: xot::Node, source: &str) -> Option<(usize, usize)> {
    let (sl, sc, el, ec) = get_node_span(xot, node)?;
    let start = line_col_to_byte_offset(source, sl, sc)?;
    let end = line_col_to_byte_offset(source, el, ec)?;
    Some((start, end))
}

/// Replace all text content of a node with new text.
fn replace_text_content(xot: &mut Xot, node: xot::Node, new_text: &str) -> Result<(), xot::Error> {
    // Remove all existing children
    let children: Vec<xot::Node> = xot.children(node).collect();
    for child in children {
        xot.detach(child)?;
    }
    // Add new text
    let text_node = xot.new_text(new_text);
    xot.append(node, text_node)?;
    Ok(())
}

/// Walk the data tree to find the deepest existing element matching the key path.
///
/// Before matching user keys, descends through structural wrapper nodes
/// (e.g., `<document>` in YAML) that aren't part of the user's key path.
fn find_deepest_ancestor(
    xot: &Xot,
    container: xot::Node,
    key_path: &[String],
) -> (usize, xot::Node) {
    // Descend through structural wrappers that the user doesn't address in XPath.
    // e.g., YAML's <document> sits between <File> and the actual mapping keys.
    let mut current = container;
    loop {
        let element_children: Vec<_> = xot.children(current)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        if element_children.len() == 1 {
            let child_name = get_element_name(xot, element_children[0]);
            if matches!(child_name.as_deref(), Some("document")) {
                current = element_children[0];
                continue;
            }
        }
        break;
    }

    let mut depth = 0;
    for key in key_path {
        let found = xot.children(current).find(|&child| {
            get_element_name(xot, child)
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

/// Add nested children to a node for the missing key path steps.
fn add_nested_children(
    xot: &mut Xot,
    parent: xot::Node,
    keys: &[String],
    leaf_value: &str,
) -> Result<(), xot::Error> {
    let mut current = parent;

    for (i, key) in keys.iter().enumerate() {
        let name = xot.add_name(key);
        let element = xot.new_element(name);

        // Mark as property
        let field_attr = xot.add_name("field");
        xot.attributes_mut(element).insert(field_attr, key.clone());

        if i == keys.len() - 1 {
            // Leaf: set value as text content, default to string kind
            let text_node = xot.new_text(leaf_value);
            xot.append(element, text_node)?;

            let kind_attr = xot.add_name("kind");
            xot.attributes_mut(element).insert(kind_attr, "string".to_string());
        }

        xot.append(current, element)?;
        current = element;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// XPath parsing
// ---------------------------------------------------------------------------

/// Parse an XPath into a simple key path (element names from root to leaf).
///
/// Handles simple paths like `//name`, `//db/host`, `/Files/File/name`.
/// Strips axis prefixes (`//`, `/`) and ignores predicates for now.
fn xpath_to_key_path(xpath: &str) -> Result<Vec<String>, UpsertError> {
    let mut keys = Vec::new();
    let trimmed = xpath.trim();

    // Split on `/` and filter out empty segments and structural names
    for segment in trimmed.split('/') {
        let segment = segment.trim();
        if segment.is_empty() || segment == "Files" || segment == "File" {
            continue;
        }

        // Strip predicates: `name[@attr='val']` → `name`
        let name = if let Some(bracket_pos) = segment.find('[') {
            &segment[..bracket_pos]
        } else {
            segment
        };

        if !name.is_empty() && name != "*" {
            keys.push(name.to_string());
        }
    }

    Ok(keys)
}


// ---------------------------------------------------------------------------
// Source utilities
// ---------------------------------------------------------------------------

/// Parse a "line:col" position string.
fn parse_position(pos: &str) -> Option<(u32, u32)> {
    let mut parts = pos.split(':');
    let line: u32 = parts.next()?.parse().ok()?;
    let col: u32 = parts.next()?.parse().ok()?;
    Some((line, col))
}

/// Convert 1-based line:column to byte offset.
fn line_col_to_byte_offset(content: &str, line: u32, col: u32) -> Option<usize> {
    if line == 0 {
        return None;
    }
    let col_offset = (col as usize).saturating_sub(1);
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

/// Detect render options from source (indentation style, newline style).
fn detect_render_options(source: &str) -> RenderOptions {
    let newline = if source.contains("\r\n") { "\r\n" } else { "\n" };

    // Detect indent from first indented line
    let indent = source.lines()
        .find(|line| line.starts_with(' ') || line.starts_with('\t'))
        .map(|line| {
            let trimmed = line.trim_start();
            &line[..line.len() - trimmed.len()]
        })
        .unwrap_or("  ");

    RenderOptions {
        indent: indent.to_string(),
        indent_level: 0,
        newline: newline.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------------
    // Update tests
    // ---------------------------------------------------------------------------

    #[test]
    fn update_existing_string() {
        let source = r#"{"name": "Alice", "age": 30}"#;
        let result = upsert(source, "json", "//name", "Bob").unwrap();
        assert!(!result.inserted);
        assert!(result.source.contains("Bob"));
        assert!(result.source.contains("30"));
        // Must be valid JSON
        let _: serde_json::Value = serde_json::from_str(&result.source).unwrap();
    }

    #[test]
    fn update_existing_number() {
        let source = r#"{"name": "Alice", "age": 30}"#;
        let result = upsert(source, "json", "//age", "31").unwrap();
        assert!(!result.inserted);
        assert!(result.source.contains("31"));
        assert!(result.source.contains("Alice"));
    }

    #[test]
    fn update_preserves_surrounding_formatting() {
        let source = "{\n    \"name\": \"Alice\",\n    \"age\": 30\n}";
        let result = upsert(source, "json", "//name", "Bob").unwrap();
        // Surrounding formatting (the 4-space indent, other fields) preserved
        assert!(result.source.contains("Bob"));
        assert!(result.source.contains("    \"age\": 30"));
    }

    // ---------------------------------------------------------------------------
    // Insert tests
    // ---------------------------------------------------------------------------

    #[test]
    fn insert_simple_property() {
        let source = r#"{"name": "Alice"}"#;
        let result = upsert(source, "json", "//age", "30").unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["name"], "Alice");
        // New values are always inserted as strings
        assert_eq!(parsed["age"], "30");
    }

    #[test]
    fn insert_string_value() {
        let source = r#"{"name": "Alice"}"#;
        let result = upsert(source, "json", "//city", "NYC").unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["name"], "Alice");
        assert_eq!(parsed["city"], "NYC");
    }

    #[test]
    fn insert_boolean_value() {
        let source = r#"{"name": "Alice"}"#;
        let result = upsert(source, "json", "//active", "true").unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        // New values are always inserted as strings
        assert_eq!(parsed["active"], "true");
    }

    #[test]
    fn insert_into_multiline() {
        let source = "{\n  \"name\": \"Alice\"\n}";
        let result = upsert(source, "json", "//age", "30").unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["name"], "Alice");
        // New values are always inserted as strings
        assert_eq!(parsed["age"], "30");
    }

    #[test]
    fn insert_nested_property() {
        let source = r#"{"name": "Alice"}"#;
        let result = upsert(source, "json", "//db/host", "localhost").unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["name"], "Alice");
        assert_eq!(parsed["db"]["host"], "localhost");
    }

    #[test]
    fn insert_into_existing_parent() {
        let source = r#"{"db": {"host": "localhost"}}"#;
        let result = upsert(source, "json", "//db/port", "5432").unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["db"]["host"], "localhost");
        // New values are always inserted as strings
        assert_eq!(parsed["db"]["port"], "5432");
    }

    #[test]
    fn unsupported_language_error() {
        let result = upsert("{}", "brainfuck", "//x", "1");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), UpsertError::UnsupportedLanguage(_)));
    }

    // ---------------------------------------------------------------------------
    // YAML update tests
    // ---------------------------------------------------------------------------

    #[test]
    fn yaml_update_existing_string() {
        let source = "name: Alice\nage: 30\n";
        let result = upsert(source, "yaml", "//name", "Bob").unwrap();
        assert!(!result.inserted);
        assert!(result.source.contains("Bob"));
        assert!(result.source.contains("age: 30"));
    }

    #[test]
    fn yaml_update_existing_number() {
        let source = "name: Alice\nage: 30\n";
        let result = upsert(source, "yaml", "//age", "31").unwrap();
        assert!(!result.inserted);
        assert!(result.source.contains("31"));
        assert!(result.source.contains("Alice"));
    }

    #[test]
    fn yaml_update_preserves_surrounding_formatting() {
        let source = "name: Alice\ndatabase:\n  host: localhost\n  port: 5432\n";
        let result = upsert(source, "yaml", "//database/host", "db.example.com").unwrap();
        assert!(!result.inserted);
        assert!(result.source.contains("db.example.com"));
        assert!(result.source.contains("  port: 5432"));
    }

    #[test]
    fn yaml_update_nested() {
        let source = "db:\n  host: localhost\n  port: 5432\n";
        let result = upsert(source, "yaml", "//db/host", "db.example.com").unwrap();
        assert!(!result.inserted);
        assert!(result.source.contains("db.example.com"));
        assert!(result.source.contains("port: 5432"));
    }

    // ---------------------------------------------------------------------------
    // YAML insert tests
    // ---------------------------------------------------------------------------

    #[test]
    fn yaml_insert_into_existing_parent() {
        let source = "db:\n  host: localhost\n";
        let result = upsert(source, "yaml", "//db/port", "5432").unwrap();
        assert!(result.inserted, "source: {:?}", result.source);
        assert!(result.source.contains("host: localhost"), "source: {:?}", result.source);
        assert!(result.source.contains("port: 5432"), "source: {:?}", result.source);
    }

    #[test]
    fn yaml_insert_simple_property() {
        let source = "name: Alice\n";
        let result = upsert(source, "yaml", "//age", "30").unwrap();
        assert!(result.inserted, "source: {:?}", result.source);
        assert!(result.source.contains("name: Alice"), "source: {:?}", result.source);
        assert!(result.source.contains("age: 30"), "source: {:?}", result.source);
    }

    #[test]
    fn yaml_insert_nested_property() {
        let source = "name: Alice\n";
        let result = upsert(source, "yaml", "//db/host", "localhost").unwrap();
        assert!(result.inserted, "source: {:?}", result.source);
        assert!(result.source.contains("name: Alice"), "source: {:?}", result.source);
        assert!(result.source.contains("host: localhost"), "source: {:?}", result.source);
    }

    // ---------------------------------------------------------------------------
    // XPath parsing tests
    // ---------------------------------------------------------------------------

    #[test]
    fn xpath_to_key_path_simple() {
        assert_eq!(xpath_to_key_path("//name").unwrap(), vec!["name"]);
    }

    #[test]
    fn xpath_to_key_path_nested() {
        assert_eq!(
            xpath_to_key_path("//db/host").unwrap(),
            vec!["db", "host"]
        );
    }

    #[test]
    fn xpath_to_key_path_strips_structural() {
        assert_eq!(
            xpath_to_key_path("/Files/File/name").unwrap(),
            vec!["name"]
        );
    }

    #[test]
    fn xpath_to_key_path_strips_predicates() {
        assert_eq!(
            xpath_to_key_path("//item[@type='x']/name").unwrap(),
            vec!["item", "name"]
        );
    }
}
