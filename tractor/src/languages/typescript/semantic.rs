/// Semantic element names — tractor's TypeScript/JavaScript XML
/// vocabulary after transform.
use crate::languages::NodeSpec;
use crate::output::syntax_highlight::SyntaxCategory;

// Named constants retained for use by the transform code. The NODES
// table below is the source of truth for marker/container role and
// syntax category.

// Top-level / declarations
pub const PROGRAM: &str = "program";
pub const CLASS: &str = "class";
pub const INTERFACE: &str = "interface";
pub const ENUM: &str = "enum";
pub const FUNCTION: &str = "function";
pub const METHOD: &str = "method";
pub const PROPERTY: &str = "property";
pub const CONSTRUCTOR: &str = "constructor";
pub const INDEXER: &str = "indexer";
pub const ALIAS: &str = "alias";
pub const VARIABLE: &str = "variable";
pub const ARROW: &str = "arrow";

// Members
pub const FIELD: &str = "field";
pub const PARAMETER: &str = "parameter";
pub const EXTENDS: &str = "extends";
pub const IMPLEMENTS: &str = "implements";

// Type vocabulary
pub const TYPE: &str = "type";
pub const GENERIC: &str = "generic";
pub const GENERICS: &str = "generics";
pub const PREDICATE: &str = "predicate";
pub const ANNOTATION: &str = "annotation";

// Control flow
pub const BLOCK: &str = "block";
pub const RETURN: &str = "return";
pub const IF: &str = "if";
pub const ELSE: &str = "else";
pub const ELSE_IF: &str = "else_if";
pub const FOR: &str = "for";
pub const WHILE: &str = "while";
pub const TRY: &str = "try";
pub const CATCH: &str = "catch";
pub const THROW: &str = "throw";
pub const FINALLY: &str = "finally";
pub const SWITCH: &str = "switch";
pub const CASE: &str = "case";
pub const BREAK: &str = "break";
pub const CONTINUE: &str = "continue";
pub const BODY: &str = "body";

// Expressions
pub const CALL: &str = "call";
pub const NEW: &str = "new";
pub const MEMBER: &str = "member";
pub const ASSIGN: &str = "assign";
pub const BINARY: &str = "binary";
pub const UNARY: &str = "unary";
pub const TERNARY: &str = "ternary";
pub const AWAIT: &str = "await";
pub const YIELD: &str = "yield";
pub const AS: &str = "as";
pub const SATISFIES: &str = "satisfies";
pub const INDEX: &str = "index";
pub const PATTERN: &str = "pattern";
pub const SPREAD: &str = "spread";
pub const REST: &str = "rest";

// Imports / exports
pub const IMPORT: &str = "import";
pub const EXPORT: &str = "export";
pub const IMPORTS: &str = "imports";
pub const SPEC: &str = "spec";
pub const CLAUSE: &str = "clause";
pub const NAMESPACE: &str = "namespace";

// Templates
pub const TEMPLATE: &str = "template";
pub const INTERPOLATION: &str = "interpolation";

// JSX
pub const ELEMENT: &str = "element";
pub const OPENING: &str = "opening";
pub const CLOSING: &str = "closing";
pub const PROP: &str = "prop";
pub const VALUE: &str = "value";
pub const TEXT: &str = "text";

// Enum members (enum_assignment → <constant>)
pub const CONSTANT: &str = "constant";

// Object-literal / destructuring entry: `{ a: 1 }` pair emitted as
// a container around the key and value.
pub const PAIR: &str = "pair";

// Literals
pub const STRING: &str = "string";
pub const NUMBER: &str = "number";
pub const BOOL: &str = "bool";
pub const NULL: &str = "null";
pub const UNDEFINED: &str = "undefined";

// Keyword expressions — distinct tree-sitter leaf kinds.
pub const THIS: &str = "this";
pub const SUPER: &str = "super";

// Generic-parameter constraint: `<T extends Shape>`.
pub const CONSTRAINT: &str = "constraint";

// Identifiers / comments
pub const NAME: &str = "name";
pub const COMMENT: &str = "comment";

// Comment markers — emitted by the shared CommentClassifier.
pub const TRAILING: &str = "trailing";
pub const LEADING: &str = "leading";

// Switch default — dual-use; see NODES below.
pub const DEFAULT: &str = "default";

// Operator child
pub const OP: &str = "op";

// Accessibility / modifier markers
pub const PUBLIC: &str = "public";
pub const PRIVATE: &str = "private";
pub const PROTECTED: &str = "protected";
pub const OVERRIDE: &str = "override";
pub const READONLY: &str = "readonly";
pub const ABSTRACT: &str = "abstract";
pub const OPTIONAL: &str = "optional";
pub const REQUIRED: &str = "required";

// Function markers
pub const ASYNC: &str = "async";
pub const GENERATOR: &str = "generator";
pub const GET: &str = "get";
pub const SET: &str = "set";

// Variable-keyword markers
pub const LET: &str = "let";
pub const CONST: &str = "const";
pub const VAR: &str = "var";

// Type-shape markers
pub const UNION: &str = "union";
pub const INTERSECTION: &str = "intersection";
pub const ARRAY: &str = "array";
pub const LITERAL: &str = "literal";
pub const TUPLE: &str = "tuple";
pub const PARENTHESIZED: &str = "parenthesized";
pub const OBJECT: &str = "object";
pub const CONDITIONAL: &str = "conditional";
pub const INFER: &str = "infer";
pub const LOOKUP: &str = "lookup";
pub const KEYOF: &str = "keyof";

use SyntaxCategory::*;

/// Per-name metadata — single source of truth for every element
/// name this language's transform can emit.
///
/// Dual-use names set BOTH `marker: true` and `container: true`:
///   - EXPORT   — marker via extract_keyword_modifiers + structural
///                container from `export_statement`.
///   - DEFAULT  — marker via extract_keyword_modifiers + structural
///                container from `switch_default`.
///   - FUNCTION — structural container (`function_declaration`) +
///                marker on type (`function_type`).
///   - TEMPLATE — structural container (`template_string`) + marker
///                on type (`template_type` / `template_literal_type`).
///   - ARRAY    — marker on `<type>` / `<pattern>` + structural
///                container for the tree-sitter `array` literal kind.
///   - OBJECT   — marker on `<type>` / `<pattern>` + structural
///                container for the tree-sitter `object` literal kind.
pub const NODES: &[NodeSpec] = &[
    // Top-level / declarations
    NodeSpec { name: PROGRAM,     marker: false, container: true, syntax: Default },
    NodeSpec { name: CLASS,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: INTERFACE,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ENUM,        marker: false, container: true, syntax: Keyword },
    // FUNCTION is dual-use.
    NodeSpec { name: FUNCTION,    marker: true,  container: true, syntax: Keyword },
    NodeSpec { name: METHOD,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: PROPERTY,    marker: false, container: true, syntax: Default },
    NodeSpec { name: CONSTRUCTOR, marker: false, container: true, syntax: Default },
    NodeSpec { name: INDEXER,     marker: false, container: true, syntax: Default },
    NodeSpec { name: ALIAS,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: VARIABLE,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ARROW,       marker: false, container: true, syntax: Function },

    // Members
    NodeSpec { name: FIELD,      marker: false, container: true, syntax: Default },
    NodeSpec { name: PARAMETER,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: EXTENDS,    marker: false, container: true, syntax: Default },
    NodeSpec { name: IMPLEMENTS, marker: false, container: true, syntax: Default },

    // Type vocabulary
    NodeSpec { name: TYPE,       marker: false, container: true, syntax: Type },
    NodeSpec { name: GENERIC,    marker: false, container: true, syntax: Type },
    NodeSpec { name: GENERICS,   marker: false, container: true, syntax: Type },
    NodeSpec { name: PREDICATE,  marker: false, container: true, syntax: Default },
    NodeSpec { name: ANNOTATION, marker: false, container: true, syntax: Default },

    // Control flow
    NodeSpec { name: BLOCK,    marker: false, container: true, syntax: Default },
    NodeSpec { name: RETURN,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: IF,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ELSE,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ELSE_IF,  marker: false, container: true, syntax: Default },
    NodeSpec { name: FOR,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: WHILE,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: TRY,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CATCH,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: THROW,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FINALLY,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: SWITCH,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CASE,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: BREAK,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CONTINUE, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: BODY,     marker: false, container: true, syntax: Default },

    // Expressions
    NodeSpec { name: CALL,      marker: false, container: true, syntax: Function },
    NodeSpec { name: NEW,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: MEMBER,    marker: false, container: true, syntax: Default },
    NodeSpec { name: ASSIGN,    marker: false, container: true, syntax: Operator },
    NodeSpec { name: BINARY,    marker: false, container: true, syntax: Operator },
    NodeSpec { name: UNARY,     marker: false, container: true, syntax: Operator },
    NodeSpec { name: TERNARY,   marker: false, container: true, syntax: Operator },
    NodeSpec { name: AWAIT,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: YIELD,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: AS,        marker: false, container: true, syntax: Default },
    NodeSpec { name: SATISFIES, marker: false, container: true, syntax: Default },
    NodeSpec { name: INDEX,     marker: false, container: true, syntax: Default },
    NodeSpec { name: PATTERN,   marker: false, container: true, syntax: Default },
    NodeSpec { name: SPREAD,    marker: false, container: true, syntax: Default },
    NodeSpec { name: REST,      marker: false, container: true, syntax: Default },

    // Imports / exports (EXPORT is dual-use)
    NodeSpec { name: IMPORT,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: EXPORT,    marker: true,  container: true, syntax: Keyword },
    NodeSpec { name: IMPORTS,   marker: false, container: true, syntax: Default },
    NodeSpec { name: SPEC,      marker: false, container: true, syntax: Default },
    NodeSpec { name: CLAUSE,    marker: false, container: true, syntax: Default },
    NodeSpec { name: NAMESPACE, marker: false, container: true, syntax: Default },

    // Templates (TEMPLATE is dual-use)
    NodeSpec { name: TEMPLATE,      marker: true,  container: true, syntax: Default },
    NodeSpec { name: INTERPOLATION, marker: false, container: true, syntax: Default },

    // JSX
    NodeSpec { name: ELEMENT, marker: false, container: true, syntax: Default },
    NodeSpec { name: OPENING, marker: false, container: true, syntax: Default },
    NodeSpec { name: CLOSING, marker: false, container: true, syntax: Default },
    NodeSpec { name: PROP,    marker: false, container: true, syntax: Default },
    NodeSpec { name: VALUE,   marker: false, container: true, syntax: Default },
    NodeSpec { name: TEXT,    marker: false, container: true, syntax: Default },

    // Enum members
    NodeSpec { name: CONSTANT, marker: false, container: true, syntax: Default },
    NodeSpec { name: PAIR,     marker: false, container: true, syntax: Default },

    // Literals
    NodeSpec { name: STRING,    marker: false, container: true, syntax: String },
    NodeSpec { name: NUMBER,    marker: false, container: true, syntax: Number },
    NodeSpec { name: BOOL,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: NULL,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: UNDEFINED, marker: false, container: true, syntax: Keyword },

    // Keyword expressions
    NodeSpec { name: THIS,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: SUPER,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CONSTRAINT, marker: false, container: true, syntax: Default },

    // Identifiers / comments / op
    NodeSpec { name: NAME,    marker: false, container: true, syntax: Identifier },
    NodeSpec { name: COMMENT, marker: false, container: true, syntax: Comment },
    // DEFAULT — dual-use (marker modifier AND switch_default container).
    NodeSpec { name: DEFAULT, marker: true,  container: true, syntax: Keyword },
    NodeSpec { name: OP,      marker: false, container: true, syntax: Operator },

    // Comment markers
    NodeSpec { name: TRAILING, marker: true, container: false, syntax: Default },
    NodeSpec { name: LEADING,  marker: true, container: false, syntax: Default },

    // Accessibility / modifier markers
    NodeSpec { name: PUBLIC,    marker: true, container: false, syntax: Keyword },
    NodeSpec { name: PRIVATE,   marker: true, container: false, syntax: Keyword },
    NodeSpec { name: PROTECTED, marker: true, container: false, syntax: Keyword },
    NodeSpec { name: OVERRIDE,  marker: true, container: false, syntax: Keyword },
    NodeSpec { name: READONLY,  marker: true, container: false, syntax: Keyword },
    NodeSpec { name: ABSTRACT,  marker: true, container: false, syntax: Keyword },
    NodeSpec { name: OPTIONAL,  marker: true, container: false, syntax: Keyword },
    NodeSpec { name: REQUIRED,  marker: true, container: false, syntax: Keyword },

    // Function markers
    NodeSpec { name: ASYNC,     marker: true, container: false, syntax: Keyword },
    NodeSpec { name: GENERATOR, marker: true, container: false, syntax: Default },
    NodeSpec { name: GET,       marker: true, container: false, syntax: Default },
    NodeSpec { name: SET,       marker: true, container: false, syntax: Default },

    // Variable-keyword markers
    NodeSpec { name: LET,   marker: true, container: false, syntax: Keyword },
    NodeSpec { name: CONST, marker: true, container: false, syntax: Keyword },
    NodeSpec { name: VAR,   marker: true, container: false, syntax: Keyword },

    // Type-shape markers (ARRAY/OBJECT are dual-use)
    NodeSpec { name: UNION,         marker: true, container: false, syntax: Default },
    NodeSpec { name: INTERSECTION,  marker: true, container: false, syntax: Default },
    NodeSpec { name: ARRAY,         marker: true, container: true,  syntax: Default },
    NodeSpec { name: LITERAL,       marker: true, container: false, syntax: Default },
    NodeSpec { name: TUPLE,         marker: true, container: false, syntax: Default },
    NodeSpec { name: PARENTHESIZED, marker: true, container: false, syntax: Default },
    NodeSpec { name: OBJECT,        marker: true, container: true,  syntax: Default },
    NodeSpec { name: CONDITIONAL,   marker: true, container: false, syntax: Default },
    NodeSpec { name: INFER,         marker: true, container: false, syntax: Default },
    NodeSpec { name: LOOKUP,        marker: true, container: false, syntax: Default },
    NodeSpec { name: KEYOF,         marker: true, container: false, syntax: Default },
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
