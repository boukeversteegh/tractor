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
use super::output::TractorNode::{self, *};
use super::transformations;

pub fn rule(k: TsKind) -> Rule<TractorNode> {
    use Rule::*;
    match k {
        // ---- ExtractOpThenRename ---------------------------------------
        TsKind::AssignmentExpression          => ExtractOpThenRename(Assign),
        TsKind::AugmentedAssignmentExpression => ExtractOpThenRename(Assign),
        TsKind::BinaryExpression              => ExtractOpThenRename(Binary),
        TsKind::UnaryExpression               => ExtractOpThenRename(Unary),
        TsKind::UpdateExpression              => Custom(transformations::update_expression),

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

        // ---- Iter 18: TypeScript type-shape variants ------------------
        TsKind::ConstructorType          => RenameWithMarker(Type, Constructor),
        TsKind::ExistentialType          => RenameWithMarker(Type, Existential),
        TsKind::FlowMaybeType            => RenameWithMarker(Type, Optional),
        TsKind::OptionalType             => RenameWithMarker(Type, Optional),
        TsKind::RestType                 => RenameWithMarker(Type, Rest),
        TsKind::ThisType                 => RenameWithMarker(Type, This),
        TsKind::TypeQuery                => RenameWithMarker(Type, Typeof),
        // `Foo<T>` instantiation expression — a generically-applied
        // type/value reference. Same shape as `<type[generic]>`.
        TsKind::InstantiationExpression  => RenameWithMarker(Type, Generic),
        // `assignment_pattern` — destructure with default `[a = 1]`.
        // Sibling of `object_assignment_pattern`.
        TsKind::AssignmentPattern        => RenameWithMarker(Pattern, Default),
        // Class-level static initializer block `static { ... }`.
        TsKind::ClassStaticBlock         => RenameWithMarker(Block, Static),
        // `import x = require(y)` legacy CommonJS import alias form.
        TsKind::ImportAlias              => RenameWithMarker(Import, Alias),

        // ---- Flatten with field distribution ---------------------------
        TsKind::Arguments     => Flatten { distribute_field: Some("arguments") },
        TsKind::TypeArguments => Flatten { distribute_field: Some("arguments") },

        // ---- Pure Flatten ----------------------------------------------
        TsKind::AddingTypeAnnotation
        | TsKind::ClassBody
        | TsKind::ClassHeritage
        | TsKind::EmptyStatement
        | TsKind::EnumBody
        | TsKind::EscapeSequence
        | TsKind::ExportClause
        | TsKind::HtmlCharacterReference
        | TsKind::ImportRequireClause
        | TsKind::InterfaceBody
        | TsKind::MappedTypeClause
        | TsKind::OmittingTypeAnnotation
        | TsKind::ParenthesizedExpression
        | TsKind::PrimaryExpression
        | TsKind::PrimaryType
        | TsKind::RegexPattern
        | TsKind::SequenceExpression
        | TsKind::StringFragment
        | TsKind::TypeAnnotation
        | TsKind::VariableDeclarator => Flatten { distribute_field: None },

        // ---- Custom (language-specific logic in transformations.rs) ---
        TsKind::AbstractMethodSignature  => Custom(transformations::abstract_method_signature),
        TsKind::AccessibilityModifier    => Custom(transformations::modifier),
        TsKind::ArrowFunction            => Custom(transformations::arrow_function),
        TsKind::Comment                  => Custom(transformations::comment),
        TsKind::AwaitExpression          => Custom(transformations::await_expression),
        TsKind::ExpressionStatement      => Custom(transformations::expression_statement),
        TsKind::NonNullExpression        => Custom(transformations::non_null_expression),
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
        TsKind::BreakStatement            => RenameStripKeyword(Break, "break"),
        TsKind::CallExpression            => Rename(Call),
        TsKind::CatchClause               => Rename(Catch),
        TsKind::ClassDeclaration          => Rename(Class),
        TsKind::ConstructSignature        => Rename(Constructor),
        TsKind::ContinueStatement         => RenameStripKeyword(Continue, "continue"),
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
        // `implements A, B, C` — Principle #12 + #18: multiple
        // `<implements>` siblings, not a list container.
        TsKind::ImplementsClause          => Custom(transformations::implements_clause),
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
        TsKind::Null                      => Rename(Null),
        TsKind::Number                    => Rename(Number),
        TsKind::OptingTypeAnnotation      => Rename(Annotation),
        // `predefined_type` renames to TYPE but must also wrap its text
        // in `<name>` (Principle #14: every named type reference carries
        // its name in a <name> child). Reuses the type_identifier handler.
        TsKind::PredefinedType            => Custom(transformations::type_identifier),
        TsKind::PrivatePropertyIdentifier => Rename(Name),
        TsKind::Program                   => Rename(Program),
        TsKind::PropertySignature         => Custom(transformations::property_signature),
        TsKind::RestPattern               => Rename(Rest),
        TsKind::ReturnStatement           => RenameStripKeyword(Return, "return"),
        TsKind::SatisfiesExpression       => Rename(Satisfies),
        // `{ x }` shorthand object property — semantically equivalent
        // to `{ x: x }`. Wrap in `<pair>` with `<name>` for the key
        // so the shape matches structured pairs (Principle #5
        // within-language unification).
        TsKind::ShorthandPropertyIdentifier => Custom(transformations::shorthand_property_identifier),
        TsKind::ShorthandPropertyIdentifierPattern => Rename(Name),
        TsKind::SpreadElement             => Rename(Spread),
        TsKind::StatementBlock            => Rename(Block),
        TsKind::String                    => Rename(String),
        TsKind::SubscriptExpression       => Rename(Index),
        // `switch_statement.body` field already wraps this in <body>;
        // flatten avoids double-nested <body><body>...</body></body>.
        TsKind::SwitchBody                => Flatten { distribute_field: None },
        TsKind::SwitchCase                => Rename(Case),
        TsKind::SwitchDefault             => Rename(Default),
        TsKind::SwitchStatement           => Rename(Switch),
        TsKind::TemplateString            => Rename(Template),
        TsKind::TemplateSubstitution      => Rename(Interpolation),
        TsKind::ThrowStatement            => Rename(Throw),
        TsKind::True                      => Rename(Bool),
        TsKind::TryStatement              => Rename(Try),
        // `<T = number>` / `<T extends Shape = Shape>` — type parameter
        // with optional default. Custom handler unwraps the
        // `<value>` field-wrapper around the default so the post-pass
        // `wrap_expression_positions` doesn't add an `<expression>`
        // host (a value-namespace host) around a type slot. See iter
        // 131 for the Principle #14 motivation.
        TsKind::TypeParameter             => Custom(transformations::type_parameter),
        // `<T, U>` generic parameter list. Per Principle #12 (no list
        // containers): flatten the wrapper so each `<generic>` becomes
        // a direct sibling of the enclosing declaration with
        // `field="generics" list="true"` for JSON-array recovery.
        // Matches Java / Rust shape.
        TsKind::TypeParameters            => Flatten { distribute_field: Some("generics") },
        TsKind::TypePredicate             => Rename(Predicate),
        // `: v is Shape` — the annotation wrapper only adds a `:` text;
        // flatten so the inner type_predicate becomes the direct
        // `<predicate>` child of the function (avoids
        // `<predicate>/<predicate>` double-wrap).
        TsKind::TypePredicateAnnotation   => Flatten { distribute_field: None },
        TsKind::WhileStatement            => Rename(While),
        TsKind::YieldExpression           => Rename(Yield),

        // ---- Iter 18: pure renames for previously-passthrough kinds ---
        // `asserts X is T` — promote the inner type_predicate's children
        // and surface as `<predicate[asserts]>`.
        TsKind::AssertsAnnotation         => Custom(transformations::asserts_annotation),
        // `<T>expr` — old TS cast syntax. Same shape as `expr as T` (As).
        TsKind::TypeAssertion             => Rename(As),
        // `declare const x: T;` ambient declaration.
        TsKind::AmbientDeclaration        => Rename(Declare),
        // `[expr]: value` — computed property name.
        TsKind::ComputedPropertyName      => Rename(Name),
        // `new.target` / `import.meta` — both are member-access shapes.
        TsKind::MetaProperty              => Rename(Member),
        // `Foo.Bar` in type position — same shape as `<member>`.
        TsKind::NestedTypeIdentifier      => Rename(Member),
        // `Foo.Bar` value reference (scoped value identifier).
        TsKind::NestedIdentifier          => Rename(Member),
        // `extends Type` clause inside a type alias / mapped type.
        // `interface I extends A, B, C` — multiple targets per
        // Principle #18 + #12: `<extends>` siblings, not list wrapper.
        TsKind::ExtendsTypeClause         => Custom(transformations::extends_type_clause),
        // `import { x } with { type: 'json' }` import attributes.
        TsKind::ImportAttribute           => Rename(Attribute),
        // `key: pattern` — pair pattern in object destructure.
        TsKind::PairPattern               => Rename(Pair),
        // Bare interface signatures.
        TsKind::CallSignature             => Rename(Signature),
        TsKind::FunctionSignature         => Rename(Signature),
        // Legacy / scoped-namespace variants.
        TsKind::Module                    => Rename(Namespace),
        TsKind::InternalModule            => Rename(Namespace),
        TsKind::NamespaceExport           => Rename(Export),
        // Special-statement / control-flow shapes.
        TsKind::DebuggerStatement         => Rename(Debugger),
        TsKind::DoStatement               => Rename(Do),
        TsKind::WithStatement             => Rename(With),
        TsKind::LabeledStatement          => Rename(Label),
        TsKind::StatementIdentifier       => Rename(Name),
        // JSX / HTML.
        TsKind::JsxNamespaceName          => Rename(Name),
        TsKind::HtmlComment               => Rename(Comment),
        // Regex literal.
        TsKind::Regex                     => Rename(Regex),
        TsKind::RegexFlags                => Rename(Flags),
        // `#!/usr/bin/env node` shebang line.
        TsKind::HashBangLine              => Rename(Hashbang),

        // ---- Passthrough — kind name already matches the vocabulary,
        //      OR the kind is unhandled and survives as raw kind name.

        // `<T extends Shape>` — generic constraint. Per Principle #18,
        // name the relationship after the source keyword. TS source
        // uses `extends`, so the element becomes `<extends>` matching
        // Java/Rust shape, with `field="extends" list="true"` for
        // JSON-array consistency.
        TsKind::Constraint                => Custom(transformations::constraint),

        // Already matches our vocabulary.
        TsKind::Array
        | TsKind::Object
        | TsKind::Pair
        | TsKind::Super
        | TsKind::This
        | TsKind::Undefined => Passthrough,

        // `asserts` keyword token of `asserts x is T`. Run through the
        // modifier helper to convert "asserts" text into an empty
        // `<asserts/>` marker (Principle #7); the source keyword
        // survives as a dangling sibling so the parent `<predicate>`'s
        // string-value still includes it.
        TsKind::Asserts => Custom(transformations::modifier),

        // ---- Single-word passthroughs.
        // `class` (class-as-expression — the `class` keyword in a value
        // position; the `class_declaration` form already renames). The
        // rest are tree-sitter structural supertypes that almost never
        // surface in the parsed output.
        // `import` / `decorator` keep their kind name (single word).
        TsKind::Class
        | TsKind::Decorator
        | TsKind::Import
        | TsKind::Declaration
        | TsKind::Expression
        | TsKind::Pattern
        | TsKind::Statement
        | TsKind::Type => Passthrough,
    }
}
