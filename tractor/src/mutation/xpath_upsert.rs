//! XPath-based upsert: insert or update values in data files.
//!
//! Implements the language-agnostic patching architecture described in
//! `specs/patching.md`. The algorithm:
//!
//! 1. Parse the source once into a data tree with source spans
//! 2. Query with XPath to determine update vs insert
//! 3. Record the splice node's original source span
//! 4. Mutate the data tree (update value or insert new children)
//! 5. Render the modified tree with span tracking
//! 6. Look up the splice node's new span from the renderer's span map
//! 7. Splice the new span into the original source
//!
//! The source is parsed exactly once. The renderer annotates each node
//! with its byte span in the output, keyed by the node's original source
//! position (`start` attribute). This avoids any re-parsing or re-querying.
//!
//! All language-specific knowledge lives in the parser and renderer.
//! The upsert algorithm itself is language-agnostic.

use crate::parser::{parse, parse_string_to_xot, ParseInput, ParseOptions, XeeParseResult};
use crate::tree_mode::TreeMode;
pub use crate::xpath::Match;

#[cfg(feature = "native")]
use crate::tree::data::{DataTree, ScalarKind};

/// Languages whose upsert path is implemented (`json` / `yaml` /
/// `yml`, all via the typed-tree pipeline). Other languages return
/// [`UpsertError::UnsupportedLanguage`] so the executor's fallback
/// (text-replacement for string values) can take over.
fn lang_supports_upsert(lang: &str) -> bool {
    matches!(lang, "json" | "yaml" | "yml")
}

/// Result of an upsert operation.
#[derive(Debug)]
pub struct UpsertResult {
    /// The modified source string.
    pub source: String,
    /// Whether an insertion was made (vs. an update of existing value).
    pub inserted: bool,
    /// Number of matches that were updated (0 for inserts).
    pub matches_updated: usize,
    /// The matches that were updated (with original locations).
    /// Empty for inserts or when no matches were found.
    pub matches: Vec<Match>,
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

/// Update-only: modify existing matched nodes without creating new structure.
///
/// Like [`upsert`], but if the XPath does not match any existing nodes, no
/// changes are made (no intermediate nodes are created). Returns an
/// `UpsertResult` with `matches_updated == 0` when nothing matched.
pub fn update_only(
    source: &str,
    lang: &str,
    xpath: &str,
    value: &str,
    limit: Option<usize>,
) -> Result<UpsertResult, UpsertError> {
    if !lang_supports_upsert(lang) {
        return Err(UpsertError::UnsupportedLanguage(lang.to_string()));
    }

    // Parse source into data tree
    let mut result = parse(
        ParseInput::Inline {
            content: source,
            file_label: "<update>",
        },
        ParseOptions {
            language: Some(lang),
            tree_mode: Some(TreeMode::Data),
            ignore_whitespace: false,
            parse_depth: None,
        },
    )
    .map_err(|e| UpsertError::Parse(e.to_string()))?;

    // Query with XPath
    let existing = result.query(xpath)
        .map_err(|e| UpsertError::Query(e.to_string()))?;

    if existing.is_empty() {
        // No matches — return unchanged source, zero updates
        return Ok(UpsertResult {
            source: source.to_string(),
            inserted: false,
            matches_updated: 0,
            matches: vec![],
            description: "no matches found".to_string(),
        });
    }

    let matches = if let Some(n) = limit {
        &existing[..n.min(existing.len())]
    } else {
        &existing
    };
    update_existing(source, lang, value, matches, result)
}

/// Upsert a value into a source string at the path given by an XPath expression.
///
/// If the XPath already matches an element, its text content is replaced with
/// `value`. If the XPath does not match, the minimal structure is created and
/// inserted.
///
/// Internally parses with [`TreeMode::Data`] and renders back via the data-mode
/// renderer. XPath expressions must therefore use data-level paths (e.g.
/// `//database/host`), not AST-level node names.
pub fn upsert(
    source: &str,
    lang: &str,
    xpath: &str,
    value: &str,
    limit: Option<usize>,
) -> Result<UpsertResult, UpsertError> {
    upsert_typed(source, lang, xpath, value, limit, Some("string"))
}

/// Like [`upsert`] but with explicit control over the value kind annotation.
///
/// `value_kind` controls the type annotation on inserted leaf nodes:
///   - `Some("string")` — force string (default for `--value`)
///   - `Some("null")` / `Some("number")` etc. — force that kind
///   - `None` — let the renderer auto-detect from the value text
pub fn upsert_typed(
    source: &str,
    lang: &str,
    xpath: &str,
    value: &str,
    limit: Option<usize>,
    value_kind: Option<&str>,
) -> Result<UpsertResult, UpsertError> {
    if !lang_supports_upsert(lang) {
        return Err(UpsertError::UnsupportedLanguage(lang.to_string()));
    }

    // Step 1: Parse source into data tree
    let mut result = parse(
        ParseInput::Inline {
            content: source,
            file_label: "<upsert>",
        },
        ParseOptions {
            language: Some(lang),
            tree_mode: Some(TreeMode::Data),
            ignore_whitespace: false,
            parse_depth: None,
        },
    )
    .map_err(|e| UpsertError::Parse(e.to_string()))?;

    // Query with XPath to determine update vs insert
    let existing = result.query(xpath)
        .map_err(|e| UpsertError::Query(e.to_string()))?;

    if !existing.is_empty() {
        // Update path — handle all matches in a single pass
        let matches = if let Some(n) = limit {
            &existing[..n.min(existing.len())]
        } else {
            &existing
        };
        update_existing(source, lang, value, matches, result)
    } else {
        // Insert path
        insert_new(source, lang, xpath, value, value_kind, result)
    }
}

/// Update existing nodes' values using render-with-spans-splice.
///
/// All currently-supported upsert languages (`json` / `yaml` / `yml`)
/// take the typed-`DataTree` reverse path; the entry-point allowlist
/// rejects everything else with [`UpsertError::UnsupportedLanguage`]
/// before reaching here, so this function is a thin dispatcher.
fn update_existing(
    source: &str,
    lang: &str,
    value: &str,
    matches: &[Match],
    _result: XeeParseResult,
) -> Result<UpsertResult, UpsertError> {
    #[cfg(feature = "native")]
    {
        update_existing_via_data_ir(source, lang, value, matches)
    }
    #[cfg(not(feature = "native"))]
    {
        let _ = (source, lang, value, matches);
        Err(UpsertError::UnsupportedLanguage(lang.to_string()))
    }
}

/// tree-direct update for data languages (json / yaml). Mirror of
/// [`update_existing`]'s splice loop but reading the typed
/// [`DataTree`] tree — no `XmlNode` intermediate.
#[cfg(feature = "native")]
fn update_existing_via_data_ir(
    source: &str,
    lang: &str,
    value: &str,
    matches: &[Match],
) -> Result<UpsertResult, UpsertError> {
    let parsed = parse_string_to_xot(
        source,
        lang,
        "<update>".to_string(),
        Some(TreeMode::Data),
    )
    .map_err(|e| UpsertError::Parse(e.to_string()))?;
    let mut tree = *parsed.data_tree.ok_or_else(|| {
        UpsertError::Parse(format!(
            "language '{}' did not produce a DataTree on the tree pipeline",
            lang,
        ))
    })?;

    // Step 1: Record original byte spans and mutate every matched
    // value in the typed tree. Each match's source position lines up
    // with the value-scalar leaf in `DataTree` (the tree's keyed render
    // sets element line/col to value.span()).
    let mut splice_info: Vec<(usize, usize, (u32, u32))> = Vec::new();

    for matched in matches {
        let orig_start = line_col_to_byte_offset(source, matched.line, matched.column)
            .ok_or_else(|| UpsertError::NoInsertionPoint("start position out of bounds".into()))?;
        let orig_end = line_col_to_byte_offset(source, matched.end_line, matched.end_column)
            .ok_or_else(|| UpsertError::NoInsertionPoint("end position out of bounds".into()))?;

        let target = tree.find_at_offset_mut(orig_start as u32).ok_or_else(|| {
            UpsertError::NoInsertionPoint(format!(
                "could not locate node at byte offset {} in DataTree",
                orig_start,
            ))
        })?;

        // Preserve the target's existing scalar variant so that
        // updates respect the original kind (string stays string,
        // number stays number) — matches the legacy behavior where
        // the kind attribute on the rendered xot was not modified
        // by `replace_text_content`.
        let kind = scalar_kind_of(target);
        target
            .set_scalar(value, kind)
            .map_err(|e| UpsertError::Render(format!("set_scalar: {}", e)))?;

        let span_key = (matched.line, matched.column);
        splice_info.push((orig_start, orig_end, span_key));
    }

    // Step 2: Render the modified tree with span tracking.
    let (rendered, span_map) = render_data_ir_with_spans(&tree, lang, source);

    // Step 3: Sort splices by descending position and apply.
    splice_info.sort_by(|a, b| b.0.cmp(&a.0));
    splice_info.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);

    let mut new_source = source.to_string();
    let mut applied = 0;
    for (orig_start, orig_end, span_key) in &splice_info {
        let (new_start, new_end) = span_map.get(span_key).ok_or_else(|| {
            UpsertError::NoInsertionPoint(format!(
                "node at {}:{} not found in tree span map",
                span_key.0, span_key.1,
            ))
        })?;
        new_source.replace_range(*orig_start..*orig_end, &rendered[*new_start..*new_end]);
        applied += 1;
    }

    Ok(UpsertResult {
        source: new_source,
        inserted: false,
        matches_updated: applied,
        matches: matches.to_vec(),
        description: format!(
            "updated {} existing value{}",
            applied,
            if applied == 1 { "" } else { "s" },
        ),
    })
}

#[cfg(feature = "native")]
fn scalar_kind_of(tree: &DataTree) -> ScalarKind {
    match tree {
        DataTree::String { .. } => ScalarKind::String,
        DataTree::Number { .. } => ScalarKind::Number,
        DataTree::Bool { .. } => ScalarKind::Bool,
        DataTree::Null { .. } => ScalarKind::Null,
        _ => ScalarKind::String,
    }
}

#[cfg(feature = "native")]
fn render_data_ir_with_spans(
    tree: &DataTree,
    lang: &str,
    source: &str,
) -> (String, std::collections::HashMap<(u32, u32), (usize, usize)>) {
    let (indent, newline) = detect_indent_and_newline(source);
    match lang {
        "json" => {
            let opts = crate::tree::source::data_json::JsonRenderOptions {
                indent,
                newline,
                indent_level: 0,
            };
            crate::tree::source::data_json::render_json_with_spans(tree, &opts)
        }
        "yaml" | "yml" => {
            let opts = crate::tree::source::data_yaml::YamlRenderOptions {
                indent,
                newline,
                indent_level: 0,
            };
            crate::tree::source::data_yaml::render_yaml_with_spans(tree, &opts)
        }
        _ => unreachable!("render_data_ir_with_spans called for non-data language: {}", lang),
    }
}

/// Insert new structure using render-with-spans-splice. All
/// supported upsert languages (`json` / `yaml` / `yml`) flow through
/// the typed-`DataTree` insertion path; the entry-point allowlist
/// rejects everything else upstream.
fn insert_new(
    source: &str,
    lang: &str,
    xpath: &str,
    value: &str,
    value_kind: Option<&str>,
    result: XeeParseResult,
) -> Result<UpsertResult, UpsertError> {
    #[cfg(feature = "native")]
    {
        return insert_new_via_data_ir(source, lang, xpath, value, value_kind, result);
    }
    #[cfg(not(feature = "native"))]
    {
        let _ = (source, lang, xpath, value, value_kind, result);
        Err(UpsertError::UnsupportedLanguage(lang.to_string()))
    }
}

/// tree-direct insert for data languages (json / yaml). The xee XPath
/// engine resolves the deepest matching prefix the same way as the
/// legacy path; mutation + render flows through [`DataTree`] only.
#[cfg(feature = "native")]
fn insert_new_via_data_ir(
    source: &str,
    lang: &str,
    xpath: &str,
    value: &str,
    value_kind: Option<&str>,
    mut result: XeeParseResult,
) -> Result<UpsertResult, UpsertError> {
    let (key_path, raw_segments) = xpath_to_key_path(xpath)?;
    if key_path.is_empty() {
        return Err(UpsertError::NoInsertionPoint(
            "XPath resolves to empty path".into(),
        ));
    }

    let xpath_prefix = if xpath.trim().starts_with("//") { "//" } else { "/" };

    // Use the xee XPath engine to find the deepest matching prefix —
    // identical to the legacy path. We only need (line, column) of
    // the deepest existing ancestor, not its xot node.
    let mut existing_depth = 0usize;
    let mut ancestor_offset: u32 = 0;
    let mut ancestor_span_key: Option<(u32, u32)> = None;

    for depth in (1..raw_segments.len()).rev() {
        let prefix_xpath = format!(
            "{}{}",
            xpath_prefix,
            raw_segments[..depth].join("/"),
        );
        let matches = result.query(&prefix_xpath).unwrap_or_default();
        if let Some(matched) = matches.first() {
            let off = line_col_to_byte_offset(source, matched.line, matched.column)
                .ok_or_else(|| UpsertError::NoInsertionPoint(
                    "ancestor span out of bounds".into(),
                ))?;
            existing_depth = depth;
            ancestor_offset = off as u32;
            ancestor_span_key = Some((matched.line, matched.column));
            break;
        }
    }

    let missing_keys = &key_path[existing_depth..];
    if missing_keys.is_empty() {
        return Err(UpsertError::NoInsertionPoint(
            "all path elements exist but XPath didn't match — predicate mismatch?".into(),
        ));
    }
    for raw_seg in raw_segments.iter().skip(existing_depth) {
        if raw_seg.contains('[') {
            return Err(UpsertError::NoInsertionPoint(format!(
                "cannot insert: segment '{}' contains a predicate which cannot be applied during node creation",
                raw_seg,
            )));
        }
    }

    let is_root_splice = existing_depth == 0;

    // Re-parse the source to obtain a mutably-owned DataTree.
    let parsed = parse_string_to_xot(
        source,
        lang,
        "<insert>".to_string(),
        Some(TreeMode::Data),
    )
    .map_err(|e| UpsertError::Parse(e.to_string()))?;
    let mut tree = *parsed.data_tree.ok_or_else(|| {
        UpsertError::Parse(format!(
            "language '{}' did not produce a DataTree on the tree pipeline",
            lang,
        ))
    })?;

    // Locate the insertion target — the deepest container
    // (Mapping / Sequence / Section) whose source range starts at
    // the ancestor offset (or `0` for root splice).
    let target_offset = if is_root_splice { 0 } else { ancestor_offset };
    let target = find_insertion_target_at_offset(&mut tree, target_offset).ok_or_else(|| {
        UpsertError::NoInsertionPoint(format!(
            "could not locate container at byte offset {} in DataTree",
            target_offset,
        ))
    })?;
    let kind = scalar_kind_from_str(value_kind);
    let missing: Vec<&str> = missing_keys.iter().map(|s| s.as_str()).collect();
    target
        .insert_nested_pair(&missing, value, kind)
        .map_err(|e| UpsertError::Render(format!("insert_nested_pair: {}", e)))?;

    let (rendered, span_map) = render_data_ir_with_spans(&tree, lang, source);

    let new_content = if is_root_splice {
        rendered.trim_end().to_string()
    } else {
        let span_key = ancestor_span_key.expect("ancestor_span_key set when !is_root_splice");
        let (new_start, new_end) = span_map.get(&span_key).ok_or_else(|| {
            UpsertError::NoInsertionPoint(format!(
                "ancestor at {}:{} not found in tree span map",
                span_key.0, span_key.1,
            ))
        })?;
        rendered[*new_start..*new_end].to_string()
    };

    let (orig_start, orig_end) = if is_root_splice {
        (0, source.len())
    } else {
        // For non-root splices the original range is the matched
        // ancestor's value span. We don't have the xot end_line / end_col
        // here directly, but we can recover them by re-querying the
        // ancestor span via DataTree's range.
        let target = tree.find_at_offset(ancestor_offset).ok_or_else(|| {
            UpsertError::NoInsertionPoint(
                "ancestor disappeared from DataTree after insert".into(),
            )
        })?;
        let r = target.range();
        (r.start as usize, r.end as usize)
    };

    let mut new_source = String::with_capacity(source.len() + value.len());
    new_source.push_str(&source[..orig_start]);
    new_source.push_str(&new_content);
    new_source.push_str(&source[orig_end..]);

    Ok(UpsertResult {
        source: new_source,
        inserted: true,
        matches_updated: 0,
        matches: vec![],
        description: format!("inserted {}", missing_keys.join("/")),
    })
}

/// Map `--kind <s>` into a [`ScalarKind`]. None → Auto.
#[cfg(feature = "native")]
fn scalar_kind_from_str(kind: Option<&str>) -> ScalarKind {
    match kind {
        Some("string") => ScalarKind::String,
        Some("number") => ScalarKind::Number,
        Some("bool") | Some("boolean") | Some("true") | Some("false") => ScalarKind::Bool,
        Some("null") => ScalarKind::Null,
        _ => ScalarKind::Auto,
    }
}

/// Walk the tree and return the deepest container
/// (Mapping / Sequence / Section) whose source range starts at the
/// given byte offset. Skips `Document` wrappers (we never insert
/// directly into a Document — the Document's first child container
/// is the user-visible root, mirroring how
/// [`descend_structural_wrappers`] handled the xot equivalent).
///
/// Used by [`insert_new_via_data_ir`] to map an XPath-derived
/// ancestor position to the right insertion target. Distinct from
/// [`DataTree::find_at_offset_mut`] which drills to the deepest match
/// — fine for value-replacement (where the target is the leaf
/// scalar) but wrong for insertion (where the target is the
/// surrounding container).
#[cfg(feature = "native")]
fn find_insertion_target_at_offset(tree: &mut DataTree, offset: u32) -> Option<&mut DataTree> {
    // Two-phase walk to keep the borrow checker happy: an immutable
    // pre-pass decides whether to descend (a deeper container
    // matches) or return `self`; the mutable descent commits to
    // exactly one borrow path.
    let has_deeper = tree
        .children_iter()
        .any(|c| has_container_at(c, offset));

    if has_deeper {
        match tree {
            DataTree::Document { children, .. }
            | DataTree::Sequence { items: children, .. }
            | DataTree::Section { children, .. } => {
                for child in children.iter_mut() {
                    if let Some(f) = find_insertion_target_at_offset(child, offset) {
                        return Some(f);
                    }
                }
                None
            }
            DataTree::Mapping { pairs, .. } => {
                for child in pairs.iter_mut() {
                    if let Some(f) = find_insertion_target_at_offset(child, offset) {
                        return Some(f);
                    }
                }
                None
            }
            DataTree::Pair { key, value, .. } => {
                if let Some(f) = find_insertion_target_at_offset(key, offset) {
                    return Some(f);
                }
                find_insertion_target_at_offset(value, offset)
            }
            _ => None,
        }
    } else {
        let is_container = matches!(
            tree,
            DataTree::Mapping { .. } | DataTree::Sequence { .. } | DataTree::Section { .. }
        );
        if is_container && tree.range().start == offset {
            Some(tree)
        } else {
            None
        }
    }
}

#[cfg(feature = "native")]
fn has_container_at(tree: &DataTree, offset: u32) -> bool {
    let is_container = matches!(
        tree,
        DataTree::Mapping { .. } | DataTree::Sequence { .. } | DataTree::Section { .. }
    );
    if is_container && tree.range().start == offset {
        return true;
    }
    tree.children_iter().any(|c| has_container_at(c, offset))
}

// ---------------------------------------------------------------------------
// XPath parsing
// ---------------------------------------------------------------------------

/// Parse an XPath into paired key-path entries: bare element names (for tree
/// walking) alongside the original raw segments (for predicate checking).
///
/// Handles simple paths like `//name`, `//db/host`, `/Files/File/name`.
/// Strips axis prefixes (`//`, `/`) and predicates (e.g. `[@attr='val']`).
fn xpath_to_key_path(xpath: &str) -> Result<(Vec<String>, Vec<String>), UpsertError> {
    let mut keys = Vec::new();
    let mut raw = Vec::new();
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
            raw.push(segment.to_string());
        }
    }

    Ok((keys, raw))
}


// ---------------------------------------------------------------------------
// Source utilities
// ---------------------------------------------------------------------------

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

/// Detect indent and newline conventions from the source: returns
/// `(indent, newline)`. Two-space default for indent if the source
/// has no indented line; `\n` default for newline unless `\r\n` is
/// present.
fn detect_indent_and_newline(source: &str) -> (String, String) {
    let newline = if source.contains("\r\n") { "\r\n" } else { "\n" };
    let indent = source
        .lines()
        .find(|line| line.starts_with(' ') || line.starts_with('\t'))
        .map(|line| {
            let trimmed = line.trim_start();
            &line[..line.len() - trimmed.len()]
        })
        .unwrap_or("  ");
    (indent.to_string(), newline.to_string())
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
        let result = upsert(source, "json", "//name", "Bob", None).unwrap();
        assert!(!result.inserted);
        assert!(result.source.contains("Bob"));
        assert!(result.source.contains("30"));
        // Must be valid JSON
        let _: serde_json::Value = serde_json::from_str(&result.source).unwrap();
    }

    #[test]
    fn update_existing_number() {
        let source = r#"{"name": "Alice", "age": 30}"#;
        let result = upsert(source, "json", "//age", "31", None).unwrap();
        assert!(!result.inserted);
        assert!(result.source.contains("31"));
        assert!(result.source.contains("Alice"));
    }

    #[test]
    fn update_preserves_surrounding_formatting() {
        let source = "{\n    \"name\": \"Alice\",\n    \"age\": 30\n}";
        let result = upsert(source, "json", "//name", "Bob", None).unwrap();
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
        let result = upsert(source, "json", "//age", "30", None).unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["name"], "Alice");
        // New values are always inserted as strings
        assert_eq!(parsed["age"], "30");
    }

    #[test]
    fn insert_string_value() {
        let source = r#"{"name": "Alice"}"#;
        let result = upsert(source, "json", "//city", "NYC", None).unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["name"], "Alice");
        assert_eq!(parsed["city"], "NYC");
    }

    #[test]
    fn insert_boolean_value() {
        let source = r#"{"name": "Alice"}"#;
        let result = upsert(source, "json", "//active", "true", None).unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        // New values are always inserted as strings
        assert_eq!(parsed["active"], "true");
    }

    #[test]
    fn insert_into_multiline() {
        let source = "{\n  \"name\": \"Alice\"\n}";
        let result = upsert(source, "json", "//age", "30", None).unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["name"], "Alice");
        // New values are always inserted as strings
        assert_eq!(parsed["age"], "30");
    }

    #[test]
    fn insert_nested_property() {
        let source = r#"{"name": "Alice"}"#;
        let result = upsert(source, "json", "//db/host", "localhost", None).unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["name"], "Alice");
        assert_eq!(parsed["db"]["host"], "localhost");
    }

    #[test]
    fn insert_into_existing_parent() {
        let source = r#"{"db": {"host": "localhost"}}"#;
        let result = upsert(source, "json", "//db/port", "5432", None).unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["db"]["host"], "localhost");
        // New values are always inserted as strings
        assert_eq!(parsed["db"]["port"], "5432");
    }

    #[test]
    fn unsupported_language_error() {
        let result = upsert("{}", "brainfuck", "//x", "1", None);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), UpsertError::UnsupportedLanguage(_)));
    }

    // ---------------------------------------------------------------------------
    // YAML update tests
    // ---------------------------------------------------------------------------

    #[test]
    fn yaml_update_existing_string() {
        let source = "name: Alice\nage: 30\n";
        let result = upsert(source, "yaml", "//name", "Bob", None).unwrap();
        assert!(!result.inserted);
        assert!(result.source.contains("Bob"));
        assert!(result.source.contains("age: 30"));
    }

    #[test]
    fn yaml_update_existing_number() {
        let source = "name: Alice\nage: 30\n";
        let result = upsert(source, "yaml", "//age", "31", None).unwrap();
        assert!(!result.inserted);
        assert!(result.source.contains("31"));
        assert!(result.source.contains("Alice"));
    }

    #[test]
    fn yaml_update_preserves_surrounding_formatting() {
        let source = "name: Alice\ndatabase:\n  host: localhost\n  port: 5432\n";
        let result = upsert(source, "yaml", "//database/host", "db.example.com", None).unwrap();
        assert!(!result.inserted);
        assert!(result.source.contains("db.example.com"));
        assert!(result.source.contains("  port: 5432"));
    }

    #[test]
    fn yaml_update_nested() {
        let source = "db:\n  host: localhost\n  port: 5432\n";
        let result = upsert(source, "yaml", "//db/host", "db.example.com", None).unwrap();
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
        let result = upsert(source, "yaml", "//db/port", "5432", None).unwrap();
        assert!(result.inserted, "source: {:?}", result.source);
        assert!(result.source.contains("host: localhost"), "source: {:?}", result.source);
        assert!(result.source.contains("port: 5432"), "source: {:?}", result.source);
    }

    #[test]
    fn insert_with_predicate_on_existing_parent() {
        // Predicates on existing ancestors are fine — only the new leaf is created
        let source = "user:\n  name: john\n";
        let result = upsert(source, "yaml", "//user[name='john']/age", "30", None).unwrap();
        assert!(result.inserted, "source: {:?}", result.source);
        assert!(result.source.contains("name: john"), "source: {:?}", result.source);
        assert!(result.source.contains("age: 30"), "source: {:?}", result.source);
    }

    #[test]
    fn insert_rejects_predicate_on_new_segment() {
        // Predicates on segments that need creation should error
        let source = "name: Alice\n";
        let result = upsert(source, "yaml", "//item[@type='x']/value", "42", None);
        assert!(result.is_err(), "should reject predicate on to-be-created segment");
        assert!(result.unwrap_err().to_string().contains("predicate"));
    }

    #[test]
    fn yaml_insert_simple_property() {
        let source = "name: Alice\n";
        let result = upsert(source, "yaml", "//age", "30", None).unwrap();
        assert!(result.inserted, "source: {:?}", result.source);
        assert!(result.source.contains("name: Alice"), "source: {:?}", result.source);
        assert!(result.source.contains("age: 30"), "source: {:?}", result.source);
    }

    #[test]
    fn yaml_insert_nested_property() {
        let source = "name: Alice\n";
        let result = upsert(source, "yaml", "//db/host", "localhost", None).unwrap();
        assert!(result.inserted, "source: {:?}", result.source);
        assert!(result.source.contains("name: Alice"), "source: {:?}", result.source);
        assert!(result.source.contains("host: localhost"), "source: {:?}", result.source);
    }

    // ---------------------------------------------------------------------------
    // XPath parsing tests
    // ---------------------------------------------------------------------------

    #[test]
    fn xpath_to_key_path_simple() {
        let (keys, raw) = xpath_to_key_path("//name").unwrap();
        assert_eq!(keys, vec!["name"]);
        assert_eq!(raw, vec!["name"]);
    }

    #[test]
    fn xpath_to_key_path_nested() {
        let (keys, raw) = xpath_to_key_path("//db/host").unwrap();
        assert_eq!(keys, vec!["db", "host"]);
        assert_eq!(raw, vec!["db", "host"]);
    }

    #[test]
    fn xpath_to_key_path_strips_structural() {
        let (keys, _) = xpath_to_key_path("/Files/File/name").unwrap();
        assert_eq!(keys, vec!["name"]);
    }

    #[test]
    fn xpath_to_key_path_with_predicates() {
        let (keys, raw) = xpath_to_key_path("//item[@type='x']/name").unwrap();
        // keys have predicates stripped for tree walking
        assert_eq!(keys, vec!["item", "name"]);
        // raw preserves predicates for later checking
        assert_eq!(raw, vec!["item[@type='x']", "name"]);
    }

    #[test]
    fn yaml_update_with_value_predicate() {
        let source = "servers:\n  - name: web-1\n    port: 8080\n  - name: web-2\n    port: 8080\n  - name: web-3\n    port: 9090\n";
        let result = upsert(source, "yaml", "//servers/port[.='8080']", "3000", None).unwrap();
        assert!(result.source.contains("port: 3000"), "first match should be updated: {}", result.source);
    }

    // ---------------------------------------------------------------------------
    // Multi-match batch update tests
    // ---------------------------------------------------------------------------

    #[test]
    fn json_update_all_matches_in_single_call() {
        let source = r#"{"items": [{"val": 1}, {"val": 1}, {"val": 2}]}"#;
        let result = upsert(source, "json", "//items/val", "99", None).unwrap();
        assert!(!result.inserted);
        assert_eq!(result.matches_updated, 3, "should update all 3 val nodes");
        // All values should be updated
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        for item in parsed["items"].as_array().unwrap() {
            assert_eq!(item["val"], 99, "source: {}", result.source);
        }
    }

    #[test]
    fn yaml_update_all_matches_in_single_call() {
        let source = "servers:\n  - name: web-1\n    port: 8080\n  - name: web-2\n    port: 8080\n  - name: web-3\n    port: 9090\n";
        let result = upsert(source, "yaml", "//servers/port", "3000", None).unwrap();
        assert!(!result.inserted);
        assert_eq!(result.matches_updated, 3, "should update all 3 port nodes");
        // All ports should now be 3000
        assert!(!result.source.contains("8080"), "source: {}", result.source);
        assert!(!result.source.contains("9090"), "source: {}", result.source);
        assert_eq!(result.source.matches("port: 3000").count(), 3, "source: {}", result.source);
    }

    // ---------------------------------------------------------------------------
    // Formatting preservation tests
    // ---------------------------------------------------------------------------

    #[test]
    fn json_update_preserves_2space_indent() {
        let source = "{\n  \"name\": \"Alice\",\n  \"age\": 30\n}\n";
        let result = upsert(source, "json", "//name", "Bob", None).unwrap();
        assert!(!result.inserted);
        assert_eq!(result.source, "{\n  \"name\": \"Bob\",\n  \"age\": 30\n}\n",
            "2-space indent should be preserved: {:?}", result.source);
    }

    #[test]
    fn json_update_preserves_4space_indent() {
        let source = "{\n    \"name\": \"Alice\",\n    \"age\": 30\n}\n";
        let result = upsert(source, "json", "//name", "Bob", None).unwrap();
        assert!(!result.inserted);
        assert_eq!(result.source, "{\n    \"name\": \"Bob\",\n    \"age\": 30\n}\n",
            "4-space indent should be preserved: {:?}", result.source);
    }

    #[test]
    fn json_update_preserves_tab_indent() {
        let source = "{\n\t\"name\": \"Alice\",\n\t\"age\": 30\n}\n";
        let result = upsert(source, "json", "//name", "Bob", None).unwrap();
        assert!(!result.inserted);
        assert_eq!(result.source, "{\n\t\"name\": \"Bob\",\n\t\"age\": 30\n}\n",
            "tab indent should be preserved: {:?}", result.source);
    }

    #[test]
    fn json_insert_matches_2space_indent() {
        let source = "{\n  \"name\": \"Alice\"\n}\n";
        let result = upsert(source, "json", "//age", "30", None).unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["name"], "Alice");
        assert_eq!(parsed["age"], "30");
        // Inserted property should use 2-space indent like existing content
        assert!(result.source.contains("\n  \"age\""),
            "inserted property should use 2-space indent: {:?}", result.source);
    }

    #[test]
    fn json_insert_matches_4space_indent() {
        let source = "{\n    \"name\": \"Alice\"\n}\n";
        let result = upsert(source, "json", "//age", "30", None).unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["name"], "Alice");
        assert_eq!(parsed["age"], "30");
        // Inserted property should use 4-space indent like existing content
        assert!(result.source.contains("\n    \"age\""),
            "inserted property should use 4-space indent: {:?}", result.source);
    }

    #[test]
    fn json_insert_matches_tab_indent() {
        let source = "{\n\t\"name\": \"Alice\"\n}\n";
        let result = upsert(source, "json", "//age", "30", None).unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["name"], "Alice");
        assert_eq!(parsed["age"], "30");
        // Inserted property should use tab indent like existing content
        assert!(result.source.contains("\n\t\"age\""),
            "inserted property should use tab indent: {:?}", result.source);
    }

    #[test]
    fn json_nested_insert_preserves_indent_depth() {
        let source = "{\n  \"db\": {\n    \"host\": \"localhost\"\n  }\n}\n";
        let result = upsert(source, "json", "//db/port", "5432", None).unwrap();
        assert!(result.inserted);
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["db"]["host"], "localhost");
        assert_eq!(parsed["db"]["port"], "5432");
        // Nested insert should use 4 spaces (2 levels deep)
        assert!(result.source.contains("\n    \"port\""),
            "nested insert should use 2 levels of 2-space indent: {:?}", result.source);
    }

    #[test]
    fn json_update_preserves_crlf_newlines() {
        let source = "{\r\n  \"name\": \"Alice\",\r\n  \"age\": 30\r\n}\r\n";
        let result = upsert(source, "json", "//name", "Bob", None).unwrap();
        assert!(!result.inserted);
        assert!(result.source.contains("\r\n"),
            "CRLF newlines should be preserved: {:?}", result.source);
        assert!(result.source.contains("Bob"));
    }

    // ---------------------------------------------------------------------------
    // Edge case formatting tests: minified, inline, mixed styles
    // ---------------------------------------------------------------------------

    #[test]
    fn json_update_minified_stays_minified() {
        // Minified JSON with no whitespace
        let source = r#"{"name":"Alice","age":30}"#;
        let result = upsert(source, "json", "//name", "Bob", None).unwrap();
        assert!(!result.inserted);
        eprintln!("minified update result: {:?}", result.source);
        // Update via splice should preserve the compact style
        assert_eq!(result.source, r#"{"name":"Bob","age":30}"#,
            "minified JSON should stay minified on update: {:?}", result.source);
    }

    #[test]
    fn json_insert_into_minified() {
        // What happens when we insert into minified JSON?
        let source = r#"{"name":"Alice"}"#;
        let result = upsert(source, "json", "//age", "30", None).unwrap();
        assert!(result.inserted);
        eprintln!("minified insert result: {:?}", result.source);
        // Should still produce valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert_eq!(parsed["name"], "Alice");
        assert_eq!(parsed["age"], "30");
    }

    // ---------------------------------------------------------------------------
    // update_only tests
    // ---------------------------------------------------------------------------

    #[test]
    fn update_only_existing_string() {
        let source = r#"{"name": "Alice", "age": 30}"#;
        let result = update_only(source, "json", "//name", "Bob", None).unwrap();
        assert!(!result.inserted);
        assert_eq!(result.matches_updated, 1);
        assert!(result.source.contains("Bob"));
        assert!(result.source.contains("30"));
    }

    #[test]
    fn update_only_no_match_returns_unchanged() {
        let source = r#"{"name": "Alice"}"#;
        let result = update_only(source, "json", "//nonexistent", "value", None).unwrap();
        assert!(!result.inserted);
        assert_eq!(result.matches_updated, 0);
        assert_eq!(result.source, source, "source should be unchanged when no match");
    }

    #[test]
    fn update_only_does_not_create_missing_path() {
        let source = r#"{"name": "Alice"}"#;
        let result = update_only(source, "json", "//db/host", "localhost", None).unwrap();
        assert!(!result.inserted);
        assert_eq!(result.matches_updated, 0);
        assert_eq!(result.source, source, "should not create //db/host");
        // Verify no db key was added
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        assert!(parsed.get("db").is_none(), "db key should not exist");
    }

    #[test]
    fn update_only_multiple_matches() {
        let source = r#"{"items": [{"val": 1}, {"val": 2}, {"val": 3}]}"#;
        let result = update_only(source, "json", "//items/val", "99", None).unwrap();
        assert!(!result.inserted);
        assert_eq!(result.matches_updated, 3);
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        for item in parsed["items"].as_array().unwrap() {
            assert_eq!(item["val"], 99);
        }
    }

    #[test]
    fn update_only_respects_limit() {
        let source = r#"{"items": [{"val": 1}, {"val": 2}, {"val": 3}]}"#;
        let result = update_only(source, "json", "//items/val", "99", Some(1)).unwrap();
        assert!(!result.inserted);
        assert_eq!(result.matches_updated, 1);
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        let vals: Vec<_> = parsed["items"].as_array().unwrap()
            .iter().map(|i| i["val"].as_i64().unwrap()).collect();
        assert_eq!(vals[0], 99, "first should be updated");
        // At least one should remain unchanged
        assert!(vals[1] != 99 || vals[2] != 99, "limit should prevent updating all");
    }

    #[test]
    fn update_only_unsupported_language() {
        let result = update_only("{}", "brainfuck", "//x", "1", None);
        assert!(matches!(result.unwrap_err(), UpsertError::UnsupportedLanguage(_)));
    }

    #[test]
    fn yaml_update_only_existing() {
        let source = "name: Alice\nage: 30\n";
        let result = update_only(source, "yaml", "//name", "Bob", None).unwrap();
        assert!(!result.inserted);
        assert_eq!(result.matches_updated, 1);
        assert!(result.source.contains("Bob"));
        assert!(result.source.contains("age: 30"));
    }

    #[test]
    fn yaml_update_only_no_match() {
        let source = "name: Alice\n";
        let result = update_only(source, "yaml", "//nonexistent", "value", None).unwrap();
        assert_eq!(result.matches_updated, 0);
        assert_eq!(result.source, source);
    }

    #[test]
    fn yaml_update_only_does_not_create_missing_path() {
        let source = "name: Alice\n";
        let result = update_only(source, "yaml", "//db/host", "localhost", None).unwrap();
        assert_eq!(result.matches_updated, 0);
        assert_eq!(result.source, source, "should not create //db/host");
        assert!(!result.source.contains("db"), "db key should not be created");
        assert!(!result.source.contains("host"), "host key should not be created");
    }

    #[test]
    fn yaml_update_only_nested_existing() {
        let source = "db:\n  host: localhost\n  port: 5432\n";
        let result = update_only(source, "yaml", "//db/host", "db.example.com", None).unwrap();
        assert_eq!(result.matches_updated, 1);
        assert!(result.source.contains("db.example.com"));
        assert!(result.source.contains("port: 5432"));
    }

    #[test]
    fn yaml_update_only_partial_path_no_create() {
        // db exists but port doesn't — update_only should NOT create port
        let source = "db:\n  host: localhost\n";
        let result = update_only(source, "yaml", "//db/port", "5432", None).unwrap();
        assert_eq!(result.matches_updated, 0);
        assert_eq!(result.source, source, "should not create missing port under existing db");
    }

    #[test]
    fn json_update_inline_object_stays_inline() {
        // Object that fits on one line with spaces
        let source = r#"{"items": [{"name": "a", "val": 1}, {"name": "b", "val": 2}]}"#;
        let result = upsert(source, "json", "//items/val", "99", None).unwrap();
        assert!(!result.inserted);
        eprintln!("inline update result: {:?}", result.source);
        // Updates via splice should preserve the inline style
        let parsed: serde_json::Value = serde_json::from_str(&result.source).unwrap();
        for item in parsed["items"].as_array().unwrap() {
            assert_eq!(item["val"], 99);
        }
    }
}
