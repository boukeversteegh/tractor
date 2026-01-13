//! Semantic tree transformation for TreeSitter output

use std::io::Write;
use super::config::{LanguageConfig, DEFAULT_CONFIG};
use super::csharp::CSHARP_CONFIG;
use super::typescript::TYPESCRIPT_CONFIG;

/// Get the language configuration for a given language
pub fn get_config(lang: &str) -> &'static LanguageConfig {
    match lang {
        "csharp" | "cs" => &CSHARP_CONFIG,
        "typescript" | "ts" | "tsx" | "javascript" | "js" | "jsx" => &TYPESCRIPT_CONFIG,
        _ => &DEFAULT_CONFIG,
    }
}

/// Extract the first anonymous child's text (e.g., keyword like "let", "const", "var")
fn get_first_anonymous_child_text<'a>(node: tree_sitter::Node<'a>, source: &'a str) -> Option<&'a str> {
    if let Some(first_child) = node.child(0) {
        if !first_child.is_named() {
            return first_child.utf8_text(source.as_bytes()).ok();
        }
    }
    None
}

/// Check if a node is inside a namespace declaration (walk up ancestors)
fn is_in_namespace_declaration(node: tree_sitter::Node) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        match parent.kind() {
            "namespace_declaration" => return true,
            // Stop searching if we hit a type declaration - we're not in a namespace name
            "class_declaration" | "struct_declaration" | "interface_declaration" |
            "enum_declaration" | "record_declaration" => return false,
            _ => current = parent.parent(),
        }
    }
    false
}

/// Extract the full namespace name from a namespace_declaration node
fn get_namespace_full_name<'a>(node: tree_sitter::Node<'a>, source: &'a str) -> Option<&'a str> {
    // Find the qualified_name or identifier child that has the "name" field
    let mut cursor = node.walk();
    cursor.goto_first_child();
    loop {
        if cursor.field_name() == Some("name") {
            let name_node = cursor.node();
            return name_node.utf8_text(source.as_bytes()).ok();
        }
        if !cursor.goto_next_sibling() {
            break;
        }
    }
    None
}

/// Write TreeSitter node to XML in semantic format
pub fn write_semantic_node(
    out: &mut impl Write,
    node: tree_sitter::Node,
    source: &str,
    indent: usize,
    use_color: bool,
    config: &LanguageConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    write_semantic_node_with_field(out, node, source, indent, use_color, config, None)
}

/// Write TreeSitter node to XML, with optional field name context from parent
fn write_semantic_node_with_field(
    out: &mut impl Write,
    node: tree_sitter::Node,
    source: &str,
    indent: usize,
    use_color: bool,
    config: &LanguageConfig,
    _field_name: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Skip anonymous nodes (punctuation, etc.)
    if !node.is_named() {
        return Ok(());
    }

    let kind = node.kind();

    // Rule: Skip certain node kinds entirely
    if config.should_skip(kind) {
        return Ok(());
    }

    // Flatten Declaration Lists: promote children to parent level
    if config.should_flatten(kind) {
        let mut cursor = node.walk();
        cursor.goto_first_child();
        loop {
            let child = cursor.node();
            if child.is_named() {
                let child_field = cursor.field_name();
                // Apply field wrapping even when flattening
                if let Some(field) = child_field {
                    if should_wrap_field(field) {
                        write_field_wrapper_open(out, field, indent, use_color)?;
                        write_semantic_node_with_field(out, child, source, indent + 1, use_color, config, Some(field))?;
                        write_field_wrapper_close(out, field, indent, use_color)?;
                    } else {
                        write_semantic_node_with_field(out, child, source, indent, use_color, config, child_field)?;
                    }
                } else {
                    write_semantic_node_with_field(out, child, source, indent, use_color, config, child_field)?;
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        return Ok(());
    }

    // Lift Modifiers: modifiers become empty elements
    if config.is_modifier_kind(kind) {
        let text = node.utf8_text(source.as_bytes()).unwrap_or("");
        if config.is_known_modifier(text) {
            write_empty_element(out, text, indent, use_color)?;
            return Ok(());
        }
        // Unknown modifier - fall through to normal processing
    }

    // For identifiers wrapped in a field element, output just the text
    // UNLESS it's a type context (like "returns") where we need to emit <type>
    if matches!(kind, "identifier" | "type_identifier" | "property_identifier") {
        if let Some(field) = _field_name {
            if should_wrap_field(field) {
                let text = node.utf8_text(source.as_bytes()).unwrap_or("");
                let indent_str = "  ".repeat(indent);

                // In type contexts, still emit the <type> element
                if is_type_context_field(field) {
                    writeln!(out, "{}<type>{}</type>", indent_str, escape_xml(text))?;
                } else {
                    // Name/value contexts - just output text directly
                    writeln!(out, "{}{}", indent_str, escape_xml(text))?;
                }
                return Ok(());
            }
        }
    }

    // Rename Identifier to Name/Type based on context
    let element_name = if kind == "identifier" || kind == "type_identifier" || kind == "property_identifier" {
        // Detect if this identifier is a "name" or a "type reference" based on context
        let parent_kind = node.parent().map(|p| p.kind()).unwrap_or("");
        let next_sibling_kind = node.next_named_sibling().map(|s| s.kind());

        let is_name = match parent_kind {
            // In declarations, identifier followed by parameter_list is the method/function name
            "method_declaration" | "constructor_declaration" | "function_item" | "function_definition" |
            "method_definition" | "function_declaration" => {
                next_sibling_kind == Some("parameter_list") || next_sibling_kind == Some("parameters") ||
                next_sibling_kind == Some("formal_parameters")
            }
            // In type declarations, the identifier IS the name (class Foo, struct Bar, etc.)
            "class_declaration" | "struct_declaration" | "interface_declaration" |
            "enum_declaration" | "record_declaration" | "namespace_declaration" |
            "type_alias_declaration" => true,
            // In property/field, check if next sibling is accessors/equals (then this is the name)
            "property_declaration" => {
                next_sibling_kind == Some("accessor_list") || next_sibling_kind == Some("accessors") ||
                next_sibling_kind == Some("equals_value_clause")
            }
            "variable_declarator" => true,
            // In parameter, the identifier is the parameter name
            "parameter" | "required_parameter" | "optional_parameter" => true,
            // Inside generic_name, the identifier is the type name (like List in List<T>) - treat as type
            "generic_name" => false,
            // Inside qualified_name, check if it's part of a namespace declaration
            "qualified_name" => is_in_namespace_declaration(node),
            // Default: treat as type reference
            _ => false,
        };

        if is_name { "name" } else { "type" }
    } else {
        config.map_element_name(kind)
    };

    // Compact Location: start="line:col" end="line:col"
    let start = node.start_position();
    let end = node.end_position();
    let start_attr = format!("{}:{}", start.row + 1, start.column + 1);
    let end_attr = format!("{}:{}", end.row + 1, end.column + 1);

    let named_child_count = node.named_child_count();

    if named_child_count == 0 {
        // Leaf node - include text content
        let text = node.utf8_text(source.as_bytes()).unwrap_or("");
        write_element_compact_with_text(out, element_name, &start_attr, &end_attr, Some(text), indent, use_color)?;
    } else {
        // Node with children - check for namespace declaration to add name attribute
        let extra_attr = if kind == "namespace_declaration" {
            get_namespace_full_name(node, source).map(|name| ("name", name))
        } else {
            None
        };
        write_element_open_compact_with_attr(out, element_name, &start_attr, &end_attr, extra_attr, indent, use_color)?;

        // For variable declarations, emit the keyword (let/const/var) as a modifier
        if matches!(kind, "lexical_declaration" | "variable_declaration") {
            if let Some(keyword) = get_first_anonymous_child_text(node, source) {
                if config.is_known_modifier(keyword) {
                    write_empty_element(out, keyword, indent + 1, use_color)?;
                }
            }
        }

        // Process children with field information
        let mut cursor = node.walk();
        cursor.goto_first_child();
        loop {
            let child = cursor.node();
            if child.is_named() {
                let child_field = cursor.field_name();

                // Wrap child in field element if it's a meaningful field
                if let Some(field) = child_field {
                    if should_wrap_field(field) {
                        write_field_wrapper_open(out, field, indent + 1, use_color)?;
                        write_semantic_node_with_field(out, child, source, indent + 2, use_color, config, Some(field))?;
                        write_field_wrapper_close(out, field, indent + 1, use_color)?;
                    } else {
                        write_semantic_node_with_field(out, child, source, indent + 1, use_color, config, child_field)?;
                    }
                } else {
                    write_semantic_node_with_field(out, child, source, indent + 1, use_color, config, child_field)?;
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }

        write_close_tag(out, element_name, indent, use_color)?;
    }

    Ok(())
}

// Helper functions for XML output

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}

/// Write an empty element like <public/> for modifiers
fn write_empty_element(out: &mut impl Write, name: &str, indent: usize, _use_color: bool) -> std::io::Result<()> {
    let indent_str = "  ".repeat(indent);
    writeln!(out, "{}<{}/>", indent_str, name)
}

/// Write element with compact location and an extra attribute (e.g., fullName)
fn write_element_open_compact_with_attr(
    out: &mut impl Write,
    name: &str,
    start: &str,
    end: &str,
    extra_attr: Option<(&str, &str)>,
    indent: usize,
    _use_color: bool
) -> std::io::Result<()> {
    let indent_str = "  ".repeat(indent);
    write!(out, "{}<{} start=\"{}\" end=\"{}\"", indent_str, escape_xml(name), start, end)?;
    if let Some((attr_name, attr_value)) = extra_attr {
        write!(out, " {}=\"{}\"", attr_name, escape_xml(attr_value))?;
    }
    writeln!(out, ">")
}

/// Write leaf element with compact location and text content
fn write_element_compact_with_text(out: &mut impl Write, name: &str, start: &str, end: &str, text: Option<&str>, indent: usize, _use_color: bool) -> std::io::Result<()> {
    let indent_str = "  ".repeat(indent);
    let escaped_name = escape_xml(name);
    write!(out, "{}<{} start=\"{}\" end=\"{}\">", indent_str, escaped_name, start, end)?;
    if let Some(t) = text {
        write!(out, "{}", escape_xml(t))?;
    }
    writeln!(out, "</{}>", escaped_name)
}

/// Write closing tag with proper indentation
fn write_close_tag(out: &mut impl Write, name: &str, indent: usize, _use_color: bool) -> std::io::Result<()> {
    let indent_str = "  ".repeat(indent);
    writeln!(out, "{}</{}>", indent_str, name)
}

/// Fields that should be wrapped in a semantic element
const WRAPPED_FIELDS: &[&str] = &[
    "name",        // variable/function/class name
    "value",       // assigned/initial value
    "left",        // binary expression left operand
    "right",       // binary expression right operand
    "body",        // function/class/loop body
    "parameters",  // function parameters
    "condition",   // if/while/for condition
    "consequence", // if true branch
    "alternative", // if else branch
    "returns",     // return type
    "arguments",   // call arguments
];

/// Check if a field should be wrapped in a semantic element
fn should_wrap_field(field: &str) -> bool {
    WRAPPED_FIELDS.contains(&field)
}

/// Fields that represent type contexts (identifiers in these should become <type> elements)
const TYPE_CONTEXT_FIELDS: &[&str] = &[
    "returns",     // return type
    "type",        // type annotation
];

/// Check if a field is a type context (identifiers should emit as <type>)
fn is_type_context_field(field: &str) -> bool {
    TYPE_CONTEXT_FIELDS.contains(&field)
}

/// Write opening tag for field wrapper
fn write_field_wrapper_open(out: &mut impl Write, field: &str, indent: usize, _use_color: bool) -> std::io::Result<()> {
    let indent_str = "  ".repeat(indent);
    writeln!(out, "{}<{}>", indent_str, field)
}

/// Write closing tag for field wrapper
fn write_field_wrapper_close(out: &mut impl Write, field: &str, indent: usize, _use_color: bool) -> std::io::Result<()> {
    let indent_str = "  ".repeat(indent);
    writeln!(out, "{}</{}>", indent_str, field)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse C# source and return semantic XML output
    fn parse_csharp(source: &str) -> String {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let mut output = Vec::new();
        let config = get_config("csharp");
        write_semantic_node(&mut output, tree.root_node(), source, 0, false, config).unwrap();

        String::from_utf8(output).unwrap()
    }

    use crate::XPathEngine;

    /// Query XML with XPath and return the count of matches
    fn xpath_count(xml: &str, xpath: &str) -> usize {
        let engine = XPathEngine::new();
        let full_xml = format!("<root>{}</root>", xml);
        match engine.query(&full_xml, xpath, &[], "") {
            Ok(matches) => matches.len(),
            Err(_) => 0,
        }
    }

    /// Check if XPath query matches at least once
    fn xpath_exists(xml: &str, xpath: &str) -> bool {
        xpath_count(xml, xpath) > 0
    }

    /// Check if a <name> element contains the given text
    fn has_name(xml: &str, name: &str) -> bool {
        // Match <name> elements that contain the text directly or as a child element
        let xpath = format!("//name[contains(., '{}')]", name);
        xpath_exists(xml, &xpath)
    }

    /// Check if a <type> element contains the given text
    fn has_type(xml: &str, type_name: &str) -> bool {
        let xpath = format!("//type[contains(., '{}')]", type_name);
        xpath_exists(xml, &xpath)
    }

    #[test]
    fn method_name_vs_return_type() {
        let source = r#"
public class Foo {
    public Task DoSomething() { }
    public void Simple() { }
    public string GetName() { return ""; }
}
"#;
        let output = parse_csharp(source);

        // Method names should be <name>
        assert!(has_name(&output, "DoSomething"), "DoSomething should be <name>");
        assert!(has_name(&output, "Simple"), "Simple should be <name>");
        assert!(has_name(&output, "GetName"), "GetName should be <name>");

        // Return types should be <type>
        assert!(has_type(&output, "Task"), "Task should be <type>");
        assert!(has_type(&output, "string"), "string should be <type>");

        // void is a predefined_type, mapped to 'type'
        assert!(output.contains(">void</type>"), "void should be <type>");
    }

    #[test]
    fn class_name() {
        let source = "public class MyClass { }";
        let output = parse_csharp(source);

        assert!(has_name(&output, "MyClass"), "Class name should be <name>");
    }

    #[test]
    fn modifiers_as_empty_elements() {
        let source = r#"
public static async class MyClass {
    private readonly int _value;
}
"#;
        let output = parse_csharp(source);

        // Modifiers should be empty elements
        assert!(output.contains("<public"), "public modifier should be present");
        assert!(output.contains("<static"), "static modifier should be present");
        assert!(output.contains("<async"), "async modifier should be present");
        assert!(output.contains("<private"), "private modifier should be present");
        assert!(output.contains("<readonly"), "readonly modifier should be present");
    }

    // TypeScript tests

    /// Parse TypeScript source and return semantic XML output
    fn parse_typescript(source: &str) -> String {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let mut output = Vec::new();
        let config = get_config("typescript");
        write_semantic_node(&mut output, tree.root_node(), source, 0, false, config).unwrap();

        String::from_utf8(output).unwrap()
    }

    #[test]
    fn typescript_variable_with_field_wrappers() {
        let source = "let x = 1;";
        let output = parse_typescript(source);

        // Should have <name> wrapper for variable name
        assert!(output.contains("<name>"), "should have <name> wrapper");
        // Should have <value> wrapper for initial value
        assert!(output.contains("<value>"), "should have <value> wrapper");
        // Should have let modifier
        assert!(output.contains("<let/>"), "should have <let/> modifier");
    }

    #[test]
    fn typescript_variable_with_expression() {
        let source = "let x = 1 + 2;";
        let output = parse_typescript(source);

        // Should have <value> wrapper containing the expression
        assert!(output.contains("<value>"), "should have <value> wrapper");
        // Binary expression should have <left> and <right>
        assert!(output.contains("<left>"), "should have <left> wrapper");
        assert!(output.contains("<right>"), "should have <right> wrapper");
    }

    #[test]
    fn typescript_const_variable() {
        let source = "const y = 42;";
        let output = parse_typescript(source);

        assert!(output.contains("<const/>"), "should have <const/> modifier");
        assert!(output.contains("<name>"), "should have <name> wrapper");
        assert!(output.contains("<value>"), "should have <value> wrapper");
    }
}
