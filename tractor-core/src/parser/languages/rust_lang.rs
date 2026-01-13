//! Rust language configuration

use crate::parser::transform::{LangTransforms, IdentifierKind, default_compute_context};

/// Rust identifier classification
fn classify_identifier(parent_kind: &str, has_param_sibling: bool, _in_special_context: bool) -> IdentifierKind {
    match parent_kind {
        "function_item" if has_param_sibling => IdentifierKind::Name,
        "struct_item" | "enum_item" | "trait_item" | "mod_item" | "type_item" => IdentifierKind::Name,
        "let_declaration" => IdentifierKind::Name,
        "parameter" => IdentifierKind::Name,
        _ => IdentifierKind::Type,
    }
}

/// Rust transform configuration
pub static RUST_TRANSFORMS: LangTransforms = LangTransforms {
    element_mappings: &[
        ("source_file", "file"),
        ("function_item", "function"),
        ("impl_item", "impl"),
        ("struct_item", "struct"),
        ("enum_item", "enum"),
        ("trait_item", "trait"),
        ("mod_item", "mod"),
        ("use_declaration", "use"),
        ("const_item", "const"),
        ("static_item", "static"),
        ("type_item", "typedef"),
        ("parameters", "params"),
        ("parameter", "param"),
        ("self_parameter", "self"),
        ("type_identifier", "type"),
        ("primitive_type", "type"),
        ("reference_type", "ref"),
        ("generic_type", "generic"),
        ("scoped_type_identifier", "path"),
        ("return_expression", "return"),
        ("if_expression", "if"),
        ("else_clause", "else"),
        ("for_expression", "for"),
        ("while_expression", "while"),
        ("loop_expression", "loop"),
        ("match_expression", "match"),
        ("match_arm", "arm"),
        ("call_expression", "call"),
        ("method_call_expression", "methodcall"),
        ("field_expression", "field"),
        ("index_expression", "index"),
        ("binary_expression", "binary"),
        ("unary_expression", "unary"),
        ("closure_expression", "closure"),
        ("await_expression", "await"),
        ("try_expression", "try"),
        ("let_declaration", "let"),
        ("macro_invocation", "macro"),
        ("string_literal", "string"),
        ("raw_string_literal", "rawstring"),
        ("integer_literal", "int"),
        ("float_literal", "float"),
        ("boolean_literal", "bool"),
        ("identifier", "name"),
    ],
    flatten_kinds: &["block", "declaration_list"],
    skip_kinds: &["expression_statement"],
    operator_kinds: &["binary_expression", "unary_expression"],
    keyword_modifier_kinds: &["let_declaration"],
    known_modifiers: &["pub", "mut", "async", "unsafe", "const", "static"],
    modifier_wrapper_kinds: &["visibility_modifier"],
    extract_name_attr_kinds: &[],
    classify_identifier,
    compute_identifier_context: default_compute_context,
};
