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
        JavaKind::TypePattern                   => RenameWithMarker(Pattern, Type),

        // ---- Flatten with field distribution ---------------------------
        JavaKind::ArgumentList     => Flatten { distribute_field: Some("arguments") },
        JavaKind::FormalParameters => Flatten { distribute_field: Some("parameters") },
        JavaKind::TypeArguments    => Flatten { distribute_field: Some("arguments") },

        // ---- Pure Flatten ----------------------------------------------
        JavaKind::AnnotationArgumentList
        | JavaKind::Block
        | JavaKind::ClassBody
        | JavaKind::ConstructorBody
        | JavaKind::EnumBody
        | JavaKind::EnumBodyDeclarations
        | JavaKind::InterfaceBody
        | JavaKind::StringFragment
        | JavaKind::TypeList => Flatten { distribute_field: None },

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
        JavaKind::FieldAccess                 => Rename(Member),
        JavaKind::FinallyClause               => Rename(Finally),
        JavaKind::ForStatement                => Rename(For),
        JavaKind::FormalParameter             => Rename(Parameter),
        JavaKind::HexIntegerLiteral           => Rename(Int),
        JavaKind::ImportDeclaration           => Custom(transformations::import_declaration),
        JavaKind::LambdaExpression            => Rename(Lambda),
        JavaKind::LocalVariableDeclaration    => Rename(Variable),
        JavaKind::MarkerAnnotation            => Rename(Annotation),
        JavaKind::MethodInvocation            => Rename(Call),
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
        JavaKind::SuperInterfaces             => Rename(Implements),
        JavaKind::Superclass                  => Rename(Extends),
        // `switch_expression.body` field already wraps this in <body>;
        // flatten avoids double-nested <body><body>...</body></body>.
        JavaKind::SwitchBlock                 => Flatten { distribute_field: None },
        JavaKind::SwitchBlockStatementGroup   => Rename(Case),
        JavaKind::SwitchExpression            => Rename(Switch),
        JavaKind::SwitchLabel                 => Rename(Label),
        JavaKind::SwitchRule                  => Rename(Arm),
        JavaKind::ThrowStatement              => Rename(Throw),
        JavaKind::True                        => Rename(True),
        JavaKind::TryStatement                => Rename(Try),
        JavaKind::TypeBound                   => Rename(Extends),
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
        | JavaKind::This
        | JavaKind::Throws => Passthrough,

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
        JavaKind::ResourceSpecification     => Flatten { distribute_field: None },

        // ---- Expressions -----------------------------------------------
        JavaKind::ArrayCreationExpression => RenameWithMarker(New, Array),
        JavaKind::CastExpression          => Rename(Cast),
        JavaKind::InstanceofExpression    => Rename(Instanceof),
        JavaKind::MethodReference         => Rename(Reference),

        // ---- Catch / extends -------------------------------------------
        JavaKind::CatchFormalParameter => Rename(Parameter),
        JavaKind::CatchType            => Flatten { distribute_field: None },
        JavaKind::ExtendsInterfaces    => Rename(Extends),

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
        JavaKind::InferredParameters => Flatten { distribute_field: None },
        JavaKind::ReceiverParameter  => RenameWithMarker(Parameter, Receiver),
        JavaKind::StaticInitializer  => RenameWithMarker(Block, Static),

        // ---- String / template interpolation ---------------------------
        JavaKind::EscapeSequence          => Flatten { distribute_field: None },
        JavaKind::MultilineStringFragment => Flatten { distribute_field: None },
        JavaKind::StringInterpolation     => Rename(Interpolation),
        JavaKind::TemplateExpression      => Rename(Template),

        // ---- Annotation-type (@interface) ------------------------------
        JavaKind::AnnotatedType                    => Flatten { distribute_field: None },
        JavaKind::AnnotationTypeBody               => Flatten { distribute_field: None },
        JavaKind::AnnotationTypeDeclaration        => RenameWithMarker(Interface, Annotation),
        JavaKind::AnnotationTypeElementDeclaration => Rename(Method),

        // ---- Patterns --------------------------------------------------
        JavaKind::UnderscorePattern     => RenameWithMarker(Pattern, Wildcard),
        JavaKind::RecordPatternBody     => Flatten { distribute_field: None },
        JavaKind::RecordPatternComponent => Flatten { distribute_field: None },

        // ---- Interface constants / misc --------------------------------
        JavaKind::ConstantDeclaration => Rename(Field),

        // ---- Module-info (Java 9+ JPMS) --------------------------------
        JavaKind::ModuleDeclaration       => Rename(Module),
        JavaKind::ModuleBody              => Flatten { distribute_field: None },
        JavaKind::ModuleDirective         => Flatten { distribute_field: None },
        JavaKind::ExportsModuleDirective  => RenameWithMarker(Directive, Exports),
        JavaKind::OpensModuleDirective    => RenameWithMarker(Directive, Opens),
        JavaKind::ProvidesModuleDirective => RenameWithMarker(Directive, Provides),
        JavaKind::RequiresModuleDirective => RenameWithMarker(Directive, Requires),
        JavaKind::UsesModuleDirective     => RenameWithMarker(Directive, Uses),
        JavaKind::RequiresModifier        => Flatten { distribute_field: None },

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
        JavaKind::PrimaryExpression => Flatten { distribute_field: None },
    }
}
