//! WASM bindings for tractor-core
//!
//! Provides JavaScript-callable functions for parsing source code to XML
//! using a serialized TreeSitter AST.

use wasm_bindgen::prelude::*;
use crate::wasm_ast::{SerializedNode, ParseRequest, ParseResponse};
use crate::xot_builder::XotBuilder;
use crate::xot_transform::walk_transform;
use crate::languages::get_transform;
use crate::output::RenderOptions;

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

    let xml = parse_ast_to_xml(&request.ast, &request.source, &request.language, &request.file_path, request.raw_mode, request.include_locations, request.pretty_print)
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

    parse_ast_to_xml(&ast, source, language, "input", raw_mode, include_locations, pretty_print)
        .map_err(|e| JsValue::from_str(&e))
}

/// Internal function to convert AST to XML
fn parse_ast_to_xml(
    ast: &SerializedNode,
    source: &str,
    language: &str,
    file_path: &str,
    raw_mode: bool,
    include_locations: bool,
    pretty_print: bool,
) -> Result<String, String> {
    // Build the raw xot document
    let mut builder = XotBuilder::new();
    let root = builder.build_raw_from_serialized(ast, source, file_path)
        .map_err(|e| format!("Failed to build XML: {}", e))?;

    let mut xot = builder.into_xot();

    // Apply transforms if not in raw mode
    if !raw_mode {
        let transform_fn = get_transform(language);
        walk_transform(&mut xot, root, transform_fn)
            .map_err(|e| format!("Transform failed: {}", e))?;
    }

    // Render to XML string
    let options = RenderOptions {
        use_color: false,
        include_locations,
        indent: "  ".to_string(),
        max_depth: None,
        highlights: None,
        pretty_print,
    };

    Ok(crate::output::render_document(&xot, root, &options))
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

    let ast: crate::wasm_ast::SerializedNode = serde_json::from_str(ast_json)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse AST: {}", e)))?;

    // Build the xot document (same as parse_ast_to_xml)
    let mut builder = XotBuilder::new();
    let root = builder.build_raw_from_serialized(&ast, source, "input")
        .map_err(|e| JsValue::from_str(&format!("Failed to build XML: {}", e)))?;

    let mut xot = builder.into_xot();

    // Apply transforms if not in raw mode
    if !raw_mode {
        let transform_fn = get_transform(language);
        walk_transform(&mut xot, root, transform_fn)
            .map_err(|e| JsValue::from_str(&format!("Transform failed: {}", e)))?;
    }

    // Collect schema from the xot tree
    let mut collector = SchemaCollector::new();
    collector.collect_from_xot(&xot, root);

    // Convert to serializable tree, unwrapping the Files/File wrapper
    // (web playground only handles a single file)
    let mut schema_tree = collector.to_schema_tree();
    if schema_tree.len() == 1 && schema_tree[0].name == "Files" {
        let files_node = schema_tree.remove(0);
        schema_tree = files_node.children;
        if schema_tree.len() == 1 && schema_tree[0].name == "File" {
            let file_node = schema_tree.remove(0);
            schema_tree = file_node.children;
        }
    }

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
/// Returns JSON with: { valid: boolean, error?: string, warnings: string[] }
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
    let options = RenderOptions {
        use_color,
        include_locations,
        indent: "  ".to_string(),
        max_depth: None,
        highlights: None,
        pretty_print: true,
    };
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
        assert!(!has_transforms("json"));
        assert!(!has_transforms("xml"));
    }
}
