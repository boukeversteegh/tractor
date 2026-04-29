//! Per-kind transformation rules for Go: the `GoKind` → `Rule` table.
//!
//! Read this file to find the rule for a specific kind. Read
//! [`super::transformations`] for the body of any `Rule::Custom`
//! handler the rule references by name. Read [`super::output`] for
//! the output vocabulary (semantic names + NodeSpec metadata).
//!
//! Exhaustive over `GoKind` — the compiler enforces coverage. When
//! the grammar ships a new kind, regenerating `input.rs` adds a
//! variant and this match fails to build until the new kind is
//! classified.
//!
//! Pure data variants (`Rename`, `RenameWithMarker`, `Flatten`,
//! `ExtractOpThenRename`) are executed by the shared
//! [`crate::languages::rule::dispatch`] helper. Custom logic lives in
//! [`super::transformations`].

use crate::languages::rule::Rule;

use super::input::GoKind;
use super::output::*;
use super::transformations;

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

        // ---- Custom (language-specific logic in transformations.rs) ---
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
