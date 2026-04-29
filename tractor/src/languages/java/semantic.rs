/// Semantic element names — tractor's Java XML vocabulary after transform.
use crate::languages::{KindEntry, KindHandling, NodeSpec};
use crate::output::syntax_highlight::SyntaxCategory;

// Named constants retained for use by the transform code. The NODES
// table below is the source of truth for marker/container role and
// syntax category.

// Top-level / declarations
pub const PROGRAM: &str = "program";
pub const CLASS: &str = "class";
pub const INTERFACE: &str = "interface";
pub const ENUM: &str = "enum";
pub const RECORD: &str = "record";
pub const METHOD: &str = "method";
pub const CONSTRUCTOR: &str = "constructor";
pub const FIELD: &str = "field";
pub const VARIABLE: &str = "variable";
pub const DECLARATOR: &str = "declarator";
pub const CONSTANT: &str = "constant";

// Members
pub const PARAMETER: &str = "parameter";
pub const GENERIC: &str = "generic";
pub const GENERICS: &str = "generics";
pub const EXTENDS: &str = "extends";
pub const IMPLEMENTS: &str = "implements";

// Type vocabulary
pub const TYPE: &str = "type";
pub const PATH: &str = "path";
pub const RETURNS: &str = "returns";
pub const DIMENSIONS: &str = "dimensions";

// Control flow
pub const RETURN: &str = "return";
pub const IF: &str = "if";
pub const ELSE: &str = "else";
pub const ELSE_IF: &str = "else_if";
pub const FOR: &str = "for";
pub const FOREACH: &str = "foreach";
pub const WHILE: &str = "while";
pub const TRY: &str = "try";
pub const CATCH: &str = "catch";
pub const FINALLY: &str = "finally";
pub const THROW: &str = "throw";
pub const THROWS: &str = "throws";
pub const SWITCH: &str = "switch";
pub const ARM: &str = "arm";
pub const LABEL: &str = "label";
pub const CASE: &str = "case";
pub const PATTERN: &str = "pattern";
pub const GUARD: &str = "guard";
pub const BODY: &str = "body";

// Expressions
pub const CALL: &str = "call";
pub const NEW: &str = "new";
pub const MEMBER: &str = "member";
pub const INDEX: &str = "index";
pub const ASSIGN: &str = "assign";
pub const BINARY: &str = "binary";
pub const UNARY: &str = "unary";
pub const LAMBDA: &str = "lambda";
pub const TERNARY: &str = "ternary";
pub const ANNOTATION: &str = "annotation";

// Imports
pub const IMPORT: &str = "import";
pub const PACKAGE: &str = "package";

// Literals
pub const STRING: &str = "string";
pub const INT: &str = "int";
pub const FLOAT: &str = "float";
pub const TRUE: &str = "true";
pub const FALSE: &str = "false";
pub const NULL: &str = "null";

// Identifiers / comments / op
pub const NAME: &str = "name";
pub const COMMENT: &str = "comment";
pub const OP: &str = "op";

// Comment markers — emitted by the shared CommentClassifier
// (see `languages::comments`).
pub const TRAILING: &str = "trailing";
pub const LEADING: &str = "leading";

// Access modifiers.
pub const PUBLIC: &str = "public";
pub const PRIVATE: &str = "private";
pub const PROTECTED: &str = "protected";

// Other modifiers.
pub const STATIC: &str = "static";
pub const FINAL: &str = "final";
pub const ABSTRACT: &str = "abstract";
pub const SYNCHRONIZED: &str = "synchronized";
pub const VOLATILE: &str = "volatile";
pub const TRANSIENT: &str = "transient";
pub const NATIVE: &str = "native";
pub const STRICTFP: &str = "strictfp";

// Special markers.
pub const VOID: &str = "void";
pub const THIS: &str = "this";
pub const SUPER: &str = "super";
pub const ARRAY: &str = "array";
pub const VARIADIC: &str = "variadic";
pub const COMPACT: &str = "compact";

use SyntaxCategory::*;

/// Per-name metadata — single source of truth for every element
/// name this language's transform can emit.
///
/// Dual-use names set BOTH `marker: true` and `container: true`:
///   - PACKAGE — structural (package_declaration) + marker (implicit
///               access when no access modifier is present).
///   - RECORD  — structural (record_declaration) + marker (record_pattern).
///   - TYPE    — structural (type references) + marker (type_pattern).
///   - THIS    — marker on `<call[this]>` + structural for bare `this`.
pub const NODES: &[NodeSpec] = &[
    // Top-level / declarations (RECORD dual-use)
    NodeSpec { name: PROGRAM,     marker: false, container: true, syntax: Default },
    NodeSpec { name: CLASS,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: INTERFACE,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ENUM,        marker: false, container: true, syntax: Keyword },
    NodeSpec { name: RECORD,      marker: true,  container: true, syntax: Default },
    NodeSpec { name: METHOD,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CONSTRUCTOR, marker: false, container: true, syntax: Default },
    NodeSpec { name: FIELD,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: VARIABLE,    marker: false, container: true, syntax: Default },
    NodeSpec { name: DECLARATOR,  marker: false, container: true, syntax: Default },
    NodeSpec { name: CONSTANT,    marker: false, container: true, syntax: Default },

    // Members
    NodeSpec { name: PARAMETER,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: GENERIC,    marker: false, container: true, syntax: Type },
    NodeSpec { name: GENERICS,   marker: false, container: true, syntax: Default },
    NodeSpec { name: EXTENDS,    marker: false, container: true, syntax: Default },
    NodeSpec { name: IMPLEMENTS, marker: false, container: true, syntax: Default },

    // Type vocabulary (TYPE dual-use)
    NodeSpec { name: TYPE,       marker: true,  container: true, syntax: Type },
    NodeSpec { name: PATH,       marker: false, container: true, syntax: Default },
    NodeSpec { name: RETURNS,    marker: false, container: true, syntax: Default },
    NodeSpec { name: DIMENSIONS, marker: false, container: true, syntax: Default },

    // Control flow
    NodeSpec { name: RETURN,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: IF,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ELSE,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ELSE_IF, marker: false, container: true, syntax: Default },
    NodeSpec { name: FOR,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FOREACH, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: WHILE,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: TRY,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CATCH,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FINALLY, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: THROW,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: THROWS,  marker: false, container: true, syntax: Default },
    NodeSpec { name: SWITCH,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ARM,     marker: false, container: true, syntax: Default },
    NodeSpec { name: LABEL,   marker: false, container: true, syntax: Default },
    NodeSpec { name: CASE,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: PATTERN, marker: false, container: true, syntax: Default },
    NodeSpec { name: GUARD,   marker: false, container: true, syntax: Default },
    NodeSpec { name: BODY,    marker: false, container: true, syntax: Default },

    // Expressions
    NodeSpec { name: CALL,       marker: false, container: true, syntax: Function },
    NodeSpec { name: NEW,        marker: false, container: true, syntax: Keyword },
    NodeSpec { name: MEMBER,     marker: false, container: true, syntax: Default },
    NodeSpec { name: INDEX,      marker: false, container: true, syntax: Default },
    NodeSpec { name: ASSIGN,     marker: false, container: true, syntax: Operator },
    NodeSpec { name: BINARY,     marker: false, container: true, syntax: Operator },
    NodeSpec { name: UNARY,      marker: false, container: true, syntax: Operator },
    NodeSpec { name: LAMBDA,     marker: false, container: true, syntax: Function },
    NodeSpec { name: TERNARY,    marker: false, container: true, syntax: Operator },
    NodeSpec { name: ANNOTATION, marker: false, container: true, syntax: Default },

    // Imports (PACKAGE dual-use)
    NodeSpec { name: IMPORT,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: PACKAGE, marker: true,  container: true, syntax: Keyword },

    // Literals
    NodeSpec { name: STRING, marker: false, container: true, syntax: String },
    NodeSpec { name: INT,    marker: false, container: true, syntax: Number },
    NodeSpec { name: FLOAT,  marker: false, container: true, syntax: Number },
    NodeSpec { name: TRUE,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FALSE,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: NULL,   marker: false, container: true, syntax: Keyword },

    // Identifiers / comments / op
    NodeSpec { name: NAME,    marker: false, container: true, syntax: Identifier },
    NodeSpec { name: COMMENT, marker: false, container: true, syntax: Comment },
    NodeSpec { name: OP,      marker: false, container: true, syntax: Operator },

    // Comment markers
    NodeSpec { name: TRAILING, marker: true, container: false, syntax: Default },
    NodeSpec { name: LEADING,  marker: true, container: false, syntax: Default },

    // Access modifiers — markers only.
    NodeSpec { name: PUBLIC,    marker: true, container: false, syntax: Keyword },
    NodeSpec { name: PRIVATE,   marker: true, container: false, syntax: Keyword },
    NodeSpec { name: PROTECTED, marker: true, container: false, syntax: Keyword },

    // Other modifiers — markers only.
    NodeSpec { name: STATIC,       marker: true, container: false, syntax: Keyword },
    NodeSpec { name: FINAL,        marker: true, container: false, syntax: Keyword },
    NodeSpec { name: ABSTRACT,     marker: true, container: false, syntax: Keyword },
    NodeSpec { name: SYNCHRONIZED, marker: true, container: false, syntax: Keyword },
    NodeSpec { name: VOLATILE,     marker: true, container: false, syntax: Keyword },
    NodeSpec { name: TRANSIENT,    marker: true, container: false, syntax: Keyword },
    NodeSpec { name: NATIVE,       marker: true, container: false, syntax: Keyword },
    NodeSpec { name: STRICTFP,     marker: true, container: false, syntax: Keyword },

    // Special markers (THIS dual-use)
    NodeSpec { name: VOID,     marker: true, container: false, syntax: Default },
    NodeSpec { name: THIS,     marker: true, container: true,  syntax: Keyword },
    NodeSpec { name: SUPER,    marker: true, container: false, syntax: Keyword },
    NodeSpec { name: ARRAY,    marker: true, container: false, syntax: Type },
    NodeSpec { name: VARIADIC, marker: true, container: false, syntax: Default },
    NodeSpec { name: COMPACT,  marker: true, container: false, syntax: Default },
];

/// Tree-sitter kind catalogue — single source of truth for every
/// kind the Java transform handles. Sorted alphabetically by kind name.
/// See `KindHandling` for variant semantics.
pub const KINDS: &[KindEntry] = &[
    KindEntry { kind: "annotation",                    handling: KindHandling::Rename(ANNOTATION) },
    KindEntry { kind: "annotation_argument_list",      handling: KindHandling::Flatten },
    KindEntry { kind: "argument_list",                 handling: KindHandling::Flatten },
    KindEntry { kind: "array_access",                  handling: KindHandling::Rename(INDEX) },
    KindEntry { kind: "array_type",                    handling: KindHandling::RenameWithMarker(TYPE, ARRAY) },
    KindEntry { kind: "assignment_expression",         handling: KindHandling::CustomThenRename(ASSIGN) },
    KindEntry { kind: "binary_expression",             handling: KindHandling::CustomThenRename(BINARY) },
    KindEntry { kind: "binary_integer_literal",        handling: KindHandling::Rename(INT) },
    KindEntry { kind: "block",                         handling: KindHandling::Flatten },
    KindEntry { kind: "block_comment",                 handling: KindHandling::Custom },
    KindEntry { kind: "boolean_type",                  handling: KindHandling::Custom },
    KindEntry { kind: "catch_clause",                  handling: KindHandling::Rename(CATCH) },
    KindEntry { kind: "class_body",                    handling: KindHandling::Flatten },
    KindEntry { kind: "class_declaration",             handling: KindHandling::CustomThenRename(CLASS) },
    KindEntry { kind: "compact_constructor_declaration", handling: KindHandling::RenameWithMarker(CONSTRUCTOR, COMPACT) },
    KindEntry { kind: "constructor_body",              handling: KindHandling::Flatten },
    KindEntry { kind: "constructor_declaration",       handling: KindHandling::CustomThenRename(CONSTRUCTOR) },
    KindEntry { kind: "decimal_floating_point_literal", handling: KindHandling::Rename(FLOAT) },
    KindEntry { kind: "decimal_integer_literal",       handling: KindHandling::Rename(INT) },
    KindEntry { kind: "enhanced_for_statement",        handling: KindHandling::Rename(FOREACH) },
    KindEntry { kind: "enum_body",                     handling: KindHandling::Flatten },
    KindEntry { kind: "enum_body_declarations",        handling: KindHandling::Flatten },
    KindEntry { kind: "enum_constant",                 handling: KindHandling::Rename(CONSTANT) },
    KindEntry { kind: "enum_declaration",              handling: KindHandling::CustomThenRename(ENUM) },
    KindEntry { kind: "explicit_constructor_invocation", handling: KindHandling::Custom },
    KindEntry { kind: "expression_statement",          handling: KindHandling::Flatten },
    KindEntry { kind: "false",                         handling: KindHandling::Rename(FALSE) },
    KindEntry { kind: "field_access",                  handling: KindHandling::Rename(MEMBER) },
    KindEntry { kind: "field_declaration",             handling: KindHandling::CustomThenRename(FIELD) },
    KindEntry { kind: "finally_clause",                handling: KindHandling::Rename(FINALLY) },
    KindEntry { kind: "floating_point_type",           handling: KindHandling::Custom },
    KindEntry { kind: "for_statement",                 handling: KindHandling::Rename(FOR) },
    KindEntry { kind: "formal_parameter",              handling: KindHandling::Rename(PARAMETER) },
    KindEntry { kind: "formal_parameters",             handling: KindHandling::Flatten },
    KindEntry { kind: "generic_type",                  handling: KindHandling::Custom },
    // `case L when COND ->` — the guard expression. Tree-sitter emits
    // `guard`, which already matches our semantic vocabulary — pass through.
    KindEntry { kind: "guard",                         handling: KindHandling::PassThrough },
    KindEntry { kind: "hex_integer_literal",           handling: KindHandling::Rename(INT) },
    KindEntry { kind: "identifier",                    handling: KindHandling::Custom },
    KindEntry { kind: "if_statement",                  handling: KindHandling::Custom },
    KindEntry { kind: "import_declaration",            handling: KindHandling::Rename(IMPORT) },
    KindEntry { kind: "integral_type",                 handling: KindHandling::Custom },
    KindEntry { kind: "interface_body",                handling: KindHandling::Flatten },
    KindEntry { kind: "interface_declaration",         handling: KindHandling::CustomThenRename(INTERFACE) },
    KindEntry { kind: "lambda_expression",             handling: KindHandling::Rename(LAMBDA) },
    KindEntry { kind: "line_comment",                  handling: KindHandling::Custom },
    KindEntry { kind: "local_variable_declaration",    handling: KindHandling::Rename(VARIABLE) },
    KindEntry { kind: "marker_annotation",             handling: KindHandling::Rename(ANNOTATION) },
    KindEntry { kind: "method_declaration",            handling: KindHandling::CustomThenRename(METHOD) },
    KindEntry { kind: "method_invocation",             handling: KindHandling::Rename(CALL) },
    KindEntry { kind: "modifiers",                     handling: KindHandling::Custom },
    KindEntry { kind: "null_literal",                  handling: KindHandling::Rename(NULL) },
    KindEntry { kind: "object_creation_expression",    handling: KindHandling::Rename(NEW) },
    KindEntry { kind: "octal_integer_literal",         handling: KindHandling::Rename(INT) },
    KindEntry { kind: "package_declaration",           handling: KindHandling::Rename(PACKAGE) },
    KindEntry { kind: "parenthesized_expression",      handling: KindHandling::Flatten },
    // `case L when COND ->` — the matched pattern. Tree-sitter emits
    // `pattern`, which already matches our semantic vocabulary — pass through.
    KindEntry { kind: "pattern",                       handling: KindHandling::PassThrough },
    KindEntry { kind: "program",                       handling: KindHandling::Rename(PROGRAM) },
    KindEntry { kind: "record_declaration",            handling: KindHandling::Rename(RECORD) },
    KindEntry { kind: "record_pattern",                handling: KindHandling::RenameWithMarker(PATTERN, RECORD) },
    KindEntry { kind: "return_statement",              handling: KindHandling::Rename(RETURN) },
    KindEntry { kind: "scoped_identifier",             handling: KindHandling::Rename(PATH) },
    KindEntry { kind: "scoped_type_identifier",        handling: KindHandling::Rename(PATH) },
    KindEntry { kind: "spread_parameter",              handling: KindHandling::RenameWithMarker(PARAMETER, VARIADIC) },
    KindEntry { kind: "string_fragment",               handling: KindHandling::Flatten },
    KindEntry { kind: "string_literal",                handling: KindHandling::Rename(STRING) },
    // Tree-sitter leafs for the `super` and `this` keywords inside an
    // `explicit_constructor_invocation`. The dispatcher arm for that
    // wrapper consumes them; if they appear elsewhere they pass through.
    KindEntry { kind: "super",                         handling: KindHandling::PassThrough },
    KindEntry { kind: "super_interfaces",              handling: KindHandling::Rename(IMPLEMENTS) },
    KindEntry { kind: "superclass",                    handling: KindHandling::Rename(EXTENDS) },
    KindEntry { kind: "switch_block",                  handling: KindHandling::Rename(BODY) },
    KindEntry { kind: "switch_block_statement_group",  handling: KindHandling::Rename(CASE) },
    KindEntry { kind: "switch_expression",             handling: KindHandling::Rename(SWITCH) },
    KindEntry { kind: "switch_label",                  handling: KindHandling::Rename(LABEL) },
    KindEntry { kind: "switch_rule",                   handling: KindHandling::Rename(ARM) },
    KindEntry { kind: "ternary_expression",            handling: KindHandling::Custom },
    KindEntry { kind: "this",                          handling: KindHandling::PassThrough },
    KindEntry { kind: "throw_statement",               handling: KindHandling::Rename(THROW) },
    // `throws E1, E2` — list of declared exceptions on a method header.
    // Already named `throws`, matches our vocabulary; pass through.
    KindEntry { kind: "throws",                        handling: KindHandling::PassThrough },
    KindEntry { kind: "true",                          handling: KindHandling::Rename(TRUE) },
    KindEntry { kind: "try_statement",                 handling: KindHandling::Rename(TRY) },
    KindEntry { kind: "type_arguments",                handling: KindHandling::Flatten },
    KindEntry { kind: "type_bound",                    handling: KindHandling::Rename(EXTENDS) },
    KindEntry { kind: "type_identifier",               handling: KindHandling::Custom },
    KindEntry { kind: "type_list",                     handling: KindHandling::Flatten },
    KindEntry { kind: "type_parameter",                handling: KindHandling::Custom },
    KindEntry { kind: "type_parameters",               handling: KindHandling::Flatten },
    KindEntry { kind: "type_pattern",                  handling: KindHandling::RenameWithMarker(PATTERN, TYPE) },
    KindEntry { kind: "unary_expression",              handling: KindHandling::CustomThenRename(UNARY) },
    KindEntry { kind: "variable_declarator",           handling: KindHandling::Rename(DECLARATOR) },
    KindEntry { kind: "void_type",                     handling: KindHandling::Custom },
    KindEntry { kind: "while_statement",               handling: KindHandling::Rename(WHILE) },
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
