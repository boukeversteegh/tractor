/// Semantic element names — tractor's PHP XML vocabulary after transform.
/// Tree-sitter kind strings (left side of `match` arms) stay as bare
/// literals — they are external vocabulary.
use crate::languages::{KindEntry, KindHandling, NodeSpec};
use crate::output::syntax_highlight::SyntaxCategory;

// Named constants retained for use by the transform code. The NODES
// table below is the source of truth for marker/container role and
// syntax category.

// Top-level / declarations
pub const PROGRAM: &str = "program";
pub const NAMESPACE: &str = "namespace";
pub const USE: &str = "use";
pub const CLASS: &str = "class";
pub const INTERFACE: &str = "interface";
pub const TRAIT: &str = "trait";
pub const ENUM: &str = "enum";
pub const METHOD: &str = "method";
pub const FUNCTION: &str = "function";
pub const FIELD: &str = "field";
pub const CONST: &str = "const";
pub const CONSTANT: &str = "constant";

// Members / parameters
pub const PARAMETER: &str = "parameter";
pub const ARGUMENT: &str = "argument";

// Inheritance
pub const EXTENDS: &str = "extends";
pub const IMPLEMENTS: &str = "implements";
pub const TYPES: &str = "types";

// Statements / control flow
pub const RETURN: &str = "return";
pub const IF: &str = "if";
pub const ELSE: &str = "else";
pub const ELSE_IF: &str = "else_if";
pub const FOR: &str = "for";
pub const FOREACH: &str = "foreach";
pub const WHILE: &str = "while";
pub const DO: &str = "do";
pub const SWITCH: &str = "switch";
pub const CASE: &str = "case";
pub const TRY: &str = "try";
pub const CATCH: &str = "catch";
pub const FINALLY: &str = "finally";
pub const THROW: &str = "throw";
pub const ECHO: &str = "echo";
pub const CONTINUE: &str = "continue";
pub const BREAK: &str = "break";
pub const MATCH: &str = "match";
pub const ARM: &str = "arm";
pub const YIELD: &str = "yield";
pub const REQUIRE: &str = "require";
pub const PRINT: &str = "print";
pub const EXIT: &str = "exit";
pub const DECLARE: &str = "declare";
pub const GOTO: &str = "goto";

// Expressions
pub const CALL: &str = "call";
pub const MEMBER: &str = "member";
pub const INDEX: &str = "index";
pub const NEW: &str = "new";
pub const CAST: &str = "cast";
pub const ASSIGN: &str = "assign";
pub const BINARY: &str = "binary";
pub const UNARY: &str = "unary";
pub const TERNARY: &str = "ternary";
pub const ARRAY: &str = "array";
pub const SPREAD: &str = "spread";

// Types / atoms
pub const TYPE: &str = "type";
pub const STRING: &str = "string";
pub const INT: &str = "int";
pub const FLOAT: &str = "float";
pub const BOOL: &str = "bool";
pub const NULL: &str = "null";
pub const VARIABLE: &str = "variable";

// Misc structural
pub const TAG: &str = "tag";
pub const INTERPOLATION: &str = "interpolation";
pub const ATTRIBUTE: &str = "attribute";

// Identifiers / comments / pair / op
pub const NAME: &str = "name";
pub const COMMENT: &str = "comment";
pub const PAIR: &str = "pair";
pub const OP: &str = "op";

// Comment markers — emitted by the shared CommentClassifier. PHP
// supports both `//` (and `#`) line comments and `/* */` block
// comments; the classifier carries that prefix list.
pub const TRAILING: &str = "trailing";
pub const LEADING: &str = "leading";

// Visibility / access modifiers.
pub const PUBLIC: &str = "public";
pub const PRIVATE: &str = "private";
pub const PROTECTED: &str = "protected";

// Other modifiers.
pub const FINAL: &str = "final";
pub const ABSTRACT: &str = "abstract";
pub const READONLY: &str = "readonly";

// Call / member flavor markers.
pub const INSTANCE: &str = "instance";

// Type-shape markers.
pub const PRIMITIVE: &str = "primitive";
pub const UNION: &str = "union";
pub const OPTIONAL: &str = "optional";

// Parameter-shape markers.
pub const VARIADIC: &str = "variadic";

// Anonymous / arrow function shape markers.
pub const ANONYMOUS: &str = "anonymous";
pub const ARROW: &str = "arrow";

// php_tag marker.
pub const OPEN: &str = "open";

// Dual-use names (see NODES below).
pub const STATIC: &str = "static";
pub const DEFAULT: &str = "default";

use SyntaxCategory::*;

/// Per-name metadata — single source of truth for every element
/// name this language's transform can emit.
///
/// Dual-use names set BOTH `marker: true` and `container: true`:
///   - STATIC   — `static_modifier` keyword marker + `scoped_call_expression`
///                shape marker. Also doubles with static property-access shape.
///   - CONSTANT — `enum_case` / `const_element` (container) +
///                `class_constant_access_expression` member-shape marker.
///   - DEFAULT  — `default_statement` (container) + `match_default_expression`
///                arm-shape marker (`<arm><default/>`).
///   - FUNCTION — function_definition (container) + anonymous/arrow
///                function shape markers.
pub const NODES: &[NodeSpec] = &[
    // Top-level / declarations (FUNCTION, CONSTANT dual-use)
    NodeSpec { name: PROGRAM,   marker: false, container: true, syntax: Default },
    NodeSpec { name: NAMESPACE, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: USE,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CLASS,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: INTERFACE, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: TRAIT,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ENUM,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: METHOD,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FUNCTION,  marker: true,  container: true, syntax: Keyword },
    NodeSpec { name: FIELD,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CONST,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CONSTANT,  marker: true,  container: true, syntax: Keyword },

    // Members / parameters
    NodeSpec { name: PARAMETER, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ARGUMENT,  marker: false, container: true, syntax: Default },

    // Inheritance
    NodeSpec { name: EXTENDS,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: IMPLEMENTS, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: TYPES,      marker: false, container: true, syntax: Default },

    // Statements / control flow
    NodeSpec { name: RETURN,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: IF,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ELSE,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ELSE_IF,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FOR,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FOREACH,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: WHILE,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: DO,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: SWITCH,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CASE,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: TRY,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CATCH,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FINALLY,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: THROW,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ECHO,     marker: false, container: true, syntax: Default },
    NodeSpec { name: CONTINUE, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: BREAK,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: MATCH,    marker: false, container: true, syntax: Default },
    NodeSpec { name: ARM,      marker: false, container: true, syntax: Default },
    NodeSpec { name: YIELD,    marker: false, container: true, syntax: Default },
    NodeSpec { name: REQUIRE,  marker: false, container: true, syntax: Default },
    NodeSpec { name: PRINT,    marker: false, container: true, syntax: Default },
    NodeSpec { name: EXIT,     marker: false, container: true, syntax: Default },
    NodeSpec { name: DECLARE,  marker: false, container: true, syntax: Default },
    NodeSpec { name: GOTO,     marker: false, container: true, syntax: Default },

    // Expressions
    NodeSpec { name: CALL,    marker: false, container: true, syntax: Function },
    NodeSpec { name: MEMBER,  marker: false, container: true, syntax: Default },
    NodeSpec { name: INDEX,   marker: false, container: true, syntax: Default },
    NodeSpec { name: NEW,     marker: false, container: true, syntax: Function },
    NodeSpec { name: CAST,    marker: false, container: true, syntax: Operator },
    NodeSpec { name: ASSIGN,  marker: false, container: true, syntax: Operator },
    NodeSpec { name: BINARY,  marker: false, container: true, syntax: Operator },
    NodeSpec { name: UNARY,   marker: false, container: true, syntax: Operator },
    NodeSpec { name: TERNARY, marker: false, container: true, syntax: Operator },
    NodeSpec { name: ARRAY,   marker: false, container: true, syntax: Default },
    NodeSpec { name: SPREAD,  marker: false, container: true, syntax: Default },

    // Types / atoms
    NodeSpec { name: TYPE,     marker: false, container: true, syntax: Type },
    NodeSpec { name: STRING,   marker: false, container: true, syntax: String },
    NodeSpec { name: INT,      marker: false, container: true, syntax: Number },
    NodeSpec { name: FLOAT,    marker: false, container: true, syntax: Number },
    NodeSpec { name: BOOL,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: NULL,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: VARIABLE, marker: false, container: true, syntax: Default },

    // Misc structural
    NodeSpec { name: TAG,           marker: false, container: true, syntax: Default },
    NodeSpec { name: INTERPOLATION, marker: false, container: true, syntax: Default },
    NodeSpec { name: ATTRIBUTE,     marker: false, container: true, syntax: Default },

    // Identifiers / comments / pair / op
    NodeSpec { name: NAME,    marker: false, container: true, syntax: Identifier },
    NodeSpec { name: COMMENT, marker: false, container: true, syntax: Comment },
    NodeSpec { name: PAIR,    marker: false, container: true, syntax: Default },
    NodeSpec { name: OP,      marker: false, container: true, syntax: Operator },

    // Comment markers
    NodeSpec { name: TRAILING, marker: true, container: false, syntax: Default },
    NodeSpec { name: LEADING,  marker: true, container: false, syntax: Default },

    // Access modifiers — markers only.
    NodeSpec { name: PUBLIC,    marker: true, container: false, syntax: Keyword },
    NodeSpec { name: PRIVATE,   marker: true, container: false, syntax: Keyword },
    NodeSpec { name: PROTECTED, marker: true, container: false, syntax: Keyword },

    // Other modifiers — markers only.
    NodeSpec { name: FINAL,    marker: true, container: false, syntax: Keyword },
    NodeSpec { name: ABSTRACT, marker: true, container: false, syntax: Keyword },
    NodeSpec { name: READONLY, marker: true, container: false, syntax: Keyword },

    // Call / member flavor markers.
    NodeSpec { name: INSTANCE, marker: true, container: false, syntax: Default },

    // Type-shape markers.
    NodeSpec { name: PRIMITIVE, marker: true, container: false, syntax: Default },
    NodeSpec { name: UNION,     marker: true, container: false, syntax: Default },
    NodeSpec { name: OPTIONAL,  marker: true, container: false, syntax: Default },

    // Parameter-shape markers.
    NodeSpec { name: VARIADIC, marker: true, container: false, syntax: Default },

    // Anonymous / arrow function shape markers.
    NodeSpec { name: ANONYMOUS, marker: true, container: false, syntax: Default },
    NodeSpec { name: ARROW,     marker: true, container: false, syntax: Default },

    // php_tag marker.
    NodeSpec { name: OPEN, marker: true, container: false, syntax: Default },

    // Dual-use: STATIC marker + scoped-access marker; DEFAULT marker
    // in match arm + default_statement container.
    NodeSpec { name: STATIC,  marker: true, container: true, syntax: Keyword },
    NodeSpec { name: DEFAULT, marker: true, container: true, syntax: Keyword },
];

/// Tree-sitter kind catalogue — single source of truth for every
/// kind the PHP transform handles. Sorted alphabetically by kind
/// name. See `KindHandling` for variants.
pub const KINDS: &[KindEntry] = &[
    KindEntry { kind: "abstract_modifier",             handling: KindHandling::Custom },
    KindEntry { kind: "anonymous_function",            handling: KindHandling::RenameWithMarker(FUNCTION, ANONYMOUS) },
    KindEntry { kind: "anonymous_function_use_clause", handling: KindHandling::Flatten },
    KindEntry { kind: "argument",                      handling: KindHandling::Rename(ARGUMENT) },
    KindEntry { kind: "arguments",                     handling: KindHandling::Flatten },
    KindEntry { kind: "array_creation_expression",     handling: KindHandling::Rename(ARRAY) },
    KindEntry { kind: "array_element_initializer",     handling: KindHandling::Flatten },
    KindEntry { kind: "arrow_function",                handling: KindHandling::RenameWithMarker(FUNCTION, ARROW) },
    KindEntry { kind: "assignment_expression",         handling: KindHandling::CustomThenRename(ASSIGN) },
    KindEntry { kind: "attribute",                     handling: KindHandling::Rename(ATTRIBUTE) },
    KindEntry { kind: "attribute_group",               handling: KindHandling::Flatten },
    KindEntry { kind: "attribute_list",                handling: KindHandling::Flatten },
    KindEntry { kind: "base_clause",                   handling: KindHandling::Custom },
    KindEntry { kind: "binary_expression",             handling: KindHandling::CustomThenRename(BINARY) },
    KindEntry { kind: "boolean",                       handling: KindHandling::Rename(BOOL) },
    KindEntry { kind: "break_statement",               handling: KindHandling::Rename(BREAK) },
    KindEntry { kind: "case_statement",                handling: KindHandling::Rename(CASE) },
    KindEntry { kind: "cast_expression",               handling: KindHandling::Rename(CAST) },
    KindEntry { kind: "catch_clause",                  handling: KindHandling::Rename(CATCH) },
    KindEntry { kind: "class_constant_access_expression", handling: KindHandling::RenameWithMarker(MEMBER, CONSTANT) },
    KindEntry { kind: "class_declaration",             handling: KindHandling::Rename(CLASS) },
    KindEntry { kind: "class_interface_clause",        handling: KindHandling::Custom },
    KindEntry { kind: "comment",                       handling: KindHandling::Custom },
    KindEntry { kind: "compound_statement",            handling: KindHandling::Flatten },
    KindEntry { kind: "conditional_expression",        handling: KindHandling::Rename(TERNARY) },
    KindEntry { kind: "const_declaration",             handling: KindHandling::Rename(CONST) },
    KindEntry { kind: "const_element",                 handling: KindHandling::Rename(CONSTANT) },
    KindEntry { kind: "continue_statement",            handling: KindHandling::Rename(CONTINUE) },
    KindEntry { kind: "declaration_list",              handling: KindHandling::Flatten },
    KindEntry { kind: "declare_directive",             handling: KindHandling::Flatten },
    KindEntry { kind: "declare_statement",             handling: KindHandling::Rename(DECLARE) },
    KindEntry { kind: "default_statement",             handling: KindHandling::Rename(DEFAULT) },
    KindEntry { kind: "do_statement",                  handling: KindHandling::Rename(DO) },
    KindEntry { kind: "echo_statement",                handling: KindHandling::Rename(ECHO) },
    KindEntry { kind: "else_clause",                   handling: KindHandling::Rename(ELSE) },
    KindEntry { kind: "else_if_clause",                handling: KindHandling::Rename(ELSE_IF) },
    KindEntry { kind: "encapsed_string",               handling: KindHandling::Custom },
    KindEntry { kind: "enum_case",                     handling: KindHandling::Rename(CONSTANT) },
    KindEntry { kind: "enum_declaration",              handling: KindHandling::Rename(ENUM) },
    KindEntry { kind: "enum_declaration_list",         handling: KindHandling::Flatten },
    KindEntry { kind: "escape_sequence",               handling: KindHandling::Flatten },
    KindEntry { kind: "exit_statement",                handling: KindHandling::Rename(EXIT) },
    KindEntry { kind: "expression_statement",          handling: KindHandling::Flatten },
    KindEntry { kind: "final_modifier",                handling: KindHandling::Custom },
    KindEntry { kind: "finally_clause",                handling: KindHandling::Rename(FINALLY) },
    KindEntry { kind: "float",                         handling: KindHandling::Rename(FLOAT) },
    KindEntry { kind: "for_statement",                 handling: KindHandling::Rename(FOR) },
    KindEntry { kind: "foreach_statement",             handling: KindHandling::Rename(FOREACH) },
    KindEntry { kind: "formal_parameters",             handling: KindHandling::Flatten },
    KindEntry { kind: "function_call_expression",      handling: KindHandling::Rename(CALL) },
    KindEntry { kind: "function_definition",           handling: KindHandling::Rename(FUNCTION) },
    KindEntry { kind: "goto_statement",                handling: KindHandling::Rename(GOTO) },
    KindEntry { kind: "if_statement",                  handling: KindHandling::Rename(IF) },
    KindEntry { kind: "include_expression",            handling: KindHandling::Rename(REQUIRE) },
    KindEntry { kind: "include_once_expression",       handling: KindHandling::Rename(REQUIRE) },
    KindEntry { kind: "integer",                       handling: KindHandling::Rename(INT) },
    KindEntry { kind: "interface_declaration",         handling: KindHandling::Rename(INTERFACE) },
    KindEntry { kind: "match_block",                   handling: KindHandling::Flatten },
    KindEntry { kind: "match_condition_list",          handling: KindHandling::Flatten },
    KindEntry { kind: "match_conditional_expression",  handling: KindHandling::Rename(ARM) },
    KindEntry { kind: "match_default_expression",      handling: KindHandling::RenameWithMarker(ARM, DEFAULT) },
    KindEntry { kind: "match_expression",              handling: KindHandling::Rename(MATCH) },
    KindEntry { kind: "member_access_expression",      handling: KindHandling::RenameWithMarker(MEMBER, INSTANCE) },
    KindEntry { kind: "member_call_expression",        handling: KindHandling::RenameWithMarker(CALL, INSTANCE) },
    KindEntry { kind: "method_declaration",            handling: KindHandling::CustomThenRename(METHOD) },
    // PHP `name` leaf — a bare identifier (e.g. method names, namespace
    // segments). Pass through; the field-wrap pass + builder-wrapper
    // dispatcher inlines these as appropriate.
    KindEntry { kind: "name",                          handling: KindHandling::PassThrough },
    KindEntry { kind: "named_type",                    handling: KindHandling::Rename(TYPE) },
    KindEntry { kind: "namespace_definition",          handling: KindHandling::Rename(NAMESPACE) },
    KindEntry { kind: "namespace_name",                handling: KindHandling::Flatten },
    KindEntry { kind: "namespace_use_clause",          handling: KindHandling::Flatten },
    KindEntry { kind: "namespace_use_declaration",     handling: KindHandling::Rename(USE) },
    KindEntry { kind: "namespace_use_group",           handling: KindHandling::Flatten },
    KindEntry { kind: "null",                          handling: KindHandling::Rename(NULL) },
    KindEntry { kind: "object_creation_expression",    handling: KindHandling::Rename(NEW) },
    KindEntry { kind: "optional_type",                 handling: KindHandling::RenameWithMarker(TYPE, OPTIONAL) },
    // `[a => 1]` entry in an array literal — already named `pair`,
    // matches our semantic vocabulary; pass through.
    KindEntry { kind: "pair",                          handling: KindHandling::PassThrough },
    KindEntry { kind: "parenthesized_expression",      handling: KindHandling::Flatten },
    KindEntry { kind: "php_tag",                       handling: KindHandling::RenameWithMarker(TAG, OPEN) },
    KindEntry { kind: "primitive_type",                handling: KindHandling::RenameWithMarker(TYPE, PRIMITIVE) },
    KindEntry { kind: "print_intrinsic",               handling: KindHandling::Rename(PRINT) },
    KindEntry { kind: "program",                       handling: KindHandling::Rename(PROGRAM) },
    KindEntry { kind: "property_declaration",          handling: KindHandling::CustomThenRename(FIELD) },
    KindEntry { kind: "property_element",              handling: KindHandling::Flatten },
    KindEntry { kind: "qualified_name",                handling: KindHandling::Flatten },
    KindEntry { kind: "readonly_modifier",             handling: KindHandling::Custom },
    KindEntry { kind: "require_expression",            handling: KindHandling::Rename(REQUIRE) },
    KindEntry { kind: "require_once_expression",       handling: KindHandling::Rename(REQUIRE) },
    KindEntry { kind: "return_statement",              handling: KindHandling::Rename(RETURN) },
    KindEntry { kind: "scoped_call_expression",        handling: KindHandling::RenameWithMarker(CALL, STATIC) },
    KindEntry { kind: "scoped_property_access_expression", handling: KindHandling::RenameWithMarker(MEMBER, STATIC) },
    KindEntry { kind: "simple_parameter",              handling: KindHandling::Rename(PARAMETER) },
    KindEntry { kind: "static_modifier",               handling: KindHandling::Custom },
    KindEntry { kind: "string",                        handling: KindHandling::Rename(STRING) },
    KindEntry { kind: "string_content",                handling: KindHandling::Flatten },
    KindEntry { kind: "subscript_expression",          handling: KindHandling::Rename(INDEX) },
    KindEntry { kind: "switch_statement",              handling: KindHandling::Rename(SWITCH) },
    KindEntry { kind: "text_interpolation",            handling: KindHandling::Rename(INTERPOLATION) },
    KindEntry { kind: "throw_expression",              handling: KindHandling::Rename(THROW) },
    KindEntry { kind: "trait_declaration",             handling: KindHandling::Rename(TRAIT) },
    KindEntry { kind: "try_statement",                 handling: KindHandling::Rename(TRY) },
    KindEntry { kind: "type_list",                     handling: KindHandling::Rename(TYPES) },
    KindEntry { kind: "unary_op_expression",           handling: KindHandling::CustomThenRename(UNARY) },
    KindEntry { kind: "union_type",                    handling: KindHandling::RenameWithMarker(TYPE, UNION) },
    KindEntry { kind: "use_declaration",               handling: KindHandling::Rename(USE) },
    KindEntry { kind: "variable_name",                 handling: KindHandling::Rename(VARIABLE) },
    KindEntry { kind: "variadic_parameter",            handling: KindHandling::RenameWithMarker(PARAMETER, VARIADIC) },
    KindEntry { kind: "variadic_unpacking",            handling: KindHandling::Rename(SPREAD) },
    KindEntry { kind: "visibility_modifier",           handling: KindHandling::Custom },
    KindEntry { kind: "while_statement",               handling: KindHandling::Rename(WHILE) },
    KindEntry { kind: "yield_expression",              handling: KindHandling::Rename(YIELD) },
];

/// Look up the rename target for a tree-sitter `kind` in this
/// language's catalogue. Used by `transform::map_element_name`.
pub fn rename_target(kind: &str) -> Option<(&'static str, Option<&'static str>)> {
    KINDS.iter().find(|k| k.kind == kind).and_then(|k| match k.handling {
        KindHandling::Rename(s) | KindHandling::CustomThenRename(s) => Some((s, None)),
        KindHandling::RenameWithMarker(s, m)
        | KindHandling::CustomThenRenameWithMarker(s, m) => Some((s, Some(m))),
        _ => None,
    })
}

pub fn spec(name: &str) -> Option<&'static NodeSpec> {
    NODES.iter().find(|n| n.name == name)
}

pub fn all_names() -> impl Iterator<Item = &'static str> {
    NODES.iter().map(|n| n.name)
}

pub fn is_marker_only(name: &str) -> bool {
    spec(name).map_or(false, |s| s.marker && !s.container)
}

pub fn is_declared(name: &str) -> bool {
    spec(name).is_some()
}
