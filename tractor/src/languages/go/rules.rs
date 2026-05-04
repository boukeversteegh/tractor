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
        // `~int` — type approximation in generic constraints, NOT
        // negation. Means "any type whose underlying type is int".
        // Marker name matches Go developer terminology (Goal #5,
        // Principle #1) — `~` is the "tilde / approximation"
        // operator in the spec, not "negation".
        GoKind::NegatedType         => RenameWithMarker(Type, Approximation),
        GoKind::TypeSwitchStatement => RenameWithMarker(Switch, Type),

        // ---- Flatten with field distribution --------------------------
        GoKind::ArgumentList => Flatten { distribute_list: Some("arguments") },

        // `X: 1` inside a composite literal — preserve the pairing
        // structurally as `<pair><name>X</name><value/expression/int=1/></pair>`
        // (Principle #19 role-mixed wrap). Without this, the inner
        // literal_value's body distributed list= per element name and
        // produced parallel `name: [X, Y]` / `value: [1, 2]` arrays
        // that lost the X↔1 / Y↔2 pairing.
        GoKind::KeyedElement              => Rename(Pair),

        // ---- Pure Flatten ---------------------------------------------
        GoKind::Block
        | GoKind::FieldDeclarationList
        | GoKind::ExpressionList
        | GoKind::LiteralElement
        | GoKind::LiteralValue
        | GoKind::ForClause
        | GoKind::TypeElem
        | GoKind::TypeConstraint
        | GoKind::InterpretedStringLiteralContent
        | GoKind::RawStringLiteralContent
        | GoKind::EscapeSequence => Flatten { distribute_list: None },

        // `[T any, U comparable]` — generic parameter list. Per
        // Principle #12 (no list containers): flatten with
        // `list="generics"` distribution so each parameter becomes
        // a flat `<generic>` sibling of the enclosing declaration.
        // Matches Java / Rust / TS shape.
        GoKind::TypeParameterList        => Flatten { distribute_list: Some("generics") },
        GoKind::TypeParameterDeclaration => Rename(Generic),

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
        // `pkg.Name` qualified type — receiver and type-name play
        // different roles. Wrap the package identifier in `<package>`
        // and rename to `<type>` so the type-name stays as bare
        // `<name>` (the canonical singleton property of a `<type>`).
        // Per Principle #19 + iter 147 (the member-access analog).
        GoKind::QualifiedType         => Custom(transformations::qualified_type),
        GoKind::Comment               => Custom(transformations::comment),

        // ---- Pure Rename ----------------------------------------------
        GoKind::Identifier               => Rename(Name),
        // Iter 340: extract `=` / `+=` / `-=` operator into `<op>` to
        // match TS/C#/Java/PHP/Rust shape (Principle #5). Source-text
        // preservation holds via `prepend_op_element`'s detach +
        // before/op/after splice — bare `=` text leaf becomes
        // `<op>=</op>`, no duplication.
        GoKind::AssignmentStatement      => ExtractOpThenRename(Assign),
        GoKind::BlankIdentifier          => Rename(Name),
        GoKind::BreakStatement           => RenameStripKeyword(Break, "break"),
        GoKind::CallExpression           => Rename(Call),
        GoKind::ChannelType              => Rename(Chan),
        GoKind::CommunicationCase        => Rename(Case),
        GoKind::CompositeLiteral         => Rename(Literal),
        // ConstDeclaration / VarDeclaration mirror import handling:
        // strip the bare keyword + parens, flatten so each
        // ConstSpec / VarSpec becomes its own sibling. The single-
        // binding case yields one sibling; the block form yields
        // multiple. (Group preservation deferred — devs can
        // reconstruct from source position if needed.)
        GoKind::ConstDeclaration         => Custom(transformations::const_or_var_declaration),
        GoKind::ConstSpec                => Rename(Const),
        GoKind::ContinueStatement        => RenameStripKeyword(Continue, "continue"),
        GoKind::DecStatement             => ExtractOpThenRename(Unary),
        GoKind::DefaultCase              => Rename(Default),
        GoKind::DeferStatement           => Rename(Defer),
        GoKind::ExpressionSwitchStatement => Rename(Switch),
        GoKind::False                    => Rename(False),
        GoKind::FieldIdentifier          => Rename(Name),
        GoKind::FloatLiteral             => Rename(Float),
        GoKind::ForStatement             => Custom(transformations::for_statement),
        GoKind::FuncLiteral              => Rename(Closure),
        GoKind::GoStatement              => RenameStripKeyword(Go, "go"),
        GoKind::GotoStatement            => RenameStripKeyword(Goto, "goto"),
        GoKind::ImportDeclaration        => Custom(transformations::import_declaration),
        GoKind::ImportSpec               => Custom(transformations::import_spec),
        GoKind::ImportSpecList           => Custom(transformations::import_spec_list),
        GoKind::IncStatement             => ExtractOpThenRename(Unary),
        GoKind::IndexExpression          => Custom(transformations::index_expression),
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
        GoKind::ReturnStatement          => RenameStripKeyword(Return, "return"),
        GoKind::RuneLiteral              => Rename(Char),
        GoKind::SelectStatement          => Rename(Select),
        // `obj.field` — receiver and accessed-field play different
        // roles. Per Principle #19: each role gets a slot-named
        // container (`<object>` / `<property>`). Matches TS / Java /
        // Python (iter 147) shape.
        GoKind::SelectorExpression       => Custom(transformations::selector_expression),
        GoKind::SendStatement            => Rename(Send),
        GoKind::SliceType                => Rename(Slice),
        GoKind::SourceFile               => Rename(File),
        GoKind::StructType               => Rename(Struct),
        GoKind::True                     => Rename(True),
        // `[T, string]` — generic type arguments. Per Principle #12,
        // each argument becomes a flat `<type>` sibling under
        // `<type[generic]>` with `list="arguments"` for
        // JSON-array recovery. The Custom handler also lifts the
        // inner `type_elem` wrappers so the field attribute lands on
        // the real type, not on the disappearing wrapper.
        GoKind::TypeArguments            => Custom(transformations::type_arguments),
        GoKind::TypeAssertionExpression  => Rename(Assert),
        GoKind::VarDeclaration           => Custom(transformations::const_or_var_declaration),
        GoKind::VarSpec                  => Rename(Var),
        GoKind::VarSpecList              => Custom(transformations::import_spec_list),
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
        GoKind::TypeCase       => Rename(Case),

        // Parens are pure grouping with no semantic content; flatten
        // so the inner expression / type bubbles up. Matches the
        // treatment in csharp, typescript, etc.
        GoKind::ParenthesizedExpression
        | GoKind::ParenthesizedType => Flatten { distribute_list: None },

        // `empty_statement` (a bare `;`) carries no semantic content.
        GoKind::EmptyStatement => Flatten { distribute_list: None },

        // `fallthrough_statement` is real Go control-flow; renames to
        // `<fallthrough>` alongside `<break>`, `<continue>`, `<goto>`.
        GoKind::FallthroughStatement => RenameStripKeyword(Fallthrough, "fallthrough"),

        // `imaginary_literal` (`1i`) is a number-shaped literal,
        // grouped with floats.
        GoKind::ImaginaryLiteral => Rename(Float),

        // `slice_expression` (`s[i:j]`, `s[i:j:k]`, `s[:]`) shares the
        // index-access shape with a `<slice/>` marker — `//index[slice]`
        // picks slice ops out. The Custom handler wraps `field="operand"`
        // in `<object>` (matching index_expression iter 284) and the
        // bounds `field="start"`/`field="end"`/`field="capacity"` in
        // `<from>` / `<to>` / `<capacity>` slots so two-`<int>` siblings
        // can't collide on a singleton JSON key.
        // (Slice is dual-use: container for slice types, marker here.)
        GoKind::SliceExpression => Custom(transformations::slice_expression),

        // `type_conversion_expression` (`T(x)`) is semantically a call
        // whose callee position is a type. `<call[type]>` so
        // `//call[type]` matches every type conversion uniformly.
        GoKind::TypeConversionExpression => RenameWithMarker(Call, Type),

        // `type_instantiation_expression` (`Foo[T]`) is generic
        // application — same `<type[generic]>` shape as `generic_type`.
        // `Map[int, string]` standalone (e.g. `var f = Map[int, string]`).
        // Same conceptual shape as `type_arguments` inside `generic_type`
        // (`Container[T]`); custom handler tags non-head type siblings
        // with `list="arguments"` for JSON-array recovery.
        GoKind::TypeInstantiationExpression => Custom(transformations::type_instantiation_expression),

        // `variadic_argument` (`args...`) renames to `<spread>` —
        // matches the cross-language spread vocabulary (TS / Python /
        // Ruby) so `//spread` finds variadic call sites uniformly.
        GoKind::VariadicArgument => Rename(Spread),
    }
}
