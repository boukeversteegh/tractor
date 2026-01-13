//! C# language configuration

use crate::parser::transform::{LangTransforms, IdentifierKind};

/// C# identifier classification
fn classify_identifier(parent_kind: &str, has_param_sibling: bool, in_namespace_decl: bool) -> IdentifierKind {
    // In namespace path, it's a name
    if parent_kind == "qualified_name" && in_namespace_decl {
        return IdentifierKind::Name;
    }

    match parent_kind {
        // Method/function names are followed by parameter list
        "method_declaration" | "constructor_declaration"
            if has_param_sibling => IdentifierKind::Name,

        // Type declarations - the identifier IS the name
        "class_declaration" | "struct_declaration" | "interface_declaration" |
        "enum_declaration" | "record_declaration" | "namespace_declaration" => IdentifierKind::Name,

        // Variable declarator - the identifier is the name
        "variable_declarator" => IdentifierKind::Name,

        // Parameter - the identifier is the parameter name
        "parameter" => IdentifierKind::Name,

        // Generic name - the identifier is the generic type name
        "generic_name" => IdentifierKind::Type,

        // Default to type
        _ => IdentifierKind::Type,
    }
}

/// C# context computation - check if in namespace declaration path
fn compute_namespace_context(parent_chain: &[&str]) -> bool {
    for kind in parent_chain {
        match *kind {
            "namespace_declaration" => return true,
            // Stop if we hit a type declaration - not in namespace name
            "class_declaration" | "struct_declaration" | "interface_declaration" |
            "enum_declaration" | "record_declaration" => return false,
            _ => continue,
        }
    }
    false
}

/// C# transform configuration
pub static CSHARP_TRANSFORMS: LangTransforms = LangTransforms {
    element_mappings: &[
        ("compilation_unit", "unit"),
        ("class_declaration", "class"),
        ("struct_declaration", "struct"),
        ("interface_declaration", "interface"),
        ("enum_declaration", "enum"),
        ("record_declaration", "record"),
        ("method_declaration", "method"),
        ("constructor_declaration", "ctor"),
        ("property_declaration", "prop"),
        ("field_declaration", "field"),
        ("namespace_declaration", "namespace"),
        ("parameter_list", "params"),
        ("parameter", "param"),
        ("argument_list", "args"),
        ("argument", "arg"),
        ("generic_name", "generic"),
        ("predefined_type", "type"),
        ("nullable_type", "nullable"),
        ("array_type", "array"),
        ("block", "block"),
        ("return_statement", "return"),
        ("if_statement", "if"),
        ("else_clause", "else"),
        ("for_statement", "for"),
        ("foreach_statement", "foreach"),
        ("while_statement", "while"),
        ("try_statement", "try"),
        ("catch_clause", "catch"),
        ("throw_statement", "throw"),
        ("using_statement", "using"),
        ("invocation_expression", "call"),
        ("member_access_expression", "member"),
        ("object_creation_expression", "new"),
        ("assignment_expression", "assign"),
        ("binary_expression", "binary"),
        ("unary_expression", "unary"),
        ("conditional_expression", "ternary"),
        ("lambda_expression", "lambda"),
        ("await_expression", "await"),
        ("variable_declaration", "var"),
        ("variable_declarator", "decl"),
        ("local_declaration_statement", "local"),
        ("string_literal", "string"),
        ("integer_literal", "int"),
        ("real_literal", "float"),
        ("boolean_literal", "bool"),
        ("null_literal", "null"),
        ("attribute_list", "attrs"),
        ("attribute", "attr"),
        ("using_directive", "import"),
        ("identifier", "name"),
    ],
    flatten_kinds: &["declaration_list"],
    skip_kinds: &[],
    operator_kinds: &["binary_expression", "unary_expression", "assignment_expression"],
    keyword_modifier_kinds: &[],
    known_modifiers: &[
        "public", "private", "protected", "internal",
        "static", "async", "abstract", "virtual", "override",
        "sealed", "readonly", "const", "partial",
    ],
    modifier_wrapper_kinds: &["modifier"],
    extract_name_attr_kinds: &["namespace_declaration"],
    classify_identifier,
    compute_identifier_context: compute_namespace_context,
};
