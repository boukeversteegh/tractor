/// Semantic element names — tractor's PHP XML vocabulary after transform.
/// Tree-sitter kind strings (left side of `match` arms) stay as bare
/// literals — they are external vocabulary.
use crate::languages::NodeSpec;
use crate::output::syntax_highlight::SyntaxCategory;

// Named constants retained for use by the transform code. The NODES
// table below is the source of truth for marker/container role and
// syntax category.

// Top-level / declarations
pub const PROGRAM: &str = "program";
pub const NAMESPACE: &str = "namespace";
pub const USE: &str = "use";
pub const CLASS: &str = "class";
pub const INTERFACE: &str = "interface";
pub const TRAIT: &str = "trait";
pub const ENUM: &str = "enum";
pub const METHOD: &str = "method";
pub const FUNCTION: &str = "function";
pub const FIELD: &str = "field";
pub const CONST: &str = "const";
pub const CONSTANT: &str = "constant";

// Members / parameters
pub const PARAMETER: &str = "parameter";
pub const ARGUMENT: &str = "argument";

// Inheritance
pub const EXTENDS: &str = "extends";
pub const IMPLEMENTS: &str = "implements";
pub const TYPES: &str = "types";

// Statements / control flow
pub const RETURN: &str = "return";
pub const IF: &str = "if";
pub const ELSE: &str = "else";
pub const ELSE_IF: &str = "else_if";
pub const FOR: &str = "for";
pub const FOREACH: &str = "foreach";
pub const WHILE: &str = "while";
pub const DO: &str = "do";
pub const SWITCH: &str = "switch";
pub const CASE: &str = "case";
pub const TRY: &str = "try";
pub const CATCH: &str = "catch";
pub const FINALLY: &str = "finally";
pub const THROW: &str = "throw";
pub const ECHO: &str = "echo";
pub const CONTINUE: &str = "continue";
pub const BREAK: &str = "break";
pub const MATCH: &str = "match";
pub const ARM: &str = "arm";
pub const YIELD: &str = "yield";
pub const REQUIRE: &str = "require";
pub const PRINT: &str = "print";
pub const EXIT: &str = "exit";
pub const DECLARE: &str = "declare";
pub const GOTO: &str = "goto";

// Expressions
pub const CALL: &str = "call";
pub const MEMBER: &str = "member";
pub const INDEX: &str = "index";
pub const NEW: &str = "new";
pub const CAST: &str = "cast";
pub const ASSIGN: &str = "assign";
pub const BINARY: &str = "binary";
pub const UNARY: &str = "unary";
pub const TERNARY: &str = "ternary";
pub const ARRAY: &str = "array";
pub const SPREAD: &str = "spread";

// Types / atoms
pub const TYPE: &str = "type";
pub const STRING: &str = "string";
pub const INT: &str = "int";
pub const FLOAT: &str = "float";
pub const BOOL: &str = "bool";
pub const NULL: &str = "null";
pub const VARIABLE: &str = "variable";

// Misc structural
pub const TAG: &str = "tag";
pub const INTERPOLATION: &str = "interpolation";
pub const ATTRIBUTE: &str = "attribute";

// Identifiers / comments / pair / op
pub const NAME: &str = "name";
pub const COMMENT: &str = "comment";
pub const PAIR: &str = "pair";
pub const OP: &str = "op";

// Comment markers — emitted by the shared CommentClassifier. PHP
// supports both `//` (and `#`) line comments and `/* */` block
// comments; the classifier carries that prefix list.
pub const TRAILING: &str = "trailing";
pub const LEADING: &str = "leading";

// Visibility / access modifiers.
pub const PUBLIC: &str = "public";
pub const PRIVATE: &str = "private";
pub const PROTECTED: &str = "protected";

// Other modifiers.
pub const FINAL: &str = "final";
pub const ABSTRACT: &str = "abstract";
pub const READONLY: &str = "readonly";

// Call / member flavor markers.
pub const INSTANCE: &str = "instance";

// Type-shape markers.
pub const PRIMITIVE: &str = "primitive";
pub const UNION: &str = "union";
pub const OPTIONAL: &str = "optional";

// Parameter-shape markers.
pub const VARIADIC: &str = "variadic";

// Anonymous / arrow function shape markers.
pub const ANONYMOUS: &str = "anonymous";
pub const ARROW: &str = "arrow";

// php_tag marker.
pub const OPEN: &str = "open";

// Dual-use names (see NODES below).
pub const STATIC: &str = "static";
pub const DEFAULT: &str = "default";

use SyntaxCategory::*;

/// Per-name metadata — single source of truth for every element
/// name this language's transform can emit.
///
/// Dual-use names set BOTH `marker: true` and `container: true`:
///   - STATIC   — `static_modifier` keyword marker + `scoped_call_expression`
///                shape marker. Also doubles with static property-access shape.
///   - CONSTANT — `enum_case` / `const_element` (container) +
///                `class_constant_access_expression` member-shape marker.
///   - DEFAULT  — `default_statement` (container) + `match_default_expression`
///                arm-shape marker (`<arm><default/>`).
///   - FUNCTION — function_definition (container) + anonymous/arrow
///                function shape markers.
pub const NODES: &[NodeSpec] = &[
    // Top-level / declarations (FUNCTION, CONSTANT dual-use)
    NodeSpec { name: PROGRAM,   marker: false, container: true, syntax: Default },
    NodeSpec { name: NAMESPACE, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: USE,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CLASS,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: INTERFACE, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: TRAIT,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ENUM,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: METHOD,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FUNCTION,  marker: true,  container: true, syntax: Keyword },
    NodeSpec { name: FIELD,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CONST,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CONSTANT,  marker: true,  container: true, syntax: Keyword },

    // Members / parameters
    NodeSpec { name: PARAMETER, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ARGUMENT,  marker: false, container: true, syntax: Default },

    // Inheritance
    NodeSpec { name: EXTENDS,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: IMPLEMENTS, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: TYPES,      marker: false, container: true, syntax: Default },

    // Statements / control flow
    NodeSpec { name: RETURN,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: IF,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ELSE,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ELSE_IF,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FOR,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FOREACH,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: WHILE,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: DO,       marker: false, container: true, syntax: Keyword },
    NodeSpec { name: SWITCH,   marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CASE,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: TRY,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: CATCH,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FINALLY,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: THROW,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ECHO,     marker: false, container: true, syntax: Default },
    NodeSpec { name: CONTINUE, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: BREAK,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: MATCH,    marker: false, container: true, syntax: Default },
    NodeSpec { name: ARM,      marker: false, container: true, syntax: Default },
    NodeSpec { name: YIELD,    marker: false, container: true, syntax: Default },
    NodeSpec { name: REQUIRE,  marker: false, container: true, syntax: Default },
    NodeSpec { name: PRINT,    marker: false, container: true, syntax: Default },
    NodeSpec { name: EXIT,     marker: false, container: true, syntax: Default },
    NodeSpec { name: DECLARE,  marker: false, container: true, syntax: Default },
    NodeSpec { name: GOTO,     marker: false, container: true, syntax: Default },

    // Expressions
    NodeSpec { name: CALL,    marker: false, container: true, syntax: Function },
    NodeSpec { name: MEMBER,  marker: false, container: true, syntax: Default },
    NodeSpec { name: INDEX,   marker: false, container: true, syntax: Default },
    NodeSpec { name: NEW,     marker: false, container: true, syntax: Function },
    NodeSpec { name: CAST,    marker: false, container: true, syntax: Operator },
    NodeSpec { name: ASSIGN,  marker: false, container: true, syntax: Operator },
    NodeSpec { name: BINARY,  marker: false, container: true, syntax: Operator },
    NodeSpec { name: UNARY,   marker: false, container: true, syntax: Operator },
    NodeSpec { name: TERNARY, marker: false, container: true, syntax: Operator },
    NodeSpec { name: ARRAY,   marker: false, container: true, syntax: Default },
    NodeSpec { name: SPREAD,  marker: false, container: true, syntax: Default },

    // Types / atoms
    NodeSpec { name: TYPE,     marker: false, container: true, syntax: Type },
    NodeSpec { name: STRING,   marker: false, container: true, syntax: String },
    NodeSpec { name: INT,      marker: false, container: true, syntax: Number },
    NodeSpec { name: FLOAT,    marker: false, container: true, syntax: Number },
    NodeSpec { name: BOOL,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: NULL,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: VARIABLE, marker: false, container: true, syntax: Default },

    // Misc structural
    NodeSpec { name: TAG,           marker: false, container: true, syntax: Default },
    NodeSpec { name: INTERPOLATION, marker: false, container: true, syntax: Default },
    NodeSpec { name: ATTRIBUTE,     marker: false, container: true, syntax: Default },

    // Identifiers / comments / pair / op
    NodeSpec { name: NAME,    marker: false, container: true, syntax: Identifier },
    NodeSpec { name: COMMENT, marker: false, container: true, syntax: Comment },
    NodeSpec { name: PAIR,    marker: false, container: true, syntax: Default },
    NodeSpec { name: OP,      marker: false, container: true, syntax: Operator },

    // Comment markers
    NodeSpec { name: TRAILING, marker: true, container: false, syntax: Default },
    NodeSpec { name: LEADING,  marker: true, container: false, syntax: Default },

    // Access modifiers — markers only.
    NodeSpec { name: PUBLIC,    marker: true, container: false, syntax: Keyword },
    NodeSpec { name: PRIVATE,   marker: true, container: false, syntax: Keyword },
    NodeSpec { name: PROTECTED, marker: true, container: false, syntax: Keyword },

    // Other modifiers — markers only.
    NodeSpec { name: FINAL,    marker: true, container: false, syntax: Keyword },
    NodeSpec { name: ABSTRACT, marker: true, container: false, syntax: Keyword },
    NodeSpec { name: READONLY, marker: true, container: false, syntax: Keyword },

    // Call / member flavor markers.
    NodeSpec { name: INSTANCE, marker: true, container: false, syntax: Default },

    // Type-shape markers.
    NodeSpec { name: PRIMITIVE, marker: true, container: false, syntax: Default },
    NodeSpec { name: UNION,     marker: true, container: false, syntax: Default },
    NodeSpec { name: OPTIONAL,  marker: true, container: false, syntax: Default },

    // Parameter-shape markers.
    NodeSpec { name: VARIADIC, marker: true, container: false, syntax: Default },

    // Anonymous / arrow function shape markers.
    NodeSpec { name: ANONYMOUS, marker: true, container: false, syntax: Default },
    NodeSpec { name: ARROW,     marker: true, container: false, syntax: Default },

    // php_tag marker.
    NodeSpec { name: OPEN, marker: true, container: false, syntax: Default },

    // Dual-use: STATIC marker + scoped-access marker; DEFAULT marker
    // in match arm + default_statement container.
    NodeSpec { name: STATIC,  marker: true, container: true, syntax: Keyword },
    NodeSpec { name: DEFAULT, marker: true, container: true, syntax: Keyword },
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
