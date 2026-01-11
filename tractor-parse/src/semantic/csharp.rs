use super::config::LanguageConfig;

/// C# language configuration for semantic tree transformation
pub static CSHARP_CONFIG: LanguageConfig = LanguageConfig {
    element_mappings: &[
        // Declarations
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
        ("event_declaration", "event"),
        ("delegate_declaration", "delegate"),
        ("namespace_declaration", "namespace"),

        // Parameters and arguments
        ("parameter_list", "params"),
        ("parameter", "param"),
        ("argument_list", "args"),
        ("argument", "arg"),
        ("type_parameter_list", "typeparams"),
        ("type_parameter", "typeparam"),
        ("type_argument_list", "typeargs"),

        // Types
        ("generic_name", "generic"),
        ("predefined_type", "type"),
        ("nullable_type", "nullable"),
        ("array_type", "array"),

        // Statements
        ("block", "block"),
        ("return_statement", "return"),
        ("if_statement", "if"),
        ("else_clause", "else"),
        ("for_statement", "for"),
        ("foreach_statement", "foreach"),
        ("while_statement", "while"),
        ("do_statement", "do"),
        ("switch_statement", "switch"),
        ("switch_section", "case"),
        ("try_statement", "try"),
        ("catch_clause", "catch"),
        ("finally_clause", "finally"),
        ("throw_statement", "throw"),
        ("using_statement", "using"),
        ("lock_statement", "lock"),

        // Expressions
        ("invocation_expression", "call"),
        ("member_access_expression", "member"),
        ("object_creation_expression", "new"),
        ("assignment_expression", "assign"),
        ("binary_expression", "binary"),
        ("unary_expression", "unary"),
        ("conditional_expression", "ternary"),
        ("lambda_expression", "lambda"),
        ("await_expression", "await"),

        // Variables
        ("variable_declaration", "var"),
        ("variable_declarator", "decl"),
        ("local_declaration_statement", "local"),

        // Literals
        ("string_literal", "string"),
        ("integer_literal", "int"),
        ("real_literal", "float"),
        ("boolean_literal", "bool"),
        ("null_literal", "null"),

        // Attributes
        ("attribute_list", "attrs"),
        ("attribute", "attr"),
        ("attribute_argument_list", "args"),
        ("attribute_argument", "arg"),

        // Property accessors
        ("accessor_list", "accessors"),
        ("accessor_declaration", "accessor"),

        // Other
        ("comment", "comment"),
        ("using_directive", "import"),

        // Identifier -> name (for primary identifiers)
        ("identifier", "name"),
    ],

    modifier_kinds: &["modifier"],

    known_modifiers: &[
        // Access modifiers
        "public", "private", "protected", "internal",
        // Other modifiers
        "static", "async", "abstract", "virtual", "override",
        "sealed", "readonly", "const", "partial", "extern",
        "volatile", "unsafe", "new", "ref", "out", "in",
        // Extension method marker
        "this",
    ],

    flatten_kinds: &[
        "declaration_list",  // Class body wrapper
    ],

    skip_kinds: &[],
};
