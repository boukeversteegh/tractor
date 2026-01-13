use super::config::LanguageConfig;

/// TypeScript/JavaScript language configuration for semantic tree transformation
pub static TYPESCRIPT_CONFIG: LanguageConfig = LanguageConfig {
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

        // Blocks and bodies
        ("statement_block", "block"),
        ("class_body", "body"),

        // Statements
        ("return_statement", "return"),
        ("if_statement", "if"),
        ("else_clause", "else"),
        ("for_statement", "for"),
        ("for_in_statement", "forin"),
        ("for_of_statement", "forof"),
        ("while_statement", "while"),
        ("do_statement", "do"),
        ("switch_statement", "switch"),
        ("switch_case", "case"),
        ("switch_default", "default"),
        ("try_statement", "try"),
        ("catch_clause", "catch"),
        ("finally_clause", "finally"),
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
        ("export_clause", "exports"),
        ("export_specifier", "exportspec"),
        ("import_clause", "imports"),
        ("import_specifier", "importspec"),

        // Literals
        ("string", "string"),
        ("template_string", "template"),
        ("number", "number"),
        ("true", "true"),
        ("false", "false"),
        ("null", "null"),
        ("undefined", "undefined"),

        // Types (TypeScript)
        ("type_annotation", "typeof"),
        ("type_parameters", "typeparams"),
        ("type_parameter", "typeparam"),
        ("type_arguments", "typeargs"),

        // Other
        ("comment", "comment"),
        ("object", "object"),
        ("array", "array"),
        ("pair", "pair"),
        ("spread_element", "spread"),
    ],

    // TypeScript doesn't use a wrapper "modifier" node - modifiers are direct children
    modifier_kinds: &[],

    known_modifiers: &[
        // Access modifiers
        "public", "private", "protected",
        // Other modifiers
        "static", "async", "readonly", "abstract", "override",
        // Export/import related
        "export", "default",
        // Variable keywords (from kind attribute)
        "let", "const", "var",
    ],

    flatten_kinds: &[
        "class_body",           // Flatten so methods are direct children of class
        "variable_declarator",  // Flatten so name/value are direct children of variable
    ],

    skip_kinds: &[
        "expression_statement",  // Unwrap expression statements
    ],
};
