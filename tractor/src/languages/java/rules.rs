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
        JavaKind::ExpressionStatement          => Custom(transformations::skip),
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
        JavaKind::ImportDeclaration           => Rename(Import),
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
        JavaKind::SwitchBlock                 => Rename(Body),
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
        | JavaKind::Throws => Custom(transformations::passthrough),

        // ---- Unhandled in the previous dispatcher — survive as raw
        //      kind names. Most are TODO candidates for real semantics.
        //      Grouped by theme; see Step 7 follow-up commit for proposals.

        // TODO: Java module-info kinds (Java 9+ JPMS). Currently
        // unhandled. Likely each gets a dedicated semantic name.
        JavaKind::ExportsModuleDirective
        | JavaKind::ModuleBody
        | JavaKind::ModuleDeclaration
        | JavaKind::ModuleDirective
        | JavaKind::OpensModuleDirective
        | JavaKind::ProvidesModuleDirective
        | JavaKind::RequiresModifier
        | JavaKind::RequiresModuleDirective
        | JavaKind::UsesModuleDirective => Custom(transformations::passthrough),

        // TODO: Java annotation-type kinds (the @interface form).
        // Currently unhandled. Likely:
        //   annotation_type_declaration → Rename(INTERFACE) with marker
        //   annotation_type_body → Flatten
        //   annotation_type_element_declaration → Rename(METHOD)?
        JavaKind::AnnotationTypeBody
        | JavaKind::AnnotationTypeDeclaration
        | JavaKind::AnnotationTypeElementDeclaration => Custom(transformations::passthrough),

        // TODO: pattern combinators sit alongside `type_pattern` and
        // `record_pattern` already in the rule table. Each should
        // rename to PATTERN with a marker:
        //   underscore_pattern → RenameWithMarker(PATTERN, ?)
        // Test impact: minimal in current snapshots; new pattern fixtures.
        JavaKind::UnderscorePattern => Custom(transformations::passthrough),

        // TODO: array creation is the call-shaped sibling of
        // `object_creation_expression` → NEW. Likely RenameWithMarker(NEW, ARRAY).
        JavaKind::ArrayCreationExpression => Custom(transformations::passthrough),

        // TODO: special-statement forms — `break`, `continue`, `do`,
        // `assert`, `synchronized`, `yield`, `labeled`. Each
        // currently survives as its grammar kind. Most should rename
        // to a new keyword constant.
        JavaKind::AssertStatement
        | JavaKind::BreakStatement
        | JavaKind::ContinueStatement
        | JavaKind::DoStatement
        | JavaKind::LabeledStatement
        | JavaKind::SynchronizedStatement
        | JavaKind::YieldStatement => Custom(transformations::passthrough),

        // TODO: try-with-resources is the resource-managing sibling of
        // `try_statement` → TRY. Likely RenameWithMarker(TRY, RESOURCE)
        // or own semantic. `resource` / `resource_specification` are
        // its body shape.
        JavaKind::Resource
        | JavaKind::ResourceSpecification
        | JavaKind::TryWithResourcesStatement => Custom(transformations::passthrough),

        // TODO: cast expression is `(int)x`; instanceof_expression is
        // `x instanceof Foo`. Both are conversion-related.
        JavaKind::CastExpression
        | JavaKind::InstanceofExpression => Custom(transformations::passthrough),

        // TODO: increment / decrement — `++x`, `x--`. Same shape as
        // unary_expression (extract op + rename UNARY).
        JavaKind::UpdateExpression => Custom(transformations::passthrough),

        // TODO: catch_formal_parameter is the variable inside a catch
        // clause — Rename(PARAMETER); catch_type is the exception type
        // expression — Rename(TYPE)? `extends_interfaces` is sibling of
        // `super_interfaces` → IMPLEMENTS; likely Rename(EXTENDS).
        JavaKind::CatchFormalParameter
        | JavaKind::CatchType
        | JavaKind::ExtendsInterfaces => Custom(transformations::passthrough),

        // TODO: literal kinds not yet renamed.
        //   character_literal → Rename(STRING)? own CHAR semantic?
        //   class_literal → Rename(TYPE) with marker (`Foo.class`)
        //   hex_floating_point_literal → Rename(FLOAT)
        JavaKind::CharacterLiteral
        | JavaKind::ClassLiteral
        | JavaKind::HexFloatingPointLiteral => Custom(transformations::passthrough),

        // TODO: array / annotation initializer / element kinds.
        //   array_initializer → similar to `<call>` arguments?
        //   element_value_array_initializer → annotation array form
        //   element_value_pair → key=value annotation argument
        JavaKind::ArrayInitializer
        | JavaKind::ElementValueArrayInitializer
        | JavaKind::ElementValuePair => Custom(transformations::passthrough),

        // TODO: dimensions (the `[]` after a type) and dimensions_expr
        // (the size in `new int[5]`). Already in NODES via DIMENSIONS.
        JavaKind::Dimensions
        | JavaKind::DimensionsExpr => Custom(transformations::passthrough),

        // TODO: receiver_parameter (instance method's `this` param);
        // method_reference (`Foo::bar`); inferred_parameters (lambda's
        // implicit param list); permits (sealed-class permits clause);
        // static_initializer (static {} block).
        JavaKind::InferredParameters
        | JavaKind::MethodReference
        | JavaKind::Permits
        | JavaKind::ReceiverParameter
        | JavaKind::StaticInitializer => Custom(transformations::passthrough),

        // TODO: string interpolation (Java 21 string templates).
        //   template_expression → Rename(STRING) with marker?
        //   string_interpolation → Flatten?
        //   multiline_string_fragment → Flatten (text block content)
        //   escape_sequence → Flatten
        JavaKind::EscapeSequence
        | JavaKind::MultilineStringFragment
        | JavaKind::StringInterpolation
        | JavaKind::TemplateExpression => Custom(transformations::passthrough),

        // TODO: declaration markers — annotated_type wraps a type with
        // annotations: `@NonNull String`. Likely Rename(TYPE) with
        // additional handling.
        JavaKind::AnnotatedType => Custom(transformations::passthrough),

        // TODO: misc grammar kinds.
        //   constant_declaration → interface field; Rename(FIELD) with marker?
        //   asterisk → import wildcard `import java.util.*;`
        //   wildcard → generic wildcard `<?>`; Rename(GENERIC) with marker?
        //   record_pattern_body / record_pattern_component → record-pattern subparts
        JavaKind::Asterisk
        | JavaKind::ConstantDeclaration
        | JavaKind::RecordPatternBody
        | JavaKind::RecordPatternComponent
        | JavaKind::Wildcard => Custom(transformations::passthrough),

        // ---- Truly raw structural supertypes. Tree-sitter exposes
        //      these as named kinds for grammar-introspection but they
        //      almost never appear in parsed output; preserved as
        //      passthrough for completeness.
        JavaKind::Declaration
        | JavaKind::Expression
        | JavaKind::PrimaryExpression
        | JavaKind::Statement => Custom(transformations::passthrough),
    }
}
