//! TypeScript/JavaScript language configuration

use crate::parser::transform::{LangTransforms, IdentifierKind, default_compute_context};

/// TypeScript identifier classification
fn classify_identifier(parent_kind: &str, has_param_sibling: bool, _in_special_context: bool) -> IdentifierKind {
    match parent_kind {
        // Method/function names are followed by parameter list
        "method_definition" | "function_declaration" | "arrow_function"
            if has_param_sibling => IdentifierKind::Name,

        // Class/interface declarations - the identifier IS the name
        "class_declaration" | "interface_declaration" | "type_alias_declaration" |
        "enum_declaration" => IdentifierKind::Name,

        // Variable declarator - the identifier is the name
        "variable_declarator" => IdentifierKind::Name,

        // Parameter - the identifier is the parameter name
        "required_parameter" | "optional_parameter" => IdentifierKind::Name,

        // Property assignment - the key is a name
        "pair" => IdentifierKind::Name,

        // Default to type
        _ => IdentifierKind::Type,
    }
}

/// TypeScript/JavaScript transform configuration
pub static TYPESCRIPT_TRANSFORMS: LangTransforms = LangTransforms {
    element_mappings: &[
        // Declarations
        ("program", "program"),
        ("class_declaration", "class"),
        ("function_declaration", "function"),
        ("method_definition", "method"),
        ("arrow_function", "lambda"),
        ("interface_declaration", "interface"),
        ("type_alias_declaration", "typealias"),
        ("enum_declaration", "enum"),
        ("lexical_declaration", "variable"),
        ("variable_declaration", "variable"),
        ("variable_declarator", "decl"),
        // Identifiers
        ("property_identifier", "name"),
        ("type_identifier", "type"),
        ("identifier", "name"),
        // Parameters
        ("formal_parameters", "params"),
        ("required_parameter", "param"),
        ("optional_parameter", "param"),
        // Blocks
        ("statement_block", "block"),
        ("class_body", "body"),
        // Statements
        ("return_statement", "return"),
        ("if_statement", "if"),
        ("else_clause", "else"),
        ("for_statement", "for"),
        ("while_statement", "while"),
        ("try_statement", "try"),
        ("catch_clause", "catch"),
        ("throw_statement", "throw"),
        // Expressions
        ("call_expression", "call"),
        ("new_expression", "new"),
        ("member_expression", "member"),
        ("assignment_expression", "assign"),
        ("binary_expression", "binary"),
        ("unary_expression", "unary"),
        ("ternary_expression", "ternary"),
        ("await_expression", "await"),
        // Imports/Exports
        ("import_statement", "import"),
        ("export_statement", "export"),
        // Literals
        ("string", "string"),
        ("number", "number"),
        ("true", "true"),
        ("false", "false"),
        ("null", "null"),
        // Types
        ("type_annotation", "typeof"),
        ("type_parameters", "typeparams"),
        ("type_parameter", "typeparam"),
    ],
    flatten_kinds: &[
        "class_body",
        "variable_declarator",
    ],
    skip_kinds: &[
        "expression_statement",
    ],
    operator_kinds: &[
        "binary_expression",
        "unary_expression",
        "assignment_expression",
        "augmented_assignment_expression",
        "update_expression",
    ],
    keyword_modifier_kinds: &[
        "lexical_declaration",
        "variable_declaration",
    ],
    known_modifiers: &[
        "public", "private", "protected",
        "static", "async", "readonly", "abstract",
        "export", "default",
        "let", "const", "var",
    ],
    modifier_wrapper_kinds: &[],
    extract_name_attr_kinds: &[],
    classify_identifier,
    compute_identifier_context: default_compute_context,
};
