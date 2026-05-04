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
        RustKind::BinaryExpression     => ExtractOpThenRename(Binary),
        RustKind::UnaryExpression      => ExtractOpThenRename(Unary),
        // `lhs = rhs` (non-compound). Sibling of `compound_assignment_expr`.
        RustKind::AssignmentExpression => ExtractOpThenRename(Assign),

        // ---- RenameWithMarker ------------------------------------------
        RustKind::AbstractType          => RenameWithMarker(Type, Abstract),
        RustKind::ArrayType             => RenameWithMarker(Type, Array),
        // Both the trait declaration site (`type Canvas;` inside a
        // trait body) AND the use-site binding (`Canvas = Vec<u8>` inside
        // generic args, e.g. `Drawable<Canvas = Vec<u8>>`) emit
        // `<type[associated]>` per Principle #5 — same concept, same
        // name. The use-site adds a `<type>` child for the bound value.
        RustKind::AssociatedType        => RenameWithMarker(Type, Associated),
        RustKind::TypeBinding           => RenameWithMarker(Type, Associated),
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

        // ---- Iter 16: pattern variants under <pattern> ---------------
        // Each is a distinct pattern shape; query as `//pattern` for the
        // broad case and `[capture]` / `[ref]` / `[slice]` etc. to narrow.
        RustKind::CapturedPattern             => RenameWithMarker(Pattern, Capture),
        RustKind::GenericPattern              => RenameWithMarker(Pattern, Generic),
        RustKind::ReferencePattern            => RenameWithMarker(Pattern, Ref),
        RustKind::RemainingFieldPattern       => RenameWithMarker(Pattern, Rest),
        RustKind::SlicePattern                => RenameWithMarker(Pattern, Slice),
        RustKind::TuplePattern                => RenameWithMarker(Pattern, Tuple),
        RustKind::TokenBindingPattern         => RenameWithMarker(Pattern, Binding),

        // ---- Iter 16: macro grammar ----------------------------------
        // `macro_rules! name { ... }` — declaration-level, default-private.
        // Use `Definition` marker to distinguish from `macro_invocation`
        // (which renames to `<macro>` directly).
        RustKind::MacroDefinition  => RenameWithMarker(Macro, Definition),
        // Each macro_rule is `(pat) => { body }` — same shape as match arm.
        RustKind::MacroRule        => Rename(Arm),
        // `:expr`, `:ident`, `:ty`, ... — fragment-kind specifier on `$x:ident`.
        RustKind::FragmentSpecifier => Rename(Fragment),
        // `$(...)*` — repetition in macro body and pattern. Same shape both
        // sides, parent (rule arm pattern vs body) disambiguates.
        RustKind::TokenRepetition         => Rename(Repetition),
        RustKind::TokenRepetitionPattern  => Rename(Repetition),

        // ---- Iter 16: items / declarations ---------------------------
        // `extern crate alloc;` strips the `crate` keyword child via a
        // Custom handler (the bare keyword would violate marker-empty),
        // then renames to `<use>` + `<extern/>` marker.
        RustKind::ExternCrateDeclaration => Custom(transformations::extern_crate_declaration),
        // `extern "C" { ... }` — foreign mod block. The `extern`
        // keyword leads the block; promote to <extern/> marker
        // alongside <foreign/> so `//mod[extern]` finds the FFI form.
        RustKind::ForeignModItem         => Custom(transformations::foreign_mod_item),
        RustKind::GenBlock               => RenameWithMarker(Block, Gen),
        RustKind::UnionItem              => da(Union),

        // ---- Iter 16: type-shaped grammar ----------------------------
        RustKind::HigherRankedTraitBound  => RenameWithMarker(Bound, Higher),
        RustKind::RemovedTraitBound       => RenameWithMarker(Bound, Optional),
        RustKind::GenericTypeWithTurbofish => RenameWithMarker(Type, Turbofish),
        RustKind::ConstParameter          => RenameWithMarker(Parameter, Const),
        RustKind::VariadicParameter       => RenameWithMarker(Parameter, Variadic),
        RustKind::UseBounds               => Rename(Bounds),

        // ---- Iter 16: expression shapes ------------------------------
        RustKind::ArrayExpression  => Rename(Array),
        RustKind::NegativeLiteral  => RenameWithMarker(Literal, Negative),
        RustKind::UnitExpression   => RenameWithMarker(Literal, Unit),
        RustKind::YieldExpression  => Rename(Yield),

        // ---- Flatten with field distribution ---------------------------
        RustKind::Arguments     => Flatten { distribute_list: Some("arguments") },
        RustKind::Parameters    => Flatten { distribute_list: Some("parameters") },
        RustKind::TypeArguments => Flatten { distribute_list: Some("arguments") },

        // ---- Pure Flatten ----------------------------------------------
        RustKind::AttributeItem
        | RustKind::Block
        | RustKind::BracketedType
        | RustKind::DeclarationList
        | RustKind::EmptyStatement
        | RustKind::EnumVariantList
        | RustKind::EscapeSequence
        | RustKind::ExternModifier
        | RustKind::FieldDeclarationList
        | RustKind::FieldInitializerList
        | RustKind::ForLifetimes
        | RustKind::InnerDocCommentMarker
        | RustKind::LetChain
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
        | RustKind::TokenTreePattern
        | RustKind::TupleStructPattern
        | RustKind::UseAsClause
        | RustKind::UseList
        | RustKind::UseWildcard => Flatten { distribute_list: None },

        // ---- DefaultAccessThenRename — 8 declaration kinds.
        //      Default access in Rust is `private` when `pub` is absent.
        RustKind::ConstItem    => da(Const),
        RustKind::EnumItem     => da(Enum),
        RustKind::FunctionItem => da(Function),
        RustKind::ModItem      => da(Mod),
        RustKind::StaticItem   => Custom(transformations::static_item),
        RustKind::StructItem   => da(Struct),
        RustKind::TraitItem    => da(Trait),
        RustKind::TypeItem     => da(Alias),

        // ---- Custom (language-specific logic in transformations.rs) ---
        RustKind::BlockComment        => Custom(transformations::comment),
        RustKind::DocComment          => Custom(transformations::comment),
        RustKind::AwaitExpression     => Custom(transformations::await_expression),
        RustKind::ExpressionStatement => Custom(transformations::expression_statement),
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
        RustKind::TryExpression       => Custom(transformations::try_expression),
        RustKind::TypeIdentifier      => Custom(transformations::type_identifier),
        RustKind::TypeParameter       => Custom(transformations::type_parameter),
        RustKind::TypeParameters      => Custom(transformations::type_parameters),
        RustKind::VisibilityModifier  => Custom(transformations::visibility_modifier),

        // ---- Pure Rename -----------------------------------------------
        RustKind::BooleanLiteral           => Rename(Bool),
        RustKind::BreakExpression          => Rename(Break),
        RustKind::CallExpression           => Rename(Call),
        RustKind::CharLiteral              => Rename(Char),
        // `|x| x` / `|x| { ... }` — closure. Custom handler renames
        // the `<body>` wrapper (from tree-sitter `field="body"`) to
        // `<value>` so `wrap_expression_positions` treats the body
        // as an expression slot. Without this, single-expression
        // bodies like `|x| x` ended up with `body/name[@list="name"]="x"`
        // — the `list="name"` was misleading (closure body is one
        // value, not a list of names).
        RustKind::ClosureExpression        => Custom(transformations::closure_expression),
        // `|x|` bare-name closure params flatten the inner identifier
        // into a bare `<name>`; typed forms (`|x: i32|`) already
        // produce `<parameter>{name, type}`. Wrap bare names in
        // `<parameter>` so the shape is uniform across both forms.
        RustKind::ClosureParameters        => Custom(transformations::closure_parameters),
        RustKind::CompoundAssignmentExpr   => Rename(Assign),
        RustKind::ContinueExpression       => Rename(Continue),
        RustKind::ElseClause               => Rename(Else),
        RustKind::EnumVariant              => Rename(Variant),
        RustKind::FieldDeclaration         => Rename(Field),
        RustKind::FieldExpression          => Rename(Field),
        RustKind::FieldInitializer         => Rename(Field),
        RustKind::FloatLiteral             => Rename(Float),
        RustKind::ForExpression            => Rename(For),
        RustKind::FunctionModifiers        => Custom(transformations::function_modifiers),
        RustKind::FunctionSignatureItem    => Rename(Signature),
        RustKind::IfExpression             => Rename(If),
        RustKind::ImplItem                 => Custom(transformations::impl_item),
        RustKind::IndexExpression          => Rename(Index),
        RustKind::IntegerLiteral           => Rename(Int),
        RustKind::Lifetime                 => Rename(Lifetime),
        RustKind::LifetimeParameter        => Rename(Lifetime),
        RustKind::LoopExpression           => Rename(Loop),
        RustKind::MacroInvocation          => Rename(Macro),
        RustKind::MatchArm                 => Rename(Arm),
        RustKind::MatchExpression          => Rename(Match),
        RustKind::Parameter                => Rename(Parameter),
        RustKind::RangeExpression          => Custom(transformations::range),
        RustKind::RangePattern             => Custom(transformations::range),
        RustKind::ReferenceExpression      => Rename(Ref),
        RustKind::ReturnExpression         => Rename(Return),
        RustKind::ScopedIdentifier         => Rename(Path),
        RustKind::ScopedTypeIdentifier     => Rename(Path),
        // `self_parameter` — Custom handler strips the inner `<self>`
        // element + bare `&`/`mut` text leaves, prepends `<self/>` (and
        // `<borrowed/>`/`<mut/>` markers as applicable), then renames
        // to `<parameter>`. Replaces the previous `RenameWithMarker`
        // which left the `<self>self</self>` element to collide with
        // the marker on JSON `self` key (iter 188).
        RustKind::SelfParameter            => Custom(transformations::self_parameter),
        RustKind::ShorthandFieldInitializer => Rename(Field),
        RustKind::SourceFile               => Rename(File),
        RustKind::StringLiteral            => Rename(String),
        // `: Clone + Send + 'static` — generic / trait constraint
        // list. Per Principle #12 + #18: flatten to `<extends>`
        // siblings (matches Java/cross-language relationship-naming).
        RustKind::TraitBounds              => Custom(transformations::trait_bounds),
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
        | RustKind::Super => Passthrough,

        // `$ident` inside macro body / macro pattern — text leaf that
        // names a metavariable. Renames to `<name>` so the bound name is
        // queryable like any other identifier; the `$` sigil stays in
        // the leaf text to distinguish from regular names.
        RustKind::Metavariable => Custom(transformations::identifier),

        // ---- Unhandled in the previous dispatcher — survive as raw
        //      kind names. Pending real-semantics candidates tracked
        //      in todo/36-rule-todo-followups.md.

        // `shebang` — `#!/usr/bin/env` line at the top of a script.
        RustKind::Shebang => Passthrough,
    }
}
