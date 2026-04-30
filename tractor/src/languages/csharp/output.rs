/// Output element names — tractor's C# XML vocabulary after transform.
/// These are the names that appear in tractor's output and that the
/// renderer reads. The tree-sitter kind strings are external vocabulary,
/// surfaced as the typed [`super::input::CsKind`] enum. The kind→output
/// table lives in [`super::rules::rule`].
use crate::languages::NodeSpec;
use crate::output::syntax_highlight::SyntaxCategory;

// Named constants retained for use by the transform code. The NODES
// table below is the source of truth for marker/container role and
// syntax category.

// Top-level / structural
pub const UNIT: &str = "unit";
pub const NAMESPACE: &str = "namespace";
pub const IMPORT: &str = "import";
pub const BODY: &str = "body";

// Type declarations
pub const CLASS: &str = "class";
pub const STRUCT: &str = "struct";
pub const INTERFACE: &str = "interface";
pub const ENUM: &str = "enum";
pub const RECORD: &str = "record";

// Members
pub const METHOD: &str = "method";
pub const CONSTRUCTOR: &str = "constructor";
pub const PROPERTY: &str = "property";
pub const FIELD: &str = "field";
pub const COMMENT: &str = "comment";
pub const EVENT: &str = "event";
pub const DELEGATE: &str = "delegate";
pub const DESTRUCTOR: &str = "destructor";
pub const INDEXER: &str = "indexer";
pub const OPERATOR: &str = "operator";

// Shared children
pub const NAME: &str = "name";
pub const TYPE: &str = "type";
pub const ACCESSORS: &str = "accessors";
pub const ACCESSOR: &str = "accessor";
pub const ATTRIBUTES: &str = "attributes";
pub const ATTRIBUTE: &str = "attribute";
pub const ARGUMENTS: &str = "arguments";
pub const ARGUMENT: &str = "argument";
pub const PARAMETERS: &str = "parameters";
pub const PARAMETER: &str = "parameter";
pub const VARIABLE: &str = "variable";
pub const DECLARATOR: &str = "declarator";
pub const EXTENDS: &str = "extends";
pub const PROPERTIES: &str = "properties";
pub const ELEMENT: &str = "element";
pub const SECTION: &str = "section";
pub const ARM: &str = "arm";
pub const LABEL: &str = "label";
pub const CHAIN: &str = "chain";
pub const FILTER: &str = "filter";
pub const WHEN: &str = "when";
pub const WHERE: &str = "where";

// Statements / control flow
pub const RETURN: &str = "return";
pub const IF: &str = "if";
pub const ELSE: &str = "else";
pub const ELSE_IF: &str = "else_if";
pub const FOR: &str = "for";
pub const FOREACH: &str = "foreach";
pub const WHILE: &str = "while";
pub const DO: &str = "do";
pub const TRY: &str = "try";
pub const CATCH: &str = "catch";
pub const FINALLY: &str = "finally";
pub const THROW: &str = "throw";
pub const USING: &str = "using";
pub const BREAK: &str = "break";
pub const CONTINUE: &str = "continue";
pub const SWITCH: &str = "switch";
pub const BLOCK: &str = "block";
pub const EXPRESSION: &str = "expression";
pub const RANGE: &str = "range";

// Expressions
pub const CALL: &str = "call";
pub const MEMBER: &str = "member";
pub const NEW: &str = "new";
pub const ASSIGN: &str = "assign";
pub const BINARY: &str = "binary";
pub const UNARY: &str = "unary";
pub const LAMBDA: &str = "lambda";
pub const AWAIT: &str = "await";
pub const TERNARY: &str = "ternary";
pub const INDEX: &str = "index";
pub const IS: &str = "is";
pub const TUPLE: &str = "tuple";
pub const LITERAL: &str = "literal";
pub const PATTERN: &str = "pattern";

// Generics
pub const GENERIC: &str = "generic";

// LINQ
pub const QUERY: &str = "query";
pub const FROM: &str = "from";
pub const SELECT: &str = "select";
pub const ORDER: &str = "order";
pub const GROUP: &str = "group";
pub const LET: &str = "let";
pub const JOIN: &str = "join";
pub const ORDERING: &str = "ordering";

// Enum members — `enum_member_declaration` → `<constant>`. Also
// appears as a pattern marker (`constant_pattern`) — dual-use, see
// NODES below.
pub const CONSTANT: &str = "constant";
// `catch_declaration` → `<declaration>`. Also used as a pattern
// marker — dual-use, see NODES below.
pub const DECLARATION: &str = "declaration";

// Literals / atoms
pub const STRING: &str = "string";
pub const INTERPOLATION: &str = "interpolation";
pub const INT: &str = "int";
pub const FLOAT: &str = "float";
pub const BOOL: &str = "bool";
pub const NULL: &str = "null";

// Patterns — `subpattern` is a property-pattern clause `{ Name: X }`
// emitted as a container around its member/value pair.
pub const SUBPATTERN: &str = "subpattern";

// `_` — discard pattern in switch arms / deconstructions. Leaf
// container that carries its underscore text.
pub const DISCARD: &str = "discard";

// Unknown modifier passthrough — when the grammar wraps a keyword
// we don't recognise in `<modifier>…</modifier>` the wrapper
// survives as a container (e.g. `file` on `file sealed class`).
pub const MODIFIER: &str = "modifier";

// Operator child
pub const OP: &str = "op";

// Type markers
pub const NULLABLE: &str = "nullable";

// Comment markers
pub const TRAILING: &str = "trailing";
pub const LEADING: &str = "leading";

// Member-access / pattern / type shape markers.
pub const INSTANCE: &str = "instance";
pub const CONDITIONAL: &str = "conditional";
pub const ARRAY: &str = "array";
pub const POINTER: &str = "pointer";
pub const FUNCTION: &str = "function";
pub const REF: &str = "ref";
pub const RECURSIVE: &str = "recursive";
pub const RELATIONAL: &str = "relational";
pub const LOGICAL: &str = "logical";
pub const PREFIX: &str = "prefix";
pub const LOOKUP: &str = "lookup";

// Access modifiers — markers only.
pub const PUBLIC: &str = "public";
pub const PRIVATE: &str = "private";
pub const PROTECTED: &str = "protected";
pub const INTERNAL: &str = "internal";

// Other modifiers — markers only.
pub const STATIC: &str = "static";
pub const ABSTRACT: &str = "abstract";
pub const VIRTUAL: &str = "virtual";
pub const OVERRIDE: &str = "override";
pub const SEALED: &str = "sealed";
pub const READONLY: &str = "readonly";
pub const CONST: &str = "const";
pub const PARTIAL: &str = "partial";
pub const ASYNC: &str = "async";
pub const EXTERN: &str = "extern";
pub const UNSAFE: &str = "unsafe";
// NEW above doubles as a structural container (`object_creation_expression`)
// and a modifier marker — dual-use, see NODES below.
pub const THIS: &str = "this";

// Accessor declarations. These are containers when emitted from C#
// property/event accessor declarations.
pub const GET: &str = "get";
pub const SET: &str = "set";
pub const INIT: &str = "init";
pub const ADD: &str = "add";
pub const REMOVE: &str = "remove";

// Generic-constraint markers — emitted by `attach_where_clause_constraints`
// in `languages/mod.rs`.
pub const NOTNULL: &str = "notnull";
pub const UNMANAGED: &str = "unmanaged";

use SyntaxCategory::*;

/// Per-name metadata — single source of truth for every element
/// name this language's transform can emit.
///
/// Dual-use names set BOTH `marker: true` and `container: true`:
///   - NEW      — container for `object_creation_expression`; marker for
///                the `new` modifier.
///   - TUPLE    — container for tuple expressions; marker for tuple
///                patterns / type-shape.
///   - CONSTANT — container for enum members; marker for
///                `constant_pattern`.
///   - DECLARATION — container for `catch_declaration`; marker for
///                declaration patterns.
///   - GENERIC  — container for generic type applications; marker for
///                type-parameter shape in a few spots.
///   - CONST    — container for `const` declarations; marker for const
///                modifier.
///   - LOGICAL  — container for logical expressions; also emitted as
///                an <op> child marker (e.g. `<op><logical><and/>…>`).
pub const NODES: &[NodeSpec] = &[
    // Top-level / structural
    NodeSpec { name: UNIT,      marker: false, container: true, syntax: Default },
    NodeSpec { name: NAMESPACE, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: IMPORT,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: BODY,      marker: false, container: true, syntax: Default },

    // Type declarations
    NodeSpec { name: CLASS,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: STRUCT,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: INTERFACE, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ENUM,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: RECORD,    marker: false, container: true, syntax: Keyword },

    // Members
    NodeSpec { name: METHOD,      marker: false, container: true, syntax: Default },
    NodeSpec { name: CONSTRUCTOR, marker: false, container: true, syntax: Default },
    NodeSpec { name: PROPERTY,    marker: false, container: true, syntax: Default },
    NodeSpec { name: FIELD,       marker: false, container: true, syntax: Default },
    NodeSpec { name: COMMENT,     marker: false, container: true, syntax: Comment },
    NodeSpec { name: EVENT,       marker: false, container: true, syntax: Default },
    NodeSpec { name: DELEGATE,    marker: false, container: true, syntax: Default },
    NodeSpec { name: DESTRUCTOR,  marker: false, container: true, syntax: Default },
    NodeSpec { name: INDEXER,     marker: false, container: true, syntax: Default },
    NodeSpec { name: OPERATOR,    marker: false, container: true, syntax: Default },

    // Shared children
    NodeSpec { name: NAME,       marker: false, container: true, syntax: Identifier },
    NodeSpec { name: TYPE,       marker: false, container: true, syntax: Type },
    NodeSpec { name: ACCESSORS,  marker: false, container: true, syntax: Default },
    NodeSpec { name: ACCESSOR,   marker: false, container: true, syntax: Default },
    NodeSpec { name: ATTRIBUTES, marker: false, container: true, syntax: Type },
    NodeSpec { name: ATTRIBUTE,  marker: false, container: true, syntax: Type },
    NodeSpec { name: ARGUMENTS,  marker: false, container: true, syntax: Default },
    NodeSpec { name: ARGUMENT,   marker: false, container: true, syntax: Default },
    NodeSpec { name: PARAMETERS, marker: false, container: true, syntax: Default },
    NodeSpec { name: PARAMETER,  marker: false, container: true, syntax: Default },
    NodeSpec { name: VARIABLE,   marker: false, container: true, syntax: Default },
    NodeSpec { name: DECLARATOR, marker: false, container: true, syntax: Default },
    NodeSpec { name: EXTENDS,    marker: false, container: true, syntax: Default },
    NodeSpec { name: PROPERTIES, marker: false, container: true, syntax: Default },
    NodeSpec { name: ELEMENT,    marker: false, container: true, syntax: Default },
    NodeSpec { name: SECTION,    marker: false, container: true, syntax: Default },
    NodeSpec { name: ARM,        marker: false, container: true, syntax: Default },
    NodeSpec { name: LABEL,      marker: false, container: true, syntax: Default },
    NodeSpec { name: CHAIN,      marker: false, container: true, syntax: Default },
    NodeSpec { name: FILTER,     marker: false, container: true, syntax: Default },
    NodeSpec { name: WHEN,       marker: false, container: true, syntax: Default },
    NodeSpec { name: WHERE,      marker: false, container: true, syntax: Default },

    // Statements / control flow
    NodeSpec { name: RETURN,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: IF,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ELSE,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ELSE_IF,  marker: false, container: true, syntax: Default },
    NodeSpec { name: FOR,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FOREACH,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: WHILE,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: DO,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: TRY,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CATCH,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FINALLY,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: THROW,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: USING,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: BREAK,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CONTINUE, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: SWITCH,   marker: false, container: true, syntax: Default },
    NodeSpec { name: BLOCK,    marker: false, container: true, syntax: Default },
    NodeSpec { name: EXPRESSION, marker: false, container: true, syntax: Default },
    NodeSpec { name: RANGE,    marker: false, container: true, syntax: Default },

    // Expressions (dual-use: NEW, TUPLE)
    NodeSpec { name: CALL,    marker: false, container: true, syntax: Default },
    NodeSpec { name: MEMBER,  marker: false, container: true, syntax: Default },
    NodeSpec { name: NEW,     marker: true,  container: true, syntax: Keyword },
    NodeSpec { name: ASSIGN,  marker: false, container: true, syntax: Operator },
    NodeSpec { name: BINARY,  marker: false, container: true, syntax: Operator },
    NodeSpec { name: UNARY,   marker: false, container: true, syntax: Operator },
    NodeSpec { name: LAMBDA,  marker: false, container: true, syntax: Function },
    NodeSpec { name: AWAIT,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: TERNARY, marker: false, container: true, syntax: Operator },
    NodeSpec { name: INDEX,   marker: false, container: true, syntax: Default },
    NodeSpec { name: IS,      marker: false, container: true, syntax: Default },
    NodeSpec { name: TUPLE,   marker: true,  container: true, syntax: Default },
    NodeSpec { name: LITERAL, marker: false, container: true, syntax: Default },
    NodeSpec { name: PATTERN, marker: false, container: true, syntax: Default },

    // Generics (dual-use)
    NodeSpec { name: GENERIC, marker: true, container: true, syntax: Type },

    // LINQ
    NodeSpec { name: QUERY,    marker: false, container: true, syntax: Default },
    NodeSpec { name: FROM,     marker: false, container: true, syntax: Default },
    NodeSpec { name: SELECT,   marker: false, container: true, syntax: Default },
    NodeSpec { name: ORDER,    marker: false, container: true, syntax: Default },
    NodeSpec { name: GROUP,    marker: false, container: true, syntax: Default },
    NodeSpec { name: LET,      marker: false, container: true, syntax: Default },
    NodeSpec { name: JOIN,     marker: false, container: true, syntax: Default },
    NodeSpec { name: ORDERING, marker: false, container: true, syntax: Default },

    // Dual-use: container or pattern marker.
    NodeSpec { name: CONSTANT,    marker: true, container: true, syntax: Default },
    NodeSpec { name: DECLARATION, marker: true, container: true, syntax: Default },

    // Literals / atoms
    NodeSpec { name: STRING,        marker: false, container: true, syntax: String },
    NodeSpec { name: INTERPOLATION, marker: false, container: true, syntax: Default },
    NodeSpec { name: INT,           marker: false, container: true, syntax: Number },
    NodeSpec { name: FLOAT,         marker: false, container: true, syntax: Number },
    NodeSpec { name: BOOL,          marker: false, container: true, syntax: Keyword },
    NodeSpec { name: NULL,          marker: false, container: true, syntax: Keyword },

    // Patterns / leaves
    NodeSpec { name: SUBPATTERN, marker: false, container: true, syntax: Default },
    NodeSpec { name: DISCARD,    marker: false, container: true, syntax: Default },
    NodeSpec { name: MODIFIER,   marker: false, container: true, syntax: Default },
    NodeSpec { name: OP,         marker: false, container: true, syntax: Operator },

    // Marker-only type shape
    NodeSpec { name: NULLABLE, marker: true, container: false, syntax: Type },

    // Comment markers
    NodeSpec { name: TRAILING, marker: true, container: false, syntax: Default },
    NodeSpec { name: LEADING,  marker: true, container: false, syntax: Default },

    // Marker-only: member-access / pattern / type shape
    NodeSpec { name: INSTANCE,    marker: true, container: false, syntax: Default },
    NodeSpec { name: CONDITIONAL, marker: true, container: false, syntax: Default },
    NodeSpec { name: ARRAY,       marker: true, container: false, syntax: Type },
    NodeSpec { name: POINTER,     marker: true, container: false, syntax: Default },
    NodeSpec { name: FUNCTION,    marker: true, container: false, syntax: Default },
    NodeSpec { name: REF,         marker: true, container: false, syntax: Identifier },
    NodeSpec { name: RECURSIVE,   marker: true, container: false, syntax: Default },
    NodeSpec { name: RELATIONAL,  marker: true, container: false, syntax: Default },
    // LOGICAL — dual-use: structural container for logical expressions,
    // and marker child of <op> (e.g. `<op><logical><and/></logical>…>`).
    NodeSpec { name: LOGICAL,     marker: true, container: true,  syntax: Operator },
    NodeSpec { name: PREFIX,      marker: true, container: false, syntax: Default },
    NodeSpec { name: LOOKUP,      marker: true, container: false, syntax: Default },

    // Access modifiers — markers only.
    NodeSpec { name: PUBLIC,    marker: true, container: false, syntax: Keyword },
    NodeSpec { name: PRIVATE,   marker: true, container: false, syntax: Keyword },
    NodeSpec { name: PROTECTED, marker: true, container: false, syntax: Keyword },
    NodeSpec { name: INTERNAL,  marker: true, container: false, syntax: Keyword },

    // Other modifiers — markers only.
    NodeSpec { name: STATIC,   marker: true, container: false, syntax: Keyword },
    NodeSpec { name: ABSTRACT, marker: true, container: false, syntax: Keyword },
    NodeSpec { name: VIRTUAL,  marker: true, container: false, syntax: Keyword },
    NodeSpec { name: OVERRIDE, marker: true, container: false, syntax: Keyword },
    NodeSpec { name: SEALED,   marker: true, container: false, syntax: Keyword },
    NodeSpec { name: READONLY, marker: true, container: false, syntax: Keyword },
    // CONST — dual-use: `const` declaration container AND `const` modifier marker.
    NodeSpec { name: CONST,    marker: true, container: true,  syntax: Keyword },
    NodeSpec { name: PARTIAL,  marker: true, container: false, syntax: Keyword },
    NodeSpec { name: ASYNC,    marker: true, container: false, syntax: Keyword },
    NodeSpec { name: EXTERN,   marker: true, container: false, syntax: Keyword },
    NodeSpec { name: UNSAFE,   marker: true, container: false, syntax: Keyword },
    NodeSpec { name: THIS,     marker: true, container: false, syntax: Keyword },

    // Accessor declarations.
    NodeSpec { name: GET,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: SET,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: INIT,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ADD,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: REMOVE, marker: false, container: true, syntax: Keyword },

    // Generic-constraint markers.
    NodeSpec { name: NOTNULL,   marker: true, container: false, syntax: Default },
    NodeSpec { name: UNMANAGED, marker: true, container: false, syntax: Default },
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
pub fn is_marker_only(name: &str) -> bool {
    spec(name).map_or(false, |s| s.marker && !s.container)
}

/// True iff `name` is declared in this language's NODES table.
pub fn is_declared(name: &str) -> bool {
    spec(name).is_some()
}
