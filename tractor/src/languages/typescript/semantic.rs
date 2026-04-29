/// Semantic element names — tractor's TypeScript/JavaScript XML
/// vocabulary after transform.
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

/// Tree-sitter kind catalogue — single source of truth for every
/// kind the TypeScript / JavaScript transform handles. Sorted
/// alphabetically by kind name. See `KindHandling` for variants.
pub const KINDS: &[KindEntry] = &[
    KindEntry { kind: "abstract_class_declaration",    handling: KindHandling::RenameWithMarker(CLASS, ABSTRACT) },
    KindEntry { kind: "abstract_method_signature",     handling: KindHandling::CustomThenRenameWithMarker(METHOD, ABSTRACT) },
    KindEntry { kind: "accessibility_modifier",        handling: KindHandling::Custom },
    // Object/array literal leaves emitted by tree-sitter — match our
    // semantic vocabulary already, pass through.
    KindEntry { kind: "array",                         handling: KindHandling::PassThrough },
    KindEntry { kind: "array_pattern",                 handling: KindHandling::RenameWithMarker(PATTERN, ARRAY) },
    KindEntry { kind: "array_type",                    handling: KindHandling::RenameWithMarker(TYPE, ARRAY) },
    KindEntry { kind: "arguments",                     handling: KindHandling::Flatten },
    KindEntry { kind: "arrow_function",                handling: KindHandling::CustomThenRename(ARROW) },
    KindEntry { kind: "as_expression",                 handling: KindHandling::Rename(AS) },
    KindEntry { kind: "assignment_expression",         handling: KindHandling::CustomThenRename(ASSIGN) },
    KindEntry { kind: "augmented_assignment_expression", handling: KindHandling::CustomThenRename(ASSIGN) },
    KindEntry { kind: "await_expression",              handling: KindHandling::Rename(AWAIT) },
    KindEntry { kind: "binary_expression",             handling: KindHandling::CustomThenRename(BINARY) },
    KindEntry { kind: "break_statement",               handling: KindHandling::Rename(BREAK) },
    KindEntry { kind: "call_expression",               handling: KindHandling::CustomThenRename(CALL) },
    KindEntry { kind: "catch_clause",                  handling: KindHandling::Rename(CATCH) },
    KindEntry { kind: "class_body",                    handling: KindHandling::Flatten },
    KindEntry { kind: "class_declaration",             handling: KindHandling::Rename(CLASS) },
    KindEntry { kind: "class_heritage",                handling: KindHandling::Flatten },
    KindEntry { kind: "comment",                       handling: KindHandling::Custom },
    KindEntry { kind: "conditional_type",              handling: KindHandling::RenameWithMarker(TYPE, CONDITIONAL) },
    KindEntry { kind: "construct_signature",           handling: KindHandling::Rename(CONSTRUCTOR) },
    // `<T extends Shape>` constraint child — already named `constraint`
    // in tree-sitter, matches our vocabulary as-is.
    KindEntry { kind: "constraint",                    handling: KindHandling::PassThrough },
    KindEntry { kind: "continue_statement",            handling: KindHandling::Rename(CONTINUE) },
    KindEntry { kind: "default_type",                  handling: KindHandling::RenameWithMarker(TYPE, DEFAULT) },
    KindEntry { kind: "else_clause",                   handling: KindHandling::Rename(ELSE) },
    KindEntry { kind: "enum_assignment",               handling: KindHandling::Rename(CONSTANT) },
    KindEntry { kind: "enum_body",                     handling: KindHandling::Flatten },
    KindEntry { kind: "enum_declaration",              handling: KindHandling::Rename(ENUM) },
    KindEntry { kind: "export_clause",                 handling: KindHandling::Flatten },
    KindEntry { kind: "export_specifier",              handling: KindHandling::Rename(SPEC) },
    KindEntry { kind: "export_statement",              handling: KindHandling::Rename(EXPORT) },
    KindEntry { kind: "expression_statement",          handling: KindHandling::Flatten },
    KindEntry { kind: "extends_clause",                handling: KindHandling::Custom },
    // Boolean and null leaf kinds.
    KindEntry { kind: "false",                         handling: KindHandling::Rename(BOOL) },
    KindEntry { kind: "finally_clause",                handling: KindHandling::Rename(FINALLY) },
    KindEntry { kind: "for_in_statement",              handling: KindHandling::Rename(FOR) },
    KindEntry { kind: "for_statement",                 handling: KindHandling::Rename(FOR) },
    KindEntry { kind: "formal_parameters",             handling: KindHandling::Flatten },
    KindEntry { kind: "function_declaration",          handling: KindHandling::CustomThenRename(FUNCTION) },
    KindEntry { kind: "function_expression",           handling: KindHandling::CustomThenRename(FUNCTION) },
    KindEntry { kind: "function_type",                 handling: KindHandling::RenameWithMarker(TYPE, FUNCTION) },
    KindEntry { kind: "generator_function",            handling: KindHandling::CustomThenRename(FUNCTION) },
    KindEntry { kind: "generator_function_declaration", handling: KindHandling::CustomThenRename(FUNCTION) },
    KindEntry { kind: "generic_type",                  handling: KindHandling::Custom },
    KindEntry { kind: "identifier",                    handling: KindHandling::Custom },
    KindEntry { kind: "if_statement",                  handling: KindHandling::Rename(IF) },
    KindEntry { kind: "implements_clause",             handling: KindHandling::Rename(IMPLEMENTS) },
    KindEntry { kind: "import_clause",                 handling: KindHandling::Rename(CLAUSE) },
    KindEntry { kind: "import_specifier",              handling: KindHandling::Rename(SPEC) },
    KindEntry { kind: "import_statement",              handling: KindHandling::Rename(IMPORT) },
    KindEntry { kind: "index_signature",               handling: KindHandling::Rename(INDEXER) },
    KindEntry { kind: "index_type_query",              handling: KindHandling::RenameWithMarker(TYPE, KEYOF) },
    KindEntry { kind: "infer_type",                    handling: KindHandling::RenameWithMarker(TYPE, INFER) },
    KindEntry { kind: "interface_body",                handling: KindHandling::Flatten },
    KindEntry { kind: "interface_declaration",         handling: KindHandling::Rename(INTERFACE) },
    KindEntry { kind: "intersection_type",             handling: KindHandling::RenameWithMarker(TYPE, INTERSECTION) },
    KindEntry { kind: "jsx_attribute",                 handling: KindHandling::Rename(PROP) },
    KindEntry { kind: "jsx_closing_element",           handling: KindHandling::Rename(CLOSING) },
    KindEntry { kind: "jsx_element",                   handling: KindHandling::Rename(ELEMENT) },
    KindEntry { kind: "jsx_expression",                handling: KindHandling::Rename(VALUE) },
    KindEntry { kind: "jsx_opening_element",           handling: KindHandling::Rename(OPENING) },
    KindEntry { kind: "jsx_self_closing_element",      handling: KindHandling::Rename(ELEMENT) },
    KindEntry { kind: "jsx_text",                      handling: KindHandling::Rename(TEXT) },
    KindEntry { kind: "lexical_declaration",           handling: KindHandling::Custom },
    KindEntry { kind: "literal_type",                  handling: KindHandling::RenameWithMarker(TYPE, LITERAL) },
    KindEntry { kind: "lookup_type",                   handling: KindHandling::RenameWithMarker(TYPE, LOOKUP) },
    KindEntry { kind: "mapped_type_clause",            handling: KindHandling::Flatten },
    KindEntry { kind: "member_expression",             handling: KindHandling::CustomThenRename(MEMBER) },
    KindEntry { kind: "method_definition",             handling: KindHandling::CustomThenRename(METHOD) },
    KindEntry { kind: "method_signature",              handling: KindHandling::Rename(METHOD) },
    KindEntry { kind: "named_imports",                 handling: KindHandling::Rename(IMPORTS) },
    KindEntry { kind: "namespace_import",              handling: KindHandling::Rename(NAMESPACE) },
    KindEntry { kind: "new_expression",                handling: KindHandling::Rename(NEW) },
    KindEntry { kind: "non_null_expression",           handling: KindHandling::Rename(UNARY) },
    KindEntry { kind: "null",                          handling: KindHandling::Rename(NULL) },
    KindEntry { kind: "number",                        handling: KindHandling::Rename(NUMBER) },
    KindEntry { kind: "object",                        handling: KindHandling::PassThrough },
    KindEntry { kind: "object_assignment_pattern",     handling: KindHandling::RenameWithMarker(PATTERN, DEFAULT) },
    KindEntry { kind: "object_pattern",                handling: KindHandling::RenameWithMarker(PATTERN, OBJECT) },
    KindEntry { kind: "object_type",                   handling: KindHandling::RenameWithMarker(TYPE, OBJECT) },
    KindEntry { kind: "opting_type_annotation",        handling: KindHandling::Rename(ANNOTATION) },
    KindEntry { kind: "optional_chain",                handling: KindHandling::RenameWithMarker(MEMBER, OPTIONAL) },
    KindEntry { kind: "optional_parameter",            handling: KindHandling::Custom },
    KindEntry { kind: "override_modifier",             handling: KindHandling::Custom },
    // `{ a: 1 }` entry in an object literal — tree-sitter calls it
    // `pair`, which already matches our semantic vocabulary.
    KindEntry { kind: "pair",                          handling: KindHandling::PassThrough },
    KindEntry { kind: "parenthesized_expression",      handling: KindHandling::Flatten },
    KindEntry { kind: "parenthesized_type",            handling: KindHandling::RenameWithMarker(TYPE, PARENTHESIZED) },
    KindEntry { kind: "predefined_type",               handling: KindHandling::Rename(TYPE) },
    KindEntry { kind: "private_property_identifier",   handling: KindHandling::Rename(NAME) },
    KindEntry { kind: "program",                       handling: KindHandling::Rename(PROGRAM) },
    KindEntry { kind: "property_identifier",           handling: KindHandling::Custom },
    KindEntry { kind: "property_signature",            handling: KindHandling::Rename(PROPERTY) },
    KindEntry { kind: "public_field_definition",       handling: KindHandling::CustomThenRename(FIELD) },
    KindEntry { kind: "readonly_type",                 handling: KindHandling::RenameWithMarker(TYPE, READONLY) },
    KindEntry { kind: "required_parameter",            handling: KindHandling::Custom },
    KindEntry { kind: "rest_pattern",                  handling: KindHandling::Rename(REST) },
    KindEntry { kind: "return_statement",              handling: KindHandling::Rename(RETURN) },
    KindEntry { kind: "satisfies_expression",          handling: KindHandling::Rename(SATISFIES) },
    KindEntry { kind: "shorthand_property_identifier", handling: KindHandling::Rename(NAME) },
    KindEntry { kind: "shorthand_property_identifier_pattern", handling: KindHandling::Rename(NAME) },
    KindEntry { kind: "spread_element",                handling: KindHandling::Rename(SPREAD) },
    KindEntry { kind: "statement_block",               handling: KindHandling::Rename(BLOCK) },
    KindEntry { kind: "string",                        handling: KindHandling::Rename(STRING) },
    KindEntry { kind: "string_fragment",               handling: KindHandling::Flatten },
    KindEntry { kind: "subscript_expression",          handling: KindHandling::Rename(INDEX) },
    // Tree-sitter leaf for the `super` and `this` keywords. Pass through.
    KindEntry { kind: "super",                         handling: KindHandling::PassThrough },
    KindEntry { kind: "switch_body",                   handling: KindHandling::Rename(BODY) },
    KindEntry { kind: "switch_case",                   handling: KindHandling::Rename(CASE) },
    KindEntry { kind: "switch_default",                handling: KindHandling::Rename(DEFAULT) },
    KindEntry { kind: "switch_statement",              handling: KindHandling::Rename(SWITCH) },
    KindEntry { kind: "template_literal_type",         handling: KindHandling::RenameWithMarker(TYPE, TEMPLATE) },
    KindEntry { kind: "template_string",               handling: KindHandling::Rename(TEMPLATE) },
    KindEntry { kind: "template_substitution",         handling: KindHandling::Rename(INTERPOLATION) },
    KindEntry { kind: "template_type",                 handling: KindHandling::RenameWithMarker(TYPE, TEMPLATE) },
    KindEntry { kind: "ternary_expression",            handling: KindHandling::Custom },
    KindEntry { kind: "this",                          handling: KindHandling::PassThrough },
    KindEntry { kind: "throw_statement",               handling: KindHandling::Rename(THROW) },
    KindEntry { kind: "true",                          handling: KindHandling::Rename(BOOL) },
    KindEntry { kind: "try_statement",                 handling: KindHandling::Rename(TRY) },
    KindEntry { kind: "tuple_type",                    handling: KindHandling::RenameWithMarker(TYPE, TUPLE) },
    KindEntry { kind: "type_alias_declaration",        handling: KindHandling::Custom },
    KindEntry { kind: "type_annotation",               handling: KindHandling::Flatten },
    KindEntry { kind: "type_arguments",                handling: KindHandling::Flatten },
    KindEntry { kind: "type_identifier",               handling: KindHandling::Custom },
    KindEntry { kind: "type_parameter",                handling: KindHandling::Rename(GENERIC) },
    KindEntry { kind: "type_parameters",               handling: KindHandling::Rename(GENERICS) },
    KindEntry { kind: "type_predicate",                handling: KindHandling::Rename(PREDICATE) },
    KindEntry { kind: "type_predicate_annotation",     handling: KindHandling::Rename(PREDICATE) },
    KindEntry { kind: "unary_expression",              handling: KindHandling::CustomThenRename(UNARY) },
    KindEntry { kind: "undefined",                     handling: KindHandling::PassThrough },
    KindEntry { kind: "union_type",                    handling: KindHandling::RenameWithMarker(TYPE, UNION) },
    KindEntry { kind: "update_expression",             handling: KindHandling::CustomThenRename(UNARY) },
    KindEntry { kind: "variable_declaration",          handling: KindHandling::Custom },
    KindEntry { kind: "variable_declarator",           handling: KindHandling::Flatten },
    KindEntry { kind: "while_statement",               handling: KindHandling::Rename(WHILE) },
    KindEntry { kind: "yield_expression",              handling: KindHandling::Rename(YIELD) },
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
