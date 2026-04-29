/// Semantic element names — tractor's Ruby XML vocabulary after transform.
/// Tree-sitter kind strings (left side of `match` arms) stay as bare
/// literals — they are external vocabulary.
use crate::languages::{KindEntry, KindHandling, NodeSpec};
use crate::output::syntax_highlight::SyntaxCategory;

// Named constants retained for use by the transform code. The NODES
// table below is the source of truth for marker/container role and
// syntax category.

// Top-level / declarations
pub const PROGRAM: &str = "program";
pub const MODULE: &str = "module";
pub const CLASS: &str = "class";
pub const METHOD: &str = "method";

// Statements / control flow
pub const IF: &str = "if";
pub const UNLESS: &str = "unless";
pub const ELSE: &str = "else";
pub const ELSE_IF: &str = "else_if";
pub const CASE: &str = "case";
pub const THEN: &str = "then";
pub const WHILE: &str = "while";
pub const UNTIL: &str = "until";
pub const FOR: &str = "for";
pub const BEGIN: &str = "begin";
pub const RESCUE: &str = "rescue";
pub const ENSURE: &str = "ensure";
pub const BREAK: &str = "break";
pub const CONTINUE: &str = "continue";

// Members / parameters
pub const PARAMETER: &str = "parameter";
pub const VARIABLE: &str = "variable";

// Expressions
pub const CALL: &str = "call";
pub const ASSIGN: &str = "assign";
pub const BINARY: &str = "binary";
pub const UNARY: &str = "unary";
pub const CONDITIONAL: &str = "conditional";
pub const RANGE: &str = "range";
pub const LAMBDA: &str = "lambda";
pub const YIELD: &str = "yield";
pub const SPREAD: &str = "spread";
pub const LEFT: &str = "left";

// Pattern-matching (case/in).
pub const WHEN: &str = "when";
pub const IN: &str = "in";
pub const PATTERN: &str = "pattern";

// Control-flow keyword leaves.
pub const NEXT: &str = "next";
pub const REDO: &str = "redo";
pub const RETRY: &str = "retry";

// Rescue / class header metadata.
pub const EXCEPTIONS: &str = "exceptions";
pub const SUPERCLASS: &str = "superclass";

// Collections / atoms
pub const ARRAY: &str = "array";
pub const HASH: &str = "hash";
pub const PAIR: &str = "pair";
pub const STRING: &str = "string";
pub const INTERPOLATION: &str = "interpolation";
pub const SYMBOL: &str = "symbol";
pub const INT: &str = "int";
pub const FLOAT: &str = "float";
pub const REGEX: &str = "regex";

// Literal atoms.
pub const TRUE: &str = "true";
pub const FALSE: &str = "false";
pub const NIL: &str = "nil";
pub const SELF: &str = "self";

// Identifiers
pub const NAME: &str = "name";
pub const CONSTANT: &str = "constant";
pub const COMMENT: &str = "comment";

// Comment markers — emitted by the shared CommentClassifier.
pub const TRAILING: &str = "trailing";
pub const LEADING: &str = "leading";

// Spread-shape markers.
pub const LIST: &str = "list";
pub const DICT: &str = "dict";

// Parameter-shape markers.
pub const KEYWORD: &str = "keyword";
pub const DEFAULT: &str = "default";

// Block-shape / dual-use markers.
pub const DO: &str = "do";

// Symbol-shape marker.
pub const DELIMITED: &str = "delimited";

// Class / method singleton markers.
pub const SINGLETON: &str = "singleton";

// Dual-use (block container + `<parameter><block/>` marker).
pub const BLOCK: &str = "block";

use SyntaxCategory::*;

/// Per-name metadata — single source of truth for every element
/// name this language's transform can emit.
///
/// Dual-use names set BOTH `marker: true` and `container: true`:
///   - STRING — `<string>` literal container + `<array><string/>` shape marker.
///   - SYMBOL — `<symbol>` literal container + `<array><symbol/>` shape marker.
///   - BLOCK  — `<block>` container (do/begin blocks) +
///              `<parameter><block/>` shape marker.
///   - BEGIN  — `<begin>` container + `<block><begin/>` marker.
///   - DO     — `<block><do/>` marker + structural `do` container
///              (body of while/until/for loops).
pub const NODES: &[NodeSpec] = &[
    // Top-level / declarations
    NodeSpec { name: PROGRAM, marker: false, container: true, syntax: Default },
    NodeSpec { name: MODULE,  marker: false, container: true, syntax: Default },
    NodeSpec { name: CLASS,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: METHOD,  marker: false, container: true, syntax: Keyword },

    // Statements / control flow (BEGIN dual-use)
    NodeSpec { name: IF,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: UNLESS,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ELSE,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ELSE_IF,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CASE,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: THEN,     marker: false, container: true, syntax: Default },
    NodeSpec { name: WHILE,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: UNTIL,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FOR,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: BEGIN,    marker: true,  container: true, syntax: Keyword },
    NodeSpec { name: RESCUE,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ENSURE,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: BREAK,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CONTINUE, marker: false, container: true, syntax: Default },

    // Members / parameters
    NodeSpec { name: PARAMETER, marker: false, container: true, syntax: Default },
    NodeSpec { name: VARIABLE,  marker: false, container: true, syntax: Default },

    // Expressions
    NodeSpec { name: CALL,        marker: false, container: true, syntax: Function },
    NodeSpec { name: ASSIGN,      marker: false, container: true, syntax: Operator },
    NodeSpec { name: BINARY,      marker: false, container: true, syntax: Operator },
    NodeSpec { name: UNARY,       marker: false, container: true, syntax: Operator },
    NodeSpec { name: CONDITIONAL, marker: false, container: true, syntax: Default },
    NodeSpec { name: RANGE,       marker: false, container: true, syntax: Default },
    NodeSpec { name: LAMBDA,      marker: false, container: true, syntax: Function },
    NodeSpec { name: YIELD,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: SPREAD,      marker: false, container: true, syntax: Default },
    NodeSpec { name: LEFT,        marker: false, container: true, syntax: Default },

    // Pattern-matching
    NodeSpec { name: WHEN,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: IN,      marker: false, container: true, syntax: Default },
    NodeSpec { name: PATTERN, marker: false, container: true, syntax: Default },

    // Control-flow keyword leaves
    NodeSpec { name: NEXT,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: REDO,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: RETRY, marker: false, container: true, syntax: Keyword },

    // Rescue / class header metadata
    NodeSpec { name: EXCEPTIONS, marker: false, container: true, syntax: Default },
    NodeSpec { name: SUPERCLASS, marker: false, container: true, syntax: Default },

    // Collections / atoms (STRING, SYMBOL dual-use)
    NodeSpec { name: ARRAY,         marker: false, container: true, syntax: Type },
    NodeSpec { name: HASH,          marker: false, container: true, syntax: Type },
    NodeSpec { name: PAIR,          marker: false, container: true, syntax: Default },
    NodeSpec { name: STRING,        marker: true,  container: true, syntax: String },
    NodeSpec { name: INTERPOLATION, marker: false, container: true, syntax: Default },
    NodeSpec { name: SYMBOL,        marker: true,  container: true, syntax: String },
    NodeSpec { name: INT,           marker: false, container: true, syntax: Number },
    NodeSpec { name: FLOAT,         marker: false, container: true, syntax: Number },
    NodeSpec { name: REGEX,         marker: false, container: true, syntax: Default },

    // Literal atoms
    NodeSpec { name: TRUE,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FALSE, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: NIL,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: SELF,  marker: false, container: true, syntax: Keyword },

    // Identifiers
    NodeSpec { name: NAME,     marker: false, container: true, syntax: Identifier },
    NodeSpec { name: CONSTANT, marker: false, container: true, syntax: Default },
    NodeSpec { name: COMMENT,  marker: false, container: true, syntax: Comment },

    // Comment markers
    NodeSpec { name: TRAILING, marker: true, container: false, syntax: Default },
    NodeSpec { name: LEADING,  marker: true, container: false, syntax: Default },

    // Spread-shape markers
    NodeSpec { name: LIST, marker: true, container: false, syntax: Default },
    NodeSpec { name: DICT, marker: true, container: false, syntax: Default },

    // Parameter-shape markers
    NodeSpec { name: KEYWORD, marker: true, container: false, syntax: Default },
    NodeSpec { name: DEFAULT, marker: true, container: false, syntax: Default },

    // Block-shape / dual-use: DO is both marker (on block) and
    // container (loop body).
    NodeSpec { name: DO, marker: true, container: true, syntax: Keyword },

    // Symbol-shape marker
    NodeSpec { name: DELIMITED, marker: true, container: false, syntax: Default },

    // Class / method singleton markers
    NodeSpec { name: SINGLETON, marker: true, container: false, syntax: Default },

    // Dual-use: block container + `<parameter><block/>` marker.
    NodeSpec { name: BLOCK, marker: true, container: true, syntax: Default },
];

/// Tree-sitter kind catalogue — single source of truth for every
/// kind the Ruby transform handles. Sorted alphabetically by kind
/// name. See `KindHandling` for variants.
pub const KINDS: &[KindEntry] = &[
    KindEntry { kind: "argument_list",                 handling: KindHandling::Flatten },
    KindEntry { kind: "array",                         handling: KindHandling::Rename(ARRAY) },
    KindEntry { kind: "assignment",                    handling: KindHandling::Rename(ASSIGN) },
    KindEntry { kind: "bare_string",                   handling: KindHandling::Flatten },
    KindEntry { kind: "bare_symbol",                   handling: KindHandling::Flatten },
    KindEntry { kind: "begin",                         handling: KindHandling::Rename(BEGIN) },
    KindEntry { kind: "begin_block",                   handling: KindHandling::RenameWithMarker(BLOCK, BEGIN) },
    KindEntry { kind: "binary",                        handling: KindHandling::Rename(BINARY) },
    KindEntry { kind: "block",                         handling: KindHandling::PassThrough },
    KindEntry { kind: "block_body",                    handling: KindHandling::Flatten },
    KindEntry { kind: "block_parameter",               handling: KindHandling::RenameWithMarker(PARAMETER, BLOCK) },
    KindEntry { kind: "block_parameters",              handling: KindHandling::Flatten },
    KindEntry { kind: "body_statement",                handling: KindHandling::Flatten },
    // Tree-sitter leaves — keyword tokens that already match our
    // semantic vocabulary as marker/leaf tokens; pass through.
    KindEntry { kind: "break",                         handling: KindHandling::PassThrough },
    KindEntry { kind: "call",                          handling: KindHandling::Rename(CALL) },
    KindEntry { kind: "case",                          handling: KindHandling::Rename(CASE) },
    KindEntry { kind: "class",                         handling: KindHandling::Rename(CLASS) },
    KindEntry { kind: "comment",                       handling: KindHandling::Custom },
    KindEntry { kind: "conditional",                   handling: KindHandling::PassThrough },
    KindEntry { kind: "constant",                      handling: KindHandling::PassThrough },
    KindEntry { kind: "delimited_symbol",              handling: KindHandling::RenameWithMarker(SYMBOL, DELIMITED) },
    KindEntry { kind: "do",                            handling: KindHandling::PassThrough },
    KindEntry { kind: "do_block",                      handling: KindHandling::RenameWithMarker(BLOCK, DO) },
    KindEntry { kind: "else",                          handling: KindHandling::Rename(ELSE) },
    KindEntry { kind: "elsif",                         handling: KindHandling::Rename(ELSE_IF) },
    KindEntry { kind: "ensure",                        handling: KindHandling::Rename(ENSURE) },
    KindEntry { kind: "escape_sequence",               handling: KindHandling::Flatten },
    KindEntry { kind: "exception_variable",            handling: KindHandling::Rename(VARIABLE) },
    KindEntry { kind: "exceptions",                    handling: KindHandling::PassThrough },
    KindEntry { kind: "false",                         handling: KindHandling::PassThrough },
    KindEntry { kind: "for",                           handling: KindHandling::Rename(FOR) },
    KindEntry { kind: "global_variable",               handling: KindHandling::Custom },
    KindEntry { kind: "hash",                          handling: KindHandling::Rename(HASH) },
    KindEntry { kind: "hash_key_symbol",               handling: KindHandling::Flatten },
    KindEntry { kind: "hash_splat_argument",           handling: KindHandling::RenameWithMarker(SPREAD, DICT) },
    KindEntry { kind: "hash_splat_parameter",          handling: KindHandling::RenameWithMarker(SPREAD, DICT) },
    KindEntry { kind: "heredoc_beginning",             handling: KindHandling::Flatten },
    KindEntry { kind: "heredoc_body",                  handling: KindHandling::Flatten },
    KindEntry { kind: "heredoc_content",               handling: KindHandling::Flatten },
    KindEntry { kind: "heredoc_end",                   handling: KindHandling::Flatten },
    KindEntry { kind: "identifier",                    handling: KindHandling::Custom },
    KindEntry { kind: "if",                            handling: KindHandling::Rename(IF) },
    KindEntry { kind: "if_modifier",                   handling: KindHandling::Custom },
    KindEntry { kind: "in",                            handling: KindHandling::PassThrough },
    KindEntry { kind: "instance_variable",             handling: KindHandling::Custom },
    KindEntry { kind: "class_variable",                handling: KindHandling::Custom },
    KindEntry { kind: "integer",                       handling: KindHandling::Rename(INT) },
    // f-string-style interpolation `#{…}` — already named `interpolation`.
    KindEntry { kind: "interpolation",                 handling: KindHandling::PassThrough },
    KindEntry { kind: "keyword_parameter",             handling: KindHandling::RenameWithMarker(PARAMETER, KEYWORD) },
    KindEntry { kind: "lambda",                        handling: KindHandling::PassThrough },
    KindEntry { kind: "lambda_parameters",             handling: KindHandling::Flatten },
    KindEntry { kind: "left_assignment_list",          handling: KindHandling::Rename(LEFT) },
    KindEntry { kind: "method",                        handling: KindHandling::Rename(METHOD) },
    KindEntry { kind: "method_parameters",             handling: KindHandling::Flatten },
    KindEntry { kind: "module",                        handling: KindHandling::Rename(MODULE) },
    KindEntry { kind: "next",                          handling: KindHandling::PassThrough },
    KindEntry { kind: "nil",                           handling: KindHandling::PassThrough },
    KindEntry { kind: "operator",                      handling: KindHandling::PassThrough },
    KindEntry { kind: "operator_assignment",           handling: KindHandling::Rename(ASSIGN) },
    KindEntry { kind: "optional_parameter",            handling: KindHandling::RenameWithMarker(PARAMETER, DEFAULT) },
    // `{ a: 1 }` entry — already named `pair`, matches semantic vocabulary.
    KindEntry { kind: "pair",                          handling: KindHandling::PassThrough },
    KindEntry { kind: "parenthesized_statements",      handling: KindHandling::Flatten },
    KindEntry { kind: "pattern",                       handling: KindHandling::PassThrough },
    KindEntry { kind: "program",                       handling: KindHandling::Rename(PROGRAM) },
    KindEntry { kind: "range",                         handling: KindHandling::PassThrough },
    KindEntry { kind: "redo",                          handling: KindHandling::PassThrough },
    KindEntry { kind: "regex",                         handling: KindHandling::PassThrough },
    KindEntry { kind: "rescue",                        handling: KindHandling::Rename(RESCUE) },
    KindEntry { kind: "rest_assignment",               handling: KindHandling::Rename(SPREAD) },
    KindEntry { kind: "retry",                         handling: KindHandling::PassThrough },
    KindEntry { kind: "self",                          handling: KindHandling::PassThrough },
    KindEntry { kind: "simple_symbol",                 handling: KindHandling::Flatten },
    KindEntry { kind: "singleton_class",               handling: KindHandling::RenameWithMarker(CLASS, SINGLETON) },
    KindEntry { kind: "singleton_method",              handling: KindHandling::RenameWithMarker(METHOD, SINGLETON) },
    KindEntry { kind: "splat_argument",                handling: KindHandling::RenameWithMarker(SPREAD, LIST) },
    KindEntry { kind: "splat_parameter",               handling: KindHandling::RenameWithMarker(SPREAD, LIST) },
    KindEntry { kind: "string",                        handling: KindHandling::Rename(STRING) },
    KindEntry { kind: "string_array",                  handling: KindHandling::RenameWithMarker(ARRAY, STRING) },
    KindEntry { kind: "string_content",                handling: KindHandling::Flatten },
    KindEntry { kind: "superclass",                    handling: KindHandling::PassThrough },
    KindEntry { kind: "symbol_array",                  handling: KindHandling::RenameWithMarker(ARRAY, SYMBOL) },
    KindEntry { kind: "then",                          handling: KindHandling::PassThrough },
    KindEntry { kind: "true",                          handling: KindHandling::PassThrough },
    KindEntry { kind: "unary",                         handling: KindHandling::PassThrough },
    KindEntry { kind: "unless",                        handling: KindHandling::Rename(UNLESS) },
    KindEntry { kind: "unless_modifier",               handling: KindHandling::Custom },
    KindEntry { kind: "until",                         handling: KindHandling::Rename(UNTIL) },
    KindEntry { kind: "until_modifier",                handling: KindHandling::Custom },
    KindEntry { kind: "when",                          handling: KindHandling::PassThrough },
    KindEntry { kind: "while",                         handling: KindHandling::Rename(WHILE) },
    KindEntry { kind: "while_modifier",                handling: KindHandling::Custom },
    KindEntry { kind: "yield",                         handling: KindHandling::PassThrough },
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
