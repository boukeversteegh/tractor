//! Per-kind transformation rules for Rust: the `RustKind` → `Rule` table.
//!
//! Read this file to find the rule for a specific kind. Read
//! [`super::transformations`] for the body of any `Rule::Custom`
//! handler. Read [`super::semantic`] for the output vocabulary.
//!
//! Exhaustive over `RustKind` — the compiler enforces coverage.

use crate::languages::rule::Rule;

use super::input::RustKind;
use super::output::TractorNode::{self, *};
use super::transformations;

/// Shorthand for the `default-access-then-rename` shape used by all 8
/// Rust declaration kinds (function / struct / enum / trait / const /
/// static / type / mod). Bakes in Rust's resolver — default access is
/// always `private` (no `pub` modifier means item-private).
fn da(to: TractorNode) -> Rule<TractorNode> {
    Rule::DefaultAccessThenRename {
        to,
        default_access: transformations::default_access_for_declaration,
    }
}

pub fn rule(k: RustKind) -> Rule<TractorNode> {
    use Rule::*;
    match k {
        // ---- ExtractOpThenRename ---------------------------------------
        RustKind::BinaryExpression => ExtractOpThenRename(Binary),
        RustKind::UnaryExpression  => ExtractOpThenRename(Unary),

        // ---- RenameWithMarker ------------------------------------------
        RustKind::AbstractType          => RenameWithMarker(Type, Abstract),
        RustKind::ArrayType             => RenameWithMarker(Type, Array),
        RustKind::AssociatedType        => RenameWithMarker(Type, Associated),
        RustKind::AsyncBlock            => RenameWithMarker(Block, Async),
        RustKind::BaseFieldInitializer  => RenameWithMarker(Field, Base),
        RustKind::BoundedType           => RenameWithMarker(Type, Bounded),
        RustKind::ConstBlock            => RenameWithMarker(Block, Const),
        RustKind::DynamicType           => RenameWithMarker(Type, Dynamic),
        RustKind::FieldPattern          => RenameWithMarker(Pattern, Field),
        RustKind::FunctionType          => RenameWithMarker(Type, Function),
        RustKind::GenericFunction       => RenameWithMarker(Call, Generic),
        RustKind::MutPattern            => RenameWithMarker(Pattern, Mut),
        RustKind::NeverType             => RenameWithMarker(Type, Never),
        RustKind::OrPattern             => RenameWithMarker(Pattern, Or),
        RustKind::PointerType           => RenameWithMarker(Type, Pointer),
        RustKind::RefPattern            => RenameWithMarker(Pattern, Ref),
        RustKind::StructPattern         => RenameWithMarker(Pattern, Struct),
        RustKind::TryBlock              => RenameWithMarker(Block, Try),
        RustKind::TupleType             => RenameWithMarker(Type, Tuple),
        RustKind::UnitType              => RenameWithMarker(Type, Unit),

        // ---- Flatten with field distribution ---------------------------
        RustKind::Arguments     => Flatten { distribute_field: Some("arguments") },
        RustKind::Parameters    => Flatten { distribute_field: Some("parameters") },
        RustKind::TypeArguments => Flatten { distribute_field: Some("arguments") },

        // ---- Pure Flatten ----------------------------------------------
        RustKind::AttributeItem
        | RustKind::Block
        | RustKind::ClosureParameters
        | RustKind::DeclarationList
        | RustKind::EnumVariantList
        | RustKind::EscapeSequence
        | RustKind::FieldDeclarationList
        | RustKind::FieldInitializerList
        | RustKind::InnerDocCommentMarker
        | RustKind::LetCondition
        | RustKind::MatchBlock
        | RustKind::MutableSpecifier
        | RustKind::OrderedFieldDeclarationList
        | RustKind::OuterDocCommentMarker
        | RustKind::ParenthesizedExpression
        | RustKind::QualifiedType
        | RustKind::ScopedUseList
        | RustKind::StringContent
        | RustKind::TokenTree
        | RustKind::TupleStructPattern
        | RustKind::TypeBinding
        | RustKind::UseAsClause
        | RustKind::UseList
        | RustKind::UseWildcard => Flatten { distribute_field: None },

        // ---- DefaultAccessThenRename — 8 declaration kinds.
        //      Default access in Rust is `private` when `pub` is absent.
        RustKind::ConstItem    => da(Const),
        RustKind::EnumItem     => da(Enum),
        RustKind::FunctionItem => da(Function),
        RustKind::ModItem      => da(Mod),
        RustKind::StaticItem   => da(Static),
        RustKind::StructItem   => da(Struct),
        RustKind::TraitItem    => da(Trait),
        RustKind::TypeItem     => da(Alias),

        // ---- Custom (language-specific logic in transformations.rs) ---
        RustKind::BlockComment        => Custom(transformations::comment),
        RustKind::DocComment          => Custom(transformations::comment),
        RustKind::ExpressionStatement => Custom(transformations::skip),
        RustKind::FieldIdentifier     => Custom(transformations::identifier),
        RustKind::GenericType         => Custom(transformations::generic_type),
        RustKind::Identifier          => Custom(transformations::identifier),
        RustKind::InnerAttributeItem  => Custom(transformations::inner_attribute_item),
        RustKind::LetDeclaration      => Custom(transformations::let_declaration),
        RustKind::LineComment         => Custom(transformations::comment),
        RustKind::MatchPattern        => Custom(transformations::match_pattern),
        RustKind::PrimitiveType       => Custom(transformations::type_identifier),
        RustKind::RawStringLiteral    => Custom(transformations::raw_string_literal),
        RustKind::ReferenceType       => Custom(transformations::reference_type),
        RustKind::ShorthandFieldIdentifier => Custom(transformations::identifier),
        RustKind::StructExpression    => Custom(transformations::struct_expression),
        RustKind::TypeIdentifier      => Custom(transformations::type_identifier),
        RustKind::TypeParameter       => Custom(transformations::type_parameter),
        RustKind::TypeParameters      => Custom(transformations::type_parameters),
        RustKind::VisibilityModifier  => Custom(transformations::visibility_modifier),

        // ---- Pure Rename -----------------------------------------------
        RustKind::AwaitExpression          => Rename(Await),
        RustKind::BooleanLiteral           => Rename(Bool),
        RustKind::BreakExpression          => Rename(Break),
        RustKind::CallExpression           => Rename(Call),
        RustKind::CharLiteral              => Rename(Char),
        RustKind::ClosureExpression        => Rename(Closure),
        RustKind::CompoundAssignmentExpr   => Rename(Assign),
        RustKind::ContinueExpression       => Rename(Continue),
        RustKind::ElseClause               => Rename(Else),
        RustKind::EnumVariant              => Rename(Variant),
        RustKind::FieldDeclaration         => Rename(Field),
        RustKind::FieldExpression          => Rename(Field),
        RustKind::FieldInitializer         => Rename(Field),
        RustKind::FloatLiteral             => Rename(Float),
        RustKind::ForExpression            => Rename(For),
        RustKind::FunctionModifiers        => Rename(Modifiers),
        RustKind::FunctionSignatureItem    => Rename(Signature),
        RustKind::IfExpression             => Rename(If),
        RustKind::ImplItem                 => Rename(Impl),
        RustKind::IndexExpression          => Rename(Index),
        RustKind::IntegerLiteral           => Rename(Int),
        RustKind::Lifetime                 => Rename(Lifetime),
        RustKind::LifetimeParameter        => Rename(Lifetime),
        RustKind::LoopExpression           => Rename(Loop),
        RustKind::MacroInvocation          => Rename(Macro),
        RustKind::MatchArm                 => Rename(Arm),
        RustKind::MatchExpression          => Rename(Match),
        RustKind::Parameter                => Rename(Parameter),
        RustKind::RangeExpression          => Rename(Range),
        RustKind::RangePattern             => Rename(Range),
        RustKind::ReferenceExpression      => Rename(Ref),
        RustKind::ReturnExpression         => Rename(Return),
        RustKind::ScopedIdentifier         => Rename(Path),
        RustKind::ScopedTypeIdentifier     => Rename(Path),
        RustKind::SelfParameter            => Rename(Self_),
        RustKind::ShorthandFieldInitializer => Rename(Field),
        RustKind::SourceFile               => Rename(File),
        RustKind::StringLiteral            => Rename(String),
        RustKind::TraitBounds              => Rename(Bounds),
        RustKind::TryExpression            => Rename(Try),
        RustKind::TupleExpression          => Rename(Tuple),
        RustKind::TypeCastExpression       => Rename(Cast),
        RustKind::UnsafeBlock              => Rename(Unsafe),
        RustKind::UseDeclaration           => Rename(Use),
        RustKind::WhereClause              => Rename(Where),
        RustKind::WherePredicate           => Rename(Bound),
        RustKind::WhileExpression          => Rename(While),

        // ---- Passthrough — kind name already matches the vocabulary,
        //      OR the kind is unhandled and the dispatcher leaves it as
        //      its raw grammar name.

        // Already matches our vocabulary.
        RustKind::Attribute
        | RustKind::Crate
        | RustKind::Label
        | RustKind::Self_
        | RustKind::Super => Custom(transformations::passthrough),

        // ---- Unhandled in the previous dispatcher — survive as raw
        //      kind names. TODO candidates for real semantics.

        // TODO: array/array_expression/unit/negative literals — sibling
        // of integer/float/string. Each needs a Rename target.
        //   array_expression    → Rename(ARRAY)? Rename(LITERAL) marker?
        //   unit_expression     → Rename(UNIT)? RenameWithMarker(LITERAL, UNIT)?
        //   negative_literal    → Rename(INT/FLOAT) marker?
        //   assignment_expression → ExtractOpThenRename(ASSIGN) (currently
        //     compound_assignment_expr handles this; assignment_expression
        //     would be a sibling)
        RustKind::ArrayExpression
        | RustKind::AssignmentExpression
        | RustKind::NegativeLiteral
        | RustKind::UnitExpression
        | RustKind::YieldExpression => Custom(transformations::passthrough),

        // TODO: pattern variants — sibling of mut/or/ref/struct patterns
        // already in the rule table. Each should rename to PATTERN with
        // a marker:
        //   captured_pattern        → RenameWithMarker(PATTERN, CAPTURED)?
        //   generic_pattern         → RenameWithMarker(PATTERN, GENERIC)?
        //   reference_pattern       → RenameWithMarker(PATTERN, REF) (REF exists)
        //   slice_pattern           → RenameWithMarker(PATTERN, SLICE)?
        //   tuple_pattern           → RenameWithMarker(PATTERN, TUPLE)?
        //   remaining_field_pattern → RenameWithMarker(PATTERN, REST)?
        //   token_binding_pattern   → for macro_rules — different shape
        RustKind::CapturedPattern
        | RustKind::GenericPattern
        | RustKind::ReferencePattern
        | RustKind::RemainingFieldPattern
        | RustKind::SlicePattern
        | RustKind::TokenBindingPattern
        | RustKind::TuplePattern => Custom(transformations::passthrough),

        // TODO: macro and meta-syntactic kinds.
        //   macro_definition    → Rename(MACRO)? RenameWithMarker(MACRO, DEFINITION)?
        //   macro_rule          → Rename(ARM)? Rename(RULE)?
        //   metavariable        → marker for `$ident` in macro body
        //   fragment_specifier  → marker for `:expr`/`:ident`/etc.
        //   token_repetition / token_repetition_pattern / token_tree_pattern
        //     → grammar shapes inside macro definitions
        RustKind::FragmentSpecifier
        | RustKind::MacroDefinition
        | RustKind::MacroRule
        | RustKind::Metavariable
        | RustKind::TokenRepetition
        | RustKind::TokenRepetitionPattern
        | RustKind::TokenTreePattern => Custom(transformations::passthrough),

        // TODO: extra declaration / item kinds the catalogue didn't
        // cover — should join the DefaultAccessThenRename family or
        // similar.
        //   extern_crate_declaration → Rename(USE) marker?
        //   extern_modifier          → modifier helper
        //   foreign_mod_item         → RenameWithMarker(MOD, FOREIGN)?
        //   gen_block                → RenameWithMarker(BLOCK, GEN)?
        //   union_item               → da(UNION) (new constant)?
        RustKind::ExternCrateDeclaration
        | RustKind::ExternModifier
        | RustKind::ForeignModItem
        | RustKind::GenBlock
        | RustKind::UnionItem => Custom(transformations::passthrough),

        // TODO: type-related grammar shapes not yet renamed.
        //   bracketed_type      → Flatten or Rename(TYPE) marker?
        //   for_lifetimes       → Rename(LIFETIMES) wrapper?
        //   higher_ranked_trait_bound → for `for<'a> Fn(...)` bounds
        //   removed_trait_bound → `?Sized` etc; marker?
        //   generic_type_with_turbofish → Rename(TYPE) marker?
        //   const_parameter / variadic_parameter → parameter variants
        //   use_bounds          → use-bound impl items
        RustKind::BracketedType
        | RustKind::ConstParameter
        | RustKind::ForLifetimes
        | RustKind::GenericTypeWithTurbofish
        | RustKind::HigherRankedTraitBound
        | RustKind::RemovedTraitBound
        | RustKind::UseBounds
        | RustKind::VariadicParameter => Custom(transformations::passthrough),

        // TODO: control-flow / structural odds and ends.
        //   empty_statement → Flatten or Skip
        //   let_chain       → Rename(LET) marker?
        //   shebang         → top-of-file `#!/usr/bin/env`
        RustKind::EmptyStatement
        | RustKind::LetChain
        | RustKind::Shebang => Custom(transformations::passthrough),
    }
}
