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
