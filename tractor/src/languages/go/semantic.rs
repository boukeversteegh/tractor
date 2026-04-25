/// Semantic element names — tractor's Go XML vocabulary after transform.
/// These are the names that appear in tractor's output. The tree-sitter
/// kind strings (left side of `match` arms, arguments to `get_kind`)
/// are external vocabulary and stay as bare strings.
use crate::languages::NodeSpec;
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

    // Marker-only
    NodeSpec { name: RAW,        marker: true, container: false, syntax: Default },
    NodeSpec { name: SHORT,      marker: true, container: false, syntax: Default },
    NodeSpec { name: EXPORTED,   marker: true, container: false, syntax: Keyword },
    NodeSpec { name: UNEXPORTED, marker: true, container: false, syntax: Keyword },
    NodeSpec { name: NEGATED,    marker: true, container: false, syntax: Default },
    NodeSpec { name: GENERIC,    marker: true, container: false, syntax: Default },
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
