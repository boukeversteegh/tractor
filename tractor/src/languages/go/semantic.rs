/// Semantic element names — tractor's Go XML vocabulary after transform.
/// These are the names that appear in tractor's output. The tree-sitter
/// kind strings are external vocabulary, surfaced as the typed
/// [`super::kind::GoKind`] enum.
use crate::languages::rule::Rule;
use crate::languages::NodeSpec;
use crate::output::syntax_highlight::SyntaxCategory;

use super::kind::GoKind;
use super::transformations;

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

// Comment markers — emitted by the shared CommentClassifier.
pub const TRAILING: &str = "trailing";
pub const LEADING: &str = "leading";

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

    // Comment markers
    NodeSpec { name: TRAILING, marker: true, container: false, syntax: Default },
    NodeSpec { name: LEADING,  marker: true, container: false, syntax: Default },

    // Marker-only
    NodeSpec { name: RAW,        marker: true, container: false, syntax: Default },
    NodeSpec { name: SHORT,      marker: true, container: false, syntax: Default },
    NodeSpec { name: EXPORTED,   marker: true, container: false, syntax: Keyword },
    NodeSpec { name: UNEXPORTED, marker: true, container: false, syntax: Keyword },
    NodeSpec { name: NEGATED,    marker: true, container: false, syntax: Default },
    NodeSpec { name: GENERIC,    marker: true, container: false, syntax: Default },
];

// `KINDS: &[KindEntry]` and `rename_target` were dropped when Go
// migrated to the rule-driven dispatcher. Per-kind handling now lives
// in `rule(GoKind) -> Rule` below — an exhaustive match over the
// typed enum that the compiler enforces. Other languages still use
// `KindEntry` / `KindHandling` until they migrate.

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

/// Per-kind transformation rule for Go.
///
/// Exhaustive over `GoKind` — the compiler enforces coverage. When
/// the grammar ships a new kind, regenerating `kind.rs` adds a
/// variant and this match fails to build until the new kind is
/// classified.
///
/// Pure data variants (`Rename`, `RenameWithMarker`, `Flatten`,
/// `ExtractOpThenRename`) are executed by the shared
/// [`crate::languages::rule::dispatch`] helper. Custom logic lives in
/// [`super::handlers`].
pub fn rule(k: GoKind) -> Rule {
    use Rule::*;
    match k {
        // ---- ExtractOpThenRename ---------------------------------------
        GoKind::BinaryExpression => ExtractOpThenRename(BINARY),
        GoKind::UnaryExpression  => ExtractOpThenRename(UNARY),

        // ---- RenameWithMarker -----------------------------------------
        GoKind::FunctionType        => RenameWithMarker(TYPE, FUNCTION),
        GoKind::GenericType         => RenameWithMarker(TYPE, GENERIC),
        GoKind::NegatedType         => RenameWithMarker(TYPE, NEGATED),
        GoKind::TypeSwitchStatement => RenameWithMarker(SWITCH, TYPE),

        // ---- Flatten with field distribution --------------------------
        GoKind::ArgumentList => Flatten { distribute_field: Some("arguments") },

        // ---- Pure Flatten ---------------------------------------------
        GoKind::Block
        | GoKind::FieldDeclarationList
        | GoKind::ExpressionList
        | GoKind::ImportSpec
        | GoKind::ConstSpec
        | GoKind::VarSpec
        | GoKind::LiteralElement
        | GoKind::KeyedElement
        | GoKind::LiteralValue
        | GoKind::VarSpecList
        | GoKind::ImportSpecList
        | GoKind::ForClause
        | GoKind::TypeParameterList
        | GoKind::TypeParameterDeclaration
        | GoKind::TypeElem
        | GoKind::TypeConstraint
        | GoKind::QualifiedType
        | GoKind::TypeCase
        | GoKind::InterpretedStringLiteralContent
        | GoKind::RawStringLiteralContent
        | GoKind::EscapeSequence => Flatten { distribute_field: None },

        // ---- Custom (language-specific logic in handlers.rs) ----------
        GoKind::ExpressionStatement   => Custom(transformations::expression_statement),
        GoKind::ParameterList         => Custom(transformations::parameter_list),
        GoKind::TypeDeclaration       => Custom(transformations::type_declaration),
        GoKind::RawStringLiteral      => Custom(transformations::raw_string_literal),
        GoKind::ShortVarDeclaration   => Custom(transformations::short_var_declaration),
        GoKind::FunctionDeclaration   => Custom(transformations::function_declaration),
        GoKind::MethodDeclaration     => Custom(transformations::method_declaration),
        GoKind::FieldDeclaration      => Custom(transformations::field_declaration),
        GoKind::TypeSpec              => Custom(transformations::type_spec),
        GoKind::TypeAlias             => Custom(transformations::type_alias),
        GoKind::IfStatement           => Custom(transformations::if_statement),
        GoKind::TypeIdentifier        => Custom(transformations::type_identifier),
        GoKind::Comment               => Custom(transformations::comment),

        // ---- Pure Rename ----------------------------------------------
        GoKind::Identifier               => Rename(NAME),
        GoKind::AssignmentStatement      => Rename(ASSIGN),
        GoKind::BlankIdentifier          => Rename(NAME),
        GoKind::BreakStatement           => Rename(BREAK),
        GoKind::CallExpression           => Rename(CALL),
        GoKind::ChannelType              => Rename(CHAN),
        GoKind::CommunicationCase        => Rename(CASE),
        GoKind::CompositeLiteral         => Rename(LITERAL),
        GoKind::ConstDeclaration         => Rename(CONST),
        GoKind::ContinueStatement        => Rename(CONTINUE),
        GoKind::DecStatement             => Rename(UNARY),
        GoKind::DefaultCase              => Rename(DEFAULT),
        GoKind::DeferStatement           => Rename(DEFER),
        GoKind::ExpressionSwitchStatement => Rename(SWITCH),
        GoKind::False                    => Rename(FALSE),
        GoKind::FieldIdentifier          => Rename(NAME),
        GoKind::FloatLiteral             => Rename(FLOAT),
        GoKind::ForStatement             => Rename(FOR),
        GoKind::FuncLiteral              => Rename(CLOSURE),
        GoKind::GoStatement              => Rename(GO),
        GoKind::GotoStatement            => Rename(GOTO),
        GoKind::ImportDeclaration        => Rename(IMPORT),
        GoKind::IncStatement             => Rename(UNARY),
        GoKind::IndexExpression          => Rename(INDEX),
        GoKind::InterfaceType            => Rename(INTERFACE),
        GoKind::InterpretedStringLiteral => Rename(STRING),
        GoKind::IntLiteral               => Rename(INT),
        GoKind::LabelName                => Rename(LABEL),
        GoKind::LabeledStatement         => Rename(LABELED),
        GoKind::MapType                  => Rename(MAP),
        GoKind::MethodElem               => Rename(METHOD),
        GoKind::Nil                      => Rename(NIL),
        GoKind::PackageClause            => Rename(PACKAGE),
        GoKind::PackageIdentifier        => Rename(NAME),
        GoKind::ParameterDeclaration     => Rename(PARAMETER),
        GoKind::PointerType              => Rename(POINTER),
        GoKind::RangeClause              => Rename(RANGE),
        GoKind::ReceiveStatement         => Rename(RECEIVE),
        GoKind::ReturnStatement          => Rename(RETURN),
        GoKind::RuneLiteral              => Rename(CHAR),
        GoKind::SelectStatement          => Rename(SELECT),
        GoKind::SelectorExpression       => Rename(MEMBER),
        GoKind::SendStatement            => Rename(SEND),
        GoKind::SliceType                => Rename(SLICE),
        GoKind::SourceFile               => Rename(FILE),
        GoKind::StructType               => Rename(STRUCT),
        GoKind::True                     => Rename(TRUE),
        GoKind::TypeArguments            => Rename(ARGUMENTS),
        GoKind::TypeAssertionExpression  => Rename(ASSERT),
        GoKind::VarDeclaration           => Rename(VAR),
        GoKind::VariadicParameterDeclaration => Rename(PARAMETER),

        // ---- Passthrough (kind name already matches our vocabulary) ---

        // `iota` — already in NODES, intentionally a leaf. Correct.
        GoKind::Iota => Custom(transformations::passthrough),

        // TODO: `array_type` should be `Rename(ARRAY)` (new semantic
        // constant) for consistency with sibling type kinds:
        //   slice_type   → SLICE
        //   map_type     → MAP
        //   pointer_type → POINTER
        //   channel_type → CHAN
        // Also fold `implicit_length_array_type` into the same target.
        // Currently `<array_type>` survives as a raw kind name — not in
        // NODES, only avoided in invariant tests because no fixture
        // exercises it.
        GoKind::ArrayType
        | GoKind::ImplicitLengthArrayType => Custom(transformations::passthrough),

        // TODO: `dot` (the `.` in `import . "pkg"`) is a name placeholder.
        // Likely should be `Rename(NAME)` like other identifier-like
        // leaves (`blank_identifier`, `field_identifier`).
        GoKind::Dot => Custom(transformations::passthrough),

        // TODO: `expression_case` should be `Rename(CASE)` for
        // consistency — its sibling kinds already do this:
        //   communication_case → CASE
        //   default_case       → DEFAULT
        //   expression_case    → ??  ← currently `<expression_case>` (passthrough)
        // Test impact: `tests/transform/go/switch_markers.rs` queries
        // `expression_case/value/int='1'` — would update to
        // `case/value/int='1'`. No snapshot impact (blueprint doesn't
        // exercise expression switches).
        GoKind::ExpressionCase => Custom(transformations::passthrough),

        // TODO: `parenthesized_expression` and `parenthesized_type`
        // should be `Flatten { distribute_field: None }` — parens are
        // pure grammar grouping with no semantic content. Other
        // languages already flatten this kind (csharp, typescript, etc.).
        GoKind::ParenthesizedExpression
        | GoKind::ParenthesizedType => Custom(transformations::passthrough),

        // TODO: `empty_statement` (a bare `;`) carries no semantic
        // content; either `Flatten` or skip it entirely.
        GoKind::EmptyStatement => Custom(transformations::passthrough),

        // TODO: `fallthrough_statement` is a real Go control-flow
        // construct. Likely wants its own semantic name (FALLTHROUGH)
        // alongside BREAK / CONTINUE / GOTO, with a corresponding
        // NodeSpec entry.
        GoKind::FallthroughStatement => Custom(transformations::passthrough),

        // TODO: `imaginary_literal` (`1i`) is a number-shaped literal.
        // Likely `Rename(FLOAT)` (or a new IMAG semantic) — currently
        // `<imaginary_literal>` survives as raw kind.
        GoKind::ImaginaryLiteral => Custom(transformations::passthrough),

        // TODO: `slice_expression` (`s[i:j]`) is structurally similar
        // to `index_expression` (`s[i]`). Either reuse `Rename(INDEX)`
        // with a `<slice/>` marker, or introduce a SLICE_OP semantic.
        GoKind::SliceExpression => Custom(transformations::passthrough),

        // TODO: `type_conversion_expression` (`T(x)`) is semantically
        // a call. Likely `Rename(CALL)` with a `<type/>` marker, so
        // `//call[type]` matches every type conversion uniformly.
        GoKind::TypeConversionExpression => Custom(transformations::passthrough),

        // TODO: `type_instantiation_expression` (`Foo[T]`) is generic
        // application. Could share the `<type><generic/>...` shape
        // already used for `generic_type`, or get its own semantic.
        GoKind::TypeInstantiationExpression => Custom(transformations::passthrough),

        // TODO: `variadic_argument` (`args...`) is an argument variant.
        // Likely `Rename(ARGUMENT)` with a `<variadic/>` marker so
        // `//argument[variadic]` picks them out.
        GoKind::VariadicArgument => Custom(transformations::passthrough),
    }
}
