//! Per-kind transformation rules for Go: the `GoKind` → `Rule` table.
//!
//! Read this file to find the rule for a specific kind. Read
//! [`super::transformations`] for the body of any `Rule::Custom`
//! handler the rule references by name. Read [`super::output`] for
//! the output vocabulary (semantic names + TractorNodeSpec metadata).
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
use super::output::TractorNode::{self, *};
use super::transformations;

pub fn rule(k: GoKind) -> Rule<TractorNode> {
    use Rule::*;
    match k {
        // ---- ExtractOpThenRename ---------------------------------------
        GoKind::BinaryExpression => ExtractOpThenRename(Binary),
        GoKind::UnaryExpression  => ExtractOpThenRename(Unary),

        // ---- RenameWithMarker -----------------------------------------
        GoKind::FunctionType        => RenameWithMarker(Type, Function),
        GoKind::GenericType         => RenameWithMarker(Type, Generic),
        GoKind::NegatedType         => RenameWithMarker(Type, Negated),
        GoKind::TypeSwitchStatement => RenameWithMarker(Switch, Type),

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
        GoKind::Identifier               => Rename(Name),
        GoKind::AssignmentStatement      => Rename(Assign),
        GoKind::BlankIdentifier          => Rename(Name),
        GoKind::BreakStatement           => Custom(transformations::break_statement),
        GoKind::CallExpression           => Rename(Call),
        GoKind::ChannelType              => Rename(Chan),
        GoKind::CommunicationCase        => Rename(Case),
        GoKind::CompositeLiteral         => Rename(Literal),
        GoKind::ConstDeclaration         => Rename(Const),
        GoKind::ContinueStatement        => Custom(transformations::continue_statement),
        GoKind::DecStatement             => ExtractOpThenRename(Unary),
        GoKind::DefaultCase              => Rename(Default),
        GoKind::DeferStatement           => Rename(Defer),
        GoKind::ExpressionSwitchStatement => Rename(Switch),
        GoKind::False                    => Rename(False),
        GoKind::FieldIdentifier          => Rename(Name),
        GoKind::FloatLiteral             => Rename(Float),
        GoKind::ForStatement             => Rename(For),
        GoKind::FuncLiteral              => Rename(Closure),
        GoKind::GoStatement              => Custom(transformations::go_statement),
        GoKind::GotoStatement            => Custom(transformations::goto_statement),
        GoKind::ImportDeclaration        => Rename(Import),
        GoKind::IncStatement             => ExtractOpThenRename(Unary),
        GoKind::IndexExpression          => Rename(Index),
        GoKind::InterfaceType            => Rename(Interface),
        GoKind::InterpretedStringLiteral => Rename(String),
        GoKind::IntLiteral               => Rename(Int),
        GoKind::LabelName                => Rename(Label),
        GoKind::LabeledStatement         => Rename(Labeled),
        GoKind::MapType                  => Rename(Map),
        GoKind::MethodElem               => Rename(Method),
        GoKind::Nil                      => Rename(Nil),
        GoKind::PackageClause            => Rename(Package),
        GoKind::PackageIdentifier        => Rename(Name),
        GoKind::ParameterDeclaration     => Rename(Parameter),
        GoKind::PointerType              => Rename(Pointer),
        GoKind::RangeClause              => Rename(Range),
        GoKind::ReceiveStatement         => Rename(Receive),
        GoKind::ReturnStatement          => Custom(transformations::return_statement),
        GoKind::RuneLiteral              => Rename(Char),
        GoKind::SelectStatement          => Rename(Select),
        GoKind::SelectorExpression       => Rename(Member),
        GoKind::SendStatement            => Rename(Send),
        GoKind::SliceType                => Rename(Slice),
        GoKind::SourceFile               => Rename(File),
        GoKind::StructType               => Rename(Struct),
        GoKind::True                     => Rename(True),
        GoKind::TypeArguments            => Rename(Arguments),
        GoKind::TypeAssertionExpression  => Rename(Assert),
        GoKind::VarDeclaration           => Rename(Var),
        GoKind::VariadicParameterDeclaration => Rename(Parameter),

        // ---- Passthrough (kind name already matches our vocabulary) ---

        // `iota` — already in NODES, intentionally a leaf. Correct.
        GoKind::Iota => Passthrough,

        // `array_type` joins the sibling type kinds (slice/map/pointer/chan)
        // under their semantic name. `implicit_length_array_type` (`[...]T`)
        // shares the array shape with an `<implicit/>` marker so
        // `//array[implicit]` picks out compiler-inferred lengths.
        GoKind::ArrayType                => Rename(Array),
        GoKind::ImplicitLengthArrayType  => RenameWithMarker(Array, Implicit),

        // `dot` — the `.` placeholder in `import . "pkg"`. Treated as
        // an identifier-like leaf, same as `blank_identifier` and
        // `field_identifier`.
        GoKind::Dot => Rename(Name),

        // `expression_case` joins `communication_case → Case` and
        // `default_case → Default` under `<case>`.
        GoKind::ExpressionCase => Rename(Case),

        // Parens are pure grouping with no semantic content; flatten
        // so the inner expression / type bubbles up. Matches the
        // treatment in csharp, typescript, etc.
        GoKind::ParenthesizedExpression
        | GoKind::ParenthesizedType => Flatten { distribute_field: None },

        // `empty_statement` (a bare `;`) carries no semantic content.
        GoKind::EmptyStatement => Flatten { distribute_field: None },

        // `fallthrough_statement` is real Go control-flow; renames to
        // `<fallthrough>` alongside `<break>`, `<continue>`, `<goto>`.
        GoKind::FallthroughStatement => Custom(transformations::fallthrough_statement),

        // `imaginary_literal` (`1i`) is a number-shaped literal,
        // grouped with floats.
        GoKind::ImaginaryLiteral => Rename(Float),

        // `slice_expression` (`s[i:j]`) shares the index-access shape
        // with a `<slice/>` marker — `//index[slice]` picks slice ops out.
        // (Slice is dual-use: container for slice types, marker here.)
        GoKind::SliceExpression => RenameWithMarker(Index, Slice),

        // `type_conversion_expression` (`T(x)`) is semantically a call
        // whose callee position is a type. `<call[type]>` so
        // `//call[type]` matches every type conversion uniformly.
        GoKind::TypeConversionExpression => RenameWithMarker(Call, Type),

        // `type_instantiation_expression` (`Foo[T]`) is generic
        // application — same `<type[generic]>` shape as `generic_type`.
        GoKind::TypeInstantiationExpression => RenameWithMarker(Type, Generic),

        // `variadic_argument` (`args...`) renames to `<spread>` —
        // matches the cross-language spread vocabulary (TS / Python /
        // Ruby) so `//spread` finds variadic call sites uniformly.
        GoKind::VariadicArgument => Rename(Spread),
    }
}
