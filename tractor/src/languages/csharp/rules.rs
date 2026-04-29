//! Per-kind transformation rules for C#: the `CsKind` → `Rule` table.
//!
//! Read this file to find the rule for a specific kind. Read
//! [`super::transformations`] for the body of any `Rule::Custom`
//! handler the rule references by name. Read [`super::output`] for
//! the output vocabulary (semantic names + NodeSpec metadata).
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
use super::output::*;
use super::transformations;

pub fn rule(k: CsKind) -> Rule {
    use Rule::*;
    match k {
        // ---- ExtractOpThenRename ---------------------------------------
        CsKind::BinaryExpression     => ExtractOpThenRename(BINARY),
        CsKind::UnaryExpression      => ExtractOpThenRename(UNARY),
        CsKind::AssignmentExpression => ExtractOpThenRename(ASSIGN),

        // ---- RenameWithMarker ------------------------------------------
        CsKind::ArrayType                   => RenameWithMarker(TYPE, ARRAY),
        CsKind::ConditionalAccessExpression => RenameWithMarker(MEMBER, CONDITIONAL),
        CsKind::ConstantPattern             => RenameWithMarker(PATTERN, CONSTANT),
        CsKind::DeclarationPattern          => RenameWithMarker(PATTERN, DECLARATION),
        CsKind::FunctionPointerType         => RenameWithMarker(TYPE, FUNCTION),
        CsKind::MemberAccessExpression      => RenameWithMarker(MEMBER, INSTANCE),
        CsKind::MemberBindingExpression     => RenameWithMarker(MEMBER, CONDITIONAL),
        CsKind::PointerType                 => RenameWithMarker(TYPE, POINTER),
        CsKind::PrefixUnaryExpression       => RenameWithMarker(UNARY, PREFIX),
        CsKind::RecursivePattern            => RenameWithMarker(PATTERN, RECURSIVE),
        CsKind::RefType                     => RenameWithMarker(TYPE, REF),
        CsKind::RelationalPattern           => RenameWithMarker(PATTERN, RELATIONAL),
        CsKind::TuplePattern                => RenameWithMarker(PATTERN, TUPLE),
        CsKind::TupleType                   => RenameWithMarker(TYPE, TUPLE),

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

        // ---- Custom (language-specific logic in transformations.rs) ---
        CsKind::AccessorDeclaration           => Custom(transformations::accessor_declaration),
        CsKind::ClassDeclaration              => Custom(transformations::class_declaration),
        CsKind::Comment                       => Custom(transformations::comment),
        CsKind::ConditionalExpression         => Custom(transformations::conditional_expression),
        CsKind::ConstructorDeclaration        => Custom(transformations::constructor_declaration),
        CsKind::EnumDeclaration               => Custom(transformations::enum_declaration),
        CsKind::FieldDeclaration              => Custom(transformations::field_declaration),
        CsKind::GenericName                   => Custom(transformations::generic_name),
        CsKind::Identifier                    => Custom(transformations::identifier),
        CsKind::IfStatement                   => Custom(transformations::if_statement),
        CsKind::ImplicitType                  => Custom(transformations::implicit_type),
        CsKind::InterfaceDeclaration          => Custom(transformations::interface_declaration),
        CsKind::InterpolatedStringExpression  => Custom(transformations::interpolated_string_expression),
        CsKind::MethodDeclaration             => Custom(transformations::method_declaration),
        CsKind::Modifier                      => Custom(transformations::modifier),
        CsKind::NullableType                  => Custom(transformations::nullable_type),
        CsKind::PostfixUnaryExpression        => Custom(transformations::postfix_unary_expression),
        CsKind::PredefinedType                => Custom(transformations::predefined_type),
        CsKind::PropertyDeclaration           => Custom(transformations::property_declaration),
        CsKind::RecordDeclaration             => Custom(transformations::record_declaration),
        CsKind::StructDeclaration             => Custom(transformations::struct_declaration),
        CsKind::VariableDeclaration           => Custom(transformations::variable_declaration),

        // `where T : new()` / constraint-clause kinds — consumed by the
        // shared `attach_where_clause_constraints` post-transform; they
        // never reach the dispatcher in practice. Passthrough is the
        // safe noop.
        CsKind::ConstructorConstraint            => Custom(transformations::passthrough),
        CsKind::TypeParameterConstraint          => Custom(transformations::passthrough),
        CsKind::TypeParameterConstraintsClause   => Custom(transformations::passthrough),

        // ---- Pure Rename -----------------------------------------------
        CsKind::Argument                       => Rename(ARGUMENT),
        CsKind::Attribute                      => Rename(ATTRIBUTE),
        CsKind::AttributeArgument              => Rename(ARGUMENT),
        CsKind::AwaitExpression                => Rename(AWAIT),
        CsKind::BaseList                       => Rename(EXTENDS),
        CsKind::Block                          => Rename(BLOCK),
        CsKind::BooleanLiteral                 => Rename(BOOL),
        CsKind::BreakStatement                 => Rename(BREAK),
        CsKind::CatchClause                    => Rename(CATCH),
        CsKind::CatchDeclaration               => Rename(DECLARATION),
        CsKind::CatchFilterClause              => Rename(FILTER),
        CsKind::CompilationUnit                => Rename(UNIT),
        CsKind::ConstructorInitializer         => Rename(CHAIN),
        CsKind::ContinueStatement              => Rename(CONTINUE),
        CsKind::DelegateDeclaration            => Rename(DELEGATE),
        CsKind::DestructorDeclaration          => Rename(DESTRUCTOR),
        CsKind::DoStatement                    => Rename(DO),
        CsKind::ElementBindingExpression       => Rename(INDEX),
        CsKind::EnumMemberDeclaration          => Rename(CONSTANT),
        CsKind::EventFieldDeclaration          => Rename(EVENT),
        CsKind::ExpressionStatement            => Rename(EXPRESSION),
        CsKind::FileScopedNamespaceDeclaration => Rename(NAMESPACE),
        CsKind::FinallyClause                  => Rename(FINALLY),
        CsKind::ForStatement                   => Rename(FOR),
        CsKind::ForeachStatement               => Rename(FOREACH),
        CsKind::FromClause                     => Rename(FROM),
        CsKind::GroupClause                    => Rename(GROUP),
        CsKind::ImplicitObjectCreationExpression => Rename(NEW),
        CsKind::ImplicitParameter              => Rename(PARAMETER),
        CsKind::IndexerDeclaration             => Rename(INDEXER),
        CsKind::InitializerExpression          => Rename(LITERAL),
        CsKind::IntegerLiteral                 => Rename(INT),
        CsKind::InvocationExpression           => Rename(CALL),
        CsKind::IsPatternExpression            => Rename(IS),
        CsKind::JoinClause                     => Rename(JOIN),
        CsKind::LambdaExpression               => Rename(LAMBDA),
        CsKind::LetClause                      => Rename(LET),
        CsKind::LocalFunctionStatement         => Rename(METHOD),
        CsKind::NamespaceDeclaration           => Rename(NAMESPACE),
        CsKind::NullLiteral                    => Rename(NULL),
        CsKind::ObjectCreationExpression       => Rename(NEW),
        CsKind::OperatorDeclaration            => Rename(OPERATOR),
        CsKind::OrderByClause                  => Rename(ORDER),
        CsKind::Parameter                      => Rename(PARAMETER),
        CsKind::PropertyPatternClause          => Rename(PROPERTIES),
        CsKind::QueryExpression                => Rename(QUERY),
        CsKind::RangeExpression                => Rename(RANGE),
        CsKind::RawStringLiteral               => Rename(STRING),
        CsKind::RealLiteral                    => Rename(FLOAT),
        CsKind::ReturnStatement                => Rename(RETURN),
        CsKind::SelectClause                   => Rename(SELECT),
        CsKind::StringLiteral                  => Rename(STRING),
        CsKind::SwitchBody                     => Rename(BODY),
        CsKind::SwitchExpression               => Rename(SWITCH),
        CsKind::SwitchExpressionArm            => Rename(ARM),
        CsKind::SwitchSection                  => Rename(SECTION),
        CsKind::SwitchStatement                => Rename(SWITCH),
        CsKind::ThrowStatement                 => Rename(THROW),
        CsKind::TryStatement                   => Rename(TRY),
        CsKind::TupleElement                   => Rename(ELEMENT),
        CsKind::TupleExpression                => Rename(TUPLE),
        CsKind::TypeParameter                  => Rename(GENERIC),
        CsKind::UsingDirective                 => Rename(IMPORT),
        CsKind::UsingStatement                 => Rename(USING),
        CsKind::VariableDeclarator             => Rename(DECLARATOR),
        CsKind::VerbatimStringLiteral          => Rename(STRING),
        CsKind::WhenClause                     => Rename(WHEN),
        CsKind::WhereClause                    => Rename(WHERE),
        CsKind::WhileStatement                 => Rename(WHILE),

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
        //   constant_pattern    → RenameWithMarker(PATTERN, CONSTANT)
        //   declaration_pattern → RenameWithMarker(PATTERN, DECLARATION)
        //   recursive_pattern   → RenameWithMarker(PATTERN, RECURSIVE)
        // Likely targets:
        //   and_pattern      → RenameWithMarker(PATTERN, AND)        (new marker)
        //   or_pattern       → RenameWithMarker(PATTERN, OR)         (new marker)
        //   negated_pattern  → RenameWithMarker(PATTERN, NEGATED)    (new marker)
        //   list_pattern     → RenameWithMarker(PATTERN, LIST)       (new marker)
        //   var_pattern      → RenameWithMarker(PATTERN, VAR)        (new marker)
        //   type_pattern     → RenameWithMarker(PATTERN, TYPE)       (TYPE exists)
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
        // of `is_pattern_expression` (which renames to IS). Either
        // share the IS rename with a marker, or introduce a new AS
        // semantic. Same applies to `is_expression` (the older `obj
        // is Foo` form before patterns).
        CsKind::AsExpression
        | CsKind::IsExpression => Custom(transformations::passthrough),

        // TODO: `cast_expression` (`(int)x`) and `default_expression`
        // (`default(T)`) are call-shaped operations. Could each get
        // their own semantic (CAST / DEFAULT) or share `Rename(CALL)`
        // with a marker. `throw_expression` (the expression form of
        // `throw e`) is the sibling of `throw_statement` → THROW; pick
        // one shared shape.
        CsKind::CastExpression
        | CsKind::DefaultExpression
        | CsKind::ThrowExpression => Custom(transformations::passthrough),

        // TODO: `element_access_expression` (`x[i]`) is the call-site
        // counterpart of `indexer_declaration` → INDEXER. Probably
        // `Rename(INDEX)` (already used by `element_binding_expression`).
        CsKind::ElementAccessExpression => Custom(transformations::passthrough),

        // TODO: `anonymous_method_expression` is functionally a lambda
        // (older `delegate { … }` syntax). Likely `Rename(LAMBDA)`.
        // `anonymous_object_creation_expression` (`new { X = 1 }`) is
        // a literal/object-creation shape — could share `Rename(NEW)`
        // with a marker.
        CsKind::AnonymousMethodExpression
        | CsKind::AnonymousObjectCreationExpression => Custom(transformations::passthrough),

        // TODO: array creations are siblings of
        // `object_creation_expression` → NEW. Likely `Rename(NEW)`
        // with an ARRAY marker.
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
        // dedicated WITH semantic or share Rename(NEW) with a marker.
        CsKind::WithExpression
        | CsKind::WithInitializer => Custom(transformations::passthrough),

        // TODO: `event_declaration` is the property-shaped event form
        // (with accessors); pairs with `event_field_declaration` which
        // already renames to EVENT. Should also `Rename(EVENT)`.
        // `conversion_operator_declaration` is a sibling of
        // `operator_declaration` → OPERATOR; likely the same.
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
