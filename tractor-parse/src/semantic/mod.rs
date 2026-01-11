pub mod config;
pub mod csharp;

use std::io::Write;
use config::{LanguageConfig, DEFAULT_CONFIG};
use crate::output::{write_empty_element, write_element_open_compact_with_attr, write_element_compact_with_text, write_close_tag};

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

/// Get the language configuration for a given language
pub fn get_config(lang: &str) -> &'static LanguageConfig {
    match lang {
        "csharp" | "cs" => &csharp::CSHARP_CONFIG,
        _ => &DEFAULT_CONFIG,
    }
}

/// Write TreeSitter node to XML in semantic format
pub fn write_semantic_node(
    out: &mut impl Write,
    node: tree_sitter::Node,
    source: &str,
    indent: usize,
    use_color: bool,
    config: &LanguageConfig,
) -> Result<(), Box<dyn std::error::Error>> {
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
) -> Result<(), Box<dyn std::error::Error>> {
    // Skip anonymous nodes (punctuation, etc.)
    if !node.is_named() {
        return Ok(());
    }

    let kind = node.kind();

    // Rule: Skip certain node kinds entirely
    if config.should_skip(kind) {
        return Ok(());
    }

    // Rule: Flatten certain wrapper nodes (process children at current level)
    if config.should_flatten(kind) {
        let mut cursor = node.walk();
        cursor.goto_first_child();
        loop {
            let child = cursor.node();
            if child.is_named() {
                let child_field = cursor.field_name();
                write_semantic_node_with_field(out, child, source, indent, use_color, config, child_field)?;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        return Ok(());
    }

    // Rule: Modifiers become empty elements
    if config.is_modifier_kind(kind) {
        let text = node.utf8_text(source.as_bytes()).unwrap_or("");
        if config.is_known_modifier(text) {
            write_empty_element(out, text, indent, use_color)?;
            return Ok(());
        }
        // Unknown modifier - fall through to normal processing
    }

    // Determine element name based on kind and context
    let element_name = if kind == "identifier" {
        // Detect if this identifier is a "name" or a "type reference" based on context
        let parent_kind = node.parent().map(|p| p.kind()).unwrap_or("");
        let next_sibling_kind = node.next_named_sibling().map(|s| s.kind());

        let is_name = match parent_kind {
            // In declarations, identifier followed by parameter_list is the method/function name
            "method_declaration" | "constructor_declaration" | "function_item" | "function_definition" => {
                next_sibling_kind == Some("parameter_list") || next_sibling_kind == Some("parameters")
            }
            // In type declarations, the identifier IS the name (class Foo, struct Bar, etc.)
            "class_declaration" | "struct_declaration" | "interface_declaration" |
            "enum_declaration" | "record_declaration" | "namespace_declaration" => true,
            // In property/field, check if next sibling is accessors/equals (then this is the name)
            "property_declaration" => {
                next_sibling_kind == Some("accessor_list") || next_sibling_kind == Some("accessors") ||
                next_sibling_kind == Some("equals_value_clause")
            }
            "variable_declarator" => true,
            // In parameter, the identifier is the parameter name
            "parameter" => true,
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

    // Format location as compact: start="line:col" end="line:col"
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

        // Process children with field information
        let mut cursor = node.walk();
        cursor.goto_first_child();
        loop {
            let child = cursor.node();
            if child.is_named() {
                let child_field = cursor.field_name();
                write_semantic_node_with_field(out, child, source, indent + 1, use_color, config, child_field)?;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }

        write_close_tag(out, element_name, indent, use_color)?;
    }

    Ok(())
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

    /// Check that output contains a <name> element with given text
    fn has_name(output: &str, text: &str) -> bool {
        output.contains(&format!(">{}</name>", text))
    }

    /// Check that output contains a <type> element with given text
    fn has_type(output: &str, text: &str) -> bool {
        output.contains(&format!(">{}</type>", text))
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
    fn struct_name() {
        let source = "public struct MyStruct { }";
        let output = parse_csharp(source);

        assert!(has_name(&output, "MyStruct"), "Struct name should be <name>");
    }

    #[test]
    fn interface_name() {
        let source = "public interface IMyInterface { }";
        let output = parse_csharp(source);

        assert!(has_name(&output, "IMyInterface"), "Interface name should be <name>");
    }

    #[test]
    fn enum_name() {
        let source = "public enum MyEnum { A, B, C }";
        let output = parse_csharp(source);

        assert!(has_name(&output, "MyEnum"), "Enum name should be <name>");
    }

    #[test]
    fn namespace_name() {
        let source = "namespace MyApp.Services { }";
        let output = parse_csharp(source);

        // Namespace should have name attribute
        assert!(output.contains("name=\"MyApp.Services\""), "Namespace should have name attribute");

        // Namespace parts should be split into <name> elements
        assert!(has_name(&output, "MyApp"), "MyApp should be <name>");
        assert!(has_name(&output, "Services"), "Services should be <name>");
    }

    #[test]
    fn namespace_name_deep() {
        let source = "namespace MyApp.Services.Internal { }";
        let output = parse_csharp(source);

        // Namespace should have name attribute
        assert!(output.contains("name=\"MyApp.Services.Internal\""), "Namespace should have name attribute");

        // All parts should be <name> elements
        assert!(has_name(&output, "MyApp"), "MyApp should be <name>");
        assert!(has_name(&output, "Services"), "Services should be <name>");
        assert!(has_name(&output, "Internal"), "Internal should be <name>");
    }

    #[test]
    fn property_name_vs_type() {
        let source = r#"
public class Foo {
    public string Name { get; set; }
    public int Count { get; }
}
"#;
        let output = parse_csharp(source);

        // Property names should be <name>
        assert!(has_name(&output, "Name"), "Property Name should be <name>");
        assert!(has_name(&output, "Count"), "Property Count should be <name>");

        // Property types should be <type>
        assert!(has_type(&output, "string"), "string should be <type>");
        assert!(has_type(&output, "int"), "int should be <type>");
    }

    #[test]
    fn parameter_name_vs_type() {
        let source = r#"
public class Foo {
    public void Process(string input, int count) { }
}
"#;
        let output = parse_csharp(source);

        // Parameter names should be <name>
        assert!(has_name(&output, "input"), "Parameter input should be <name>");
        assert!(has_name(&output, "count"), "Parameter count should be <name>");

        // Parameter types should be <type>
        assert!(has_type(&output, "string"), "string should be <type>");
        assert!(has_type(&output, "int"), "int should be <type>");
    }

    #[test]
    fn constructor_name() {
        let source = r#"
public class MyClass {
    public MyClass() { }
    public MyClass(int value) { }
}
"#;
        let output = parse_csharp(source);

        // Constructor names should be <name>
        // Note: constructor_declaration identifier is the class name
        assert!(has_name(&output, "MyClass"), "Constructor should have <name>MyClass</name>");
    }

    #[test]
    fn generic_type_reference() {
        let source = r#"
public class Foo {
    public List<string> Items { get; set; }
    public Dictionary<string, int> Map { get; set; }
}
"#;
        let output = parse_csharp(source);

        // Generic type names should be <type>
        assert!(has_type(&output, "List"), "List should be <type>");
        assert!(has_type(&output, "Dictionary"), "Dictionary should be <type>");
    }

    #[test]
    fn extension_method_this_parameter() {
        let source = r#"
public static class Extensions {
    public static string Reverse(this string input) { return ""; }
}
"#;
        let output = parse_csharp(source);

        // The method name should be <name>
        assert!(has_name(&output, "Reverse"), "Extension method name should be <name>");

        // The parameter name should be <name>
        assert!(has_name(&output, "input"), "Extension parameter name should be <name>");

        // The 'this' modifier should be present
        assert!(output.contains("<this"), "'this' modifier should be present");
    }

    #[test]
    fn variable_declarator_name() {
        let source = r#"
public class Foo {
    private readonly string _name;
    public int Counter = 0;
}
"#;
        let output = parse_csharp(source);

        // Field names should be <name>
        assert!(has_name(&output, "_name"), "Field _name should be <name>");
        assert!(has_name(&output, "Counter"), "Field Counter should be <name>");
    }

    #[test]
    fn async_method_with_task_return() {
        let source = r#"
public class Service {
    public async Task<string> FetchDataAsync() { return ""; }
    public async Task ProcessAsync() { }
}
"#;
        let output = parse_csharp(source);

        // Method names should be <name>
        assert!(has_name(&output, "FetchDataAsync"), "FetchDataAsync should be <name>");
        assert!(has_name(&output, "ProcessAsync"), "ProcessAsync should be <name>");

        // Task should be <type>
        assert!(has_type(&output, "Task"), "Task should be <type>");
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
}
