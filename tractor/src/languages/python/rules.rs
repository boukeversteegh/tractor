//! Per-kind transformation rules for Python: the `PyKind` → `Rule` table.
//!
//! Read this file to find the rule for a specific kind. Read
//! [`super::transformations`] for the body of any `Rule::Custom`
//! handler the rule references by name. Read [`super::semantic`] for
//! the output vocabulary (semantic names + TractorNodeSpec metadata).
//!
//! Exhaustive over `PyKind` — the compiler enforces coverage.

use crate::languages::rule::Rule;

use super::input::PyKind;
use super::output::TractorNode::{
    self, Alias, Argument, Arm, As, Assert, Assign, Binary, Break, Call, Cast, Class,
    Compare, Complex, Concatenated, Constrained, Continue, Decorator, Delete, Dict, Else,
    ElseIf, Escape, Except, Exec, False, Finally, Float, Format, From, Future,
    Args, Generator, Global, Group, If, Import, Int, Interpolation, Keyword, Kwargs, List,
    Logical, Match, Member, Module, Name, Nonlocal, Parameter, Pass, Pattern, Positional,
    Print, Raise, Return, Splat, Spread, String, Subscript, True, Try, Tuple, Type, Unary,
    Union, While, Wildcard, Yield, None as PyNone,
};
use super::transformations;

pub fn rule(k: PyKind) -> Rule<TractorNode> {
    use Rule::*;
    match k {
        // ---- ExtractOpThenRename ---------------------------------------
        PyKind::AugmentedAssignment => ExtractOpThenRename(Assign),
        PyKind::BinaryOperator      => ExtractOpThenRename(Binary),
        PyKind::BooleanOperator     => ExtractOpThenRename(Logical),
        PyKind::ComparisonOperator  => ExtractOpThenRename(Compare),
        PyKind::UnaryOperator       => ExtractOpThenRename(Unary),

        // ---- RenameWithMarker ------------------------------------------
        PyKind::ClassPattern             => RenameWithMarker(Pattern, Class),
        PyKind::DictPattern              => RenameWithMarker(Pattern, Dict),
        PyKind::DictionarySplat          => RenameWithMarker(Spread, Dict),
        // `**kwargs` parameter — wrap in `<parameter[kwsplat]>` so
        // cross-language `//parameter` finds it (Principle #5).
        PyKind::DictionarySplatPattern   => RenameWithMarker(Parameter, Kwargs),
        PyKind::ListPattern              => RenameWithMarker(Pattern, List),
        PyKind::ListSplat                => RenameWithMarker(Spread, List),
        // `*args` parameter — wrap in `<parameter[splat]>` so
        // cross-language `//parameter` finds it (Principle #5).
        PyKind::ListSplatPattern         => RenameWithMarker(Parameter, Args),
        PyKind::SplatPattern             => RenameWithMarker(Pattern, Splat),
        PyKind::UnionPattern             => RenameWithMarker(Pattern, Union),
        PyKind::UnionType                => RenameWithMarker(Type, Union),

        // ---- Flatten with field distribution ---------------------------
        // Context-aware argument_list: positional args inside a
        // class's superclasses become `<base>` siblings (Principle
        // #12 — no list container). Other contexts (regular calls)
        // distribute `list="arguments"` and flatten as before.
        PyKind::ArgumentList  => Custom(transformations::argument_list),
        PyKind::Parameters    => Custom(transformations::parameters),
        // `type_parameter` serves DOUBLE DUTY in tree-sitter Python:
        //   1. PEP 695 declaration param list — `def f[T, U]()` /
        //      `class A[T]` / `type X[T] = ...`. Should wrap in
        //      `<generic>` (matches Java/TS declaration-level shape).
        //   2. Subscript generic argument list — `Optional[str]`,
        //      `list[int]`. Already inside `<type[generic]>` parent;
        //      should Flatten so type-args become direct children
        //      (matches TS `type[generic]/{name=Map, type, type}`).
        // Custom handler dispatches by parent kind.
        PyKind::TypeParameter => Custom(transformations::type_parameter),

        // ---- Pure Flatten ----------------------------------------------
        PyKind::AsPatternTarget
        | PyKind::Block
        | PyKind::DottedName
        | PyKind::EscapeSequence
        | PyKind::ExpressionList
        | PyKind::ForInClause
        | PyKind::IfClause
        | PyKind::ImportPrefix
        | PyKind::LambdaParameters
        | PyKind::ParenthesizedExpression
        | PyKind::PatternList
        | PyKind::RelativeImport
        | PyKind::StringContent
        | PyKind::StringEnd
        | PyKind::StringStart
        | PyKind::WithClause
        | PyKind::WithItem => Flatten { distribute_list: None },

        // ---- Custom (language-specific logic in transformations.rs) ---
        PyKind::Comment                  => Custom(transformations::comment),
        PyKind::ConditionalExpression    => Custom(transformations::conditional_expression),
        PyKind::DecoratedDefinition      => Custom(transformations::decorated_definition),
        PyKind::Dictionary               => Custom(transformations::dictionary_literal),
        PyKind::DictionaryComprehension  => Custom(transformations::dictionary_comprehension),
        PyKind::Await                    => Custom(transformations::await_expression),
        PyKind::ExpressionStatement      => Custom(transformations::expression_statement),
        PyKind::FunctionDefinition       => Custom(transformations::function_definition),
        PyKind::GenericType              => Custom(transformations::generic_type),
        PyKind::List                     => Custom(transformations::list_literal),
        PyKind::ListComprehension        => Custom(transformations::list_comprehension),
        PyKind::Set                      => Custom(transformations::set_literal),
        PyKind::SetComprehension         => Custom(transformations::set_comprehension),
        PyKind::Type                     => Custom(transformations::type_node),

        // ---- Pure Rename -----------------------------------------------
        // `import X as Y` — the inner aliased_import would re-emit
        // `<import>` inside the outer ImportStatement `<import>`,
        // creating `<import><import>name=X, name=Y</import></import>`.
        // Flatten so the two names become direct children of the
        // outer `<import>`. The first name is the imported module,
        // the second is the alias — order conveys the relationship.
        PyKind::AliasedImport         => Flatten { distribute_list: None },
        PyKind::AsPattern             => Rename(As),
        PyKind::AssertStatement       => Rename(Assert),
        PyKind::Assignment            => Rename(Assign),
        // `obj.attr` — receiver and accessed-attribute play different
        // roles. Per Principle #19: each role gets a slot-named
        // container (`<object>` / `<property>`). Matches TS / Java
        // (iter 147) shape.
        PyKind::Attribute             => Custom(transformations::attribute),
        PyKind::BreakStatement        => RenameStripKeyword(Break, "break"),
        PyKind::Call                  => Rename(Call),
        PyKind::CaseClause            => Rename(Arm),
        // `case _:` becomes `<pattern[wildcard]/>` (marker form);
        // structural patterns rename normally.
        PyKind::CasePattern           => Custom(transformations::case_pattern),
        PyKind::ClassDefinition       => Rename(Class),
        PyKind::ContinueStatement     => RenameStripKeyword(Continue, "continue"),
        PyKind::Decorator             => Rename(Decorator),
        PyKind::DefaultParameter      => Rename(Parameter),
        PyKind::DeleteStatement       => Rename(Delete),
        PyKind::ElifClause            => Rename(ElseIf),
        PyKind::ElseClause            => Rename(Else),
        PyKind::ExceptClause          => Rename(Except),
        PyKind::False                 => Rename(False),
        PyKind::FinallyClause         => Rename(Finally),
        PyKind::Float                 => Rename(Float),
        // `async for` / `async with` — extract async modifier marker
        // before renaming.
        PyKind::ForStatement          => Custom(transformations::for_statement),
        PyKind::FormatSpecifier       => Rename(Format),
        PyKind::GeneratorExpression   => Rename(Generator),
        PyKind::GlobalStatement       => Rename(Global),
        PyKind::Identifier            => Rename(Name),
        PyKind::IfStatement           => Rename(If),
        PyKind::ImportFromStatement   => Rename(From),
        PyKind::ImportStatement       => Rename(Import),
        PyKind::Integer               => Rename(Int),
        PyKind::KeywordArgument       => Rename(Argument),
        // `x=0` inside `case Point(x=0):` — explicit key/value shape
        // matches the dict-pattern shape (`<pattern[dict]>{string}{value}`).
        PyKind::KeywordPattern        => Custom(transformations::keyword_pattern),
        PyKind::KeywordSeparator      => Rename(Keyword),
        // `lambda x: x + 1` — body is always a single expression in
        // Python lambdas. Re-tag `<body>` as `<value>` so
        // `wrap_expression_positions` treats it as an expression
        // position (Principle #15) and `distribute_member_list_attrs`
        // skips the over-tagging the iter-140 generic pass would have
        // applied.
        PyKind::Lambda                => Custom(transformations::lambda),
        PyKind::MatchStatement        => Rename(Match),
        PyKind::Module                => Rename(Module),
        // `name := value` (PEP 572 walrus). Extract the `:=` operator
        // into `<op>` so `//assign[op[walrus]]` matches uniformly.
        PyKind::NamedExpression       => ExtractOpThenRename(Assign),
        PyKind::None                  => Rename(PyNone),
        PyKind::NonlocalStatement     => Rename(Nonlocal),
        PyKind::PassStatement         => RenameStripKeyword(Pass, "pass"),
        PyKind::PositionalSeparator   => Rename(Positional),
        PyKind::RaiseStatement        => Rename(Raise),
        PyKind::ReturnStatement       => Rename(Return),
        PyKind::String                => Rename(String),
        PyKind::Subscript             => Rename(Subscript),
        PyKind::True                  => Rename(True),
        PyKind::TryStatement          => Rename(Try),
        PyKind::TypeConversion        => Rename(Cast),
        PyKind::TypedDefaultParameter => Rename(Parameter),
        PyKind::TypedParameter        => Rename(Parameter),
        PyKind::WhileStatement        => Rename(While),
        PyKind::WithStatement         => Custom(transformations::with_statement),
        PyKind::Yield                 => Rename(Yield),

        // ---- Passthrough — kind name already matches the vocabulary,
        //      OR the kind is unhandled and the dispatcher leaves it as
        //      its raw grammar name.

        // Already matches our vocabulary.
        PyKind::Interpolation
        | PyKind::Pair
        | PyKind::Tuple => Passthrough,

        // ---- Type vocabulary ----------------------------------------
        // Python 3.12 type-alias `type Foo = …` joins the cross-language
        // <alias> family (matches Java / TS `type Foo = …`).
        PyKind::TypeAliasStatement => Rename(Alias),

        // PEP 695 generic-type-parameter shapes:
        //   constrained_type  T: int        → <type[constrained]>
        //   member_type       module.Foo    → <type[member]>
        //   splat_type        *Ts           → <type[splat]>
        PyKind::ConstrainedType => RenameWithMarker(Type, Constrained),
        PyKind::MemberType      => RenameWithMarker(Type, Member),
        PyKind::SplatType       => RenameWithMarker(Type, Splat),

        // Match-statement patterns:
        //   tuple_pattern   case (a, b):   → <pattern[tuple]>
        //   complex_pattern case 1+2j:     → <pattern[complex]>
        PyKind::TuplePattern   => RenameWithMarker(Pattern, Tuple),
        PyKind::ComplexPattern => RenameWithMarker(Pattern, Complex),

        // PEP 654 except-group `except* E:` joins <except> with a
        // <group/> marker so `//except[group]` picks them out.
        PyKind::ExceptGroupClause => RenameWithMarker(Except, Group),

        // Python 2 leftovers — own elements (rare in modern code).
        PyKind::ExecStatement  => Rename(Exec),
        PyKind::PrintStatement => Rename(Print),

        // Import-shape markers:
        //   `from __future__ import x`  → <import[future]>
        //   `from m import *`           → <import[wildcard]>
        PyKind::FutureImportStatement => RenameWithMarker(Import, Future),
        PyKind::WildcardImport        => RenameWithMarker(Import, Wildcard),

        // String/f-string internals:
        //   concatenated_string `"a" "b"`     → <string[concatenated]>
        //   format_expression   `{expr}` body → <interpolation>
        //                                       (matches the cross-language
        //                                        interpolation shape)
        //   escape_interpolation `{{` / `}}`  → <interpolation[escape]>
        //                                       so `//interpolation[escape]`
        //                                       picks them out
        //   parenthesized_list_splat `(*a,)`  → <spread> (matches the
        //                                       cross-language splat
        //                                       vocabulary)
        //   not_operator `not x`              → <unary> with op extraction
        //                                       — sibling of unary_operator
        PyKind::ConcatenatedString    => RenameWithMarker(String, Concatenated),
        PyKind::FormatExpression      => Rename(Interpolation),
        PyKind::EscapeInterpolation   => RenameWithMarker(Interpolation, Escape),
        PyKind::ParenthesizedListSplat => Rename(Spread),
        PyKind::NotOperator           => ExtractOpThenRename(Unary),

        // Pure-whitespace continuation `\\\n` carries no semantics.
        PyKind::LineContinuation => Flatten { distribute_list: None },

        // Remaining unhandled grammar kinds — fall through with no
        // semantic name. None contain underscores after iter 13's sweep.
        PyKind::Chevron
        | PyKind::Ellipsis
        | PyKind::Parameter
        | PyKind::Slice => Passthrough,

        // ---- Truly raw structural supertypes. Tree-sitter exposes
        //      these as named kinds for grammar-introspection but they
        //      almost never appear in parsed output. PrimaryExpression
        //      is a supertype that flattens to its single child.
        PyKind::Expression
        | PyKind::Pattern => Passthrough,
        PyKind::PrimaryExpression => Flatten { distribute_list: None },
    }
}
