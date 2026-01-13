//! Syntax highlighting for source code output
//!
//! Applies ANSI color codes to source code based on XML node types.
//! Uses the XML fragment's TreeSitter node kinds to determine syntax categories.

use xot::{Xot, Node, Value};

/// ANSI color codes for syntax highlighting
pub mod ansi {
    pub const RESET: &str = "\x1b[0m";
    pub const DIM: &str = "\x1b[2m";
    pub const BLUE: &str = "\x1b[34m";   // Keywords
    pub const CYAN: &str = "\x1b[36m";   // Types, functions
    pub const YELLOW: &str = "\x1b[33m"; // Strings, numbers
    pub const WHITE: &str = "\x1b[97m";  // Identifiers
    pub const GREEN: &str = "\x1b[32m";  // Comments
}

/// Syntax categories for highlighting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxCategory {
    /// Control flow, declarations, modifiers (blue)
    Keyword,
    /// Type names and references (cyan)
    Type,
    /// Function/method names (cyan)
    Function,
    /// Variables, parameters (white)
    Identifier,
    /// String literals (yellow)
    String,
    /// Numeric literals (yellow)
    Number,
    /// Comments (green)
    Comment,
    /// Operators like +, -, = (dim)
    Operator,
    /// Punctuation like {, }, ; (dim)
    Punctuation,
    /// No specific category - no color applied
    Default,
}

impl SyntaxCategory {
    /// Get the ANSI color code for this category
    pub fn to_ansi(&self) -> Option<&'static str> {
        match self {
            SyntaxCategory::Keyword => Some(ansi::BLUE),
            SyntaxCategory::Type => Some(ansi::CYAN),
            SyntaxCategory::Function => Some(ansi::CYAN),
            SyntaxCategory::Identifier => Some(ansi::WHITE),
            SyntaxCategory::String => Some(ansi::YELLOW),
            SyntaxCategory::Number => Some(ansi::YELLOW),
            SyntaxCategory::Comment => Some(ansi::GREEN),
            SyntaxCategory::Operator => Some(ansi::DIM),
            SyntaxCategory::Punctuation => Some(ansi::DIM),
            SyntaxCategory::Default => None,
        }
    }

    /// Map an XML element name (TreeSitter node kind) to a syntax category
    pub fn from_element_name(name: &str) -> Self {
        match name {
            // Keywords - control flow
            "if" | "else" | "for" | "while" | "do" | "switch" | "case" |
            "return" | "break" | "continue" | "goto" | "throw" |
            "try" | "catch" | "finally" | "yield" | "match" |
            "if_statement" | "else_clause" | "for_statement" | "while_statement" |
            "do_statement" | "switch_statement" | "case_statement" |
            "return_statement" | "break_statement" | "continue_statement" |
            "throw_statement" | "try_statement" | "catch_clause" |
            "finally_clause" | "yield_statement" => SyntaxCategory::Keyword,

            // Keywords - declarations
            "class" | "struct" | "enum" | "interface" | "trait" | "record" |
            "namespace" | "module" | "import" | "using" | "package" | "from" |
            "fn" | "func" | "function" | "def" | "let" | "var" | "const" |
            "class_declaration" | "struct_declaration" | "enum_declaration" |
            "interface_declaration" | "record_declaration" |
            "namespace_declaration" | "using_directive" | "import_statement" |
            "function_declaration" | "method_declaration" |
            "local_declaration_statement" | "variable_declaration" => SyntaxCategory::Keyword,

            // Keywords - modifiers
            "public" | "private" | "protected" | "internal" |
            "static" | "abstract" | "virtual" | "override" | "sealed" |
            "readonly" | "async" | "await" | "unsafe" | "extern" |
            "new" | "this" | "base" | "super" | "self" |
            "modifier" | "access_modifier" => SyntaxCategory::Keyword,

            // Types
            "type" | "generic" | "nullable" | "array_type" | "pointer_type" |
            "predefined_type" | "type_parameter" | "type_argument" |
            "generic_name" | "qualified_name" | "simple_type" |
            "primitive_type" | "builtin_type" => SyntaxCategory::Type,

            // Types - pattern matching for _type suffix
            n if n.ends_with("_type") || n.contains("type_") => SyntaxCategory::Type,

            // Functions
            "method" | "constructor" | "destructor" |
            "function_definition" | "method_definition" |
            "constructor_declaration" | "destructor_declaration" |
            "invocation_expression" | "call_expression" => SyntaxCategory::Function,

            // Functions - pattern matching
            n if n.contains("function") || n.contains("method") ||
                 n.ends_with("_call") || n == "invocation" => SyntaxCategory::Function,

            // Identifiers (semantic mode names)
            "name" | "identifier" | "variable" | "parameter" |
            "simple_name" | "identifier_name" => SyntaxCategory::Identifier,

            // String literals
            "string" | "char" | "interpolated_string" | "raw_string" |
            "string_literal" | "character_literal" | "verbatim_string" |
            "interpolated_string_expression" | "string_content" => SyntaxCategory::String,

            // String - pattern matching
            n if n.contains("string") && !n.contains("interpolation") => SyntaxCategory::String,

            // Number literals
            "int" | "float" | "decimal" | "number" | "integer" |
            "integer_literal" | "real_literal" | "numeric_literal" => SyntaxCategory::Number,

            // Number - pattern matching (but exclude string_literal)
            n if n.ends_with("_literal") && !n.contains("string") && !n.contains("char") => SyntaxCategory::Number,

            // Boolean/null literals - treat as keywords
            "true" | "false" | "null" | "nil" | "none" |
            "boolean_literal" | "null_literal" => SyntaxCategory::Keyword,

            // Comments
            "comment" | "line_comment" | "block_comment" | "doc_comment" |
            "multiline_comment" | "documentation_comment" => SyntaxCategory::Comment,

            // Operators (if they appear as named nodes)
            "operator" | "binary_operator" | "unary_operator" |
            "assignment_operator" | "comparison_operator" => SyntaxCategory::Operator,

            // Operators - pattern matching
            n if n.contains("operator") || n.ends_with("_op") => SyntaxCategory::Operator,

            // Default for structural nodes
            _ => SyntaxCategory::Default,
        }
    }
}

/// A span of source code with a syntax category
#[derive(Debug, Clone)]
pub struct SyntaxSpan {
    /// 1-based start line
    pub start_line: u32,
    /// 1-based start column
    pub start_col: u32,
    /// 1-based end line
    pub end_line: u32,
    /// 1-based end column
    pub end_col: u32,
    /// The syntax category for coloring
    pub category: SyntaxCategory,
    /// Nesting depth (for "innermost wins" resolution)
    pub depth: u32,
}

impl SyntaxSpan {
    /// Check if this span contains a given position
    pub fn contains(&self, line: u32, col: u32) -> bool {
        if line < self.start_line || line > self.end_line {
            return false;
        }
        if line == self.start_line && col < self.start_col {
            return false;
        }
        if line == self.end_line && col >= self.end_col {
            return false;
        }
        true
    }
}

/// Extract syntax spans from an XML fragment
///
/// Parses the XML and walks the tree to extract position information
/// for each node, mapping element names to syntax categories.
pub fn extract_syntax_spans(xml: &str) -> Vec<SyntaxSpan> {
    if xml.is_empty() {
        return Vec::new();
    }

    let mut spans = Vec::new();
    let mut xot = Xot::new();

    // Try to parse as-is first
    if let Ok(doc) = xot.parse(xml) {
        extract_spans_recursive(&xot, doc, 0, &mut spans);
    } else {
        // Try wrapping in a root element (for fragments)
        let wrapped = format!("<_root_>{}</_root_>", xml);
        if let Ok(doc) = xot.parse(&wrapped) {
            if let Ok(doc_el) = xot.document_element(doc) {
                for child in xot.children(doc_el) {
                    extract_spans_recursive(&xot, child, 0, &mut spans);
                }
            }
        }
    }

    // Sort spans by position, then by depth (descending) for innermost-wins
    spans.sort_by(|a, b| {
        a.start_line.cmp(&b.start_line)
            .then(a.start_col.cmp(&b.start_col))
            .then(b.depth.cmp(&a.depth)) // Higher depth first
    });

    spans
}

fn extract_spans_recursive(xot: &Xot, node: Node, depth: u32, spans: &mut Vec<SyntaxSpan>) {
    match xot.value(node) {
        Value::Document => {
            for child in xot.children(node) {
                extract_spans_recursive(xot, child, depth, spans);
            }
        }
        Value::Element(element) => {
            let name = xot.local_name_str(element.name());
            let category = SyntaxCategory::from_element_name(name);

            // Only add span if we have a meaningful category
            if category != SyntaxCategory::Default {
                if let Some((start_line, start_col, end_line, end_col)) = extract_position(xot, node) {
                    spans.push(SyntaxSpan {
                        start_line,
                        start_col,
                        end_line,
                        end_col,
                        category,
                        depth,
                    });
                }
            }

            // Recurse into children
            for child in xot.children(node) {
                extract_spans_recursive(xot, child, depth + 1, spans);
            }
        }
        _ => {}
    }
}

/// Extract start and end positions from node attributes
fn extract_position(xot: &Xot, node: Node) -> Option<(u32, u32, u32, u32)> {
    let attrs = xot.attributes(node);
    let mut start: Option<(u32, u32)> = None;
    let mut end: Option<(u32, u32)> = None;

    for (attr_name_id, attr_value) in attrs.iter() {
        let attr_name = xot.local_name_str(attr_name_id);
        match attr_name {
            "start" => {
                // Format: "line:col"
                let parts: Vec<&str> = attr_value.split(':').collect();
                if parts.len() == 2 {
                    if let (Ok(line), Ok(col)) = (parts[0].parse(), parts[1].parse()) {
                        start = Some((line, col));
                    }
                }
            }
            "end" => {
                // Format: "line:col"
                let parts: Vec<&str> = attr_value.split(':').collect();
                if parts.len() == 2 {
                    if let (Ok(line), Ok(col)) = (parts[0].parse(), parts[1].parse()) {
                        end = Some((line, col));
                    }
                }
            }
            _ => {}
        }
    }

    match (start, end) {
        (Some((sl, sc)), Some((el, ec))) => Some((sl, sc, el, ec)),
        _ => None,
    }
}

/// Highlight source code using syntax spans
///
/// Takes source lines and applies ANSI color codes based on the spans.
/// Uses "innermost wins" semantics - the deepest nested span determines color.
pub fn highlight_source(
    source: &str,
    spans: &[SyntaxSpan],
    start_line: u32,
    start_col: u32,
    _end_line: u32,
    _end_col: u32,
) -> String {
    if spans.is_empty() {
        return source.to_string();
    }

    let lines: Vec<&str> = source.lines().collect();
    let mut output = String::new();
    let mut current_color: Option<&'static str> = None;

    for (line_offset, line_content) in lines.iter().enumerate() {
        let line_num = start_line + line_offset as u32;

        // The snippet is already column-precise from extract_source_snippet()
        // So character 0 of line 0 corresponds to start_col in the original
        // For subsequent lines, character 0 corresponds to column 1
        let col_offset = if line_num == start_line { start_col } else { 1 };

        // Process each character in the line
        for (char_offset, ch) in line_content.char_indices() {
            // Map snippet position to original file position
            let col = col_offset + char_offset as u32;

            // Find the deepest (highest depth) span containing this position
            let new_color = find_color_at(spans, line_num, col);

            // Switch color if needed
            if new_color != current_color {
                if current_color.is_some() {
                    output.push_str(ansi::RESET);
                }
                if let Some(color) = new_color {
                    output.push_str(color);
                }
                current_color = new_color;
            }

            output.push(ch);
        }

        // Add newline between lines (but not after the last one)
        if line_offset < lines.len() - 1 {
            // Reset color before newline to avoid background bleeding
            if current_color.is_some() {
                output.push_str(ansi::RESET);
                current_color = None;
            }
            output.push('\n');
        }
    }

    // Final reset
    if current_color.is_some() {
        output.push_str(ansi::RESET);
    }

    output
}

/// Find the color at a specific position using "innermost wins"
fn find_color_at(spans: &[SyntaxSpan], line: u32, col: u32) -> Option<&'static str> {
    // Find all spans containing this position
    let mut best_span: Option<&SyntaxSpan> = None;

    for span in spans {
        if span.contains(line, col) {
            // Take the deepest one (highest depth)
            match best_span {
                None => best_span = Some(span),
                Some(current) if span.depth > current.depth => best_span = Some(span),
                _ => {}
            }
        }
    }

    best_span.and_then(|s| s.category.to_ansi())
}

/// Highlight source lines (full lines, not column-precise)
pub fn highlight_lines(
    source_lines: &[String],
    spans: &[SyntaxSpan],
    start_line: u32,
    end_line: u32,
) -> String {
    if spans.is_empty() || source_lines.is_empty() {
        return source_lines.join("\n");
    }

    let mut output = String::new();
    let mut current_color: Option<&'static str> = None;

    for line_idx in 0..source_lines.len() {
        let line_num = start_line + line_idx as u32;
        if line_num > end_line {
            break;
        }

        let line_content = &source_lines[line_idx];

        // Process each character
        for (char_offset, ch) in line_content.char_indices() {
            let col = (char_offset + 1) as u32; // 1-based

            let new_color = find_color_at(spans, line_num, col);

            if new_color != current_color {
                if current_color.is_some() {
                    output.push_str(ansi::RESET);
                }
                if let Some(color) = new_color {
                    output.push_str(color);
                }
                current_color = new_color;
            }

            output.push(ch);
        }

        // Reset at end of line
        if current_color.is_some() {
            output.push_str(ansi::RESET);
            current_color = None;
        }

        output.push('\n');
    }

    // Remove trailing newline to match original behavior
    if output.ends_with('\n') {
        output.pop();
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_from_element_name() {
        assert_eq!(SyntaxCategory::from_element_name("if"), SyntaxCategory::Keyword);
        assert_eq!(SyntaxCategory::from_element_name("class"), SyntaxCategory::Keyword);
        assert_eq!(SyntaxCategory::from_element_name("public"), SyntaxCategory::Keyword);
        assert_eq!(SyntaxCategory::from_element_name("predefined_type"), SyntaxCategory::Type);
        assert_eq!(SyntaxCategory::from_element_name("array_type"), SyntaxCategory::Type);
        assert_eq!(SyntaxCategory::from_element_name("name"), SyntaxCategory::Identifier);
        assert_eq!(SyntaxCategory::from_element_name("string_literal"), SyntaxCategory::String);
        assert_eq!(SyntaxCategory::from_element_name("integer_literal"), SyntaxCategory::Number);
        assert_eq!(SyntaxCategory::from_element_name("comment"), SyntaxCategory::Comment);
        assert_eq!(SyntaxCategory::from_element_name("binary_expression"), SyntaxCategory::Default);
    }

    #[test]
    fn test_span_contains() {
        let span = SyntaxSpan {
            start_line: 5,
            start_col: 10,
            end_line: 5,
            end_col: 20,
            category: SyntaxCategory::Keyword,
            depth: 0,
        };

        assert!(span.contains(5, 10));
        assert!(span.contains(5, 15));
        assert!(span.contains(5, 19));
        assert!(!span.contains(5, 9));
        assert!(!span.contains(5, 20));
        assert!(!span.contains(4, 15));
        assert!(!span.contains(6, 15));
    }

    #[test]
    fn test_multiline_span_contains() {
        let span = SyntaxSpan {
            start_line: 5,
            start_col: 10,
            end_line: 7,
            end_col: 5,
            category: SyntaxCategory::String,
            depth: 0,
        };

        assert!(span.contains(5, 10));
        assert!(span.contains(5, 50));
        assert!(span.contains(6, 1));
        assert!(span.contains(6, 100));
        assert!(span.contains(7, 1));
        assert!(span.contains(7, 4));
        assert!(!span.contains(5, 9));
        assert!(!span.contains(7, 5));
        assert!(!span.contains(8, 1));
    }

    #[test]
    fn test_extract_spans_simple() {
        let xml = r#"<class start="1:1" end="1:10"><name start="1:7" end="1:10">Foo</name></class>"#;
        let spans = extract_syntax_spans(xml);

        assert_eq!(spans.len(), 2);
        // Should have class (keyword) and name (identifier)
        let categories: Vec<_> = spans.iter().map(|s| s.category).collect();
        assert!(categories.contains(&SyntaxCategory::Keyword));
        assert!(categories.contains(&SyntaxCategory::Identifier));
    }

    #[test]
    fn test_category_colors() {
        assert_eq!(SyntaxCategory::Keyword.to_ansi(), Some(ansi::BLUE));
        assert_eq!(SyntaxCategory::Type.to_ansi(), Some(ansi::CYAN));
        assert_eq!(SyntaxCategory::String.to_ansi(), Some(ansi::YELLOW));
        assert_eq!(SyntaxCategory::Comment.to_ansi(), Some(ansi::GREEN));
        assert_eq!(SyntaxCategory::Default.to_ansi(), None);
    }
}
