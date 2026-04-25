/// Semantic element names — tractor's Python XML vocabulary after transform.
use crate::languages::NodeSpec;
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
