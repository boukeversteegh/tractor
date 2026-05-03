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
        PhpKind::AssignmentExpression          => ExtractOpThenRename(Assign),
        PhpKind::AugmentedAssignmentExpression => ExtractOpThenRename(Assign),
        PhpKind::BinaryExpression              => ExtractOpThenRename(Binary),
        PhpKind::UnaryOpExpression             => ExtractOpThenRename(Unary),
        // `=&` reference assignment — same op-extraction shape as `=`,
        // sibling of compound `+=` etc.
        PhpKind::ReferenceAssignmentExpression => ExtractOpThenRename(Assign),
        // `@expr` — `@` is a unary suppress-error operator.
        PhpKind::ErrorSuppressionExpression    => ExtractOpThenRename(Unary),

        // ---- RenameWithMarker ------------------------------------------
        PhpKind::AnonymousFunction              => RenameWithMarker(Function, Anonymous),
        // `arrow_function` — re-tag `<body>` as `<value>` so
        // single-expression body wraps in `<expression>` host
        // (Principle #15). Mirrors iter 161/162/167.
        PhpKind::ArrowFunction                  => Custom(transformations::arrow_function),
        // `class_constant_access_expression` — `Foo::BAR`. Custom
        // handler wraps the two `<name>` siblings in `<object>` /
        // `<property>` so JSON doesn't collide them on the `name`
        // key (matches iter-178 C# member-access shape).
        PhpKind::ClassConstantAccessExpression  => Custom(transformations::class_constant_access),
        PhpKind::MatchDefaultExpression         => RenameWithMarker(Arm, Default),
        // Iter 255: dropped the [instance] marker — redundant
        // given the <object[access]> chain-root wrapper.
        PhpKind::MemberAccessExpression         => Rename(Member),
        PhpKind::MemberCallExpression           => Rename(Call),
        PhpKind::OptionalType                   => RenameWithMarker(Type, Optional),
        PhpKind::PhpTag                         => RenameWithMarker(Tag, Open),
        PhpKind::PrimitiveType                  => Custom(transformations::primitive_type),
        PhpKind::ScopedCallExpression           => RenameWithMarker(Call, Static),
        PhpKind::ScopedPropertyAccessExpression => RenameWithMarker(Member, Static),
        PhpKind::UnionType                      => RenameWithMarker(Type, Union),
        PhpKind::VariadicParameter              => RenameWithMarker(Parameter, Variadic),

        // ---- Iter 17: PHP 8 nullsafe + new type variants -------------
        PhpKind::NullsafeMemberAccessExpression => RenameWithMarker(Member, Nullsafe),
        PhpKind::NullsafeMemberCallExpression   => RenameWithMarker(Call, Nullsafe),
        PhpKind::AnonymousClass                 => RenameWithMarker(Class, Anonymous),
        PhpKind::BottomType                     => RenameWithMarker(Type, Bottom),
        PhpKind::IntersectionType               => RenameWithMarker(Type, Intersection),
        PhpKind::DisjunctiveNormalFormType      => RenameWithMarker(Type, Disjunctive),
        // Function-local `static $x;` / `global $x;` declare scoped vars.
        PhpKind::FunctionStaticDeclaration      => RenameWithMarker(Variable, Static),
        // `global $x;` — rename to `<variable[global]>` BUT also flatten
        // the inner variable_name children so the shape is
        // `<variable[global]><name>x</name></variable>` instead of
        // `<variable[global]><variable>{$, name=x}</variable></variable>`.
        PhpKind::GlobalDeclaration              => Custom(transformations::global_declaration),
        PhpKind::DynamicVariableName            => RenameWithMarker(Variable, Dynamic),
        // Constructor property promotion `public string $name` parameter.
        PhpKind::PropertyPromotionParameter     => RenameWithMarker(Parameter, Promoted),
        // First-class callable syntax `func(...)` — `...` placeholder
        // marks the argument list as "all args".
        PhpKind::VariadicPlaceholder            => RenameWithMarker(Argument, Variadic),

        // ---- Flatten with field distribution ---------------------------
        PhpKind::Arguments        => Flatten { distribute_list: Some("arguments") },
        PhpKind::FormalParameters => Flatten { distribute_list: Some("parameters") },

        // ---- Pure Flatten ----------------------------------------------
        PhpKind::AnonymousFunctionUseClause
        | PhpKind::ArrayElementInitializer
        | PhpKind::AttributeGroup
        | PhpKind::AttributeList
        | PhpKind::ByRef
        | PhpKind::ColonBlock
        | PhpKind::CompoundStatement
        | PhpKind::DeclarationList
        | PhpKind::DeclareDirective
        | PhpKind::EmptyStatement
        | PhpKind::EnumDeclarationList
        | PhpKind::EscapeSequence
        | PhpKind::HeredocBody
        | PhpKind::HeredocEnd
        | PhpKind::HeredocStart
        | PhpKind::MatchBlock
        | PhpKind::MatchConditionList
        | PhpKind::NamespaceName
        | PhpKind::NamespaceUseClause
        | PhpKind::NamespaceUseGroup
        | PhpKind::NowdocBody
        | PhpKind::NowdocString
        | PhpKind::ParenthesizedExpression
        | PhpKind::PrimaryExpression
        | PhpKind::PropertyElement
        | PhpKind::PropertyHookList
        | PhpKind::QualifiedName
        | PhpKind::ReferenceModifier
        | PhpKind::SequenceExpression
        | PhpKind::StringContent
        | PhpKind::SwitchBlock
        | PhpKind::Text
        | PhpKind::UseAsClause
        | PhpKind::UseInsteadOfClause
        | PhpKind::UseList => Flatten { distribute_list: None },

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
        // Deprecated `var $x` property declaration — equivalent to `public`.
        PhpKind::VarModifier        => Custom(transformations::modifier),

        // ---- Pure Rename -----------------------------------------------
        PhpKind::Argument                  => Rename(Argument),
        PhpKind::ArrayCreationExpression   => Rename(Array),
        PhpKind::Attribute                 => Rename(Attribute),
        // `class Foo extends Bar` — PHP allows only one parent class.
        // Wrap in `<extends list="extends">` so JSON serializers
        // produce a uniform `extends: [...]` array regardless of
        // single/multi (Principle #12 — field attribute on
        // collapsed-list children).
        PhpKind::BaseClause                => Custom(transformations::base_clause),
        PhpKind::Boolean                   => Rename(Bool),
        PhpKind::BreakStatement            => RenameStripKeyword(Break, "break"),
        PhpKind::CaseStatement             => Rename(Case),
        PhpKind::CastExpression            => Rename(Cast),
        PhpKind::CatchClause               => Rename(Catch),
        PhpKind::ClassDeclaration          => Rename(Class),
        // `class Foo implements A, B, C` — Principle #12 forbids the
        // list-container `<implements>{name=A, name=B, name=C}` shape;
        // produce multiple `<implements>` siblings, one per interface.
        PhpKind::ClassInterfaceClause      => Custom(transformations::class_interface_clause),
        PhpKind::ConditionalExpression     => Rename(Ternary),
        PhpKind::ConstDeclaration          => Rename(Const),
        PhpKind::ConstElement              => Rename(Constant),
        PhpKind::ContinueStatement         => RenameStripKeyword(Continue, "continue"),
        PhpKind::DeclareStatement          => Rename(Declare),
        PhpKind::DefaultStatement          => Rename(Default),
        PhpKind::DoStatement               => Rename(Do),
        PhpKind::EchoStatement             => Rename(Echo),
        PhpKind::ElseClause                => Rename(Else),
        PhpKind::ElseIfClause              => Rename(ElseIf),
        PhpKind::EnumCase                  => Rename(Constant),
        // `enum Status: string { ... }` — backed enum. The `: string`
        // declares the underlying integral storage type (PHP enums
        // can't inherit). Mark the type child with `[underlying]` so
        // cross-language `//enum/type[underlying]` queries work
        // uniformly with C# (iter 125).
        PhpKind::EnumDeclaration           => Custom(transformations::enum_declaration),
        PhpKind::ExitStatement             => Rename(Exit),
        PhpKind::FinallyClause             => Rename(Finally),
        PhpKind::Float                     => Rename(Float),
        PhpKind::ForStatement              => Rename(For),
        PhpKind::ForeachStatement          => Rename(Foreach),
        PhpKind::FunctionCallExpression    => Rename(Call),
        PhpKind::FunctionDefinition        => Rename(Function),
        PhpKind::GotoStatement             => RenameStripKeyword(Goto, "goto"),
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
        PhpKind::ReturnStatement           => RenameStripKeyword(Return, "return"),
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

        // ---- Iter 17: pure renames for previously-passthrough kinds ---
        PhpKind::CloneExpression           => Rename(Clone),
        PhpKind::UnsetStatement            => Rename(Unset),
        PhpKind::NamedLabelStatement       => Rename(Label),
        // `list($a, $b) = ...` destructuring — same shape as `[..]` array.
        PhpKind::ListLiteral               => Rename(Array),
        // Inner declarator inside `static $x = null;`. The outer
        // `function_static_declaration` already renames to
        // `<variable[static]>`; flattening the inner avoids the
        // `<variable[static]><variable>...</variable></variable>`
        // double-wrap (within-language Principle #5).
        PhpKind::StaticVariableDeclaration => Flatten { distribute_list: None },
        // `(int)` cast type. Wrap bare text in `<name>` so the shape
        // is `<type><name>int</name></type>` matching other PHP type
        // contexts (Principle #14: identifiers in `<name>`).
        PhpKind::CastType                  => Custom(transformations::cast_type),
        // `self`/`parent`/`static` keyword-scope.
        PhpKind::RelativeScope             => Rename(Scope),
        // `` `cmd` `` shell-command literal.
        PhpKind::ShellCommandExpression    => Rename(Shell),
        // PHP 8.4+ property hooks `get { ... }` / `set(value) { ... }`
        // — same shape as accessor methods.
        PhpKind::PropertyHook              => Rename(Method),

        // ---- Passthrough — kind name already matches the vocabulary,
        //      OR the kind is unhandled and the dispatcher leaves it as
        //      its raw grammar name (the previous behavior of the
        //      catch-all `_` arm when `map_element_name` returned `None`).

        // Already matches our vocabulary.
        PhpKind::Name | PhpKind::Pair => Passthrough,

        // `update_expression` covers `$x++` / `$x--` / `++$x` / `--$x`.
        // Custom dispatch detects prefix-vs-postfix from child order and
        // adds a `<prefix/>` marker for prefix forms — matches C#'s
        // explicit `prefix_unary_expression` so `//unary[prefix]` works
        // cross-language.
        PhpKind::UpdateExpression => Custom(transformations::update_expression),

        // `heredoc` / `nowdoc` — `<<<EOT` block-string variants. Both
        // surface as `<string>` with a `[heredoc]` / `[nowdoc]` marker;
        // body / start / end children flatten so the literal text
        // bubbles up. Cross-language `//string` finds them alongside
        // single- and double-quoted strings.
        PhpKind::Heredoc => RenameWithMarker(String, Heredoc),
        PhpKind::Nowdoc  => RenameWithMarker(String, Nowdoc),

        // ---- Single-word passthroughs.
        // `expression` / `literal` / `operation` / `statement` / `type`
        // are tree-sitter structural supertypes that almost never
        // surface in the parsed output.
        PhpKind::Expression
        | PhpKind::Literal
        | PhpKind::Operation
        | PhpKind::Statement
        | PhpKind::Type => Passthrough,
    }
}
