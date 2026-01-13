//! Java language configuration

use crate::parser::transform::{LangTransforms, IdentifierKind, default_compute_context};

/// Java identifier classification
fn classify_identifier(parent_kind: &str, has_param_sibling: bool, _in_special_context: bool) -> IdentifierKind {
    match parent_kind {
        "method_declaration" | "constructor_declaration" if has_param_sibling => IdentifierKind::Name,
        "class_declaration" | "interface_declaration" | "enum_declaration" => IdentifierKind::Name,
        "variable_declarator" => IdentifierKind::Name,
        "formal_parameter" => IdentifierKind::Name,
        _ => IdentifierKind::Type,
    }
}

/// Java transform configuration
pub static JAVA_TRANSFORMS: LangTransforms = LangTransforms {
    element_mappings: &[
        ("program", "program"),
        ("class_declaration", "class"),
        ("interface_declaration", "interface"),
        ("enum_declaration", "enum"),
        ("method_declaration", "method"),
        ("constructor_declaration", "ctor"),
        ("field_declaration", "field"),
        ("formal_parameters", "params"),
        ("formal_parameter", "param"),
        ("argument_list", "args"),
        ("type_identifier", "type"),
        ("generic_type", "generic"),
        ("array_type", "array"),
        ("return_statement", "return"),
        ("if_statement", "if"),
        ("else_clause", "else"),
        ("for_statement", "for"),
        ("enhanced_for_statement", "foreach"),
        ("while_statement", "while"),
        ("try_statement", "try"),
        ("catch_clause", "catch"),
        ("finally_clause", "finally"),
        ("throw_statement", "throw"),
        ("switch_expression", "switch"),
        ("switch_block_statement_group", "case"),
        ("method_invocation", "call"),
        ("object_creation_expression", "new"),
        ("field_access", "member"),
        ("array_access", "index"),
        ("assignment_expression", "assign"),
        ("binary_expression", "binary"),
        ("unary_expression", "unary"),
        ("ternary_expression", "ternary"),
        ("lambda_expression", "lambda"),
        ("string_literal", "string"),
        ("decimal_integer_literal", "int"),
        ("decimal_floating_point_literal", "float"),
        ("true", "true"),
        ("false", "false"),
        ("null_literal", "null"),
        ("import_declaration", "import"),
        ("package_declaration", "package"),
        ("identifier", "name"),
    ],
    flatten_kinds: &["class_body", "interface_body", "block"],
    skip_kinds: &["expression_statement"],
    operator_kinds: &["binary_expression", "unary_expression", "assignment_expression"],
    keyword_modifier_kinds: &[],
    known_modifiers: &[
        "public", "private", "protected",
        "static", "final", "abstract", "synchronized",
        "volatile", "transient", "native", "strictfp",
    ],
    modifier_wrapper_kinds: &["modifiers"],
    extract_name_attr_kinds: &[],
    classify_identifier,
    compute_identifier_context: default_compute_context,
};
