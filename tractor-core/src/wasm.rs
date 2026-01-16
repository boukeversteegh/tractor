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
    let languages = vec![
        "csharp", "rust", "javascript", "typescript", "python", "go",
        "java", "ruby", "cpp", "c", "json", "html", "css", "bash",
        "yaml", "php", "scala", "lua", "haskell", "ocaml", "r", "julia",
    ];
    serde_json::to_string(&languages).unwrap_or_else(|_| "[]".to_string())
}

/// Check if a language has transforms (vs passthrough)
#[wasm_bindgen(js_name = hasTransforms)]
pub fn has_transforms(language: &str) -> bool {
    matches!(language,
        "typescript" | "ts" | "tsx" | "javascript" | "js" | "jsx" |
        "csharp" | "cs" |
        "python" | "py" |
        "go" |
        "rust" | "rs" |
        "java" |
        "ruby" | "rb"
    )
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
