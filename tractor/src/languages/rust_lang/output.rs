/// Output element names — tractor's Rust XML vocabulary after transform.
/// These are the names that appear in tractor's output. The tree-sitter
/// kind strings are external vocabulary, surfaced as the typed
/// [`super::input::RustKind`] enum. The kind→output table lives in
/// [`super::rules::rule`].
use crate::languages::NodeSpec;
use crate::output::syntax_highlight::SyntaxCategory;

// Named constants retained for use by the transform code. The NODES
// table below is the source of truth for marker/container role and
// syntax category.

// Top-level / declarations
pub const FILE: &str = "file";
pub const FUNCTION: &str = "function";
pub const IMPL: &str = "impl";
pub const STRUCT: &str = "struct";
pub const ENUM: &str = "enum";
pub const TRAIT: &str = "trait";
pub const MOD: &str = "mod";
pub const USE: &str = "use";
pub const CONST: &str = "const";
pub const STATIC: &str = "static";
pub const ALIAS: &str = "alias";
pub const SIGNATURE: &str = "signature";
pub const MODIFIERS: &str = "modifiers";

// Members
pub const PARAMETER: &str = "parameter";
pub const SELF: &str = "self";
pub const FIELD: &str = "field";
pub const VARIANT: &str = "variant";
pub const LIFETIME: &str = "lifetime";
pub const ATTRIBUTE: &str = "attribute";

// Types / generics
pub const TYPE: &str = "type";
pub const GENERIC: &str = "generic";
pub const GENERICS: &str = "generics";
pub const PATH: &str = "path";
pub const BOUNDS: &str = "bounds";
pub const BOUND: &str = "bound";
pub const WHERE: &str = "where";

// Statements / control flow
pub const LET: &str = "let";
pub const RETURN: &str = "return";
pub const IF: &str = "if";
pub const ELSE: &str = "else";
pub const ELSE_IF: &str = "else_if";
pub const FOR: &str = "for";
pub const WHILE: &str = "while";
pub const LOOP: &str = "loop";
pub const MATCH: &str = "match";
pub const ARM: &str = "arm";
pub const PATTERN: &str = "pattern";
pub const BREAK: &str = "break";
pub const CONTINUE: &str = "continue";
pub const RANGE: &str = "range";
pub const SEND: &str = "send";
pub const LABEL: &str = "label";

// Expressions
pub const CALL: &str = "call";
pub const INDEX: &str = "index";
pub const BINARY: &str = "binary";
pub const UNARY: &str = "unary";
pub const ASSIGN: &str = "assign";
pub const CLOSURE: &str = "closure";
pub const AWAIT: &str = "await";
pub const TRY: &str = "try";
pub const MACRO: &str = "macro";
pub const CAST: &str = "cast";
pub const REF: &str = "ref";
pub const TUPLE: &str = "tuple";
pub const UNSAFE: &str = "unsafe";
pub const LITERAL: &str = "literal";
pub const BLOCK: &str = "block";

// Visibility
pub const PUB: &str = "pub";
pub const IN: &str = "in";

// Literals / atoms
pub const STRING: &str = "string";
pub const INT: &str = "int";
pub const FLOAT: &str = "float";
pub const BOOL: &str = "bool";
pub const CHAR: &str = "char";

// Identifiers / comments / op
pub const NAME: &str = "name";
pub const COMMENT: &str = "comment";
pub const OP: &str = "op";

// Comment markers — emitted by the shared CommentClassifier.
pub const TRAILING: &str = "trailing";
pub const LEADING: &str = "leading";

// Marker-only names.
pub const RAW: &str = "raw";
pub const INNER: &str = "inner";
pub const BORROWED: &str = "borrowed";
pub const PRIVATE: &str = "private";
pub const CRATE: &str = "crate";
pub const SUPER: &str = "super";
pub const MUT: &str = "mut";
pub const ASYNC: &str = "async";
pub const POINTER: &str = "pointer";
pub const NEVER: &str = "never";
pub const UNIT: &str = "unit";
pub const DYNAMIC: &str = "dynamic";
pub const ABSTRACT: &str = "abstract";
pub const ASSOCIATED: &str = "associated";
pub const BOUNDED: &str = "bounded";
pub const ARRAY: &str = "array";
pub const OR: &str = "or";
pub const METHOD: &str = "method";
pub const BASE: &str = "base";

// Slice marker (marker-only in emitted code).
pub const SLICE: &str = "slice";

use SyntaxCategory::*;

/// Per-name metadata — single source of truth for every element
/// name this language's transform can emit.
///
/// Dual-use names set BOTH `marker: true` and `container: true`:
///   - FUNCTION — function_item (container) vs function_type (marker)
///   - TUPLE    — tuple_expression (container) vs tuple_type (marker)
///   - TRAIT    — trait_item (container) vs trait_type (marker)
///   - REF      — reference_expression (container) vs ref_pattern (marker)
///   - FIELD    — field_expression / field_declaration (container) vs
///                field_pattern / base_field_initializer (markers)
///   - STRUCT   — struct_item (container) vs struct_pattern (marker)
///   - GENERIC  — generic_type (container) vs generic_function (marker)
///   - CONST    — const_item (container) vs const_block (marker)
///   - TRY      — try_expression (container) vs try_block (marker)
pub const NODES: &[NodeSpec] = &[
    // Top-level / declarations (FUNCTION, STRUCT, TRAIT, CONST are dual-use)
    NodeSpec { name: FILE,      marker: false, container: true, syntax: Default },
    NodeSpec { name: FUNCTION,  marker: true,  container: true, syntax: Keyword },
    NodeSpec { name: IMPL,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: STRUCT,    marker: true,  container: true, syntax: Keyword },
    NodeSpec { name: ENUM,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: TRAIT,     marker: true,  container: true, syntax: Keyword },
    NodeSpec { name: MOD,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: USE,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CONST,     marker: true,  container: true, syntax: Keyword },
    NodeSpec { name: STATIC,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ALIAS,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: SIGNATURE, marker: false, container: true, syntax: Default },
    NodeSpec { name: MODIFIERS, marker: false, container: true, syntax: Default },

    // Members (FIELD is dual-use)
    NodeSpec { name: PARAMETER, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: SELF,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FIELD,     marker: true,  container: true, syntax: Default },
    NodeSpec { name: VARIANT,   marker: false, container: true, syntax: Default },
    NodeSpec { name: LIFETIME,  marker: false, container: true, syntax: Default },
    NodeSpec { name: ATTRIBUTE, marker: false, container: true, syntax: Default },

    // Types / generics (GENERIC is dual-use)
    NodeSpec { name: TYPE,     marker: false, container: true, syntax: Type },
    NodeSpec { name: GENERIC,  marker: true,  container: true, syntax: Type },
    NodeSpec { name: GENERICS, marker: false, container: true, syntax: Default },
    NodeSpec { name: PATH,     marker: false, container: true, syntax: Type },
    NodeSpec { name: BOUNDS,   marker: false, container: true, syntax: Default },
    NodeSpec { name: BOUND,    marker: false, container: true, syntax: Default },
    NodeSpec { name: WHERE,    marker: false, container: true, syntax: Default },

    // Statements / control flow
    NodeSpec { name: LET,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: RETURN,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: IF,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ELSE,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ELSE_IF,  marker: false, container: true, syntax: Default },
    NodeSpec { name: FOR,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: WHILE,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: LOOP,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: MATCH,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ARM,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: PATTERN,  marker: false, container: true, syntax: Default },
    NodeSpec { name: BREAK,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CONTINUE, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: RANGE,    marker: false, container: true, syntax: Default },
    NodeSpec { name: SEND,     marker: false, container: true, syntax: Default },
    NodeSpec { name: LABEL,    marker: false, container: true, syntax: Default },

    // Expressions (TRY, REF are dual-use)
    NodeSpec { name: CALL,    marker: false, container: true, syntax: Function },
    NodeSpec { name: INDEX,   marker: false, container: true, syntax: Default },
    NodeSpec { name: BINARY,  marker: false, container: true, syntax: Operator },
    NodeSpec { name: UNARY,   marker: false, container: true, syntax: Operator },
    NodeSpec { name: ASSIGN,  marker: false, container: true, syntax: Default },
    NodeSpec { name: CLOSURE, marker: false, container: true, syntax: Function },
    NodeSpec { name: AWAIT,   marker: false, container: true, syntax: Default },
    NodeSpec { name: TRY,     marker: true,  container: true, syntax: Operator },
    NodeSpec { name: MACRO,   marker: false, container: true, syntax: Function },
    NodeSpec { name: CAST,    marker: false, container: true, syntax: Default },
    NodeSpec { name: REF,     marker: true,  container: true, syntax: Type },
    NodeSpec { name: TUPLE,   marker: true,  container: true, syntax: Default },
    NodeSpec { name: UNSAFE,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: LITERAL, marker: false, container: true, syntax: Default },
    NodeSpec { name: BLOCK,   marker: false, container: true, syntax: Default },

    // Visibility
    NodeSpec { name: PUB, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: IN,  marker: false, container: true, syntax: Default },

    // Literals / atoms
    NodeSpec { name: STRING, marker: false, container: true, syntax: String },
    NodeSpec { name: INT,    marker: false, container: true, syntax: Number },
    NodeSpec { name: FLOAT,  marker: false, container: true, syntax: Number },
    NodeSpec { name: BOOL,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CHAR,   marker: false, container: true, syntax: Default },

    // Identifiers / comments / op
    NodeSpec { name: NAME,    marker: false, container: true, syntax: Identifier },
    NodeSpec { name: COMMENT, marker: false, container: true, syntax: Comment },
    NodeSpec { name: OP,      marker: false, container: true, syntax: Operator },

    // Comment markers
    NodeSpec { name: TRAILING, marker: true, container: false, syntax: Default },
    NodeSpec { name: LEADING,  marker: true, container: false, syntax: Default },

    // Marker-only
    NodeSpec { name: RAW,        marker: true, container: false, syntax: Default },
    NodeSpec { name: INNER,      marker: true, container: false, syntax: Default },
    NodeSpec { name: BORROWED,   marker: true, container: false, syntax: Default },
    NodeSpec { name: PRIVATE,    marker: true, container: false, syntax: Keyword },
    NodeSpec { name: CRATE,      marker: true, container: false, syntax: Keyword },
    NodeSpec { name: SUPER,      marker: true, container: false, syntax: Keyword },
    NodeSpec { name: MUT,        marker: true, container: false, syntax: Keyword },
    NodeSpec { name: ASYNC,      marker: true, container: false, syntax: Keyword },
    NodeSpec { name: POINTER,    marker: true, container: false, syntax: Default },
    NodeSpec { name: NEVER,      marker: true, container: false, syntax: Default },
    NodeSpec { name: UNIT,       marker: true, container: false, syntax: Default },
    NodeSpec { name: DYNAMIC,    marker: true, container: false, syntax: Default },
    NodeSpec { name: ABSTRACT,   marker: true, container: false, syntax: Default },
    NodeSpec { name: ASSOCIATED, marker: true, container: false, syntax: Default },
    NodeSpec { name: BOUNDED,    marker: true, container: false, syntax: Default },
    NodeSpec { name: ARRAY,      marker: true, container: false, syntax: Default },
    NodeSpec { name: OR,         marker: true, container: false, syntax: Default },
    NodeSpec { name: METHOD,     marker: true, container: false, syntax: Default },
    NodeSpec { name: BASE,       marker: true, container: false, syntax: Default },
    NodeSpec { name: SLICE,      marker: true, container: false, syntax: Default },
];

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
