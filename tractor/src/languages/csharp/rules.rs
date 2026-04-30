//! Per-kind transformation rules for C#: the `CsKind` → `Rule<CsName>`
//! table.
//!
//! Read this file to find the rule for a specific kind. Read
//! [`super::transformations`] for the body of any `Rule::Custom`
//! handler the rule references by name. Read [`super::output`] for
//! the output vocabulary (`CsName` enum + per-name metadata).
//!
//! Exhaustive over `CsKind` — the compiler enforces coverage. When
//! the grammar ships a new kind, regenerating `input.rs` adds a
//! variant and this match fails to build until the new kind is
//! classified.
//!
//! Pure data variants (`Rename`, `RenameWithMarker`, `Flatten`,
//! `ExtractOpThenRename`) are executed by the shared
//! [`crate::languages::rule::dispatch`] helper. Custom logic lives in
//! [`super::transformations`].

use crate::languages::rule::Rule;

use super::input::CsKind;
use super::output::CsName::{self, *};
use super::transformations;

/// Shorthand for the `default-access-then-rename` shape used by all 9
/// C# declaration kinds. Bakes in C#'s default-access resolver so the
/// rule arms read as data.
fn da(to: CsName) -> Rule<CsName> {
    Rule::DefaultAccessThenRename {
        to,
        default_access: transformations::default_access_for_declaration,
    }
}

pub fn rule(k: CsKind) -> Rule<CsName> {
    use Rule::*;
    match k {
        // ---- ExtractOpThenRename ---------------------------------------
        CsKind::BinaryExpression     => ExtractOpThenRename(Binary),
        CsKind::UnaryExpression      => ExtractOpThenRename(Unary),
        CsKind::AssignmentExpression => ExtractOpThenRename(Assign),

        // ---- RenameWithMarker ------------------------------------------
        CsKind::ArrayType                   => RenameWithMarker(Type, Array),
        CsKind::ConditionalAccessExpression => RenameWithMarker(Member, Conditional),
        CsKind::ConstantPattern             => RenameWithMarker(Pattern, Constant),
        CsKind::DeclarationPattern          => RenameWithMarker(Pattern, Declaration),
        CsKind::FunctionPointerType         => RenameWithMarker(Type, Function),
        CsKind::MemberAccessExpression      => RenameWithMarker(Member, Instance),
        CsKind::MemberBindingExpression     => RenameWithMarker(Member, Conditional),
        CsKind::PointerType                 => RenameWithMarker(Type, Pointer),
        CsKind::PrefixUnaryExpression       => RenameWithMarker(Unary, Prefix),
        CsKind::RecursivePattern            => RenameWithMarker(Pattern, Recursive),
        CsKind::RefType                     => RenameWithMarker(Type, Ref),
        CsKind::RelationalPattern           => RenameWithMarker(Pattern, Relational),
        CsKind::TuplePattern                => RenameWithMarker(Pattern, Tuple),
        CsKind::TupleType                   => RenameWithMarker(Type, Tuple),

        // ---- Flatten with field distribution ---------------------------
        CsKind::AccessorList          => Flatten { distribute_field: Some("accessors") },
        CsKind::ArgumentList          => Flatten { distribute_field: Some("arguments") },
        CsKind::AttributeArgumentList => Flatten { distribute_field: Some("arguments") },
        CsKind::AttributeList         => Flatten { distribute_field: Some("attributes") },
        CsKind::BracketedParameterList => Flatten { distribute_field: Some("parameters") },
        CsKind::ParameterList         => Flatten { distribute_field: Some("parameters") },
        CsKind::TypeArgumentList      => Flatten { distribute_field: Some("arguments") },
        CsKind::TypeParameterList     => Flatten { distribute_field: Some("generics") },

        // ---- Pure Flatten ----------------------------------------------
        CsKind::ArrowExpressionClause
        | CsKind::DeclarationList
        | CsKind::EnumMemberDeclarationList
        | CsKind::EscapeSequence
        | CsKind::InterpolationBrace
        | CsKind::InterpolationStart
        | CsKind::LocalDeclarationStatement
        | CsKind::ParenthesizedExpression
        | CsKind::QualifiedName
        | CsKind::RawStringContent
        | CsKind::RawStringEnd
        | CsKind::RawStringStart
        | CsKind::StringContent
        | CsKind::StringLiteralContent => Flatten { distribute_field: None },

        // ---- DefaultAccessThenRename — declarations with implicit
        //      access modifier (see `transformations::default_access_for_declaration`).
        CsKind::ClassDeclaration       => da(Class),
        CsKind::ConstructorDeclaration => da(Constructor),
        CsKind::EnumDeclaration        => da(Enum),
        CsKind::FieldDeclaration       => da(Field),
        CsKind::InterfaceDeclaration   => da(Interface),
        CsKind::MethodDeclaration      => da(Method),
        CsKind::PropertyDeclaration    => da(Property),
        CsKind::RecordDeclaration      => da(Record),
        CsKind::StructDeclaration      => da(Struct),

        // ---- Custom (language-specific logic in transformations.rs) ---
        CsKind::AccessorDeclaration           => Custom(transformations::accessor_declaration),
        CsKind::Comment                       => Custom(transformations::comment),
        CsKind::ConditionalExpression         => Custom(transformations::conditional_expression),
        CsKind::GenericName                   => Custom(transformations::generic_name),
        CsKind::Identifier                    => Custom(transformations::identifier),
        CsKind::IfStatement                   => Custom(transformations::if_statement),
        CsKind::ImplicitType                  => Custom(transformations::implicit_type),
        CsKind::InterpolatedStringExpression  => Custom(transformations::interpolated_string_expression),
        CsKind::Modifier                      => Custom(transformations::modifier),
        CsKind::NullableType                  => Custom(transformations::nullable_type),
        CsKind::PostfixUnaryExpression        => Custom(transformations::postfix_unary_expression),
        CsKind::PredefinedType                => Custom(transformations::predefined_type),
        CsKind::VariableDeclaration           => Custom(transformations::variable_declaration),

        // `where T : new()` / constraint-clause kinds — consumed by the
        // shared `attach_where_clause_constraints` post-transform; they
        // never reach the dispatcher in practice. Passthrough is the
        // safe noop.
        CsKind::ConstructorConstraint            => Custom(transformations::passthrough),
        CsKind::TypeParameterConstraint          => Custom(transformations::passthrough),
        CsKind::TypeParameterConstraintsClause   => Custom(transformations::passthrough),

        // ---- Pure Rename -----------------------------------------------
        CsKind::Argument                       => Rename(Argument),
        CsKind::Attribute                      => Rename(Attribute),
        CsKind::AttributeArgument              => Rename(Argument),
        CsKind::AwaitExpression                => Rename(Await),
        CsKind::BaseList                       => Rename(Extends),
        CsKind::Block                          => Rename(Block),
        CsKind::BooleanLiteral                 => Rename(Bool),
        CsKind::BreakStatement                 => Rename(Break),
        CsKind::CatchClause                    => Rename(Catch),
        CsKind::CatchDeclaration               => Rename(Declaration),
        CsKind::CatchFilterClause              => Rename(Filter),
        CsKind::CompilationUnit                => Rename(Unit),
        CsKind::ConstructorInitializer         => Rename(Chain),
        CsKind::ContinueStatement              => Rename(Continue),
        CsKind::DelegateDeclaration            => Rename(Delegate),
        CsKind::DestructorDeclaration          => Rename(Destructor),
        CsKind::DoStatement                    => Rename(Do),
        CsKind::ElementBindingExpression       => Rename(Index),
        CsKind::EnumMemberDeclaration          => Rename(Constant),
        CsKind::EventFieldDeclaration          => Rename(Event),
        CsKind::ExpressionStatement            => Rename(Expression),
        CsKind::FileScopedNamespaceDeclaration => Rename(Namespace),
        CsKind::FinallyClause                  => Rename(Finally),
        CsKind::ForStatement                   => Rename(For),
        CsKind::ForeachStatement               => Rename(Foreach),
        CsKind::FromClause                     => Rename(From),
        CsKind::GroupClause                    => Rename(Group),
        CsKind::ImplicitObjectCreationExpression => Rename(New),
        CsKind::ImplicitParameter              => Rename(Parameter),
        CsKind::IndexerDeclaration             => Rename(Indexer),
        CsKind::InitializerExpression          => Rename(Literal),
        CsKind::IntegerLiteral                 => Rename(Int),
        CsKind::InvocationExpression           => Rename(Call),
        CsKind::IsPatternExpression            => Rename(Is),
        CsKind::JoinClause                     => Rename(Join),
        CsKind::LambdaExpression               => Rename(Lambda),
        CsKind::LetClause                      => Rename(Let),
        CsKind::LocalFunctionStatement         => Rename(Method),
        CsKind::NamespaceDeclaration           => Rename(Namespace),
        CsKind::NullLiteral                    => Rename(Null),
        CsKind::ObjectCreationExpression       => Rename(New),
        CsKind::OperatorDeclaration            => Rename(Operator),
        CsKind::OrderByClause                  => Rename(Order),
        CsKind::Parameter                      => Rename(Parameter),
        CsKind::PropertyPatternClause          => Rename(Properties),
        CsKind::QueryExpression                => Rename(Query),
        CsKind::RangeExpression                => Rename(Range),
        CsKind::RawStringLiteral               => Rename(String),
        CsKind::RealLiteral                    => Rename(Float),
        CsKind::ReturnStatement                => Rename(Return),
        CsKind::SelectClause                   => Rename(Select),
        CsKind::StringLiteral                  => Rename(String),
        CsKind::SwitchBody                     => Rename(Body),
        CsKind::SwitchExpression               => Rename(Switch),
        CsKind::SwitchExpressionArm            => Rename(Arm),
        CsKind::SwitchSection                  => Rename(Section),
        CsKind::SwitchStatement                => Rename(Switch),
        CsKind::ThrowStatement                 => Rename(Throw),
        CsKind::TryStatement                   => Rename(Try),
        CsKind::TupleElement                   => Rename(Element),
        CsKind::TupleExpression                => Rename(Tuple),
        CsKind::TypeParameter                  => Rename(Generic),
        CsKind::UsingDirective                 => Rename(Import),
        CsKind::UsingStatement                 => Rename(Using),
        CsKind::VariableDeclarator             => Rename(Declarator),
        CsKind::VerbatimStringLiteral          => Rename(String),
        CsKind::WhenClause                     => Rename(When),
        CsKind::WhereClause                    => Rename(Where),
        CsKind::WhileStatement                 => Rename(While),

        // ---- Passthrough — kind name already matches the vocabulary,
        //      OR the kind is unhandled and the dispatcher leaves it as
        //      its raw grammar name (the previous behavior of the
        //      catch-all `_` arm when `apply_rename` returned `None`).
        //
        // Many of these are TODO candidates for real semantic upgrades —
        // see the propagation plan. For now, preserve old behavior so
        // snapshots stay byte-identical.

        // Already matches our vocabulary.
        CsKind::AliasQualifiedName
        | CsKind::Discard
        | CsKind::Interpolation
        | CsKind::Subpattern => Custom(transformations::passthrough),

        // ---- Unhandled in the previous dispatcher — survive as raw
        //      kind names. Most are TODO candidates for real semantics.

        // TODO: pattern combinators sit alongside pattern variants
        // already in the rule table. Each should rename to PATTERN
        // with a marker — sibling shapes:
        //   constant_pattern    → RenameWithMarker(Pattern, Constant)
        //   declaration_pattern => RenameWithMarker(Pattern, Declaration)
        //   recursive_pattern   => RenameWithMarker(Pattern, Recursive)
        // Likely targets:
        //   and_pattern      → RenameWithMarker(Pattern, And)        (new marker)
        //   or_pattern       → RenameWithMarker(Pattern, Or)         (new marker)
        //   negated_pattern  → RenameWithMarker(Pattern, Negated)    (new marker)
        //   list_pattern     → RenameWithMarker(Pattern, List)       (new marker)
        //   var_pattern      → RenameWithMarker(Pattern, Var)        (new marker)
        //   type_pattern     → RenameWithMarker(Pattern, Type)       (Type exists)
        //   parenthesized_pattern → Flatten { distribute_field: None }
        // Test impact: none in current snapshots; would change new
        // pattern fixtures.
        CsKind::AndPattern
        | CsKind::OrPattern
        | CsKind::NegatedPattern
        | CsKind::ListPattern
        | CsKind::VarPattern
        | CsKind::TypePattern
        | CsKind::ParenthesizedPattern => Custom(transformations::passthrough),

        // TODO: `as_expression` (`x as Foo`) is the conversion sibling
        // of `is_pattern_expression` (which renames to Is). Either
        // share the Is rename with a marker, or introduce a new As
        // semantic. Same applies to `is_expression` (the older `obj
        // is Foo` form before patterns).
        CsKind::AsExpression
        | CsKind::IsExpression => Custom(transformations::passthrough),

        // TODO: `cast_expression` (`(int)x`) and `default_expression`
        // (`default(T)`) are call-shaped operations. Could each get
        // their own semantic (Cast / Default) or share `Rename(Call)`
        // with a marker. `throw_expression` (the expression form of
        // `throw e`) is the sibling of `throw_statement` → Throw; pick
        // one shared shape.
        CsKind::CastExpression
        | CsKind::DefaultExpression
        | CsKind::ThrowExpression => Custom(transformations::passthrough),

        // TODO: `element_access_expression` (`x[i]`) is the call-site
        // counterpart of `indexer_declaration` → Indexer. Probably
        // `Rename(Index)` (already used by `element_binding_expression`).
        CsKind::ElementAccessExpression => Custom(transformations::passthrough),

        // TODO: `anonymous_method_expression` is functionally a lambda
        // (older `delegate { … }` syntax). Likely `Rename(Lambda)`.
        // `anonymous_object_creation_expression` (`new { X = 1 }`) is
        // a literal/object-creation shape — could share `Rename(New)`
        // with a marker.
        CsKind::AnonymousMethodExpression
        | CsKind::AnonymousObjectCreationExpression => Custom(transformations::passthrough),

        // TODO: array creations are siblings of
        // `object_creation_expression` → New. Likely `Rename(New)`
        // with an Array marker.
        CsKind::ArrayCreationExpression
        | CsKind::ImplicitArrayCreationExpression
        | CsKind::ImplicitStackallocExpression => Custom(transformations::passthrough),

        // TODO: special-statement forms — `lock`, `fixed`, `unsafe`,
        // `checked`, `goto`, `yield`, `empty` (`;`), `labeled`. Each
        // currently survives as its grammar kind. Most should `Rename`
        // to a new keyword constant; `empty_statement` should likely
        // be `Flatten` or skipped entirely.
        CsKind::CheckedStatement
        | CsKind::EmptyStatement
        | CsKind::FixedStatement
        | CsKind::GotoStatement
        | CsKind::LabeledStatement
        | CsKind::LockStatement
        | CsKind::UnsafeStatement
        | CsKind::YieldStatement => Custom(transformations::passthrough),

        // TODO: `with_expression` (`record with { X = 1 }`) and
        // `with_initializer` are record-update shapes. Either get a
        // dedicated With semantic or share Rename(New) with a marker.
        CsKind::WithExpression
        | CsKind::WithInitializer => Custom(transformations::passthrough),

        // TODO: `event_declaration` is the property-shaped event form
        // (with accessors); pairs with `event_field_declaration` which
        // already renames to Event. Should also `Rename(Event)`.
        // `conversion_operator_declaration` is a sibling of
        // `operator_declaration` → Operator; likely the same.
        CsKind::EventDeclaration
        | CsKind::ConversionOperatorDeclaration => Custom(transformations::passthrough),

        // ---- Truly unhandled (preprocessor, lvalue/rvalue wrappers,
        //      C++/CLI ref types, raw structural supertypes, etc.) ---
        CsKind::ArrayRankSpecifier
        | CsKind::AttributeTargetSpecifier
        | CsKind::BracketedArgumentList
        | CsKind::CallingConvention
        | CsKind::CharacterLiteral
        | CsKind::CharacterLiteralContent
        | CsKind::CheckedExpression
        | CsKind::Declaration
        | CsKind::DeclarationExpression
        | CsKind::ExplicitInterfaceSpecifier
        | CsKind::Expression
        | CsKind::ExternAliasDirective
        | CsKind::FunctionPointerParameter
        | CsKind::GlobalAttribute
        | CsKind::GlobalStatement
        | CsKind::InterpolationAlignmentClause
        | CsKind::InterpolationFormatClause
        | CsKind::InterpolationQuote
        | CsKind::JoinIntoClause
        | CsKind::Literal
        | CsKind::LvalueExpression
        | CsKind::MakerefExpression
        | CsKind::NonLvalueExpression
        | CsKind::ParenthesizedVariableDesignation
        | CsKind::Pattern
        | CsKind::PositionalPatternClause
        | CsKind::PreprocArg
        | CsKind::PreprocDefine
        | CsKind::PreprocElif
        | CsKind::PreprocElse
        | CsKind::PreprocEndregion
        | CsKind::PreprocError
        | CsKind::PreprocIf
        | CsKind::PreprocIfInAttributeList
        | CsKind::PreprocLine
        | CsKind::PreprocNullable
        | CsKind::PreprocPragma
        | CsKind::PreprocRegion
        | CsKind::PreprocUndef
        | CsKind::PreprocWarning
        | CsKind::PrimaryConstructorBaseType
        | CsKind::RefExpression
        | CsKind::ReftypeExpression
        | CsKind::RefvalueExpression
        | CsKind::ScopedType
        | CsKind::ShebangDirective
        | CsKind::SizeofExpression
        | CsKind::StackallocExpression
        | CsKind::Statement
        | CsKind::StringLiteralEncoding
        | CsKind::Type
        | CsKind::TypeDeclaration
        | CsKind::TypeofExpression => Custom(transformations::passthrough),
    }
}
