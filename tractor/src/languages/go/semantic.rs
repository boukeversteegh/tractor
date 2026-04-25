/// Semantic element names — tractor's Go XML vocabulary after transform.
/// These are the names that appear in tractor's output. The tree-sitter
/// kind strings (left side of `match` arms, arguments to `get_kind`)
/// are external vocabulary and stay as bare strings.
use crate::languages::{KindEntry, KindHandling, NodeSpec};
use crate::output::syntax_highlight::SyntaxCategory;

// Named constants retained for use by the transform code. The NODES
// table below is the source of truth for marker/container role and
// syntax category.

// Top-level / structural
pub const FILE: &str = "file";
pub const PACKAGE: &str = "package";
pub const IMPORT: &str = "import";

// Declarations
pub const FUNCTION: &str = "function";
pub const METHOD: &str = "method";
pub const TYPE: &str = "type";
pub const STRUCT: &str = "struct";
pub const INTERFACE: &str = "interface";
pub const CONST: &str = "const";
pub const VAR: &str = "var";
pub const ALIAS: &str = "alias";
pub const VARIABLE: &str = "variable";

// Members / parameters
pub const FIELD: &str = "field";
pub const PARAMETER: &str = "parameter";
pub const ARGUMENTS: &str = "arguments";

// Types
pub const POINTER: &str = "pointer";
pub const SLICE: &str = "slice";
pub const MAP: &str = "map";
pub const CHAN: &str = "chan";

// Statements / control flow
pub const RETURN: &str = "return";
pub const IF: &str = "if";
pub const ELSE: &str = "else";
pub const FOR: &str = "for";
pub const RANGE: &str = "range";
pub const SWITCH: &str = "switch";
pub const CASE: &str = "case";
pub const DEFAULT: &str = "default";
pub const DEFER: &str = "defer";
pub const GO: &str = "go";
pub const SELECT: &str = "select";
pub const BREAK: &str = "break";
pub const CONTINUE: &str = "continue";
pub const GOTO: &str = "goto";
pub const LABELED: &str = "labeled";
pub const LABEL: &str = "label";
pub const SEND: &str = "send";
pub const RECEIVE: &str = "receive";
pub const ASSIGN: &str = "assign";

// Expressions
pub const CALL: &str = "call";
pub const MEMBER: &str = "member";
pub const INDEX: &str = "index";
pub const BINARY: &str = "binary";
pub const UNARY: &str = "unary";
pub const ASSERT: &str = "assert";
pub const CLOSURE: &str = "closure";
pub const LITERAL: &str = "literal";

// Literals / atoms
pub const STRING: &str = "string";
pub const INT: &str = "int";
pub const FLOAT: &str = "float";
pub const CHAR: &str = "char";
pub const TRUE: &str = "true";
pub const FALSE: &str = "false";
pub const NIL: &str = "nil";

// Identifiers / comments / op
pub const NAME: &str = "name";
pub const COMMENT: &str = "comment";
pub const OP: &str = "op";

// Comment markers — emitted by the shared CommentClassifier.
pub const TRAILING: &str = "trailing";
pub const LEADING: &str = "leading";

pub const IOTA: &str = "iota";
pub const ELSE_IF: &str = "else_if";

// Marker-only names.
pub const RAW: &str = "raw";
pub const SHORT: &str = "short";
pub const EXPORTED: &str = "exported";
pub const UNEXPORTED: &str = "unexported";
pub const NEGATED: &str = "negated";
pub const GENERIC: &str = "generic";

use SyntaxCategory::*;

/// Per-name metadata — single source of truth for every element
/// name this language's transform can emit.
///
/// Dual-use names set BOTH `marker: true` and `container: true`:
///   - FUNCTION — function_declaration (container) vs function_type
///                (marker on `<type>`).
///   - TYPE     — type wrapper (container) vs type_switch_statement
///                emits `<switch><type/>…>` (marker).
pub const NODES: &[NodeSpec] = &[
    // Top-level / structural
    NodeSpec { name: FILE,    marker: false, container: true, syntax: Default },
    NodeSpec { name: PACKAGE, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: IMPORT,  marker: false, container: true, syntax: Keyword },

    // Declarations (FUNCTION, TYPE dual-use)
    NodeSpec { name: FUNCTION,  marker: true,  container: true, syntax: Keyword },
    NodeSpec { name: METHOD,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: TYPE,      marker: true,  container: true, syntax: Type },
    NodeSpec { name: STRUCT,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: INTERFACE, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CONST,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: VAR,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ALIAS,     marker: false, container: true, syntax: Default },
    NodeSpec { name: VARIABLE,  marker: false, container: true, syntax: Default },

    // Members / parameters
    NodeSpec { name: FIELD,     marker: false, container: true, syntax: Default },
    NodeSpec { name: PARAMETER, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ARGUMENTS, marker: false, container: true, syntax: Default },

    // Types
    NodeSpec { name: POINTER, marker: false, container: true, syntax: Type },
    NodeSpec { name: SLICE,   marker: false, container: true, syntax: Type },
    NodeSpec { name: MAP,     marker: false, container: true, syntax: Type },
    NodeSpec { name: CHAN,    marker: false, container: true, syntax: Type },

    // Statements / control flow
    NodeSpec { name: RETURN,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: IF,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ELSE,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ELSE_IF,  marker: false, container: true, syntax: Default },
    NodeSpec { name: FOR,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: RANGE,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: SWITCH,   marker: false, container: true, syntax: Default },
    NodeSpec { name: CASE,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: DEFAULT,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: DEFER,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: GO,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: SELECT,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: BREAK,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CONTINUE, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: GOTO,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: LABELED,  marker: false, container: true, syntax: Default },
    NodeSpec { name: LABEL,    marker: false, container: true, syntax: Default },
    NodeSpec { name: SEND,     marker: false, container: true, syntax: Default },
    NodeSpec { name: RECEIVE,  marker: false, container: true, syntax: Default },
    NodeSpec { name: ASSIGN,   marker: false, container: true, syntax: Default },

    // Expressions
    NodeSpec { name: CALL,    marker: false, container: true, syntax: Function },
    NodeSpec { name: MEMBER,  marker: false, container: true, syntax: Default },
    NodeSpec { name: INDEX,   marker: false, container: true, syntax: Default },
    NodeSpec { name: BINARY,  marker: false, container: true, syntax: Operator },
    NodeSpec { name: UNARY,   marker: false, container: true, syntax: Operator },
    NodeSpec { name: ASSERT,  marker: false, container: true, syntax: Default },
    NodeSpec { name: CLOSURE, marker: false, container: true, syntax: Default },
    NodeSpec { name: LITERAL, marker: false, container: true, syntax: Default },

    // Literals / atoms
    NodeSpec { name: STRING, marker: false, container: true, syntax: String },
    NodeSpec { name: INT,    marker: false, container: true, syntax: Number },
    NodeSpec { name: FLOAT,  marker: false, container: true, syntax: Number },
    NodeSpec { name: CHAR,   marker: false, container: true, syntax: Default },
    NodeSpec { name: TRUE,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FALSE,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: NIL,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: IOTA,   marker: false, container: true, syntax: Default },

    // Identifiers / comments / op
    NodeSpec { name: NAME,    marker: false, container: true, syntax: Identifier },
    NodeSpec { name: COMMENT, marker: false, container: true, syntax: Comment },
    NodeSpec { name: OP,      marker: false, container: true, syntax: Operator },

    // Comment markers
    NodeSpec { name: TRAILING, marker: true, container: false, syntax: Default },
    NodeSpec { name: LEADING,  marker: true, container: false, syntax: Default },

    // Marker-only
    NodeSpec { name: RAW,        marker: true, container: false, syntax: Default },
    NodeSpec { name: SHORT,      marker: true, container: false, syntax: Default },
    NodeSpec { name: EXPORTED,   marker: true, container: false, syntax: Keyword },
    NodeSpec { name: UNEXPORTED, marker: true, container: false, syntax: Keyword },
    NodeSpec { name: NEGATED,    marker: true, container: false, syntax: Default },
    NodeSpec { name: GENERIC,    marker: true, container: false, syntax: Default },
];

/// Tree-sitter kind catalogue — single source of truth for every
/// kind the Go transform handles. Sorted alphabetically by kind name.
/// See `KindHandling` for variants.
pub const KINDS: &[KindEntry] = &[
    KindEntry { kind: "argument_list",                 handling: KindHandling::Flatten },
    KindEntry { kind: "array_type",                    handling: KindHandling::PassThrough },
    KindEntry { kind: "assignment_statement",          handling: KindHandling::Rename(ASSIGN) },
    KindEntry { kind: "binary_expression",             handling: KindHandling::CustomThenRename(BINARY) },
    KindEntry { kind: "blank_identifier",              handling: KindHandling::Rename(NAME) },
    KindEntry { kind: "block",                         handling: KindHandling::Flatten },
    KindEntry { kind: "break_statement",               handling: KindHandling::Rename(BREAK) },
    KindEntry { kind: "call_expression",               handling: KindHandling::Rename(CALL) },
    KindEntry { kind: "case_clause",                   handling: KindHandling::Rename(CASE) },
    KindEntry { kind: "channel_type",                  handling: KindHandling::Rename(CHAN) },
    KindEntry { kind: "comment",                       handling: KindHandling::Custom },
    KindEntry { kind: "communication_case",            handling: KindHandling::Rename(CASE) },
    KindEntry { kind: "composite_literal",             handling: KindHandling::Rename(LITERAL) },
    KindEntry { kind: "const_declaration",             handling: KindHandling::Rename(CONST) },
    KindEntry { kind: "const_spec",                    handling: KindHandling::Flatten },
    KindEntry { kind: "continue_statement",            handling: KindHandling::Rename(CONTINUE) },
    KindEntry { kind: "dec_statement",                 handling: KindHandling::Rename(UNARY) },
    KindEntry { kind: "default_case",                  handling: KindHandling::Rename(DEFAULT) },
    KindEntry { kind: "defer_statement",               handling: KindHandling::Rename(DEFER) },
    // `import . "pkg"` — tree-sitter emits a `dot` leaf token. Pass through.
    KindEntry { kind: "dot",                           handling: KindHandling::PassThrough },
    KindEntry { kind: "else_clause",                   handling: KindHandling::Rename(ELSE) },
    KindEntry { kind: "escape_sequence",               handling: KindHandling::Flatten },
    KindEntry { kind: "expression_list",               handling: KindHandling::Flatten },
    KindEntry { kind: "expression_statement",          handling: KindHandling::Flatten },
    KindEntry { kind: "expression_switch_statement",   handling: KindHandling::Rename(SWITCH) },
    KindEntry { kind: "false",                         handling: KindHandling::Rename(FALSE) },
    KindEntry { kind: "field_declaration",             handling: KindHandling::CustomThenRename(FIELD) },
    KindEntry { kind: "field_declaration_list",        handling: KindHandling::Flatten },
    KindEntry { kind: "field_identifier",              handling: KindHandling::Rename(NAME) },
    KindEntry { kind: "float_literal",                 handling: KindHandling::Rename(FLOAT) },
    KindEntry { kind: "for_clause",                    handling: KindHandling::Flatten },
    KindEntry { kind: "for_statement",                 handling: KindHandling::Rename(FOR) },
    KindEntry { kind: "func_literal",                  handling: KindHandling::Rename(CLOSURE) },
    KindEntry { kind: "function_declaration",          handling: KindHandling::CustomThenRename(FUNCTION) },
    KindEntry { kind: "function_type",                 handling: KindHandling::RenameWithMarker(TYPE, FUNCTION) },
    KindEntry { kind: "generic_type",                  handling: KindHandling::RenameWithMarker(TYPE, GENERIC) },
    KindEntry { kind: "go_statement",                  handling: KindHandling::Rename(GO) },
    KindEntry { kind: "goto_statement",                handling: KindHandling::Rename(GOTO) },
    KindEntry { kind: "identifier",                    handling: KindHandling::Custom },
    KindEntry { kind: "if_statement",                  handling: KindHandling::Custom },
    KindEntry { kind: "import_declaration",            handling: KindHandling::Rename(IMPORT) },
    KindEntry { kind: "import_spec",                   handling: KindHandling::Flatten },
    KindEntry { kind: "import_spec_list",              handling: KindHandling::Flatten },
    KindEntry { kind: "inc_statement",                 handling: KindHandling::Rename(UNARY) },
    KindEntry { kind: "index_expression",              handling: KindHandling::Rename(INDEX) },
    KindEntry { kind: "int_literal",                   handling: KindHandling::Rename(INT) },
    KindEntry { kind: "interface_type",                handling: KindHandling::Rename(INTERFACE) },
    KindEntry { kind: "interpreted_string_literal",    handling: KindHandling::Rename(STRING) },
    KindEntry { kind: "interpreted_string_literal_content", handling: KindHandling::Flatten },
    // Tree-sitter leaf for the `iota` constant. Already matches our
    // semantic vocabulary; pass through.
    KindEntry { kind: "iota",                          handling: KindHandling::PassThrough },
    KindEntry { kind: "keyed_element",                 handling: KindHandling::Flatten },
    KindEntry { kind: "label_name",                    handling: KindHandling::Rename(LABEL) },
    KindEntry { kind: "labeled_statement",             handling: KindHandling::Rename(LABELED) },
    KindEntry { kind: "literal_element",               handling: KindHandling::Flatten },
    KindEntry { kind: "literal_value",                 handling: KindHandling::Flatten },
    KindEntry { kind: "map_type",                      handling: KindHandling::Rename(MAP) },
    KindEntry { kind: "method_declaration",            handling: KindHandling::CustomThenRename(METHOD) },
    KindEntry { kind: "method_elem",                   handling: KindHandling::Rename(METHOD) },
    KindEntry { kind: "negated_type",                  handling: KindHandling::RenameWithMarker(TYPE, NEGATED) },
    KindEntry { kind: "nil",                           handling: KindHandling::Rename(NIL) },
    KindEntry { kind: "package_clause",                handling: KindHandling::Rename(PACKAGE) },
    KindEntry { kind: "package_identifier",            handling: KindHandling::Rename(NAME) },
    KindEntry { kind: "parameter_declaration",         handling: KindHandling::Rename(PARAMETER) },
    KindEntry { kind: "parameter_list",                handling: KindHandling::Custom },
    KindEntry { kind: "pointer_type",                  handling: KindHandling::Rename(POINTER) },
    KindEntry { kind: "qualified_type",                handling: KindHandling::Flatten },
    KindEntry { kind: "range_clause",                  handling: KindHandling::Rename(RANGE) },
    KindEntry { kind: "raw_string_literal",            handling: KindHandling::Custom },
    KindEntry { kind: "raw_string_literal_content",    handling: KindHandling::Flatten },
    KindEntry { kind: "receive_statement",             handling: KindHandling::Rename(RECEIVE) },
    KindEntry { kind: "return_statement",              handling: KindHandling::Rename(RETURN) },
    KindEntry { kind: "rune_literal",                  handling: KindHandling::Rename(CHAR) },
    KindEntry { kind: "select_statement",              handling: KindHandling::Rename(SELECT) },
    KindEntry { kind: "selector_expression",           handling: KindHandling::Rename(MEMBER) },
    KindEntry { kind: "send_statement",                handling: KindHandling::Rename(SEND) },
    KindEntry { kind: "short_var_declaration",         handling: KindHandling::Custom },
    KindEntry { kind: "slice_type",                    handling: KindHandling::Rename(SLICE) },
    KindEntry { kind: "source_file",                   handling: KindHandling::Rename(FILE) },
    KindEntry { kind: "struct_type",                   handling: KindHandling::Rename(STRUCT) },
    KindEntry { kind: "switch_statement",              handling: KindHandling::Rename(SWITCH) },
    KindEntry { kind: "true",                          handling: KindHandling::Rename(TRUE) },
    KindEntry { kind: "type_alias",                    handling: KindHandling::Custom },
    KindEntry { kind: "type_arguments",                handling: KindHandling::Rename(ARGUMENTS) },
    KindEntry { kind: "type_assertion_expression",     handling: KindHandling::Rename(ASSERT) },
    KindEntry { kind: "type_case",                     handling: KindHandling::Flatten },
    KindEntry { kind: "type_constraint",               handling: KindHandling::Flatten },
    KindEntry { kind: "type_declaration",              handling: KindHandling::Custom },
    KindEntry { kind: "type_elem",                     handling: KindHandling::Flatten },
    KindEntry { kind: "type_identifier",               handling: KindHandling::Custom },
    KindEntry { kind: "type_parameter_declaration",    handling: KindHandling::Flatten },
    KindEntry { kind: "type_parameter_list",           handling: KindHandling::Flatten },
    KindEntry { kind: "type_spec",                     handling: KindHandling::Custom },
    KindEntry { kind: "type_switch_statement",         handling: KindHandling::RenameWithMarker(SWITCH, TYPE) },
    KindEntry { kind: "unary_expression",              handling: KindHandling::CustomThenRename(UNARY) },
    KindEntry { kind: "var_declaration",               handling: KindHandling::Rename(VAR) },
    KindEntry { kind: "var_spec",                      handling: KindHandling::Flatten },
    KindEntry { kind: "var_spec_list",                 handling: KindHandling::Flatten },
    KindEntry { kind: "variadic_parameter_declaration", handling: KindHandling::Rename(PARAMETER) },
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
