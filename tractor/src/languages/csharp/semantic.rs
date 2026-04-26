/// Semantic element names — tractor's C# XML vocabulary after transform.
/// These are the names that appear in tractor's output and that the renderer reads.
use crate::languages::{KindEntry, KindHandling, NodeSpec};
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

/// Tree-sitter kind catalogue — single source of truth for every
/// kind the C# transform handles. Sorted alphabetically by kind name.
///
/// Variants:
///   - `Rename` / `RenameWithMarker`: pure-rename kinds, also drives
///     `transform::map_element_name`.
///   - `Custom`: kind has an explicit dispatch arm with non-trivial
///     logic that owns its own renaming (or intentionally leaves the
///     kind name in place).
///   - `CustomThenRename` / `CustomThenRenameWithMarker`: dispatch arm
///     does structural work, then defers the rename to
///     `map_element_name` (which reads back from this catalogue).
///   - `Flatten`: dispatcher drops the wrapper and promotes children.
///   - `PassThrough`: kind appears with no semantic transform — text
///     leaves carry their `kind=` attribute through unchanged.
pub const KINDS: &[KindEntry] = &[
    KindEntry { kind: "accessor_declaration",          handling: KindHandling::Custom },
    KindEntry { kind: "accessor_list",                 handling: KindHandling::Flatten },
    KindEntry { kind: "alias_qualified_name",          handling: KindHandling::PassThrough },
    KindEntry { kind: "argument",                      handling: KindHandling::Rename(ARGUMENT) },
    KindEntry { kind: "argument_list",                 handling: KindHandling::Flatten },
    KindEntry { kind: "array_type",                    handling: KindHandling::RenameWithMarker(TYPE, ARRAY) },
    KindEntry { kind: "arrow_expression_clause",       handling: KindHandling::Flatten },
    KindEntry { kind: "assignment_expression",         handling: KindHandling::CustomThenRename(ASSIGN) },
    KindEntry { kind: "attribute",                     handling: KindHandling::Rename(ATTRIBUTE) },
    KindEntry { kind: "attribute_argument",            handling: KindHandling::Rename(ARGUMENT) },
    KindEntry { kind: "attribute_argument_list",       handling: KindHandling::Flatten },
    KindEntry { kind: "attribute_list",                handling: KindHandling::Flatten },
    KindEntry { kind: "await_expression",              handling: KindHandling::Rename(AWAIT) },
    KindEntry { kind: "base_list",                     handling: KindHandling::Rename(EXTENDS) },
    KindEntry { kind: "binary_expression",             handling: KindHandling::CustomThenRename(BINARY) },
    KindEntry { kind: "block",                         handling: KindHandling::Rename(BLOCK) },
    KindEntry { kind: "boolean_literal",               handling: KindHandling::Rename(BOOL) },
    KindEntry { kind: "bracketed_parameter_list",      handling: KindHandling::Flatten },
    KindEntry { kind: "break_statement",               handling: KindHandling::Rename(BREAK) },
    KindEntry { kind: "catch_clause",                  handling: KindHandling::Rename(CATCH) },
    KindEntry { kind: "catch_declaration",             handling: KindHandling::Rename(DECLARATION) },
    KindEntry { kind: "catch_filter_clause",           handling: KindHandling::Rename(FILTER) },
    KindEntry { kind: "class_declaration",             handling: KindHandling::CustomThenRename(CLASS) },
    KindEntry { kind: "comment",                       handling: KindHandling::Custom },
    KindEntry { kind: "compact_constructor_declaration", handling: KindHandling::Rename(CONSTRUCTOR) },
    KindEntry { kind: "compilation_unit",              handling: KindHandling::Rename(UNIT) },
    KindEntry { kind: "conditional_access_expression", handling: KindHandling::RenameWithMarker(MEMBER, CONDITIONAL) },
    KindEntry { kind: "conditional_expression",        handling: KindHandling::Custom },
    KindEntry { kind: "constant_pattern",              handling: KindHandling::RenameWithMarker(PATTERN, CONSTANT) },
    // `where T : new()` shape constraint — transplanted into the matching
    // `<generic>` as an empty `<new/>` marker by `attach_where_clause_constraints`
    // (in `languages/mod.rs`), so the original kind never reaches the dispatcher.
    KindEntry { kind: "constructor_constraint",        handling: KindHandling::Custom },
    KindEntry { kind: "constructor_declaration",       handling: KindHandling::CustomThenRename(CONSTRUCTOR) },
    KindEntry { kind: "constructor_initializer",       handling: KindHandling::Rename(CHAIN) },
    KindEntry { kind: "continue_statement",            handling: KindHandling::Rename(CONTINUE) },
    KindEntry { kind: "declaration_list",              handling: KindHandling::Flatten },
    KindEntry { kind: "declaration_pattern",           handling: KindHandling::RenameWithMarker(PATTERN, DECLARATION) },
    KindEntry { kind: "delegate_declaration",          handling: KindHandling::Rename(DELEGATE) },
    KindEntry { kind: "destructor_declaration",        handling: KindHandling::Rename(DESTRUCTOR) },
    // `_` discard pattern in switch arms / deconstructions. Tree-sitter
    // emits a leaf named `discard` which already matches our semantic
    // vocabulary — pass through unchanged.
    KindEntry { kind: "discard",                       handling: KindHandling::PassThrough },
    KindEntry { kind: "do_statement",                  handling: KindHandling::Rename(DO) },
    KindEntry { kind: "element_binding_expression",    handling: KindHandling::Rename(INDEX) },
    KindEntry { kind: "else_clause",                   handling: KindHandling::Rename(ELSE) },
    KindEntry { kind: "enum_declaration",              handling: KindHandling::CustomThenRename(ENUM) },
    KindEntry { kind: "enum_member_declaration",       handling: KindHandling::Rename(CONSTANT) },
    KindEntry { kind: "enum_member_declaration_list",  handling: KindHandling::Flatten },
    KindEntry { kind: "escape_sequence",               handling: KindHandling::Flatten },
    KindEntry { kind: "event_field_declaration",       handling: KindHandling::Rename(EVENT) },
    KindEntry { kind: "expression_statement",          handling: KindHandling::Rename(EXPRESSION) },
    KindEntry { kind: "field_declaration",             handling: KindHandling::CustomThenRename(FIELD) },
    KindEntry { kind: "file_scoped_namespace_declaration", handling: KindHandling::Rename(NAMESPACE) },
    KindEntry { kind: "finally_clause",                handling: KindHandling::Rename(FINALLY) },
    KindEntry { kind: "for_statement",                 handling: KindHandling::Rename(FOR) },
    KindEntry { kind: "foreach_statement",             handling: KindHandling::Rename(FOREACH) },
    KindEntry { kind: "from_clause",                   handling: KindHandling::Rename(FROM) },
    KindEntry { kind: "function_pointer_type",         handling: KindHandling::RenameWithMarker(TYPE, FUNCTION) },
    KindEntry { kind: "generic_name",                  handling: KindHandling::Custom },
    KindEntry { kind: "group_clause",                  handling: KindHandling::Rename(GROUP) },
    KindEntry { kind: "identifier",                    handling: KindHandling::Custom },
    KindEntry { kind: "if_statement",                  handling: KindHandling::Custom },
    KindEntry { kind: "implicit_object_creation_expression", handling: KindHandling::Rename(NEW) },
    KindEntry { kind: "implicit_parameter",            handling: KindHandling::Rename(PARAMETER) },
    KindEntry { kind: "implicit_type",                 handling: KindHandling::Custom },
    KindEntry { kind: "indexer_declaration",           handling: KindHandling::Rename(INDEXER) },
    KindEntry { kind: "initializer_expression",        handling: KindHandling::Rename(LITERAL) },
    KindEntry { kind: "integer_literal",               handling: KindHandling::Rename(INT) },
    KindEntry { kind: "interface_declaration",         handling: KindHandling::CustomThenRename(INTERFACE) },
    KindEntry { kind: "interpolated_string_expression", handling: KindHandling::Custom },
    // Inner `{expr}` of an interpolated string. Tree-sitter emits a node
    // named `interpolation`, which already matches our semantic vocabulary —
    // pass through.
    KindEntry { kind: "interpolation",                 handling: KindHandling::PassThrough },
    KindEntry { kind: "interpolation_brace",           handling: KindHandling::Flatten },
    KindEntry { kind: "interpolation_start",           handling: KindHandling::Flatten },
    KindEntry { kind: "invocation_expression",         handling: KindHandling::Rename(CALL) },
    KindEntry { kind: "is_pattern_expression",         handling: KindHandling::Rename(IS) },
    KindEntry { kind: "join_clause",                   handling: KindHandling::Rename(JOIN) },
    KindEntry { kind: "lambda_expression",             handling: KindHandling::Rename(LAMBDA) },
    KindEntry { kind: "let_clause",                    handling: KindHandling::Rename(LET) },
    KindEntry { kind: "local_declaration_statement",   handling: KindHandling::Flatten },
    KindEntry { kind: "local_function_statement",      handling: KindHandling::Rename(METHOD) },
    KindEntry { kind: "logical_pattern",               handling: KindHandling::RenameWithMarker(PATTERN, LOGICAL) },
    KindEntry { kind: "lookup_type",                   handling: KindHandling::RenameWithMarker(TYPE, LOOKUP) },
    KindEntry { kind: "member_access_expression",      handling: KindHandling::RenameWithMarker(MEMBER, INSTANCE) },
    KindEntry { kind: "member_binding_expression",     handling: KindHandling::RenameWithMarker(MEMBER, CONDITIONAL) },
    KindEntry { kind: "method_declaration",            handling: KindHandling::CustomThenRename(METHOD) },
    KindEntry { kind: "modifier",                      handling: KindHandling::Custom },
    KindEntry { kind: "namespace_declaration",         handling: KindHandling::Rename(NAMESPACE) },
    KindEntry { kind: "null_literal",                  handling: KindHandling::Rename(NULL) },
    KindEntry { kind: "nullable_type",                 handling: KindHandling::Custom },
    KindEntry { kind: "object_creation_expression",    handling: KindHandling::Rename(NEW) },
    KindEntry { kind: "operator_declaration",          handling: KindHandling::Rename(OPERATOR) },
    KindEntry { kind: "order_by_clause",               handling: KindHandling::Rename(ORDER) },
    KindEntry { kind: "ordering",                      handling: KindHandling::Rename(ORDERING) },
    KindEntry { kind: "parameter",                     handling: KindHandling::Rename(PARAMETER) },
    KindEntry { kind: "parameter_list",                handling: KindHandling::Flatten },
    KindEntry { kind: "parameters",                    handling: KindHandling::Flatten },
    KindEntry { kind: "parenthesized_expression",      handling: KindHandling::Flatten },
    KindEntry { kind: "pointer_type",                  handling: KindHandling::RenameWithMarker(TYPE, POINTER) },
    KindEntry { kind: "postfix_unary_expression",      handling: KindHandling::Custom },
    KindEntry { kind: "predefined_type",               handling: KindHandling::Custom },
    KindEntry { kind: "prefix_unary_expression",       handling: KindHandling::RenameWithMarker(UNARY, PREFIX) },
    KindEntry { kind: "property_declaration",          handling: KindHandling::CustomThenRename(PROPERTY) },
    KindEntry { kind: "property_pattern_clause",       handling: KindHandling::Rename(PROPERTIES) },
    KindEntry { kind: "qualified_name",                handling: KindHandling::Flatten },
    KindEntry { kind: "query_body",                    handling: KindHandling::Rename(BODY) },
    KindEntry { kind: "query_expression",              handling: KindHandling::Rename(QUERY) },
    KindEntry { kind: "range_expression",              handling: KindHandling::Rename(RANGE) },
    KindEntry { kind: "raw_string_content",            handling: KindHandling::Flatten },
    KindEntry { kind: "raw_string_end",                handling: KindHandling::Flatten },
    KindEntry { kind: "raw_string_literal",            handling: KindHandling::Rename(STRING) },
    KindEntry { kind: "raw_string_literal_content",    handling: KindHandling::Flatten },
    KindEntry { kind: "raw_string_start",              handling: KindHandling::Flatten },
    KindEntry { kind: "real_literal",                  handling: KindHandling::Rename(FLOAT) },
    KindEntry { kind: "record_declaration",            handling: KindHandling::CustomThenRename(RECORD) },
    KindEntry { kind: "recursive_pattern",             handling: KindHandling::RenameWithMarker(PATTERN, RECURSIVE) },
    KindEntry { kind: "ref_type",                      handling: KindHandling::RenameWithMarker(TYPE, REF) },
    KindEntry { kind: "relational_pattern",            handling: KindHandling::RenameWithMarker(PATTERN, RELATIONAL) },
    KindEntry { kind: "return_statement",              handling: KindHandling::Rename(RETURN) },
    KindEntry { kind: "select_clause",                 handling: KindHandling::Rename(SELECT) },
    KindEntry { kind: "string_content",                handling: KindHandling::Flatten },
    KindEntry { kind: "string_literal",                handling: KindHandling::Rename(STRING) },
    KindEntry { kind: "string_literal_content",        handling: KindHandling::Flatten },
    KindEntry { kind: "struct_declaration",            handling: KindHandling::CustomThenRename(STRUCT) },
    // Property-pattern entry `{ Name: X }` in a recursive pattern.
    // Tree-sitter emits `subpattern`, already matching our semantic
    // vocabulary — pass through.
    KindEntry { kind: "subpattern",                    handling: KindHandling::PassThrough },
    KindEntry { kind: "switch_body",                   handling: KindHandling::Rename(BODY) },
    KindEntry { kind: "switch_expression",             handling: KindHandling::Rename(SWITCH) },
    KindEntry { kind: "switch_expression_arm",         handling: KindHandling::Rename(ARM) },
    KindEntry { kind: "switch_label",                  handling: KindHandling::Rename(LABEL) },
    KindEntry { kind: "switch_rule",                   handling: KindHandling::Rename(ARM) },
    KindEntry { kind: "switch_section",                handling: KindHandling::Rename(SECTION) },
    KindEntry { kind: "switch_statement",              handling: KindHandling::Rename(SWITCH) },
    KindEntry { kind: "throw_statement",               handling: KindHandling::Rename(THROW) },
    KindEntry { kind: "try_statement",                 handling: KindHandling::Rename(TRY) },
    KindEntry { kind: "tuple_element",                 handling: KindHandling::Rename(ELEMENT) },
    KindEntry { kind: "tuple_expression",              handling: KindHandling::Rename(TUPLE) },
    KindEntry { kind: "tuple_pattern",                 handling: KindHandling::RenameWithMarker(PATTERN, TUPLE) },
    KindEntry { kind: "tuple_type",                    handling: KindHandling::RenameWithMarker(TYPE, TUPLE) },
    KindEntry { kind: "type_argument_list",            handling: KindHandling::Flatten },
    KindEntry { kind: "type_identifier",               handling: KindHandling::Custom },
    KindEntry { kind: "type_parameter",                handling: KindHandling::Rename(GENERIC) },
    // `where T : …` constraint entry — consumed by the
    // `attach_where_clause_constraints` post-transform; never reaches the
    // dispatcher.
    KindEntry { kind: "type_parameter_constraint",     handling: KindHandling::Custom },
    KindEntry { kind: "type_parameter_constraints_clause", handling: KindHandling::Custom },
    KindEntry { kind: "type_parameter_list",           handling: KindHandling::Flatten },
    KindEntry { kind: "unary_expression",              handling: KindHandling::CustomThenRename(UNARY) },
    KindEntry { kind: "using_directive",               handling: KindHandling::Rename(IMPORT) },
    KindEntry { kind: "using_statement",               handling: KindHandling::Rename(USING) },
    KindEntry { kind: "variable_declaration",          handling: KindHandling::Rename(VARIABLE) },
    KindEntry { kind: "variable_declarator",           handling: KindHandling::Rename(DECLARATOR) },
    KindEntry { kind: "verbatim_string_literal",       handling: KindHandling::Rename(STRING) },
    KindEntry { kind: "verbatim_string_literal_content", handling: KindHandling::Flatten },
    KindEntry { kind: "when_clause",                   handling: KindHandling::Rename(WHEN) },
    KindEntry { kind: "where_clause",                  handling: KindHandling::Rename(WHERE) },
    KindEntry { kind: "while_statement",               handling: KindHandling::Rename(WHILE) },
];

/// Look up the rename target for a tree-sitter `kind` in this
/// language's catalogue. Returns `Some((semantic, marker))` for
/// `Rename` / `RenameWithMarker` / `CustomThenRename*` entries,
/// `None` for everything else. Used by `transform::map_element_name`.
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
pub fn is_marker_only(name: &str) -> bool {
    spec(name).map_or(false, |s| s.marker && !s.container)
}

/// True iff `name` is declared in this language's NODES table.
pub fn is_declared(name: &str) -> bool {
    spec(name).is_some()
}
