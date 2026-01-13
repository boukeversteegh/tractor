//! Go language configuration

use crate::parser::transform::{LangTransforms, IdentifierKind, default_compute_context};

/// Go identifier classification
fn classify_identifier(parent_kind: &str, _has_param_sibling: bool, _in_special_context: bool) -> IdentifierKind {
    match parent_kind {
        "function_declaration" | "method_declaration" => IdentifierKind::Name,
        "type_spec" => IdentifierKind::Name,
        "parameter_declaration" => IdentifierKind::Name,
        "var_spec" | "const_spec" => IdentifierKind::Name,
        _ => IdentifierKind::Type,
    }
}

/// Go transform configuration
pub static GO_TRANSFORMS: LangTransforms = LangTransforms {
    element_mappings: &[
        ("source_file", "file"),
        ("package_clause", "package"),
        ("function_declaration", "function"),
        ("method_declaration", "method"),
        ("type_declaration", "typedef"),
        ("type_spec", "typespec"),
        ("struct_type", "struct"),
        ("interface_type", "interface"),
        ("const_declaration", "const"),
        ("var_declaration", "var"),
        ("parameter_list", "params"),
        ("parameter_declaration", "param"),
        ("type_identifier", "type"),
        ("pointer_type", "pointer"),
        ("slice_type", "slice"),
        ("map_type", "map"),
        ("channel_type", "chan"),
        ("return_statement", "return"),
        ("if_statement", "if"),
        ("else_clause", "else"),
        ("for_statement", "for"),
        ("range_clause", "range"),
        ("switch_statement", "switch"),
        ("case_clause", "case"),
        ("default_case", "default"),
        ("defer_statement", "defer"),
        ("go_statement", "go"),
        ("select_statement", "select"),
        ("call_expression", "call"),
        ("selector_expression", "member"),
        ("index_expression", "index"),
        ("composite_literal", "literal"),
        ("binary_expression", "binary"),
        ("unary_expression", "unary"),
        ("interpreted_string_literal", "string"),
        ("raw_string_literal", "rawstring"),
        ("int_literal", "int"),
        ("float_literal", "float"),
        ("true", "true"),
        ("false", "false"),
        ("nil", "nil"),
        ("identifier", "name"),
        ("field_identifier", "field"),
        ("package_identifier", "pkg"),
    ],
    flatten_kinds: &["block"],
    skip_kinds: &["expression_statement"],
    operator_kinds: &["binary_expression", "unary_expression"],
    keyword_modifier_kinds: &[],
    known_modifiers: &[],
    modifier_wrapper_kinds: &[],
    extract_name_attr_kinds: &[],
    classify_identifier,
    compute_identifier_context: default_compute_context,
};
