//! Per-kind transformation rules for Python: the `PyKind` → `Rule` table.
//!
//! Read this file to find the rule for a specific kind. Read
//! [`super::transformations`] for the body of any `Rule::Custom`
//! handler the rule references by name. Read [`super::semantic`] for
//! the output vocabulary (semantic names + NodeSpec metadata).
//!
//! Exhaustive over `PyKind` — the compiler enforces coverage.

use crate::languages::rule::Rule;

use super::input::PyKind;
use super::output::PyName::{
    self, Argument, Arm, As, Assert, Assign, Await, Binary, Break, Call, Cast, Class,
    Compare, Continue, Decorator, Delete, Dict, Else, ElseIf, Except, False,
    Finally, Float, For, Format, From, Generator, Global, If, Import, Int,
    Keyword, Lambda, List, Logical, Match, Member, Module, Name, Nonlocal,
    Parameter, Pass, Pattern, Positional, Raise, Return, Splat, Spread, String,
    Subscript, True, Try, Type, Unary, Union, While, With, Yield,
    None as PyNone,
};
use super::transformations;

pub fn rule(k: PyKind) -> Rule<PyName> {
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
        PyKind::DictionarySplatPattern   => RenameWithMarker(Spread, Dict),
        PyKind::ListPattern              => RenameWithMarker(Pattern, List),
        PyKind::ListSplat                => RenameWithMarker(Spread, List),
        PyKind::ListSplatPattern         => RenameWithMarker(Spread, List),
        PyKind::SplatPattern             => RenameWithMarker(Pattern, Splat),
        PyKind::UnionPattern             => RenameWithMarker(Pattern, Union),
        PyKind::UnionType                => RenameWithMarker(Type, Union),

        // ---- Flatten with field distribution ---------------------------
        PyKind::ArgumentList  => Flatten { distribute_field: Some("arguments") },
        PyKind::Parameters    => Flatten { distribute_field: Some("parameters") },
        PyKind::TypeParameter => Flatten { distribute_field: Some("arguments") },

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
        | PyKind::WithItem => Flatten { distribute_field: None },

        // ---- Custom (language-specific logic in transformations.rs) ---
        PyKind::Comment                  => Custom(transformations::comment),
        PyKind::ConditionalExpression    => Custom(transformations::conditional_expression),
        PyKind::DecoratedDefinition      => Custom(transformations::decorated_definition),
        PyKind::Dictionary               => Custom(transformations::dictionary_literal),
        PyKind::DictionaryComprehension  => Custom(transformations::dictionary_comprehension),
        PyKind::ExpressionStatement      => Custom(transformations::skip),
        PyKind::FunctionDefinition       => Custom(transformations::function_definition),
        PyKind::GenericType              => Custom(transformations::generic_type),
        PyKind::List                     => Custom(transformations::list_literal),
        PyKind::ListComprehension        => Custom(transformations::list_comprehension),
        PyKind::Set                      => Custom(transformations::set_literal),
        PyKind::SetComprehension         => Custom(transformations::set_comprehension),
        PyKind::Type                     => Custom(transformations::type_node),

        // ---- Pure Rename -----------------------------------------------
        PyKind::AliasedImport         => Rename(Import),
        PyKind::AsPattern             => Rename(As),
        PyKind::AssertStatement       => Rename(Assert),
        PyKind::Assignment            => Rename(Assign),
        PyKind::Attribute             => Rename(Member),
        PyKind::Await                 => Rename(Await),
        PyKind::BreakStatement        => Rename(Break),
        PyKind::Call                  => Rename(Call),
        PyKind::CaseClause            => Rename(Arm),
        PyKind::CasePattern           => Rename(Pattern),
        PyKind::ClassDefinition       => Rename(Class),
        PyKind::ContinueStatement     => Rename(Continue),
        PyKind::Decorator             => Rename(Decorator),
        PyKind::DefaultParameter      => Rename(Parameter),
        PyKind::DeleteStatement       => Rename(Delete),
        PyKind::ElifClause            => Rename(ElseIf),
        PyKind::ElseClause            => Rename(Else),
        PyKind::ExceptClause          => Rename(Except),
        PyKind::False                 => Rename(False),
        PyKind::FinallyClause         => Rename(Finally),
        PyKind::Float                 => Rename(Float),
        PyKind::ForStatement          => Rename(For),
        PyKind::FormatSpecifier       => Rename(Format),
        PyKind::GeneratorExpression   => Rename(Generator),
        PyKind::GlobalStatement       => Rename(Global),
        PyKind::Identifier            => Rename(Name),
        PyKind::IfStatement           => Rename(If),
        PyKind::ImportFromStatement   => Rename(From),
        PyKind::ImportStatement       => Rename(Import),
        PyKind::Integer               => Rename(Int),
        PyKind::KeywordArgument       => Rename(Argument),
        PyKind::KeywordPattern        => Rename(Pattern),
        PyKind::KeywordSeparator      => Rename(Keyword),
        PyKind::Lambda                => Rename(Lambda),
        PyKind::MatchStatement        => Rename(Match),
        PyKind::Module                => Rename(Module),
        PyKind::NamedExpression       => Rename(Assign),
        PyKind::None                  => Rename(PyNone),
        PyKind::NonlocalStatement     => Rename(Nonlocal),
        PyKind::PassStatement         => Rename(Pass),
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
        PyKind::WithStatement         => Rename(With),
        PyKind::Yield                 => Rename(Yield),

        // ---- Passthrough — kind name already matches the vocabulary,
        //      OR the kind is unhandled and the dispatcher leaves it as
        //      its raw grammar name.

        // Already matches our vocabulary.
        PyKind::Interpolation
        | PyKind::Pair
        | PyKind::Tuple => Custom(transformations::passthrough),

        // ---- Unhandled in the previous dispatcher — survive as raw
        //      kind names. TODO candidates for real semantics.

        // TODO: Python 3.12 type-alias statement (`type Foo = …`).
        // Likely Rename(TYPE) with marker, or own ALIAS semantic.
        PyKind::TypeAliasStatement => Custom(transformations::passthrough),

        // TODO: PEP 695 generic type parameters / constraints.
        //   constrained_type → RenameWithMarker(TYPE, …)
        //   member_type      → similar to attribute access for types
        //   splat_type       → RenameWithMarker(TYPE, SPLAT)
        PyKind::ConstrainedType
        | PyKind::MemberType
        | PyKind::SplatType => Custom(transformations::passthrough),

        // TODO: tuple_pattern in match arms; pattern combinators.
        //   tuple_pattern → RenameWithMarker(PATTERN, TUPLE)?
        //   complex_pattern → RenameWithMarker(PATTERN, COMPLEX)? (numeric)
        PyKind::TuplePattern
        | PyKind::ComplexPattern => Custom(transformations::passthrough),

        // TODO: PEP 654 except-group `except* E:`. Sibling of
        // except_clause → EXCEPT. Likely RenameWithMarker(EXCEPT, GROUP).
        PyKind::ExceptGroupClause => Custom(transformations::passthrough),

        // TODO: Python 2 leftovers — `exec stmt`, `print stmt`. Pure
        // historical; rename to a generic Rename(EXEC) / Rename(PRINT)?
        PyKind::ExecStatement
        | PyKind::PrintStatement => Custom(transformations::passthrough),

        // TODO: `from __future__ import …` is grammatically a separate
        // kind from regular import_from_statement. Could share Rename(FROM)
        // with a FUTURE marker.
        PyKind::FutureImportStatement => Custom(transformations::passthrough),

        // TODO: `from x import *` wildcard. Currently passthrough; could
        // rename to IMPORT with a marker.
        PyKind::WildcardImport => Custom(transformations::passthrough),

        // TODO: f-string internals.
        //   format_expression       — the `{expr}` body in an f-string
        //   escape_interpolation    — `{{` / `}}` escape sequences
        //   chevron                 — `print >> file, …` (py2 leftover)
        //   concatenated_string     — adjacent literals: `"a" "b"`
        //   ellipsis                — `...` literal
        //   parenthesized_list_splat — `(*a,)` in tuple ctx
        //   not_operator            — `not x` (sibling of unary_operator)
        //   line_continuation       — `\\\n` (whitespace, usually skipped)
        PyKind::Chevron
        | PyKind::ConcatenatedString
        | PyKind::Ellipsis
        | PyKind::EscapeInterpolation
        | PyKind::FormatExpression
        | PyKind::LineContinuation
        | PyKind::NotOperator
        | PyKind::Parameter
        | PyKind::ParenthesizedListSplat
        | PyKind::Slice => Custom(transformations::passthrough),

        // ---- Truly raw structural supertypes. Tree-sitter exposes
        //      these as named kinds for grammar-introspection but they
        //      almost never appear in parsed output.
        PyKind::Expression
        | PyKind::Pattern
        | PyKind::PrimaryExpression => Custom(transformations::passthrough),
    }
}
