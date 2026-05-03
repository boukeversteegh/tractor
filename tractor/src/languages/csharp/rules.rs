//! Per-kind transformation rules for C#: the `CsKind` → `Rule<TractorNode>`
//! table.
//!
//! Read this file to find the rule for a specific kind. Read
//! [`super::transformations`] for the body of any `Rule::Custom`
//! handler the rule references by name. Read [`super::output`] for
//! the output vocabulary (`TractorNode` enum + per-name metadata).
//!
//! Exhaustive over `CsKind` — the compiler enforces coverage. When
//! the grammar ships a new kind, regenerating `input.rs` adds a
//! variant and this match fails to build until the new kind is
//! classified.
//!
//! Pure data variants (`Rename`, `RenameWithMarker`, `Flatten`,
//! `ExtractOpThenRename`) are executed by the shared
//! [`crate::languages::rule::dispatch`] helper. Custom logic lives in
//! [`super::transformations`].

use crate::languages::rule::Rule;

use super::input::CsKind;
use super::output::TractorNode::{self, *};
use super::transformations;

/// Shorthand for the `default-access-then-rename` shape used by all 9
/// C# declaration kinds. Bakes in C#'s default-access resolver so the
/// rule arms read as data.
fn da(to: TractorNode) -> Rule<TractorNode> {
    Rule::DefaultAccessThenRename {
        to,
        default_access: transformations::default_access_for_declaration,
    }
}

pub fn rule(k: CsKind) -> Rule<TractorNode> {
    use Rule::*;
    match k {
        // ---- ExtractOpThenRename ---------------------------------------
        CsKind::BinaryExpression     => ExtractOpThenRename(Binary),
        CsKind::UnaryExpression      => ExtractOpThenRename(Unary),
        CsKind::AssignmentExpression => ExtractOpThenRename(Assign),

        // ---- RenameWithMarker ------------------------------------------
        CsKind::ArrayType                   => RenameWithMarker(Type, Array),
        CsKind::ConditionalAccessExpression => Custom(transformations::conditional_access_expression),
        CsKind::ConstantPattern             => RenameWithMarker(Pattern, Constant),
        CsKind::DeclarationPattern          => RenameWithMarker(Pattern, Declaration),
        CsKind::FunctionPointerType         => RenameWithMarker(Type, Function),
        // `member_access_expression` — wrap receiver + property in
        // role-named `<object>`/`<property>` so the two `<name>`s no
        // longer collide on JSON `name` key (Principle #19; mirrors
        // iter 147 Java/Python/Go member-access role-wrap).
        CsKind::MemberAccessExpression      => Custom(transformations::member_access_expression),
        CsKind::MemberBindingExpression     => RenameWithMarker(Member, Optional),
        CsKind::PointerType                 => RenameWithMarker(Type, Pointer),
        CsKind::PrefixUnaryExpression       => Custom(transformations::prefix_unary_expression),
        CsKind::RecursivePattern            => RenameWithMarker(Pattern, Recursive),
        CsKind::RefType                     => RenameWithMarker(Type, Ref),
        CsKind::RelationalPattern           => RenameWithMarker(Pattern, Relational),
        CsKind::TuplePattern                => RenameWithMarker(Pattern, Tuple),
        CsKind::TupleType                   => RenameWithMarker(Type, Tuple),

        // ---- Flatten with field distribution ---------------------------
        CsKind::AccessorList          => Flatten { distribute_list: Some("accessors") },
        CsKind::ArgumentList          => Flatten { distribute_list: Some("arguments") },
        CsKind::AttributeArgumentList => Flatten { distribute_list: Some("arguments") },
        CsKind::AttributeList         => Flatten { distribute_list: Some("attributes") },
        CsKind::BracketedParameterList => Flatten { distribute_list: Some("parameters") },
        CsKind::ParameterList         => Flatten { distribute_list: Some("parameters") },
        CsKind::TypeArgumentList      => Flatten { distribute_list: Some("arguments") },
        CsKind::TypeParameterList     => Flatten { distribute_list: Some("generics") },

        // ---- Pure Flatten ----------------------------------------------
        CsKind::ArrowExpressionClause
        | CsKind::DeclarationList
        | CsKind::EnumMemberDeclarationList
        | CsKind::EscapeSequence
        | CsKind::InterpolationBrace
        | CsKind::InterpolationStart
        | CsKind::LocalDeclarationStatement
        | CsKind::ParenthesizedExpression
        | CsKind::RawStringContent
        | CsKind::RawStringEnd
        | CsKind::RawStringStart
        | CsKind::StringContent
        | CsKind::StringLiteralContent => Flatten { distribute_list: None },

        // ---- DefaultAccessThenRename — declarations with implicit
        //      access modifier (see `transformations::default_access_for_declaration`).
        CsKind::ClassDeclaration       => da(Class),
        CsKind::ConstructorDeclaration => da(Constructor),
        CsKind::EnumDeclaration        => da(Enum),
        CsKind::FieldDeclaration       => da(Field),
        CsKind::InterfaceDeclaration   => da(Interface),
        CsKind::MethodDeclaration      => da(Method),
        CsKind::PropertyDeclaration    => da(Property),
        CsKind::RecordDeclaration      => da(Record),
        CsKind::StructDeclaration      => da(Struct),

        // ---- Custom (language-specific logic in transformations.rs) ---
        CsKind::AccessorDeclaration           => Custom(transformations::accessor_declaration),
        CsKind::Comment                       => Custom(transformations::comment),
        CsKind::ConditionalExpression         => Custom(transformations::conditional_expression),
        CsKind::GenericName                   => Custom(transformations::generic_name),
        CsKind::Identifier                    => Custom(transformations::identifier),
        CsKind::IfStatement                   => Custom(transformations::if_statement),
        CsKind::ImplicitType                  => Custom(transformations::implicit_type),
        CsKind::InterpolatedStringExpression  => Custom(transformations::interpolated_string_expression),
        CsKind::Modifier                      => Custom(transformations::modifier),
        CsKind::NullableType                  => Custom(transformations::nullable_type),
        CsKind::PostfixUnaryExpression        => Custom(transformations::postfix_unary_expression),
        CsKind::PredefinedType                => Custom(transformations::predefined_type),
        CsKind::VariableDeclaration           => Custom(transformations::variable_declaration),

        // `where T : new()` / constraint-clause kinds — consumed by the
        // shared `attach_where_clause_constraints` post-transform (which
        // reads the original `kind` attribute, not the element name).
        // Rename to valid output names so no underscore leaks if they
        // survive to output.
        CsKind::ConstructorConstraint            => Rename(New),
        CsKind::TypeParameterConstraint          => Rename(Constraint),
        CsKind::TypeParameterConstraintsClause   => Rename(Where),

        // ---- Pure Rename -----------------------------------------------
        CsKind::Argument                       => Rename(Argument),
        CsKind::Attribute                      => Rename(Attribute),
        CsKind::AttributeArgument              => Rename(Argument),
        CsKind::AwaitExpression                => Custom(transformations::await_expression),
        // `class Foo : Base, IFace` — C# uses `:` (no `extends`/
        // `implements` keyword); the colon-list contains the base
        // class plus interfaces. Idiomatically called the "base
        // list" / "base types" in MS docs. Produce multiple `<base>`
        // siblings (Principle #12 — no list container, Goal #5 —
        // dev mental model).
        CsKind::BaseList                       => Custom(transformations::base_list),
        CsKind::Block                          => Rename(Block),
        CsKind::BooleanLiteral                 => Rename(Bool),
        CsKind::BreakStatement                 => RenameStripKeyword(Break, "break"),
        CsKind::CatchClause                    => Rename(Catch),
        CsKind::CatchDeclaration               => Rename(Declaration),
        CsKind::CatchFilterClause              => Rename(Filter),
        CsKind::CompilationUnit                => Rename(Unit),
        // `: this(...)` / `: base(...)` constructor invocation. Renames
        // to `<call>` with `[this]` / `[base]` marker — matches Java's
        // `<call[super]>` shape for the parallel construct so
        // cross-language `//call[base]` and `//call[this]` work.
        CsKind::ConstructorInitializer         => Custom(transformations::constructor_initializer),
        CsKind::ContinueStatement              => RenameStripKeyword(Continue, "continue"),
        CsKind::DelegateDeclaration            => Rename(Delegate),
        CsKind::DestructorDeclaration          => Rename(Destructor),
        CsKind::DoStatement                    => Rename(Do),
        CsKind::ElementBindingExpression       => Rename(Index),
        CsKind::EnumMemberDeclaration          => Rename(Constant),
        CsKind::EventFieldDeclaration          => Rename(Event),
        CsKind::ExpressionStatement            => Rename(Expression),
        // File-scoped namespace `namespace Foo;` — same shape as
        // block-scoped via post_transform's `unify_file_scoped_namespace`,
        // distinguished by a `<file/>` marker. Closes todo/34.
        CsKind::FileScopedNamespaceDeclaration =>
            Custom(transformations::file_scoped_namespace),
        CsKind::FinallyClause                  => Rename(Finally),
        CsKind::ForStatement                   => Rename(For),
        CsKind::ForeachStatement               => Rename(Foreach),
        // C# LINQ `from n in numbers` — wrap the source (the
        // identifier after `in`) in `<value>` slot so it doesn't
        // collide with the binding `<name>` on the JSON `name` key.
        // Iter 287.
        CsKind::FromClause                     => Custom(transformations::from_clause),
        CsKind::GroupClause                    => Rename(Group),
        CsKind::ImplicitObjectCreationExpression => Rename(New),
        CsKind::ImplicitParameter              => Rename(Parameter),
        CsKind::IndexerDeclaration             => Rename(Indexer),
        CsKind::InitializerExpression          => Rename(Literal),
        CsKind::IntegerLiteral                 => Rename(Int),
        CsKind::InvocationExpression           => Rename(Call),
        CsKind::IsPatternExpression            => Rename(Is),
        CsKind::JoinClause                     => Rename(Join),
        // `lambda_expression` — re-tag `<body>` as `<value>` for
        // single-expression bodies so `wrap_expression_positions`
        // wraps the body in `<expression>` host (Principle #15).
        // Block bodies keep `<body>`. Mirrors iter 161/162 (Rust
        // closure / TS arrow / Python lambda).
        CsKind::LambdaExpression               => Custom(transformations::lambda),
        CsKind::LetClause                      => Rename(Let),
        CsKind::LocalFunctionStatement         => Rename(Method),
        CsKind::NamespaceDeclaration           => Rename(Namespace),
        CsKind::NullLiteral                    => Rename(Null),
        CsKind::ObjectCreationExpression       => Rename(New),
        CsKind::OperatorDeclaration            => Rename(Operator),
        CsKind::OrderByClause                  => Rename(Order),
        CsKind::Parameter                      => Rename(Parameter),
        CsKind::PropertyPatternClause          => Rename(Properties),
        CsKind::QueryExpression                => Rename(Query),
        CsKind::RangeExpression                => Rename(Range),
        CsKind::RawStringLiteral               => Rename(String),
        CsKind::RealLiteral                    => Rename(Float),
        CsKind::ReturnStatement                => RenameStripKeyword(Return, "return"),
        CsKind::SelectClause                   => Rename(Select),
        CsKind::StringLiteral                  => Rename(String),
        // `switch_statement.body` field already wraps this in <body>;
        // flatten avoids double-nested <body><body>...</body></body>.
        CsKind::SwitchBody                     => Flatten { distribute_list: None },
        CsKind::SwitchExpression               => Rename(Switch),
        CsKind::SwitchExpressionArm            => Rename(Arm),
        CsKind::SwitchSection                  => Rename(Section),
        CsKind::SwitchStatement                => Rename(Switch),
        CsKind::ThrowStatement                 => RenameStripKeyword(Throw, "throw"),
        CsKind::TryStatement                   => Rename(Try),
        CsKind::QualifiedName                  => Rename(Path),
        CsKind::TupleElement                   => Rename(Element),
        CsKind::TupleExpression                => Rename(Tuple),
        CsKind::TypeParameter                  => Rename(Generic),
        CsKind::UsingDirective                 => Rename(Import),
        CsKind::UsingStatement                 => Rename(Using),
        CsKind::VariableDeclarator             => Rename(Declarator),
        CsKind::VerbatimStringLiteral          => Rename(String),
        CsKind::WhenClause                     => Rename(When),
        CsKind::WhereClause                    => Rename(Where),
        CsKind::WhileStatement                 => Rename(While),

        // ---- Passthrough — kind name already matches the vocabulary,
        //      OR the kind is unhandled and the dispatcher leaves it as
        //      its raw grammar name (the previous behavior of the
        //      catch-all `_` arm when `apply_rename` returned `None`).

        // Already matches our vocabulary (no underscore in kind name).
        CsKind::Discard
        | CsKind::Interpolation
        | CsKind::Subpattern => Passthrough,

        // ---- Unhandled in the previous dispatcher — survive as raw
        //      kind names. Pending real-semantics candidates tracked
        //      in todo/36-rule-todo-followups.md.

        // Pattern combinators — mirror the existing RenameWithMarker pattern:
        //   constant_pattern    → RenameWithMarker(Pattern, Constant)
        //   declaration_pattern → RenameWithMarker(Pattern, Declaration)
        //   recursive_pattern   → RenameWithMarker(Pattern, Recursive)
        CsKind::AndPattern              => RenameWithMarker(Pattern, And),
        CsKind::OrPattern               => RenameWithMarker(Pattern, Or),
        CsKind::NegatedPattern          => RenameWithMarker(Pattern, Negated),
        CsKind::ListPattern             => RenameWithMarker(Pattern, List),
        CsKind::VarPattern              => RenameWithMarker(Pattern, Var),
        // C# type pattern (`case Integer i:`). Drop the `[type]`
        // marker for the same reason as Java (iter 275): the
        // structural `<type>` child already signals it; the
        // redundant marker collides with the wrapper on the JSON
        // `type` key.
        CsKind::TypePattern             => Rename(Pattern),
        CsKind::ParenthesizedPattern    => Flatten { distribute_list: None },

        // `as_expression` (`x as Foo`) and `is_expression` (`obj is
        // Foo`) join `is_pattern_expression` under `<is>` — they're
        // all type-test / type-conversion siblings the developer
        // groups mentally as "is/as".
        CsKind::AsExpression
        | CsKind::IsExpression => Rename(Is),

        // Cast/default/throw expressions: each gets its own semantic.
        // `throw_expression` shares `<throw>` with `throw_statement`.
        CsKind::CastExpression      => Rename(Cast),
        CsKind::DefaultExpression   => Rename(Default),
        CsKind::ThrowExpression     => Rename(Throw),

        // `element_access_expression` (`x[i]`) is the call-site
        // counterpart of `indexer_declaration` → Indexer. Joins
        // `element_binding_expression` under `<index>`.
        CsKind::ElementAccessExpression => Rename(Index),

        // `anonymous_method_expression` is the older `delegate { … }`
        // syntax — functionally a lambda; always block-bodied so the
        // `lambda` handler's `is_block` check naturally keeps `<body>`.
        // `anonymous_object_creation_expression` (`new { X = 1 }`)
        // joins `<new>` with an anonymous marker.
        CsKind::AnonymousMethodExpression           => Custom(transformations::lambda),
        CsKind::AnonymousObjectCreationExpression   => RenameWithMarker(New, Anonymous),

        // Array and stackalloc creations join `<new>` with shape markers.
        CsKind::ArrayCreationExpression
        | CsKind::ImplicitArrayCreationExpression   => RenameWithMarker(New, Array),
        CsKind::ImplicitStackallocExpression
        | CsKind::StackallocExpression              => RenameWithMarker(New, Stackalloc),

        // Special-statement forms.
        CsKind::CheckedStatement    => Rename(Checked),
        CsKind::EmptyStatement      => Flatten { distribute_list: None },
        CsKind::FixedStatement      => Rename(Fixed),
        CsKind::GotoStatement       => Rename(Goto),
        CsKind::LabeledStatement    => Rename(Label),
        CsKind::LockStatement       => Rename(Lock),
        CsKind::UnsafeStatement     => RenameWithMarker(Block, Unsafe),
        CsKind::YieldStatement      => Rename(Yield),

        // Record update: `with` expression + its initializer body.
        CsKind::WithExpression  => Rename(With),
        CsKind::WithInitializer => Rename(Literal),

        // `event_declaration` is the property-shaped event form
        // (with accessors); pairs with `event_field_declaration`
        // which also renames to Event. `conversion_operator_declaration`
        // joins `operator_declaration` under `<operator>`.
        CsKind::EventDeclaration               => Rename(Event),
        CsKind::ConversionOperatorDeclaration  => Rename(Operator),

        // Character literals.
        CsKind::CharacterLiteral        => Rename(Char),
        CsKind::CharacterLiteralContent => Flatten { distribute_list: None },

        // Checked expression (mirrors checked_statement).
        CsKind::CheckedExpression       => Rename(Checked),

        // typeof / sizeof expressions.
        CsKind::TypeofExpression        => Rename(Typeof),
        CsKind::SizeofExpression        => Rename(Sizeof),

        // `ref x` expression wraps its operand.
        CsKind::RefExpression           => RenameWithMarker(Expression, Ref),

        // LINQ join-into clause.
        CsKind::JoinIntoClause          => Rename(Into),

        // `extern alias Foo;` is import-like.
        CsKind::ExternAliasDirective    => Rename(Import),

        // Function pointer parameter is a parameter.
        CsKind::FunctionPointerParameter => Rename(Parameter),

        // `[assembly: Attr]` — the global attribute container.
        CsKind::GlobalAttribute         => Rename(Attribute),

        // `Bar(x)` in `class Foo(int x) : Bar(x)` — primary ctor base
        // type. Flatten so the inner identifier + argument_list lift
        // into base_list as flat siblings; the base_list Custom
        // handler groups the args into the preceding type's
        // `<extends>` (iter 130). Renaming this to `<extends>`
        // directly produced `<extends>/<extends>` nesting because
        // base_list also wraps it in `<extends>`.
        CsKind::PrimaryConstructorBaseType => Flatten { distribute_list: None },

        // ---- Structural supertypes / wrappers (flatten, promote children) ---
        CsKind::AliasQualifiedName      => Flatten { distribute_list: None },
        CsKind::ArrayRankSpecifier      => Flatten { distribute_list: None },
        CsKind::AttributeTargetSpecifier => Flatten { distribute_list: None },
        CsKind::BracketedArgumentList   => Flatten { distribute_list: Some("arguments") },
        CsKind::CallingConvention       => Flatten { distribute_list: None },
        CsKind::Declaration             => Flatten { distribute_list: None },
        CsKind::DeclarationExpression   => Flatten { distribute_list: None },
        CsKind::ExplicitInterfaceSpecifier => Flatten { distribute_list: None },
        CsKind::Expression              => Flatten { distribute_list: None },
        CsKind::GlobalStatement         => Flatten { distribute_list: None },
        CsKind::InterpolationAlignmentClause => Flatten { distribute_list: None },
        CsKind::InterpolationFormatClause => Flatten { distribute_list: None },
        CsKind::InterpolationQuote      => Flatten { distribute_list: None },
        CsKind::Literal                 => Flatten { distribute_list: None },
        CsKind::LvalueExpression        => Flatten { distribute_list: None },
        CsKind::MakerefExpression       => Flatten { distribute_list: None },
        CsKind::NonLvalueExpression     => Flatten { distribute_list: None },
        CsKind::ParenthesizedVariableDesignation => Flatten { distribute_list: None },
        CsKind::Pattern                 => Flatten { distribute_list: None },
        CsKind::PositionalPatternClause => Flatten { distribute_list: None },
        CsKind::ReftypeExpression       => Flatten { distribute_list: None },
        CsKind::RefvalueExpression      => Flatten { distribute_list: None },
        CsKind::ScopedType              => Flatten { distribute_list: None },
        CsKind::Statement               => Flatten { distribute_list: None },
        CsKind::StringLiteralEncoding   => Flatten { distribute_list: None },
        CsKind::Type                    => Flatten { distribute_list: None },
        CsKind::TypeDeclaration         => Flatten { distribute_list: None },

        // Preprocessor directives and shebang — flatten to suppress raw output.
        CsKind::PreprocArg
        | CsKind::PreprocDefine
        | CsKind::PreprocElif
        | CsKind::PreprocElse
        | CsKind::PreprocEndregion
        | CsKind::PreprocError
        | CsKind::PreprocIf
        | CsKind::PreprocIfInAttributeList
        | CsKind::PreprocLine
        | CsKind::PreprocNullable
        | CsKind::PreprocPragma
        | CsKind::PreprocRegion
        | CsKind::PreprocUndef
        | CsKind::PreprocWarning
        | CsKind::ShebangDirective      => Flatten { distribute_list: None },
    }
}
