/// Semantic element names — tractor's Python XML vocabulary after transform.
use crate::languages::{KindEntry, KindHandling, NodeSpec};
use crate::output::syntax_highlight::SyntaxCategory;

// Named constants retained for use by the transform code. The NODES
// table below is the source of truth for marker/container role and
// syntax category.

// Structural — containers.

// Top-level / declarations
pub const MODULE: &str = "module";
pub const CLASS: &str = "class";
pub const FUNCTION: &str = "function";
pub const DECORATED: &str = "decorated";
pub const DECORATOR: &str = "decorator";
pub const LAMBDA: &str = "lambda";

// Members
pub const PARAMETER: &str = "parameter";
pub const ARGUMENT: &str = "argument";

// Type vocabulary
pub const TYPE: &str = "type";

// Control flow
pub const RETURN: &str = "return";
pub const IF: &str = "if";
pub const ELSE_IF: &str = "else_if";
pub const ELSE: &str = "else";
pub const FOR: &str = "for";
pub const WHILE: &str = "while";
pub const TRY: &str = "try";
pub const EXCEPT: &str = "except";
pub const FINALLY: &str = "finally";
pub const WITH: &str = "with";
pub const RAISE: &str = "raise";
pub const PASS: &str = "pass";
pub const BREAK: &str = "break";
pub const CONTINUE: &str = "continue";
pub const MATCH: &str = "match";
pub const ARM: &str = "arm";
pub const PATTERN: &str = "pattern";

// Imports / names
pub const IMPORT: &str = "import";
pub const FROM: &str = "from";
pub const ASSERT: &str = "assert";
pub const DELETE: &str = "delete";
pub const GLOBAL: &str = "global";
pub const NONLOCAL: &str = "nonlocal";

// Expressions
pub const CALL: &str = "call";
pub const MEMBER: &str = "member";
pub const SUBSCRIPT: &str = "subscript";
pub const ASSIGN: &str = "assign";
pub const BINARY: &str = "binary";
pub const UNARY: &str = "unary";
pub const COMPARE: &str = "compare";
pub const LOGICAL: &str = "logical";
pub const AWAIT: &str = "await";
pub const YIELD: &str = "yield";
pub const GENERATOR: &str = "generator";
pub const TERNARY: &str = "ternary";
pub const CAST: &str = "cast";
pub const AS: &str = "as";
pub const SPREAD: &str = "spread";
pub const FORMAT: &str = "format";
pub const TUPLE: &str = "tuple";
pub const GENERIC: &str = "generic";
pub const PAIR: &str = "pair";
pub const INTERPOLATION: &str = "interpolation";

// Function-signature separators.
pub const KEYWORD: &str = "keyword";
pub const POSITIONAL: &str = "positional";

// Collection containers (structural). ALSO appear as pattern/spread
// markers — dual-use; see NODES entries below.
pub const LIST: &str = "list";
pub const DICT: &str = "dict";
pub const SET: &str = "set";

// Literals
pub const STRING: &str = "string";
pub const INT: &str = "int";
pub const FLOAT: &str = "float";
pub const TRUE: &str = "true";
pub const FALSE: &str = "false";
pub const NONE: &str = "none";

// Identifiers / comments
pub const NAME: &str = "name";
pub const COMMENT: &str = "comment";

// Comment markers — emitted by the shared CommentClassifier.
pub const TRAILING: &str = "trailing";
pub const LEADING: &str = "leading";

// Operator child
pub const OP: &str = "op";

// Markers — always empty.

// Visibility (lifted from name convention).
pub const PUBLIC: &str = "public";
pub const PRIVATE: &str = "private";
pub const PROTECTED: &str = "protected";

// Function flags.
pub const ASYNC: &str = "async";

// Collection-construction markers (only on <list>/<dict>/<set>).
pub const LITERAL: &str = "literal";
pub const COMPREHENSION: &str = "comprehension";

// Pattern / type shape markers that don't conflict with structural.
pub const UNION: &str = "union";
pub const SPLAT: &str = "splat";

use SyntaxCategory::*;

/// Per-name metadata — single source of truth for every element
/// name this language's transform can emit. Replaces the previous
/// ALL_NAMES / MARKER_ONLY slice pair and the ad-hoc comment-based
/// dual-use documentation.
///
/// Dual-use names (e.g. LIST, DICT, SET — structural container for
/// a collection literal, but also emitted as a pattern / spread
/// marker in other contexts) set BOTH `marker: true` and
/// `container: true`.
pub const NODES: &[NodeSpec] = &[
    // Top-level / declarations
    NodeSpec { name: MODULE,   marker: false, container: true,  syntax: Keyword },
    NodeSpec { name: CLASS,    marker: false, container: true,  syntax: Keyword },
    NodeSpec { name: FUNCTION, marker: false, container: true,  syntax: Keyword },
    NodeSpec { name: DECORATED,marker: false, container: true,  syntax: Keyword },
    NodeSpec { name: DECORATOR,marker: false, container: true,  syntax: Keyword },
    NodeSpec { name: LAMBDA,   marker: false, container: true,  syntax: Function },

    // Members
    NodeSpec { name: PARAMETER, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ARGUMENT,  marker: false, container: true, syntax: Default },

    // Type vocabulary
    NodeSpec { name: TYPE, marker: false, container: true, syntax: Type },

    // Control flow
    NodeSpec { name: RETURN,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: IF,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ELSE_IF,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ELSE,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FOR,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: WHILE,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: TRY,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: EXCEPT,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FINALLY,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: WITH,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: RAISE,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: PASS,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: BREAK,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CONTINUE, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: MATCH,    marker: false, container: true, syntax: Default },
    NodeSpec { name: ARM,      marker: false, container: true, syntax: Default },
    NodeSpec { name: PATTERN,  marker: false, container: true, syntax: Default },

    // Imports / names
    NodeSpec { name: IMPORT,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FROM,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ASSERT,   marker: false, container: true, syntax: Default },
    NodeSpec { name: DELETE,   marker: false, container: true, syntax: Default },
    NodeSpec { name: GLOBAL,   marker: false, container: true, syntax: Default },
    NodeSpec { name: NONLOCAL, marker: false, container: true, syntax: Default },

    // Expressions
    NodeSpec { name: CALL,     marker: false, container: true, syntax: Function },
    NodeSpec { name: MEMBER,   marker: false, container: true, syntax: Default },
    NodeSpec { name: SUBSCRIPT,marker: false, container: true, syntax: Default },
    NodeSpec { name: ASSIGN,   marker: false, container: true, syntax: Operator },
    NodeSpec { name: BINARY,   marker: false, container: true, syntax: Operator },
    NodeSpec { name: UNARY,    marker: false, container: true, syntax: Operator },
    NodeSpec { name: COMPARE,  marker: false, container: true, syntax: Operator },
    NodeSpec { name: LOGICAL,  marker: false, container: true, syntax: Operator },
    NodeSpec { name: AWAIT,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: YIELD,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: GENERATOR,marker: false, container: true, syntax: Keyword },
    NodeSpec { name: TERNARY,  marker: false, container: true, syntax: Operator },
    NodeSpec { name: CAST,     marker: false, container: true, syntax: Default },
    NodeSpec { name: AS,       marker: false, container: true, syntax: Default },
    NodeSpec { name: SPREAD,   marker: false, container: true, syntax: Default },
    NodeSpec { name: FORMAT,   marker: false, container: true, syntax: Default },
    // `(a, b)` / `a, b, c` — container for tuple expressions.
    NodeSpec { name: TUPLE,    marker: false, container: true, syntax: Default },
    // `List[int]` — generic type application; container around the
    // base type and its `<arguments>`.
    NodeSpec { name: GENERIC,  marker: false, container: true, syntax: Default },
    // `{k: v}` dict entry / dict comprehension body — container
    // wrapping the key/value expressions.
    NodeSpec { name: PAIR,     marker: false, container: true, syntax: Default },
    // `f"{n}"` — f-string interpolation segment. Container around the
    // embedded expression and its braces.
    NodeSpec { name: INTERPOLATION, marker: false, container: true, syntax: Default },

    // Function-signature separators — rename of the `keyword_separator`
    // / `positional_separator` kinds. Container (carries the `*` / `/`
    // text), not a marker.
    NodeSpec { name: KEYWORD,    marker: false, container: true, syntax: Default },
    NodeSpec { name: POSITIONAL, marker: false, container: true, syntax: Default },

    // Collection containers (structural). ALSO appear as pattern /
    // spread markers — so both marker and container are true.
    NodeSpec { name: LIST, marker: true, container: true, syntax: Default },
    NodeSpec { name: DICT, marker: true, container: true, syntax: Default },
    NodeSpec { name: SET,  marker: true, container: true, syntax: Default },

    // Literals
    NodeSpec { name: STRING, marker: false, container: true, syntax: String },
    NodeSpec { name: INT,    marker: false, container: true, syntax: Number },
    NodeSpec { name: FLOAT,  marker: false, container: true, syntax: Number },
    NodeSpec { name: TRUE,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FALSE,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: NONE,   marker: false, container: true, syntax: Keyword },

    // Identifiers / comments / op
    NodeSpec { name: NAME,    marker: false, container: true, syntax: Identifier },
    NodeSpec { name: COMMENT, marker: false, container: true, syntax: Comment },
    NodeSpec { name: OP,      marker: false, container: true, syntax: Operator },

    // Comment markers
    NodeSpec { name: TRAILING, marker: true, container: false, syntax: Default },
    NodeSpec { name: LEADING,  marker: true, container: false, syntax: Default },

    // Markers — always empty.
    NodeSpec { name: PUBLIC,    marker: true, container: false, syntax: Keyword },
    NodeSpec { name: PRIVATE,   marker: true, container: false, syntax: Keyword },
    NodeSpec { name: PROTECTED, marker: true, container: false, syntax: Keyword },
    NodeSpec { name: ASYNC,     marker: true, container: false, syntax: Keyword },
    NodeSpec { name: LITERAL,       marker: true, container: false, syntax: Keyword },
    NodeSpec { name: COMPREHENSION, marker: true, container: false, syntax: Keyword },
    NodeSpec { name: UNION,  marker: true, container: false, syntax: Default },
    NodeSpec { name: SPLAT,  marker: true, container: false, syntax: Default },
];

/// Tree-sitter kind catalogue — single source of truth for every
/// kind the Python transform handles. Sorted alphabetically by kind
/// name. See `KindHandling` for variants.
pub const KINDS: &[KindEntry] = &[
    KindEntry { kind: "aliased_import",                handling: KindHandling::Custom },
    KindEntry { kind: "argument_list",                 handling: KindHandling::Flatten },
    KindEntry { kind: "as_pattern",                    handling: KindHandling::Rename(AS) },
    KindEntry { kind: "as_pattern_target",             handling: KindHandling::Flatten },
    KindEntry { kind: "assert_statement",              handling: KindHandling::Rename(ASSERT) },
    KindEntry { kind: "assignment",                    handling: KindHandling::Rename(ASSIGN) },
    KindEntry { kind: "attribute",                     handling: KindHandling::Rename(MEMBER) },
    KindEntry { kind: "augmented_assignment",          handling: KindHandling::CustomThenRename(ASSIGN) },
    // Tree-sitter leaf for the `await` keyword expression.
    KindEntry { kind: "await",                         handling: KindHandling::Rename(AWAIT) },
    KindEntry { kind: "binary_operator",               handling: KindHandling::CustomThenRename(BINARY) },
    KindEntry { kind: "block",                         handling: KindHandling::Flatten },
    KindEntry { kind: "boolean_operator",              handling: KindHandling::CustomThenRename(LOGICAL) },
    KindEntry { kind: "break_statement",               handling: KindHandling::Rename(BREAK) },
    KindEntry { kind: "call",                          handling: KindHandling::Rename(CALL) },
    KindEntry { kind: "case_clause",                   handling: KindHandling::Custom },
    KindEntry { kind: "case_pattern",                  handling: KindHandling::Custom },
    KindEntry { kind: "class_definition",              handling: KindHandling::Rename(CLASS) },
    KindEntry { kind: "class_pattern",                 handling: KindHandling::RenameWithMarker(PATTERN, CLASS) },
    KindEntry { kind: "comment",                       handling: KindHandling::Custom },
    KindEntry { kind: "comparison_operator",           handling: KindHandling::CustomThenRename(COMPARE) },
    KindEntry { kind: "conditional_expression",        handling: KindHandling::Custom },
    KindEntry { kind: "continue_statement",            handling: KindHandling::Rename(CONTINUE) },
    KindEntry { kind: "decorated_definition",          handling: KindHandling::Custom },
    KindEntry { kind: "decorator",                     handling: KindHandling::Rename(DECORATOR) },
    KindEntry { kind: "default_parameter",             handling: KindHandling::Rename(PARAMETER) },
    KindEntry { kind: "delete_statement",              handling: KindHandling::Rename(DELETE) },
    KindEntry { kind: "dict_pattern",                  handling: KindHandling::RenameWithMarker(PATTERN, DICT) },
    KindEntry { kind: "dictionary",                    handling: KindHandling::Custom },
    KindEntry { kind: "dictionary_comprehension",      handling: KindHandling::Custom },
    KindEntry { kind: "dictionary_splat",              handling: KindHandling::RenameWithMarker(SPREAD, DICT) },
    KindEntry { kind: "dictionary_splat_pattern",      handling: KindHandling::RenameWithMarker(SPREAD, DICT) },
    KindEntry { kind: "dotted_name",                   handling: KindHandling::Flatten },
    KindEntry { kind: "elif_clause",                   handling: KindHandling::Rename(ELSE_IF) },
    KindEntry { kind: "else_clause",                   handling: KindHandling::Rename(ELSE) },
    KindEntry { kind: "escape_sequence",               handling: KindHandling::Flatten },
    KindEntry { kind: "except_clause",                 handling: KindHandling::Rename(EXCEPT) },
    KindEntry { kind: "expression_list",               handling: KindHandling::Flatten },
    KindEntry { kind: "expression_statement",          handling: KindHandling::Flatten },
    KindEntry { kind: "false",                         handling: KindHandling::Rename(FALSE) },
    KindEntry { kind: "finally_clause",                handling: KindHandling::Rename(FINALLY) },
    KindEntry { kind: "float",                         handling: KindHandling::Rename(FLOAT) },
    KindEntry { kind: "for_in_clause",                 handling: KindHandling::Flatten },
    KindEntry { kind: "for_statement",                 handling: KindHandling::Rename(FOR) },
    KindEntry { kind: "format_specifier",              handling: KindHandling::Rename(FORMAT) },
    KindEntry { kind: "function_definition",           handling: KindHandling::Custom },
    KindEntry { kind: "generator_expression",          handling: KindHandling::Rename(GENERATOR) },
    KindEntry { kind: "generic_type",                  handling: KindHandling::Custom },
    KindEntry { kind: "global_statement",              handling: KindHandling::Rename(GLOBAL) },
    KindEntry { kind: "identifier",                    handling: KindHandling::Custom },
    KindEntry { kind: "if_clause",                     handling: KindHandling::Flatten },
    KindEntry { kind: "if_statement",                  handling: KindHandling::Rename(IF) },
    KindEntry { kind: "import_from_statement",         handling: KindHandling::Rename(FROM) },
    KindEntry { kind: "import_prefix",                 handling: KindHandling::Flatten },
    KindEntry { kind: "import_statement",              handling: KindHandling::Rename(IMPORT) },
    KindEntry { kind: "integer",                       handling: KindHandling::Rename(INT) },
    // Inner `{expr}` of an f-string. Tree-sitter emits `interpolation`,
    // already matching our semantic vocabulary; pass through.
    KindEntry { kind: "interpolation",                 handling: KindHandling::PassThrough },
    KindEntry { kind: "keyword_argument",              handling: KindHandling::Custom },
    KindEntry { kind: "keyword_pattern",               handling: KindHandling::Custom },
    KindEntry { kind: "keyword_separator",             handling: KindHandling::Custom },
    KindEntry { kind: "lambda",                        handling: KindHandling::Rename(LAMBDA) },
    KindEntry { kind: "lambda_parameters",             handling: KindHandling::Flatten },
    KindEntry { kind: "list",                          handling: KindHandling::Custom },
    KindEntry { kind: "list_comprehension",            handling: KindHandling::Custom },
    KindEntry { kind: "list_pattern",                  handling: KindHandling::RenameWithMarker(PATTERN, LIST) },
    KindEntry { kind: "list_splat",                    handling: KindHandling::RenameWithMarker(SPREAD, LIST) },
    KindEntry { kind: "list_splat_pattern",            handling: KindHandling::RenameWithMarker(SPREAD, LIST) },
    KindEntry { kind: "match_statement",               handling: KindHandling::Rename(MATCH) },
    KindEntry { kind: "module",                        handling: KindHandling::Rename(MODULE) },
    KindEntry { kind: "named_expression",              handling: KindHandling::Rename(ASSIGN) },
    KindEntry { kind: "none",                          handling: KindHandling::Rename(NONE) },
    KindEntry { kind: "nonlocal_statement",            handling: KindHandling::Rename(NONLOCAL) },
    // `{a: 1}` entry in a dict literal — already named `pair`, matches
    // semantic vocabulary; pass through.
    KindEntry { kind: "pair",                          handling: KindHandling::PassThrough },
    KindEntry { kind: "parameters",                    handling: KindHandling::Flatten },
    KindEntry { kind: "parenthesized_expression",      handling: KindHandling::Flatten },
    KindEntry { kind: "pass_statement",                handling: KindHandling::Rename(PASS) },
    KindEntry { kind: "pattern_list",                  handling: KindHandling::Flatten },
    KindEntry { kind: "positional_separator",          handling: KindHandling::Custom },
    KindEntry { kind: "raise_statement",               handling: KindHandling::Rename(RAISE) },
    KindEntry { kind: "relative_import",               handling: KindHandling::Flatten },
    KindEntry { kind: "return_statement",              handling: KindHandling::Rename(RETURN) },
    KindEntry { kind: "set",                           handling: KindHandling::Custom },
    KindEntry { kind: "set_comprehension",             handling: KindHandling::Custom },
    KindEntry { kind: "splat_pattern",                 handling: KindHandling::RenameWithMarker(PATTERN, SPLAT) },
    KindEntry { kind: "string",                        handling: KindHandling::Rename(STRING) },
    KindEntry { kind: "string_content",                handling: KindHandling::Flatten },
    KindEntry { kind: "string_end",                    handling: KindHandling::Flatten },
    KindEntry { kind: "string_start",                  handling: KindHandling::Flatten },
    KindEntry { kind: "subscript",                     handling: KindHandling::Rename(SUBSCRIPT) },
    KindEntry { kind: "true",                          handling: KindHandling::Rename(TRUE) },
    KindEntry { kind: "try_statement",                 handling: KindHandling::Rename(TRY) },
    // Tree-sitter `tuple` leaf — `(1, 2)`. Already matches semantic
    // vocabulary as a container; pass through.
    KindEntry { kind: "tuple",                         handling: KindHandling::PassThrough },
    KindEntry { kind: "type",                          handling: KindHandling::Custom },
    KindEntry { kind: "type_conversion",               handling: KindHandling::Custom },
    KindEntry { kind: "type_parameter",                handling: KindHandling::Flatten },
    KindEntry { kind: "typed_default_parameter",       handling: KindHandling::Rename(PARAMETER) },
    KindEntry { kind: "typed_parameter",               handling: KindHandling::Rename(PARAMETER) },
    KindEntry { kind: "unary_operator",                handling: KindHandling::CustomThenRename(UNARY) },
    KindEntry { kind: "union_pattern",                 handling: KindHandling::RenameWithMarker(PATTERN, UNION) },
    KindEntry { kind: "union_type",                    handling: KindHandling::RenameWithMarker(TYPE, UNION) },
    KindEntry { kind: "while_statement",               handling: KindHandling::Rename(WHILE) },
    KindEntry { kind: "with_clause",                   handling: KindHandling::Flatten },
    KindEntry { kind: "with_item",                     handling: KindHandling::Flatten },
    KindEntry { kind: "with_statement",                handling: KindHandling::Rename(WITH) },
    KindEntry { kind: "yield",                         handling: KindHandling::Rename(YIELD) },
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

/// Look up a node spec by name. Linear scan — NODES is small and cold.
pub fn spec(name: &str) -> Option<&'static NodeSpec> {
    NODES.iter().find(|n| n.name == name)
}

/// Iterate every declared semantic name.
pub fn all_names() -> impl Iterator<Item = &'static str> {
    NODES.iter().map(|n| n.name)
}

/// True iff `name` is declared as a pure marker (never a container).
/// Dual-use names (marker=true AND container=true) return false.
pub fn is_marker_only(name: &str) -> bool {
    spec(name).map_or(false, |s| s.marker && !s.container)
}

/// True iff `name` is declared in this language's NODES table.
pub fn is_declared(name: &str) -> bool {
    spec(name).is_some()
}
