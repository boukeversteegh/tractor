//! Per-kind transformation rules for TypeScript / JavaScript: the
//! `TsKind` → `Rule` table.
//!
//! Read this file to find the rule for a specific kind. Read
//! [`super::transformations`] for the body of any `Rule::Custom`
//! handler. Read [`super::semantic`] for the output vocabulary.
//!
//! Exhaustive over `TsKind` — the compiler enforces coverage. The
//! enum unions both the typescript and tsx grammars (see gen-kinds);
//! some variants only appear in TSX (jsx_*).

use crate::languages::rule::Rule;

use super::input::TsKind;
use super::output::TsName::{self, *};
use super::transformations;

pub fn rule(k: TsKind) -> Rule<TsName> {
    use Rule::*;
    match k {
        // ---- ExtractOpThenRename ---------------------------------------
        TsKind::AssignmentExpression          => ExtractOpThenRename(Assign),
        TsKind::AugmentedAssignmentExpression => ExtractOpThenRename(Assign),
        TsKind::BinaryExpression              => ExtractOpThenRename(Binary),
        TsKind::UnaryExpression               => ExtractOpThenRename(Unary),
        TsKind::UpdateExpression              => ExtractOpThenRename(Unary),

        // ---- RenameWithMarker ------------------------------------------
        TsKind::AbstractClassDeclaration => RenameWithMarker(Class, Abstract),
        TsKind::ArrayPattern             => RenameWithMarker(Pattern, Array),
        TsKind::ArrayType                => RenameWithMarker(Type, Array),
        TsKind::ConditionalType          => RenameWithMarker(Type, Conditional),
        TsKind::DefaultType              => RenameWithMarker(Type, Default),
        TsKind::FunctionType             => RenameWithMarker(Type, Function),
        TsKind::IndexTypeQuery           => RenameWithMarker(Type, Keyof),
        TsKind::InferType                => RenameWithMarker(Type, Infer),
        TsKind::IntersectionType         => RenameWithMarker(Type, Intersection),
        TsKind::LiteralType              => RenameWithMarker(Type, Literal),
        TsKind::LookupType               => RenameWithMarker(Type, Lookup),
        TsKind::ObjectAssignmentPattern  => RenameWithMarker(Pattern, Default),
        TsKind::ObjectPattern            => RenameWithMarker(Pattern, Object),
        TsKind::ObjectType               => RenameWithMarker(Type, Object),
        TsKind::OptionalChain            => RenameWithMarker(Member, Optional),
        TsKind::ParenthesizedType        => RenameWithMarker(Type, Parenthesized),
        TsKind::ReadonlyType             => RenameWithMarker(Type, Readonly),
        TsKind::TemplateLiteralType      => RenameWithMarker(Type, Template),
        TsKind::TemplateType             => RenameWithMarker(Type, Template),
        TsKind::TupleType                => RenameWithMarker(Type, Tuple),
        TsKind::UnionType                => RenameWithMarker(Type, Union),

        // ---- Flatten with field distribution ---------------------------
        TsKind::Arguments     => Flatten { distribute_field: Some("arguments") },
        TsKind::TypeArguments => Flatten { distribute_field: Some("arguments") },

        // ---- Pure Flatten ----------------------------------------------
        TsKind::ClassBody
        | TsKind::ClassHeritage
        | TsKind::EnumBody
        | TsKind::ExportClause
        | TsKind::InterfaceBody
        | TsKind::MappedTypeClause
        | TsKind::ParenthesizedExpression
        | TsKind::StringFragment
        | TsKind::TypeAnnotation
        | TsKind::VariableDeclarator => Flatten { distribute_field: None },

        // ---- Custom (language-specific logic in transformations.rs) ---
        TsKind::AbstractMethodSignature  => Custom(transformations::abstract_method_signature),
        TsKind::AccessibilityModifier    => Custom(transformations::modifier),
        TsKind::ArrowFunction            => Custom(transformations::arrow_function),
        TsKind::Comment                  => Custom(transformations::comment),
        TsKind::ExpressionStatement      => Custom(transformations::skip),
        TsKind::ExtendsClause            => Custom(transformations::extends_clause),
        TsKind::FormalParameters         => Custom(transformations::formal_parameters),
        TsKind::FunctionDeclaration      => Custom(transformations::function_declaration),
        TsKind::FunctionExpression       => Custom(transformations::function_expression),
        TsKind::GeneratorFunction        => Custom(transformations::generator_function),
        TsKind::GeneratorFunctionDeclaration => Custom(transformations::generator_function_declaration),
        TsKind::GenericType              => Custom(transformations::generic_type),
        TsKind::Identifier               => Custom(transformations::identifier),
        TsKind::LexicalDeclaration       => Custom(transformations::variable_declaration),
        TsKind::MethodDefinition         => Custom(transformations::method_definition),
        TsKind::OptionalParameter        => Custom(transformations::optional_parameter),
        TsKind::OverrideModifier         => Custom(transformations::modifier),
        TsKind::PropertyIdentifier       => Custom(transformations::identifier),
        TsKind::PublicFieldDefinition    => Custom(transformations::public_field_definition),
        TsKind::RequiredParameter        => Custom(transformations::required_parameter),
        TsKind::TernaryExpression        => Custom(transformations::ternary_expression),
        TsKind::TypeAliasDeclaration     => Custom(transformations::type_alias_declaration),
        TsKind::TypeIdentifier           => Custom(transformations::type_identifier),
        TsKind::VariableDeclaration      => Custom(transformations::variable_declaration),

        // ---- Pure Rename -----------------------------------------------
        TsKind::AsExpression              => Rename(As),
        TsKind::AwaitExpression           => Rename(Await),
        TsKind::BreakStatement            => Rename(Break),
        TsKind::CallExpression            => Rename(Call),
        TsKind::CatchClause               => Rename(Catch),
        TsKind::ClassDeclaration          => Rename(Class),
        TsKind::ConstructSignature        => Rename(Constructor),
        TsKind::ContinueStatement         => Rename(Continue),
        TsKind::ElseClause                => Rename(Else),
        TsKind::EnumAssignment            => Rename(Constant),
        TsKind::EnumDeclaration           => Rename(Enum),
        TsKind::ExportSpecifier           => Rename(Spec),
        TsKind::ExportStatement           => Rename(Export),
        TsKind::False                     => Rename(Bool),
        TsKind::FinallyClause             => Rename(Finally),
        TsKind::ForInStatement            => Rename(For),
        TsKind::ForStatement              => Rename(For),
        TsKind::IfStatement               => Rename(If),
        TsKind::ImplementsClause          => Rename(Implements),
        TsKind::ImportClause              => Rename(Clause),
        TsKind::ImportSpecifier           => Rename(Spec),
        TsKind::ImportStatement           => Rename(Import),
        TsKind::IndexSignature            => Rename(Indexer),
        TsKind::InterfaceDeclaration      => Rename(Interface),
        TsKind::JsxAttribute              => Rename(Prop),
        TsKind::JsxClosingElement         => Rename(Closing),
        TsKind::JsxElement                => Rename(Element),
        TsKind::JsxExpression             => Rename(Value),
        TsKind::JsxOpeningElement         => Rename(Opening),
        TsKind::JsxSelfClosingElement     => Rename(Element),
        TsKind::JsxText                   => Rename(Text),
        TsKind::MemberExpression          => Rename(Member),
        TsKind::MethodSignature           => Rename(Method),
        TsKind::NamedImports              => Rename(Imports),
        TsKind::NamespaceImport           => Rename(Namespace),
        TsKind::NewExpression             => Rename(New),
        TsKind::NonNullExpression         => Rename(Unary),
        TsKind::Null                      => Rename(Null),
        TsKind::Number                    => Rename(Number),
        TsKind::OptingTypeAnnotation      => Rename(Annotation),
        // `predefined_type` renames to TYPE but must also wrap its text
        // in `<name>` (Principle #14: every named type reference carries
        // its name in a <name> child). Reuses the type_identifier handler.
        TsKind::PredefinedType            => Custom(transformations::type_identifier),
        TsKind::PrivatePropertyIdentifier => Rename(Name),
        TsKind::Program                   => Rename(Program),
        TsKind::PropertySignature         => Rename(Property),
        TsKind::RestPattern               => Rename(Rest),
        TsKind::ReturnStatement           => Rename(Return),
        TsKind::SatisfiesExpression       => Rename(Satisfies),
        TsKind::ShorthandPropertyIdentifier => Rename(Name),
        TsKind::ShorthandPropertyIdentifierPattern => Rename(Name),
        TsKind::SpreadElement             => Rename(Spread),
        TsKind::StatementBlock            => Rename(Block),
        TsKind::String                    => Rename(String),
        TsKind::SubscriptExpression       => Rename(Index),
        TsKind::SwitchBody                => Rename(Body),
        TsKind::SwitchCase                => Rename(Case),
        TsKind::SwitchDefault             => Rename(Default),
        TsKind::SwitchStatement           => Rename(Switch),
        TsKind::TemplateString            => Rename(Template),
        TsKind::TemplateSubstitution      => Rename(Interpolation),
        TsKind::ThrowStatement            => Rename(Throw),
        TsKind::True                      => Rename(Bool),
        TsKind::TryStatement              => Rename(Try),
        TsKind::TypeParameter             => Rename(Generic),
        TsKind::TypeParameters            => Rename(Generics),
        TsKind::TypePredicate             => Rename(Predicate),
        TsKind::TypePredicateAnnotation   => Rename(Predicate),
        TsKind::WhileStatement            => Rename(While),
        TsKind::YieldExpression           => Rename(Yield),

        // ---- Passthrough — kind name already matches the vocabulary,
        //      OR the kind is unhandled and survives as raw kind name.

        // Already matches our vocabulary.
        TsKind::Array
        | TsKind::Constraint
        | TsKind::Object
        | TsKind::Pair
        | TsKind::Super
        | TsKind::This
        | TsKind::Undefined => Custom(transformations::passthrough),

        // ---- Unhandled in the previous dispatcher — survive as raw
        //      kind names. Most are TODO candidates.

        // TODO: TypeScript 5+ type assertion / `asserts` predicate / asserts
        // annotation.
        //   asserts                  → marker on type_predicate?
        //   asserts_annotation       → annotation form
        //   adding_type_annotation   → typescript flow plus annotation
        //   omitting_type_annotation → typescript flow minus annotation
        //   type_assertion           → cast-like; Rename(CAST) marker?
        TsKind::AddingTypeAnnotation
        | TsKind::Asserts
        | TsKind::AssertsAnnotation
        | TsKind::OmittingTypeAnnotation
        | TsKind::TypeAssertion => Custom(transformations::passthrough),

        // TODO: ambient declaration (`declare …`); class_static_block;
        // computed_property_name (`[expr]: value`); decorator
        // (`@Component`); meta_property (`new.target` / `import.meta`).
        TsKind::AmbientDeclaration
        | TsKind::ClassStaticBlock
        | TsKind::ComputedPropertyName
        | TsKind::Decorator
        | TsKind::MetaProperty => Custom(transformations::passthrough),

        // TODO: more pattern shapes.
        //   assignment_pattern       → RenameWithMarker(PATTERN, ASSIGN)?
        //   pair_pattern             → key:value pattern
        TsKind::AssignmentPattern
        | TsKind::PairPattern => Custom(transformations::passthrough),

        // TODO: more type shapes.
        //   constructor_type      → RenameWithMarker(TYPE, CONSTRUCTOR)
        //   existential_type      → `*` type
        //   extends_type_clause   → for type aliases
        //   flow_maybe_type       → `?T` (Flow-only nullable)
        //   nested_type_identifier → scoped type ref
        //   optional_type         → `T?`
        //   rest_type             → `...T`
        //   this_type             → `this` in type position
        //   type_query            → `typeof X` in type
        TsKind::ConstructorType
        | TsKind::ExistentialType
        | TsKind::ExtendsTypeClause
        | TsKind::FlowMaybeType
        | TsKind::NestedTypeIdentifier
        | TsKind::OptionalType
        | TsKind::RestType
        | TsKind::ThisType
        | TsKind::TypeQuery => Custom(transformations::passthrough),

        // TODO: import / module variants.
        //   import / import_alias / import_attribute / import_require_clause
        //   internal_module / module / namespace_export
        //   nested_identifier (scoped value name)
        TsKind::Import
        | TsKind::ImportAlias
        | TsKind::ImportAttribute
        | TsKind::ImportRequireClause
        | TsKind::InternalModule
        | TsKind::Module
        | TsKind::NamespaceExport
        | TsKind::NestedIdentifier => Custom(transformations::passthrough),

        // TODO: instantiation expression (`Foo<T>`).
        TsKind::InstantiationExpression => Custom(transformations::passthrough),

        // TODO: special-statement / control-flow odds and ends.
        //   debugger_statement → Rename(DEBUGGER)?
        //   do_statement       → Rename(DO)
        //   empty_statement    → Flatten or Skip
        //   labeled_statement  → Rename(LABEL)
        //   sequence_expression → `a, b, c` comma operator
        //   with_statement     → Rename(WITH) (deprecated JS)
        TsKind::DebuggerStatement
        | TsKind::DoStatement
        | TsKind::EmptyStatement
        | TsKind::LabeledStatement
        | TsKind::SequenceExpression
        | TsKind::WithStatement => Custom(transformations::passthrough),

        // TODO: regex / hashbang / HTML kinds.
        //   regex / regex_flags / regex_pattern → literal regex
        //   hash_bang_line → `#!/usr/bin/env node`
        //   html_character_reference / html_comment → JSX text fragments
        //   jsx_namespace_name → JSX `xmlns:foo`
        TsKind::HashBangLine
        | TsKind::HtmlCharacterReference
        | TsKind::HtmlComment
        | TsKind::JsxNamespaceName
        | TsKind::Regex
        | TsKind::RegexFlags
        | TsKind::RegexPattern => Custom(transformations::passthrough),

        // TODO: function-related extras.
        //   call_signature   → similar to method_signature
        //   function_signature → bare signature in interface
        //   statement_identifier → labeled-statement target
        //   escape_sequence  → string body escape
        //   class            → class as expression
        TsKind::CallSignature
        | TsKind::Class
        | TsKind::EscapeSequence
        | TsKind::FunctionSignature
        | TsKind::StatementIdentifier => Custom(transformations::passthrough),

        // ---- Truly raw structural supertypes.
        TsKind::Declaration
        | TsKind::Expression
        | TsKind::Pattern
        | TsKind::PrimaryExpression
        | TsKind::PrimaryType
        | TsKind::Statement
        | TsKind::Type => Custom(transformations::passthrough),
    }
}
