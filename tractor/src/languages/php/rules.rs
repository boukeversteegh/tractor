//! Per-kind transformation rules for PHP: the `PhpKind` → `Rule` table.
//!
//! Read this file to find the rule for a specific kind. Read
//! [`super::transformations`] for the body of any `Rule::Custom`
//! handler the rule references by name. Read [`super::semantic`] for
//! the output vocabulary (semantic names + TractorNodeSpec metadata).
//!
//! Exhaustive over `PhpKind` — the compiler enforces coverage. When
//! the grammar ships a new kind, regenerating `input.rs` adds a
//! variant and this match fails to build until the new kind is
//! classified.
//!
//! Pure data variants (`Rename`, `RenameWithMarker`, `Flatten`,
//! `ExtractOpThenRename`, `DefaultAccessThenRename`) are executed by
//! the shared [`crate::languages::rule::dispatch`] helper. Custom logic
//! lives in [`super::transformations`].

use crate::languages::rule::Rule;

use super::input::PhpKind;
use super::output::TractorNode::{self, *};
use super::transformations;

/// Shorthand for the `default-access-then-rename` shape used by the 2
/// PHP declaration kinds (method, property) where members default to
/// public when no visibility modifier is written.
fn da(to: TractorNode) -> Rule<TractorNode> {
    Rule::DefaultAccessThenRename {
        to,
        default_access: transformations::default_access_for_declaration,
    }
}

pub fn rule(k: PhpKind) -> Rule<TractorNode> {
    use Rule::*;
    match k {
        // ---- ExtractOpThenRename ---------------------------------------
        PhpKind::AssignmentExpression => ExtractOpThenRename(Assign),
        PhpKind::BinaryExpression     => ExtractOpThenRename(Binary),
        PhpKind::UnaryOpExpression    => ExtractOpThenRename(Unary),

        // ---- RenameWithMarker ------------------------------------------
        PhpKind::AnonymousFunction              => RenameWithMarker(Function, Anonymous),
        PhpKind::ArrowFunction                  => RenameWithMarker(Function, Arrow),
        PhpKind::ClassConstantAccessExpression  => RenameWithMarker(Member, Constant),
        PhpKind::MatchDefaultExpression         => RenameWithMarker(Arm, Default),
        PhpKind::MemberAccessExpression         => RenameWithMarker(Member, Instance),
        PhpKind::MemberCallExpression           => RenameWithMarker(Call, Instance),
        PhpKind::OptionalType                   => RenameWithMarker(Type, Optional),
        PhpKind::PhpTag                         => RenameWithMarker(Tag, Open),
        PhpKind::PrimitiveType                  => RenameWithMarker(Type, Primitive),
        PhpKind::ScopedCallExpression           => RenameWithMarker(Call, Static),
        PhpKind::ScopedPropertyAccessExpression => RenameWithMarker(Member, Static),
        PhpKind::UnionType                      => RenameWithMarker(Type, Union),
        PhpKind::VariadicParameter              => RenameWithMarker(Parameter, Variadic),

        // ---- Flatten with field distribution ---------------------------
        PhpKind::Arguments        => Flatten { distribute_field: Some("arguments") },
        PhpKind::FormalParameters => Flatten { distribute_field: Some("parameters") },

        // ---- Pure Flatten ----------------------------------------------
        PhpKind::AnonymousFunctionUseClause
        | PhpKind::ArrayElementInitializer
        | PhpKind::AttributeGroup
        | PhpKind::AttributeList
        | PhpKind::CompoundStatement
        | PhpKind::DeclarationList
        | PhpKind::DeclareDirective
        | PhpKind::EnumDeclarationList
        | PhpKind::EscapeSequence
        | PhpKind::MatchBlock
        | PhpKind::MatchConditionList
        | PhpKind::NamespaceName
        | PhpKind::NamespaceUseClause
        | PhpKind::NamespaceUseGroup
        | PhpKind::ParenthesizedExpression
        | PhpKind::PropertyElement
        | PhpKind::QualifiedName
        | PhpKind::StringContent => Flatten { distribute_field: None },

        // ---- DefaultAccessThenRename — class members default to public.
        PhpKind::MethodDeclaration   => da(Method),
        PhpKind::PropertyDeclaration => da(Field),

        // ---- Custom (language-specific logic in transformations.rs) ---
        PhpKind::AbstractModifier   => Custom(transformations::modifier),
        PhpKind::Comment            => Custom(transformations::comment),
        PhpKind::EncapsedString     => Custom(transformations::encapsed_string),
        PhpKind::ExpressionStatement => Custom(transformations::expression_statement),
        PhpKind::FinalModifier      => Custom(transformations::modifier),
        PhpKind::ReadonlyModifier   => Custom(transformations::modifier),
        PhpKind::StaticModifier     => Custom(transformations::modifier),
        PhpKind::VisibilityModifier => Custom(transformations::modifier),

        // ---- Pure Rename -----------------------------------------------
        PhpKind::Argument                  => Rename(Argument),
        PhpKind::ArrayCreationExpression   => Rename(Array),
        PhpKind::Attribute                 => Rename(Attribute),
        PhpKind::BaseClause                => Rename(Extends),
        PhpKind::Boolean                   => Rename(Bool),
        PhpKind::BreakStatement            => Rename(Break),
        PhpKind::CaseStatement             => Rename(Case),
        PhpKind::CastExpression            => Rename(Cast),
        PhpKind::CatchClause               => Rename(Catch),
        PhpKind::ClassDeclaration          => Rename(Class),
        PhpKind::ClassInterfaceClause      => Rename(Implements),
        PhpKind::ConditionalExpression     => Rename(Ternary),
        PhpKind::ConstDeclaration          => Rename(Const),
        PhpKind::ConstElement              => Rename(Constant),
        PhpKind::ContinueStatement         => Rename(Continue),
        PhpKind::DeclareStatement          => Rename(Declare),
        PhpKind::DefaultStatement          => Rename(Default),
        PhpKind::DoStatement               => Rename(Do),
        PhpKind::EchoStatement             => Rename(Echo),
        PhpKind::ElseClause                => Rename(Else),
        PhpKind::ElseIfClause              => Rename(ElseIf),
        PhpKind::EnumCase                  => Rename(Constant),
        PhpKind::EnumDeclaration           => Rename(Enum),
        PhpKind::ExitStatement             => Rename(Exit),
        PhpKind::FinallyClause             => Rename(Finally),
        PhpKind::Float                     => Rename(Float),
        PhpKind::ForStatement              => Rename(For),
        PhpKind::ForeachStatement          => Rename(Foreach),
        PhpKind::FunctionCallExpression    => Rename(Call),
        PhpKind::FunctionDefinition        => Rename(Function),
        PhpKind::GotoStatement             => Rename(Goto),
        PhpKind::IfStatement               => Rename(If),
        PhpKind::IncludeExpression         => Rename(Require),
        PhpKind::IncludeOnceExpression     => Rename(Require),
        PhpKind::Integer                   => Rename(Int),
        PhpKind::InterfaceDeclaration      => Rename(Interface),
        PhpKind::MatchConditionalExpression => Rename(Arm),
        PhpKind::MatchExpression           => Rename(Match),
        PhpKind::NamedType                 => Rename(Type),
        PhpKind::NamespaceDefinition       => Rename(Namespace),
        PhpKind::NamespaceUseDeclaration   => Rename(Use),
        PhpKind::Null                      => Rename(Null),
        PhpKind::ObjectCreationExpression  => Rename(New),
        PhpKind::PrintIntrinsic            => Rename(Print),
        PhpKind::Program                   => Rename(Program),
        PhpKind::RequireExpression         => Rename(Require),
        PhpKind::RequireOnceExpression     => Rename(Require),
        PhpKind::ReturnStatement           => Rename(Return),
        PhpKind::SimpleParameter           => Rename(Parameter),
        PhpKind::String                    => Rename(String),
        PhpKind::SubscriptExpression       => Rename(Index),
        PhpKind::SwitchStatement           => Rename(Switch),
        PhpKind::TextInterpolation         => Rename(Interpolation),
        PhpKind::ThrowExpression           => Rename(Throw),
        PhpKind::TraitDeclaration          => Rename(Trait),
        PhpKind::TryStatement              => Rename(Try),
        PhpKind::TypeList                  => Rename(Types),
        PhpKind::UseDeclaration            => Rename(Use),
        PhpKind::VariableName              => Rename(Variable),
        PhpKind::VariadicUnpacking         => Rename(Spread),
        PhpKind::WhileStatement            => Rename(While),
        PhpKind::YieldExpression           => Rename(Yield),

        // ---- Passthrough — kind name already matches the vocabulary,
        //      OR the kind is unhandled and the dispatcher leaves it as
        //      its raw grammar name (the previous behavior of the
        //      catch-all `_` arm when `map_element_name` returned `None`).

        // Already matches our vocabulary.
        PhpKind::Name | PhpKind::Pair => Custom(transformations::passthrough),

        // ---- Unhandled in the previous dispatcher — survive as raw
        //      kind names. Most are TODO candidates for real semantics.
        //      Grouped by theme; see Step 7 follow-up commit for proposals.

        // TODO: PHP 8 nullsafe operator (`?->`). Sibling of
        //   member_access_expression  → RenameWithMarker(MEMBER, INSTANCE)
        //   member_call_expression    → RenameWithMarker(CALL, INSTANCE)
        // Likely need a NULLSAFE marker:
        //   nullsafe_member_access_expression → RenameWithMarker(MEMBER, NULLSAFE)
        //   nullsafe_member_call_expression   → RenameWithMarker(CALL, NULLSAFE)
        // (with INSTANCE preserved or replaced).
        PhpKind::NullsafeMemberAccessExpression
        | PhpKind::NullsafeMemberCallExpression => Custom(transformations::passthrough),

        // TODO: heredoc / nowdoc string variants (`<<<EOT` blocks).
        // Each could rename to STRING with a marker for the variant.
        //   heredoc / heredoc_body / heredoc_end / heredoc_start
        //   nowdoc / nowdoc_body / nowdoc_string
        PhpKind::Heredoc
        | PhpKind::HeredocBody
        | PhpKind::HeredocEnd
        | PhpKind::HeredocStart
        | PhpKind::Nowdoc
        | PhpKind::NowdocBody
        | PhpKind::NowdocString => Custom(transformations::passthrough),

        // TODO: PHP 8.1+ intersection-type / disjunctive-normal-form-
        // type / bottom-type. Sibling of UnionType → RenameWithMarker(TYPE, UNION).
        // Likely:
        //   intersection_type            → RenameWithMarker(TYPE, INTERSECTION)
        //   disjunctive_normal_form_type → similar
        //   bottom_type (`never`)        → RenameWithMarker(TYPE, BOTTOM)
        PhpKind::BottomType
        | PhpKind::DisjunctiveNormalFormType
        | PhpKind::IntersectionType => Custom(transformations::passthrough),

        // TODO: anonymous class — `new class { … }`. Sibling of
        // class_declaration → CLASS, just inline. Likely
        // RenameWithMarker(CLASS, ANONYMOUS).
        PhpKind::AnonymousClass => Custom(transformations::passthrough),

        // TODO: increment / decrement (`++$x`, `$x--`); augmented
        // assignment (`+=`); reference assignment (`=&`); reference
        // modifier (`&` parameter). Each is a binary/unary variant.
        //   update_expression                → ExtractOpThenRename(UNARY)
        //   augmented_assignment_expression  → ExtractOpThenRename(ASSIGN)
        //   reference_assignment_expression  → ExtractOpThenRename(ASSIGN) + REF marker?
        //   reference_modifier               → modifier helper?
        //   by_ref                           → marker?
        PhpKind::AugmentedAssignmentExpression
        | PhpKind::ByRef
        | PhpKind::ReferenceAssignmentExpression
        | PhpKind::ReferenceModifier => Custom(transformations::passthrough),

        // `update_expression` covers `$x++` / `$x--` / `++$x` / `--$x`.
        // Custom dispatch detects prefix-vs-postfix from child order and
        // adds a `<prefix/>` marker for prefix forms — matches C#'s
        // explicit `prefix_unary_expression` so `//unary[prefix]` works
        // cross-language.
        PhpKind::UpdateExpression          => Custom(transformations::update_expression),

        // TODO: special-statement forms — `clone`, `unset`, `empty`
        // (`;`), `goto`-target labels, list-literal destructuring,
        // switch_block (the `{ case … }` body of switch_statement).
        //   clone_expression       → Rename(CLONE)? own semantic?
        //   unset_statement        → Rename(UNSET)?
        //   empty_statement        → Flatten or Skip
        //   named_label_statement  → Rename(LABEL)?
        //   list_literal           → Rename(ARRAY) with marker?
        //   switch_block           → Rename(BODY) (matches Java)
        PhpKind::CloneExpression
        | PhpKind::EmptyStatement
        | PhpKind::ListLiteral
        | PhpKind::NamedLabelStatement
        | PhpKind::SwitchBlock
        | PhpKind::UnsetStatement => Custom(transformations::passthrough),

        // TODO: PHP-specific value forms.
        //   error_suppression_expression  (`@func()`) — Rename(CALL) marker?
        //   shell_command_expression      (`` `cmd` ``) — own semantic?
        //   sequence_expression           (`a, b, c` in `for(;;)`) — Flatten?
        //   relative_scope                (`self::`, `static::`) — passthrough?
        //   variadic_placeholder          (`...` in `func(...)` first-class callable) — marker?
        PhpKind::ErrorSuppressionExpression
        | PhpKind::RelativeScope
        | PhpKind::SequenceExpression
        | PhpKind::ShellCommandExpression
        | PhpKind::VariadicPlaceholder => Custom(transformations::passthrough),

        // TODO: declaration variants — function-local `static $x;`
        // (different from class static), `global $x;`, alternative
        // `:` syntax block, deprecated `var` keyword for properties.
        //   function_static_declaration   → Rename(VARIABLE) marker?
        //   global_declaration            → Rename(VARIABLE) marker?
        //   colon_block                   → Flatten?
        //   var_modifier                  → modifier helper (deprecated `var $x`)
        //   static_variable_declaration   → Rename(VARIABLE)?
        PhpKind::ColonBlock
        | PhpKind::FunctionStaticDeclaration
        | PhpKind::GlobalDeclaration
        | PhpKind::StaticVariableDeclaration
        | PhpKind::VarModifier => Custom(transformations::passthrough),

        // TODO: cast type child of cast_expression; dynamic variable
        // name (`$$foo`); raw HTML text outside php tags.
        //   cast_type             → Rename(TYPE)?
        //   dynamic_variable_name → Rename(VARIABLE) marker?
        //   text                  → Flatten or Rename(STRING)?
        PhpKind::CastType
        | PhpKind::DynamicVariableName
        | PhpKind::Text => Custom(transformations::passthrough),

        // TODO: `use Trait` inside a class — different from namespace
        // `use`. Currently both rename to USE; trait-use needs its
        // own semantic and the as/insteadof clauses likely flatten.
        //   use_as_clause         → Flatten?
        //   use_insteadof_clause  → Flatten?
        //   use_list              → Flatten
        PhpKind::UseAsClause
        | PhpKind::UseInsteadOfClause
        | PhpKind::UseList => Custom(transformations::passthrough),

        // TODO: PHP 8.4+ property hooks (`get { … }` / `set { … }`).
        // Likely Rename to a per-hook keyword, similar to C# accessors.
        //   property_hook                → Rename(?) per hook kind
        //   property_hook_list           → Flatten
        //   property_promotion_parameter → Rename(PARAMETER) with marker?
        PhpKind::PropertyHook
        | PhpKind::PropertyHookList
        | PhpKind::PropertyPromotionParameter => Custom(transformations::passthrough),

        // ---- Truly raw structural supertypes. Tree-sitter exposes
        //      these as named kinds for grammar-introspection but they
        //      almost never appear in parsed output; preserved as
        //      passthrough for completeness.
        PhpKind::Expression
        | PhpKind::Literal
        | PhpKind::Operation
        | PhpKind::PrimaryExpression
        | PhpKind::Statement
        | PhpKind::Type => Custom(transformations::passthrough),
    }
}
