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
use super::semantic::*;
use super::transformations;

pub fn rule(k: TsKind) -> Rule {
    use Rule::*;
    match k {
        // ---- ExtractOpThenRename ---------------------------------------
        TsKind::AssignmentExpression          => ExtractOpThenRename(ASSIGN),
        TsKind::AugmentedAssignmentExpression => ExtractOpThenRename(ASSIGN),
        TsKind::BinaryExpression              => ExtractOpThenRename(BINARY),
        TsKind::UnaryExpression               => ExtractOpThenRename(UNARY),
        TsKind::UpdateExpression              => ExtractOpThenRename(UNARY),

        // ---- RenameWithMarker ------------------------------------------
        TsKind::AbstractClassDeclaration => RenameWithMarker(CLASS, ABSTRACT),
        TsKind::ArrayPattern             => RenameWithMarker(PATTERN, ARRAY),
        TsKind::ArrayType                => RenameWithMarker(TYPE, ARRAY),
        TsKind::ConditionalType          => RenameWithMarker(TYPE, CONDITIONAL),
        TsKind::DefaultType              => RenameWithMarker(TYPE, DEFAULT),
        TsKind::FunctionType             => RenameWithMarker(TYPE, FUNCTION),
        TsKind::IndexTypeQuery           => RenameWithMarker(TYPE, KEYOF),
        TsKind::InferType                => RenameWithMarker(TYPE, INFER),
        TsKind::IntersectionType         => RenameWithMarker(TYPE, INTERSECTION),
        TsKind::LiteralType              => RenameWithMarker(TYPE, LITERAL),
        TsKind::LookupType               => RenameWithMarker(TYPE, LOOKUP),
        TsKind::ObjectAssignmentPattern  => RenameWithMarker(PATTERN, DEFAULT),
        TsKind::ObjectPattern            => RenameWithMarker(PATTERN, OBJECT),
        TsKind::ObjectType               => RenameWithMarker(TYPE, OBJECT),
        TsKind::OptionalChain            => RenameWithMarker(MEMBER, OPTIONAL),
        TsKind::ParenthesizedType        => RenameWithMarker(TYPE, PARENTHESIZED),
        TsKind::ReadonlyType             => RenameWithMarker(TYPE, READONLY),
        TsKind::TemplateLiteralType      => RenameWithMarker(TYPE, TEMPLATE),
        TsKind::TemplateType             => RenameWithMarker(TYPE, TEMPLATE),
        TsKind::TupleType                => RenameWithMarker(TYPE, TUPLE),
        TsKind::UnionType                => RenameWithMarker(TYPE, UNION),

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
        TsKind::AsExpression              => Rename(AS),
        TsKind::AwaitExpression           => Rename(AWAIT),
        TsKind::BreakStatement            => Rename(BREAK),
        TsKind::CallExpression            => Rename(CALL),
        TsKind::CatchClause               => Rename(CATCH),
        TsKind::ClassDeclaration          => Rename(CLASS),
        TsKind::ConstructSignature        => Rename(CONSTRUCTOR),
        TsKind::ContinueStatement         => Rename(CONTINUE),
        TsKind::ElseClause                => Rename(ELSE),
        TsKind::EnumAssignment            => Rename(CONSTANT),
        TsKind::EnumDeclaration           => Rename(ENUM),
        TsKind::ExportSpecifier           => Rename(SPEC),
        TsKind::ExportStatement           => Rename(EXPORT),
        TsKind::False                     => Rename(BOOL),
        TsKind::FinallyClause             => Rename(FINALLY),
        TsKind::ForInStatement            => Rename(FOR),
        TsKind::ForStatement              => Rename(FOR),
        TsKind::IfStatement               => Rename(IF),
        TsKind::ImplementsClause          => Rename(IMPLEMENTS),
        TsKind::ImportClause              => Rename(CLAUSE),
        TsKind::ImportSpecifier           => Rename(SPEC),
        TsKind::ImportStatement           => Rename(IMPORT),
        TsKind::IndexSignature            => Rename(INDEXER),
        TsKind::InterfaceDeclaration      => Rename(INTERFACE),
        TsKind::JsxAttribute              => Rename(PROP),
        TsKind::JsxClosingElement         => Rename(CLOSING),
        TsKind::JsxElement                => Rename(ELEMENT),
        TsKind::JsxExpression             => Rename(VALUE),
        TsKind::JsxOpeningElement         => Rename(OPENING),
        TsKind::JsxSelfClosingElement     => Rename(ELEMENT),
        TsKind::JsxText                   => Rename(TEXT),
        TsKind::MemberExpression          => Rename(MEMBER),
        TsKind::MethodSignature           => Rename(METHOD),
        TsKind::NamedImports              => Rename(IMPORTS),
        TsKind::NamespaceImport           => Rename(NAMESPACE),
        TsKind::NewExpression             => Rename(NEW),
        TsKind::NonNullExpression         => Rename(UNARY),
        TsKind::Null                      => Rename(NULL),
        TsKind::Number                    => Rename(NUMBER),
        TsKind::OptingTypeAnnotation      => Rename(ANNOTATION),
        TsKind::PredefinedType            => Rename(TYPE),
        TsKind::PrivatePropertyIdentifier => Rename(NAME),
        TsKind::Program                   => Rename(PROGRAM),
        TsKind::PropertySignature         => Rename(PROPERTY),
        TsKind::RestPattern               => Rename(REST),
        TsKind::ReturnStatement           => Rename(RETURN),
        TsKind::SatisfiesExpression       => Rename(SATISFIES),
        TsKind::ShorthandPropertyIdentifier => Rename(NAME),
        TsKind::ShorthandPropertyIdentifierPattern => Rename(NAME),
        TsKind::SpreadElement             => Rename(SPREAD),
        TsKind::StatementBlock            => Rename(BLOCK),
        TsKind::String                    => Rename(STRING),
        TsKind::SubscriptExpression       => Rename(INDEX),
        TsKind::SwitchBody                => Rename(BODY),
        TsKind::SwitchCase                => Rename(CASE),
        TsKind::SwitchDefault             => Rename(DEFAULT),
        TsKind::SwitchStatement           => Rename(SWITCH),
        TsKind::TemplateString            => Rename(TEMPLATE),
        TsKind::TemplateSubstitution      => Rename(INTERPOLATION),
        TsKind::ThrowStatement            => Rename(THROW),
        TsKind::True                      => Rename(BOOL),
        TsKind::TryStatement              => Rename(TRY),
        TsKind::TypeParameter             => Rename(GENERIC),
        TsKind::TypeParameters            => Rename(GENERICS),
        TsKind::TypePredicate             => Rename(PREDICATE),
        TsKind::TypePredicateAnnotation   => Rename(PREDICATE),
        TsKind::WhileStatement            => Rename(WHILE),
        TsKind::YieldExpression           => Rename(YIELD),

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
