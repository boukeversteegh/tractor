//! Per-kind transformation rules for Java: the `JavaKind` → `Rule` table.
//!
//! Read this file to find the rule for a specific kind. Read
//! [`super::transformations`] for the body of any `Rule::Custom`
//! handler the rule references by name. Read [`super::output`] for
//! the output vocabulary (semantic names + TractorNodeSpec metadata).
//!
//! Exhaustive over `JavaKind` — the compiler enforces coverage. When
//! the grammar ships a new kind, regenerating `input.rs` adds a
//! variant and this match fails to build until the new kind is
//! classified.
//!
//! Pure data variants (`Rename`, `RenameWithMarker`, `Flatten`,
//! `ExtractOpThenRename`, `DefaultAccessThenRename`) are executed by
//! the shared [`crate::languages::rule::dispatch`] helper. Custom logic
//! lives in [`super::transformations`].

use crate::languages::rule::Rule;

use super::input::JavaKind;
use super::output::TractorNode::{self, *};
use super::transformations;

/// Shorthand for the `default-access-then-rename` shape used by 5 of
/// Java's 6 declaration kinds (class / interface / enum / constructor
/// / field). Bakes in Java's default-access resolver so the rule arms
/// read as data. The 6th declaration kind (method) needs an extra
/// return-type wrapping step and stays a `Custom` handler.
fn da(to: TractorNode) -> Rule<TractorNode> {
    Rule::DefaultAccessThenRename {
        to,
        default_access: transformations::default_access_for_declaration,
    }
}

pub fn rule(k: JavaKind) -> Rule<TractorNode> {
    use Rule::*;
    match k {
        // ---- ExtractOpThenRename ---------------------------------------
        JavaKind::AssignmentExpression => ExtractOpThenRename(Assign),
        JavaKind::BinaryExpression     => ExtractOpThenRename(Binary),
        JavaKind::UnaryExpression      => ExtractOpThenRename(Unary),

        // ---- RenameWithMarker ------------------------------------------
        JavaKind::ArrayType                     => RenameWithMarker(Type, Array),
        JavaKind::CompactConstructorDeclaration => RenameWithMarker(Constructor, Compact),
        JavaKind::RecordPattern                 => RenameWithMarker(Pattern, Record),
        JavaKind::SpreadParameter               => RenameWithMarker(Parameter, Variadic),
        // `case Integer i ->`. The structural `<type>` child already
        // signals "this is a type pattern"; the `[type]` marker we
        // used to add was redundant AND created a JSON marker-vs-
        // wrapper collision (boolean `type: true` vs wrapper
        // `type: {...}`). Drop the marker; `//pattern[type]` still
        // queries structurally. See iter 184 marker-collision lesson
        // (the analogous `<alias/>` marker + `<alias>` wrapper case
        // that resolved by renaming the wrapper to `<aliased>`; here
        // dropping the marker is cleaner since `<type>` is the
        // canonical name for the structural child).
        JavaKind::TypePattern                   => Rename(Pattern),

        // ---- Flatten with field distribution ---------------------------
        JavaKind::ArgumentList     => Flatten { distribute_list: Some("arguments") },
        JavaKind::FormalParameters => Flatten { distribute_list: Some("parameters") },
        JavaKind::TypeArguments    => Flatten { distribute_list: Some("arguments") },

        // ---- Pure Flatten ----------------------------------------------
        JavaKind::AnnotationArgumentList
        | JavaKind::Block
        | JavaKind::ClassBody
        | JavaKind::ConstructorBody
        | JavaKind::EnumBody
        | JavaKind::EnumBodyDeclarations
        | JavaKind::InterfaceBody
        | JavaKind::StringFragment
        | JavaKind::TypeList => Flatten { distribute_list: None },

        // ---- DefaultAccessThenRename — 5 of 6 declaration kinds.
        //      method_declaration is Custom (extra return-type wrapping).
        JavaKind::ClassDeclaration       => da(Class),
        JavaKind::ConstructorDeclaration => da(Constructor),
        JavaKind::EnumDeclaration        => da(Enum),
        JavaKind::FieldDeclaration       => da(Field),
        JavaKind::InterfaceDeclaration   => da(Interface),

        // ---- Custom (language-specific logic in transformations.rs) ---
        JavaKind::BlockComment                 => Custom(transformations::comment),
        JavaKind::BooleanType                  => Custom(transformations::primitive_type),
        JavaKind::ExplicitConstructorInvocation => Custom(transformations::explicit_constructor_invocation),
        JavaKind::ExpressionStatement          => Custom(transformations::expression_statement),
        JavaKind::FloatingPointType            => Custom(transformations::primitive_type),
        JavaKind::GenericType                  => Custom(transformations::generic_type),
        JavaKind::Identifier                   => Custom(transformations::identifier),
        JavaKind::IfStatement                  => Custom(transformations::if_statement),
        JavaKind::IntegralType                 => Custom(transformations::primitive_type),
        JavaKind::LineComment                  => Custom(transformations::comment),
        JavaKind::MethodDeclaration            => Custom(transformations::method_declaration),
        JavaKind::Modifiers                    => Custom(transformations::modifiers),
        JavaKind::ParenthesizedExpression      => Custom(transformations::skip),
        JavaKind::TernaryExpression            => Custom(transformations::ternary_expression),
        JavaKind::TypeIdentifier               => Custom(transformations::type_identifier),
        JavaKind::TypeParameter                => Custom(transformations::type_parameter),
        JavaKind::TypeParameters               => Custom(transformations::type_parameters),
        JavaKind::VoidType                     => Custom(transformations::void_type),

        // ---- Pure Rename -----------------------------------------------
        JavaKind::Annotation                  => Rename(Annotation),
        JavaKind::ArrayAccess                 => Rename(Index),
        JavaKind::BinaryIntegerLiteral        => Rename(Int),
        JavaKind::CatchClause                 => Rename(Catch),
        JavaKind::DecimalFloatingPointLiteral => Rename(Float),
        JavaKind::DecimalIntegerLiteral       => Rename(Int),
        JavaKind::EnhancedForStatement        => Rename(Foreach),
        JavaKind::EnumConstant                => Rename(Constant),
        JavaKind::False                       => Rename(False),
        // `obj.field` — receiver and accessed property play different
        // roles. Per Principle #19 (role-mixed wrap): each role gets a
        // slot-named container so two `<name>` siblings don't rely on
        // sibling order to disambiguate. Matches TypeScript's
        // `<member><object/><property/></member>` shape (iter 147).
        JavaKind::FieldAccess                 => Custom(transformations::field_access),
        JavaKind::FinallyClause               => Rename(Finally),
        JavaKind::ForStatement                => Rename(For),
        JavaKind::FormalParameter             => Rename(Parameter),
        JavaKind::HexIntegerLiteral           => Rename(Int),
        JavaKind::ImportDeclaration           => Custom(transformations::import_declaration),
        JavaKind::LambdaExpression            => Rename(Lambda),
        JavaKind::LocalVariableDeclaration    => Rename(Variable),
        JavaKind::MarkerAnnotation            => Rename(Annotation),
        // `obj.method(args)` — receiver and method-name play different
        // roles. Wrap the receiver in `<object>`; the method-name
        // stays as `<name>` (the canonical singleton property of any
        // declaration / call). Per Principle #19 (iter 147).
        JavaKind::MethodInvocation            => Custom(transformations::method_invocation),
        JavaKind::NullLiteral                 => Rename(Null),
        JavaKind::ObjectCreationExpression    => Rename(New),
        JavaKind::OctalIntegerLiteral         => Rename(Int),
        JavaKind::PackageDeclaration          => Rename(Package),
        JavaKind::Program                     => Rename(Program),
        JavaKind::RecordDeclaration           => Rename(Record),
        JavaKind::ReturnStatement             => Rename(Return),
        JavaKind::ScopedIdentifier            => Rename(Path),
        JavaKind::ScopedTypeIdentifier        => Rename(Path),
        JavaKind::StringLiteral               => Rename(String),
        // `implements A, B, C` — Principle #12 forbids the
        // list-container `<implements>{type=A, type=B, type=C}` shape;
        // produce multiple `<implements>` siblings, one per interface.
        // Drops the literal `implements` keyword text leaf.
        JavaKind::SuperInterfaces             => Custom(transformations::super_interfaces),
        // `class Foo extends Bar` — Java allows only one parent class.
        // Wrap in `<extends list="extends">` for JSON-array
        // consistency (Principle #12).
        JavaKind::Superclass                  => Custom(transformations::superclass),
        // `switch_expression.body` field already wraps this in <body>;
        // flatten avoids double-nested <body><body>...</body></body>.
        JavaKind::SwitchBlock                 => Flatten { distribute_list: None },
        JavaKind::SwitchBlockStatementGroup   => Rename(Case),
        JavaKind::SwitchExpression            => Rename(Switch),
        // `case X:` / `default:` switch label. The `default` form has
        // just bare `default` keyword text; convert to a `[default]`
        // marker so the shape is `<label[default]/>` for the default
        // arm, `<label>...</label>` (with case expression) otherwise.
        JavaKind::SwitchLabel                 => Custom(transformations::switch_label),
        JavaKind::SwitchRule                  => Rename(Arm),
        JavaKind::ThrowStatement              => Rename(Throw),
        JavaKind::True                        => Rename(True),
        JavaKind::TryStatement                => Rename(Try),
        // `<T extends A & B>` — multi-bound generic constraint.
        // Per Principle #12 (no list containers) + #18 (name after
        // operator): each bound becomes a flat `<extends>` sibling
        // with `list="extends"`.
        JavaKind::TypeBound                   => Custom(transformations::type_bound),
        JavaKind::VariableDeclarator          => Rename(Declarator),
        JavaKind::WhileStatement              => Rename(While),

        // ---- Passthrough — kind name already matches the vocabulary,
        //      OR the kind is unhandled and the dispatcher leaves it as
        //      its raw grammar name (the previous behavior of the
        //      catch-all `_` arm when `map_element_name` returned `None`).

        // Already matches our vocabulary.
        JavaKind::Guard
        | JavaKind::Pattern
        | JavaKind::Super
        | JavaKind::This => Passthrough,

        // `throws E1, E2, E3` — Principle #18: name after the
        // operator/keyword. Principle #12: multiple targets →
        // multiple `<throws>` siblings, not a list container.
        JavaKind::Throws => Custom(transformations::throws_clause),

        // ---- Unhandled in the previous dispatcher — survive as raw
        //      kind names. Pending real-semantics candidates tracked
        //      in todo/36-rule-todo-followups.md, grouped by theme.

        // `update_expression` covers `++x` / `x++` / `--x` / `x--`.
        // Custom dispatch detects prefix-vs-postfix from child order and
        // adds a `<prefix/>` marker for prefix forms (parallels C#'s
        // explicit `prefix_unary_expression` kind).
        JavaKind::UpdateExpression => Custom(transformations::update_expression),

        // ---- Control flow (Principle #17 — no underscored names) ------
        JavaKind::AssertStatement        => Rename(Assert),
        JavaKind::BreakStatement         => Rename(Break),
        JavaKind::ContinueStatement      => Rename(Continue),
        JavaKind::DoStatement            => Rename(Do),
        JavaKind::LabeledStatement       => Rename(Label),
        JavaKind::SynchronizedStatement  => Custom(transformations::synchronized_statement),
        JavaKind::YieldStatement         => Rename(Yield),

        // ---- Try-with-resources ----------------------------------------
        JavaKind::TryWithResourcesStatement => RenameWithMarker(Try, Resource),
        JavaKind::ResourceSpecification     => Flatten { distribute_list: None },

        // ---- Expressions -----------------------------------------------
        JavaKind::ArrayCreationExpression => RenameWithMarker(New, Array),
        JavaKind::CastExpression          => Rename(Cast),
        JavaKind::InstanceofExpression    => Rename(Instanceof),
        JavaKind::MethodReference         => Rename(Reference),

        // ---- Catch / extends -------------------------------------------
        JavaKind::CatchFormalParameter => Rename(Parameter),
        JavaKind::CatchType            => Flatten { distribute_list: None },
        // `interface I extends A, B` — multiple parent interfaces.
        // Same shape as `super_interfaces`: flat `<extends>` siblings
        // with `list="extends"`.
        JavaKind::ExtendsInterfaces    => Custom(transformations::extends_interfaces),

        // ---- Literals --------------------------------------------------
        JavaKind::CharacterLiteral       => Rename(Char),
        JavaKind::ClassLiteral           => RenameWithMarker(Type, Class),
        JavaKind::HexFloatingPointLiteral => Rename(Float),

        // ---- Array / annotation initializers ---------------------------
        JavaKind::ArrayInitializer              => Rename(Array),
        JavaKind::ElementValueArrayInitializer  => Rename(Array),
        JavaKind::ElementValuePair              => Rename(Pair),

        // ---- Dimensions ------------------------------------------------
        JavaKind::DimensionsExpr => Rename(Dimensions),

        // ---- Method shapes ---------------------------------------------
        JavaKind::InferredParameters => Flatten { distribute_list: None },
        JavaKind::ReceiverParameter  => RenameWithMarker(Parameter, Receiver),
        JavaKind::StaticInitializer  => RenameWithMarker(Block, Static),

        // ---- String / template interpolation ---------------------------
        JavaKind::EscapeSequence          => Flatten { distribute_list: None },
        JavaKind::MultilineStringFragment => Flatten { distribute_list: None },
        JavaKind::StringInterpolation     => Rename(Interpolation),
        JavaKind::TemplateExpression      => Rename(Template),

        // ---- Annotation-type (@interface) ------------------------------
        JavaKind::AnnotatedType                    => Flatten { distribute_list: None },
        JavaKind::AnnotationTypeBody               => Flatten { distribute_list: None },
        JavaKind::AnnotationTypeDeclaration        => RenameWithMarker(Interface, Annotation),
        JavaKind::AnnotationTypeElementDeclaration => Rename(Method),

        // ---- Patterns --------------------------------------------------
        JavaKind::UnderscorePattern     => RenameWithMarker(Pattern, Wildcard),
        JavaKind::RecordPatternBody     => Flatten { distribute_list: None },
        JavaKind::RecordPatternComponent => Flatten { distribute_list: None },

        // ---- Interface constants / misc --------------------------------
        JavaKind::ConstantDeclaration => Rename(Field),

        // ---- Module-info (Java 9+ JPMS) --------------------------------
        JavaKind::ModuleDeclaration       => Rename(Module),
        JavaKind::ModuleBody              => Flatten { distribute_list: None },
        JavaKind::ModuleDirective         => Flatten { distribute_list: None },
        JavaKind::ExportsModuleDirective  => RenameWithMarker(Directive, Exports),
        JavaKind::OpensModuleDirective    => RenameWithMarker(Directive, Opens),
        JavaKind::ProvidesModuleDirective => RenameWithMarker(Directive, Provides),
        JavaKind::RequiresModuleDirective => RenameWithMarker(Directive, Requires),
        JavaKind::UsesModuleDirective     => RenameWithMarker(Directive, Uses),
        JavaKind::RequiresModifier        => Flatten { distribute_list: None },

        // ---- Passthrough — single-word names (no underscore) ----------
        // `permits` (sealed-class clause) — single-word OK.
        // `asterisk` (import wildcard) — single-word OK.
        JavaKind::Permits | JavaKind::Asterisk => Passthrough,
        // `dimensions` is the `[]` suffix on `int[]` — pure syntax.
        // The parent `<type[array]>` already conveys "array"; detach
        // the literal `[]` so it doesn't leak as `dimensions = "[]"`.
        JavaKind::Dimensions => Detach,

        // `resource` (try-with-resources variable) — like a variable declaration.
        // Named to avoid clash with the `Resource` marker on `<try[resource]>`.
        JavaKind::Resource => Rename(Variable),
        // `wildcard` (generic `<?>`, `<? extends T>`) — type-level wildcard.
        // Bare `<? >` becomes a `<wildcard/>` marker on the parent
        // type. Bounded forms (`<? extends T>`) keep the bound names
        // as children alongside the marker — handled in the custom.
        JavaKind::Wildcard => Custom(transformations::wildcard),

        // ---- Structural supertypes (single-word, almost never in output)
        JavaKind::Declaration
        | JavaKind::Expression
        | JavaKind::Statement => Passthrough,
        // PrimaryExpression has an underscore — flatten instead of passthrough.
        JavaKind::PrimaryExpression => Flatten { distribute_list: None },
    }
}
