//! WASM bindings for tractor
//!
//! Provides JavaScript-callable functions for parsing source code to XML
//! using a serialized TreeSitter AST.

pub mod ast;

use wasm_bindgen::prelude::*;
use ast::{SerializedNode, ParseRequest, ParseResponse};
use crate::languages::TreeKind;
use crate::output::RenderOptions;
use crate::tree_mode::TreeMode;

/// Initialize panic hook for better error messages in browser console
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Parse source code to XML using a pre-parsed AST from web-tree-sitter
///
/// # Arguments
/// * `request_json` - JSON string containing a ParseRequest:
///   - `ast`: Serialized TreeSitter AST from web-tree-sitter
///   - `source`: Original source code string
///   - `language`: Language identifier (e.g., "csharp", "typescript")
///   - `filePath`: Optional file path for the output (default: "input")
///   - `rawMode`: Whether to skip transforms (default: false)
///
/// # Returns
/// JSON string containing ParseResponse with the generated XML
#[wasm_bindgen(js_name = parseToXml)]
pub fn parse_to_xml(request_json: &str) -> Result<String, JsValue> {
    let request: ParseRequest = serde_json::from_str(request_json)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse request: {}", e)))?;

    // Resolve tree mode: explicit tree_mode takes precedence, then raw_mode for backwards compat
    let tree_mode = resolve_wasm_tree_mode(request.tree_mode.as_deref(), request.raw_mode);
    let xml = parse_ast_to_xml(&request.ast, &request.source, &request.language, &request.file_path, tree_mode, request.include_locations, request.pretty_print)
        .map_err(|e| JsValue::from_str(&e))?;

    let response = ParseResponse {
        xml,
        warnings: vec![],
    };

    serde_json::to_string(&response)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize response: {}", e)))
}

/// Parse with individual parameters (simpler API for direct calls)
///
/// # Arguments
/// * `ast_json` - JSON string of the serialized AST
/// * `source` - Original source code
/// * `language` - Language identifier
/// * `raw_mode` - Whether to skip transforms
/// * `include_locations` - Whether to include kind/start/end attributes
/// * `pretty_print` - Whether to format with indentation and newlines
///
/// # Returns
/// XML string directly
#[wasm_bindgen(js_name = parseAstToXml)]
pub fn parse_ast_to_xml_simple(
    ast_json: &str,
    source: &str,
    language: &str,
    raw_mode: bool,
    include_locations: bool,
    pretty_print: bool,
) -> Result<String, JsValue> {
    let ast: SerializedNode = serde_json::from_str(ast_json)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse AST: {}", e)))?;

    let tree_mode = resolve_wasm_tree_mode(None, raw_mode);
    parse_ast_to_xml(&ast, source, language, "input", tree_mode, include_locations, pretty_print)
        .map_err(|e| JsValue::from_str(&e))
}

/// Resolve tree mode from WASM request parameters
fn resolve_wasm_tree_mode(tree_mode_str: Option<&str>, raw_mode: bool) -> Option<TreeMode> {
    match tree_mode_str {
        Some("raw") => Some(TreeMode::Raw),
        Some("structure") => Some(TreeMode::Structure),
        Some("data") => Some(TreeMode::Data),
        Some(_) => None, // invalid string → auto-detect
        None if raw_mode => Some(TreeMode::Raw), // backwards compat
        None => None, // auto-detect
    }
}

/// Internal function to convert AST to XML.
///
/// For every language with a typed-tree lowering
/// (`TreeKind::Syntax` / `TreeKind::Sql` / `TreeKind::Data`), routes
/// through the same `RawNode → typed tree → render` path the native
/// CLI uses, closing the WASM↔CLI semantic-XML gap that motivated
/// S6. Raw tree mode (and any future `TreeKind::None` languages) use
/// the bare `XotBuilder` CST dump.
///
/// `pub` so the S6A/S6C parity tests can exercise the WASM code path
/// natively (without spinning up a real WASM runtime).
pub fn parse_ast_to_xml(
    ast: &SerializedNode,
    source: &str,
    language: &str,
    file_path: &str,
    tree_mode: Option<TreeMode>,
    include_locations: bool,
    pretty_print: bool,
) -> Result<String, String> {
    let resolved = TreeMode::resolve(tree_mode, language)?;

    // Route programming-language parses through the typed pipeline
    // when the registry advertises a syntax / sql / data lowering.
    // Raw mode still drops through to the bare CST dump.
    if resolved != TreeMode::Raw {
        if let Some(lang_ops) = crate::languages::get_language(language) {
            match lang_ops.tree_kind {
                TreeKind::Syntax(lower) => {
                    return parse_via_syntax_tree(ast, source, lower, include_locations, pretty_print);
                }
                TreeKind::Sql(lower) => {
                    return parse_via_sql_tree(ast, source, lower, include_locations, pretty_print);
                }
                TreeKind::Data { structure, content } => {
                    let parser = match resolved {
                        TreeMode::Structure => structure,
                        TreeMode::Data => content,
                        TreeMode::Raw => unreachable!("filtered by outer condition"),
                    };
                    return parse_via_data_tree(ast, source, parser, include_locations, pretty_print);
                }
                TreeKind::None => {}
            }
        }
    }

    // Raw mode (and any `TreeKind::None` language) — typed
    // passthrough preserving anonymous tokens. Mirrors the legacy
    // imperative dump but routed through `SyntaxTree::Raw` so the
    // imperative `XotBuilder` can retire.
    parse_via_raw_passthrough(ast, source, include_locations, pretty_print)
}

/// Bare CST dump via `lower_raw_passthrough_all` — replaces the
/// legacy `XotBuilder::build_raw_from_serialized`. Anonymous
/// tree-sitter tokens (punctuation, keywords) are preserved as text
/// nodes between named-element children.
fn parse_via_raw_passthrough(
    ast: &SerializedNode,
    source: &str,
    include_locations: bool,
    pretty_print: bool,
) -> Result<String, String> {
    let (xot, doc) = raw_passthrough_xot(ast, source)?;
    let options = RenderOptions::new()
        .with_meta(include_locations)
        .with_pretty_print(pretty_print);
    Ok(crate::output::render_document(&xot, doc, &options))
}

/// Shared helper: serialise the AST through `lower_raw_passthrough_all`
/// and render to a fresh xot document. Used by both `parse_ast_to_xml`
/// (XML output) and `get_schema_tree` (schema collection).
fn raw_passthrough_xot(
    ast: &SerializedNode,
    source: &str,
) -> Result<(xot::Xot, xot::Node), String> {
    let json = serde_json::to_string(ast)
        .map_err(|e| format!("Failed to serialise AST: {}", e))?;
    let raw: crate::raw::RawNode = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to deserialise RawNode: {}", e))?;

    let mut tree = crate::tree::lower_raw_passthrough_all(&raw, source);
    crate::tree::assign_ids_syntax(&mut tree);

    let mut xot = xot::Xot::new();
    let doc = xot.new_document();
    crate::tree::render_to_xot(&mut xot, doc, &tree, source)
        .map_err(|e| format!("tree render failed: {}", e))?;
    Ok((xot, doc))
}

/// Route a `SerializedNode` AST through the typed `SyntaxTree`
/// pipeline: RawNode (deserialised from JSON) → `lower(...)` →
/// `assign_ids_syntax` → `render_to_xot` → XML string. Mirrors the
/// native CLI's `parse_with_ir_pipeline` so WASM and CLI produce
/// byte-identical output for the same source.
fn parse_via_syntax_tree(
    ast: &SerializedNode,
    source: &str,
    lower: crate::languages::LowerToSyntaxTree,
    include_locations: bool,
    pretty_print: bool,
) -> Result<String, String> {
    // `SerializedNode` and `RawNode` are shape-identical (camelCase
    // JSON, same field names). Round-trip via JSON to convert without
    // hand-mirroring a translation pass.
    let json = serde_json::to_string(ast)
        .map_err(|e| format!("Failed to serialise AST: {}", e))?;
    let raw: crate::raw::RawNode = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to deserialise RawNode: {}", e))?;

    let mut tree = lower(&raw, source);
    crate::tree::assign_ids_syntax(&mut tree);

    let mut xot = xot::Xot::new();
    let doc = xot.new_document();
    crate::tree::render_to_xot(&mut xot, doc, &tree, source)
        .map_err(|e| format!("tree render failed: {}", e))?;

    let options = RenderOptions::new()
        .with_meta(include_locations)
        .with_pretty_print(pretty_print);

    Ok(crate::output::render_document(&xot, doc, &options))
}

/// Route through the typed `DataTree` pipeline (json / yaml / toml /
/// ini / env / markdown). Each language carries two `DataParser`s in
/// its `TreeKind::Data` variant — one per tree mode — selected by
/// the caller.
fn parse_via_data_tree(
    ast: &SerializedNode,
    source: &str,
    parser: crate::languages::DataParser,
    include_locations: bool,
    pretty_print: bool,
) -> Result<String, String> {
    let json = serde_json::to_string(ast)
        .map_err(|e| format!("Failed to serialise AST: {}", e))?;
    let raw: crate::raw::RawNode = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to deserialise RawNode: {}", e))?;

    let mut tree = (parser.lower)(&raw, source);
    crate::tree::assign_ids_data(&mut tree);

    let mut xot = xot::Xot::new();
    let doc = xot.new_document();
    (parser.render)(&mut xot, doc, &tree, source)
        .map_err(|e| format!("DataTree render failed: {}", e))?;

    let options = RenderOptions::new()
        .with_meta(include_locations)
        .with_pretty_print(pretty_print);

    Ok(crate::output::render_document(&xot, doc, &options))
}

/// Route through the typed `SqlTree` pipeline.
fn parse_via_sql_tree(
    ast: &SerializedNode,
    source: &str,
    lower: crate::languages::LowerToSqlTree,
    include_locations: bool,
    pretty_print: bool,
) -> Result<String, String> {
    let json = serde_json::to_string(ast)
        .map_err(|e| format!("Failed to serialise AST: {}", e))?;
    let raw: crate::raw::RawNode = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to deserialise RawNode: {}", e))?;

    let mut tree = lower(&raw, source);
    crate::tree::assign_ids_sql(&mut tree);

    let mut xot = xot::Xot::new();
    let doc = xot.new_document();
    crate::tree::sql::to_xot::render_sql_to_xot(&mut xot, doc, &tree, source)
        .map_err(|e| format!("SqlTree render failed: {}", e))?;

    let options = RenderOptions::new()
        .with_meta(include_locations)
        .with_pretty_print(pretty_print);

    Ok(crate::output::render_document(&xot, doc, &options))
}

/// Get schema tree from a parsed AST
///
/// Returns the same merged element tree as `tractor <file> -o schema`.
/// The result is a JSON array of schema nodes, each with name, count,
/// values (unique text content), and children.
///
/// # Arguments
/// * `ast_json` - JSON string of the serialized AST
/// * `source` - Original source code
/// * `language` - Language identifier
/// * `raw_mode` - Whether to skip transforms
///
/// # Returns
/// JSON array of SchemaNode objects
#[wasm_bindgen(js_name = getSchemaTree)]
pub fn get_schema_tree(
    ast_json: &str,
    source: &str,
    language: &str,
    raw_mode: bool,
) -> Result<String, JsValue> {
    use crate::output::SchemaCollector;

    let ast: ast::SerializedNode = serde_json::from_str(ast_json)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse AST: {}", e)))?;

    let tree_mode = resolve_wasm_tree_mode(None, raw_mode);
    let resolved = TreeMode::resolve(tree_mode, language)
        .map_err(|e| JsValue::from_str(&e))?;

    // Build the xot document — typed pipeline for every language
    // family (same selection as `parse_ast_to_xml`); bare CST dump
    // for raw mode and the (currently empty) `TreeKind::None` set.
    let (xot, root) = if resolved != TreeMode::Raw {
        match crate::languages::get_language(language).map(|l| l.tree_kind) {
            Some(TreeKind::Syntax(lower)) => {
                let json = serde_json::to_string(&ast)
                    .map_err(|e| JsValue::from_str(&format!("Failed to serialise AST: {}", e)))?;
                let raw: crate::raw::RawNode = serde_json::from_str(&json)
                    .map_err(|e| JsValue::from_str(&format!("Failed to deserialise RawNode: {}", e)))?;
                let mut tree = lower(&raw, source);
                crate::tree::assign_ids_syntax(&mut tree);
                let mut xot = xot::Xot::new();
                let doc = xot.new_document();
                crate::tree::render_to_xot(&mut xot, doc, &tree, source)
                    .map_err(|e| JsValue::from_str(&format!("tree render failed: {}", e)))?;
                (xot, doc)
            }
            Some(TreeKind::Sql(lower)) => {
                let json = serde_json::to_string(&ast)
                    .map_err(|e| JsValue::from_str(&format!("Failed to serialise AST: {}", e)))?;
                let raw: crate::raw::RawNode = serde_json::from_str(&json)
                    .map_err(|e| JsValue::from_str(&format!("Failed to deserialise RawNode: {}", e)))?;
                let mut tree = lower(&raw, source);
                crate::tree::assign_ids_sql(&mut tree);
                let mut xot = xot::Xot::new();
                let doc = xot.new_document();
                crate::tree::sql::to_xot::render_sql_to_xot(&mut xot, doc, &tree, source)
                    .map_err(|e| JsValue::from_str(&format!("SqlTree render failed: {}", e)))?;
                (xot, doc)
            }
            Some(TreeKind::Data { structure, content }) => {
                let parser = match resolved {
                    TreeMode::Structure => structure,
                    TreeMode::Data => content,
                    TreeMode::Raw => unreachable!("filtered by outer condition"),
                };
                let json = serde_json::to_string(&ast)
                    .map_err(|e| JsValue::from_str(&format!("Failed to serialise AST: {}", e)))?;
                let raw: crate::raw::RawNode = serde_json::from_str(&json)
                    .map_err(|e| JsValue::from_str(&format!("Failed to deserialise RawNode: {}", e)))?;
                let mut tree = (parser.lower)(&raw, source);
                crate::tree::assign_ids_data(&mut tree);
                let mut xot = xot::Xot::new();
                let doc = xot.new_document();
                (parser.render)(&mut xot, doc, &tree, source)
                    .map_err(|e| JsValue::from_str(&format!("DataTree render failed: {}", e)))?;
                (xot, doc)
            }
            _ => raw_passthrough_xot(&ast, source)
                .map_err(|e| JsValue::from_str(&e))?,
        }
    } else {
        // Raw mode: typed CST dump preserving anonymous tokens.
        raw_passthrough_xot(&ast, source)
            .map_err(|e| JsValue::from_str(&e))?
    };

    // Collect schema from the xot tree
    let mut collector = SchemaCollector::new();
    collector.collect_from_xot(&xot, root);

    // Convert to serializable tree
    let schema_tree = collector.to_schema_tree();

    serde_json::to_string(&schema_tree)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize schema: {}", e)))
}

/// Get the list of supported languages
#[wasm_bindgen(js_name = getSupportedLanguages)]
pub fn get_supported_languages() -> String {
    let names = crate::language_info::get_language_names();
    serde_json::to_string(&names).unwrap_or_else(|_| "[]".to_string())
}

/// Check if a language has transforms (vs passthrough)
#[wasm_bindgen(js_name = hasTransforms)]
pub fn has_transforms(language: &str) -> bool {
    crate::language_info::get_language_info(language)
        .map(|l| l.has_transforms)
        .unwrap_or(false)
}

/// Get detailed language information as JSON
///
/// Returns an array of language objects with: name, extensions, hasTransforms, grammarFile
#[wasm_bindgen(js_name = getLanguageInfo)]
pub fn get_language_info() -> String {
    let languages = crate::language_info::LANGUAGES;
    serde_json::to_string(&languages).unwrap_or_else(|_| "[]".to_string())
}

/// Get languages available in web (those with grammar files)
#[wasm_bindgen(js_name = getWebLanguages)]
pub fn get_web_languages() -> String {
    let languages = crate::language_info::get_web_languages();
    serde_json::to_string(&languages).unwrap_or_else(|_| "[]".to_string())
}

/// Validate an XPath expression without executing it
///
/// Returns JSON with: { valid: boolean, error?: string }
#[wasm_bindgen(js_name = validateXPath)]
pub fn validate_xpath(xpath: &str) -> String {
    let result = crate::xpath::validate_xpath(xpath);
    serde_json::to_string(&result).unwrap_or_else(|_| {
        r#"{"valid":false,"error":"Failed to serialize validation result"}"#.to_string()
    })
}

/// Extract source snippet given start/end positions in "line:col" format
///
/// # Arguments
/// * `source` - The full source code
/// * `start` - Start position as "line:col" (1-based)
/// * `end` - End position as "line:col" (1-based)
///
/// # Returns
/// The extracted source text between the positions
#[wasm_bindgen(js_name = extractSourceSnippet)]
pub fn extract_source_snippet(source: &str, start: &str, end: &str) -> Result<String, JsValue> {
    crate::source_utils::extract_snippet(source, start, end)
        .map_err(|e| JsValue::from_str(&e))
}

/// Get full source lines for a range given "line:col" format positions
///
/// # Arguments
/// * `source` - The full source code
/// * `start` - Start position as "line:col" (1-based)
/// * `end` - End position as "line:col" (1-based)
///
/// # Returns
/// JSON array of the full source lines from start line to end line
#[wasm_bindgen(js_name = getSourceLines)]
pub fn get_source_lines(source: &str, start: &str, end: &str) -> Result<String, JsValue> {
    let lines = crate::source_utils::get_source_lines_for_range(source, start, end)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_json::to_string(&lines)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize lines: {}", e)))
}

/// Pretty-print an XML string
///
/// # Arguments
/// * `xml` - The XML string to format
/// * `include_locations` - Whether to include start/end/kind attributes
/// * `use_color` - Whether to include ANSI color codes (for terminal or ANSI-to-HTML conversion)
///
/// # Returns
/// The pretty-printed XML string (with ANSI codes if use_color is true)
#[wasm_bindgen(js_name = prettyPrintXml)]
pub fn pretty_print_xml(xml: &str, include_locations: bool, use_color: bool) -> String {
    let options = RenderOptions::new()
        .with_color(use_color)
        .with_meta(include_locations);
    crate::output::render_xml_string(xml, &options)
}

/// Highlight the full source code with syntax coloring based on XML tree
///
/// Expects transformed/semantic XML, not raw TreeSitter XML.
/// Uses language-specific category mapping for accurate highlighting.
///
/// # Arguments
/// * `source` - The full source code
/// * `xml` - Complete semantic XML document with position attributes (start/end)
/// * `language` - The language name (e.g., "csharp", "rust", "typescript")
///
/// # Returns
/// The full source code with ANSI color codes for syntax highlighting
#[wasm_bindgen(js_name = highlightFullSource)]
pub fn highlight_full_source(source: &str, xml: &str, language: &str) -> String {
    use crate::output::syntax_highlight::{extract_syntax_spans_with_lang, highlight_source};
    use crate::languages::get_syntax_category;

    if source.is_empty() || xml.is_empty() {
        return source.to_string();
    }

    // Get the language-specific category function
    let category_fn = get_syntax_category(language);

    // Extract syntax spans from the full XML tree using language-specific mapping
    let spans = extract_syntax_spans_with_lang(xml, category_fn);

    if spans.is_empty() {
        return source.to_string();
    }

    // Count lines to get end position
    let lines: Vec<&str> = source.lines().collect();
    let end_line = lines.len() as u32;
    let end_col = lines.last().map(|l| l.len() as u32 + 1).unwrap_or(1);

    // Highlight the entire source
    highlight_source(source, &spans, 1, 1, end_line, end_col)
}

/// Highlight a source snippet with syntax coloring based on XML metadata
///
/// Currently unused by web UI - available for future use (e.g., highlighting query results).
/// Expects transformed/semantic XML, not raw TreeSitter XML.
///
/// # Arguments
/// * `source` - The full source code
/// * `xml` - Semantic XML fragment with position attributes (start/end)
/// * `start` - Start position as "line:col" (1-based)
/// * `end` - End position as "line:col" (1-based)
///
/// # Returns
/// The source snippet with ANSI color codes for syntax highlighting
#[wasm_bindgen(js_name = highlightSourceSnippet)]
pub fn highlight_source_snippet(
    source: &str,
    xml: &str,
    start: &str,
    end: &str,
) -> Result<String, JsValue> {
    use crate::output::syntax_highlight::{extract_syntax_spans, highlight_source};
    use crate::source_utils::parse_position;

    // Parse positions
    let (start_line, start_col) = parse_position(start)
        .ok_or_else(|| JsValue::from_str(&format!("Invalid start position: {}", start)))?;
    let (end_line, end_col) = parse_position(end)
        .ok_or_else(|| JsValue::from_str(&format!("Invalid end position: {}", end)))?;

    // Extract the source snippet
    let snippet = crate::source_utils::extract_snippet(source, start, end)
        .map_err(|e| JsValue::from_str(&e))?;

    if snippet.is_empty() {
        return Ok(String::new());
    }

    // Extract syntax spans from XML
    let spans = extract_syntax_spans(xml);

    if spans.is_empty() {
        // No syntax info available, return plain snippet
        return Ok(snippet);
    }

    // Apply highlighting
    let highlighted = highlight_source(&snippet, &spans, start_line, start_col, end_line, end_col);
    Ok(highlighted)
}

/// Highlight full source lines with syntax coloring based on XML metadata
///
/// Currently unused by web UI - available for future use (e.g., highlighting query results).
/// Expects transformed/semantic XML, not raw TreeSitter XML.
///
/// # Arguments
/// * `source` - The full source code
/// * `xml` - Semantic XML fragment with position attributes (start/end)
/// * `start` - Start position as "line:col" (1-based)
/// * `end` - End position as "line:col" (1-based)
///
/// # Returns
/// The full source lines (from start line to end line) with ANSI color codes
#[wasm_bindgen(js_name = highlightSourceLines)]
pub fn highlight_source_lines_wasm(
    source: &str,
    xml: &str,
    start: &str,
    end: &str,
) -> Result<String, JsValue> {
    use crate::output::syntax_highlight::{extract_syntax_spans, highlight_lines};
    use crate::source_utils::parse_position;

    // Parse positions
    let (start_line, _) = parse_position(start)
        .ok_or_else(|| JsValue::from_str(&format!("Invalid start position: {}", start)))?;
    let (end_line, _) = parse_position(end)
        .ok_or_else(|| JsValue::from_str(&format!("Invalid end position: {}", end)))?;

    // Get source lines
    let lines = crate::source_utils::get_source_lines(source, start_line, end_line);

    if lines.is_empty() {
        return Ok(String::new());
    }

    // Extract syntax spans from XML
    let spans = extract_syntax_spans(xml);

    if spans.is_empty() {
        // No syntax info available, return plain lines
        return Ok(lines.join("\n"));
    }

    // Apply highlighting
    let highlighted = highlight_lines(&lines, &spans, start_line, end_line);
    Ok(highlighted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_languages() {
        let json = get_supported_languages();
        assert!(json.contains("csharp"));
        assert!(json.contains("typescript"));
    }

    #[test]
    fn test_has_transforms() {
        assert!(has_transforms("csharp"));
        assert!(has_transforms("typescript"));
        // json gained transforms when the data-tree pipeline came
        // online; the metadata in `language_info.rs` reflects that.
        assert!(has_transforms("json"));
        assert!(!has_transforms("xml"));
    }
}
