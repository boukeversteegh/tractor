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
use super::output::*;
use super::transformations;

pub fn rule(k: PyKind) -> Rule<&'static str> {
    use Rule::*;
    match k {
        // ---- ExtractOpThenRename ---------------------------------------
        PyKind::AugmentedAssignment => ExtractOpThenRename(ASSIGN),
        PyKind::BinaryOperator      => ExtractOpThenRename(BINARY),
        PyKind::BooleanOperator     => ExtractOpThenRename(LOGICAL),
        PyKind::ComparisonOperator  => ExtractOpThenRename(COMPARE),
        PyKind::UnaryOperator       => ExtractOpThenRename(UNARY),

        // ---- RenameWithMarker ------------------------------------------
        PyKind::ClassPattern             => RenameWithMarker(PATTERN, CLASS),
        PyKind::DictPattern              => RenameWithMarker(PATTERN, DICT),
        PyKind::DictionarySplat          => RenameWithMarker(SPREAD, DICT),
        PyKind::DictionarySplatPattern   => RenameWithMarker(SPREAD, DICT),
        PyKind::ListPattern              => RenameWithMarker(PATTERN, LIST),
        PyKind::ListSplat                => RenameWithMarker(SPREAD, LIST),
        PyKind::ListSplatPattern         => RenameWithMarker(SPREAD, LIST),
        PyKind::SplatPattern             => RenameWithMarker(PATTERN, SPLAT),
        PyKind::UnionPattern             => RenameWithMarker(PATTERN, UNION),
        PyKind::UnionType                => RenameWithMarker(TYPE, UNION),

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
        PyKind::AliasedImport         => Rename(IMPORT),
        PyKind::AsPattern             => Rename(AS),
        PyKind::AssertStatement       => Rename(ASSERT),
        PyKind::Assignment            => Rename(ASSIGN),
        PyKind::Attribute             => Rename(MEMBER),
        PyKind::Await                 => Rename(AWAIT),
        PyKind::BreakStatement        => Rename(BREAK),
        PyKind::Call                  => Rename(CALL),
        PyKind::CaseClause            => Rename(ARM),
        PyKind::CasePattern           => Rename(PATTERN),
        PyKind::ClassDefinition       => Rename(CLASS),
        PyKind::ContinueStatement     => Rename(CONTINUE),
        PyKind::Decorator             => Rename(DECORATOR),
        PyKind::DefaultParameter      => Rename(PARAMETER),
        PyKind::DeleteStatement       => Rename(DELETE),
        PyKind::ElifClause            => Rename(ELSE_IF),
        PyKind::ElseClause            => Rename(ELSE),
        PyKind::ExceptClause          => Rename(EXCEPT),
        PyKind::False                 => Rename(FALSE),
        PyKind::FinallyClause         => Rename(FINALLY),
        PyKind::Float                 => Rename(FLOAT),
        PyKind::ForStatement          => Rename(FOR),
        PyKind::FormatSpecifier       => Rename(FORMAT),
        PyKind::GeneratorExpression   => Rename(GENERATOR),
        PyKind::GlobalStatement       => Rename(GLOBAL),
        PyKind::Identifier            => Rename(NAME),
        PyKind::IfStatement           => Rename(IF),
        PyKind::ImportFromStatement   => Rename(FROM),
        PyKind::ImportStatement       => Rename(IMPORT),
        PyKind::Integer               => Rename(INT),
        PyKind::KeywordArgument       => Rename(ARGUMENT),
        PyKind::KeywordPattern        => Rename(PATTERN),
        PyKind::KeywordSeparator      => Rename(KEYWORD),
        PyKind::Lambda                => Rename(LAMBDA),
        PyKind::MatchStatement        => Rename(MATCH),
        PyKind::Module                => Rename(MODULE),
        PyKind::NamedExpression       => Rename(ASSIGN),
        PyKind::None                  => Rename(NONE),
        PyKind::NonlocalStatement     => Rename(NONLOCAL),
        PyKind::PassStatement         => Rename(PASS),
        PyKind::PositionalSeparator   => Rename(POSITIONAL),
        PyKind::RaiseStatement        => Rename(RAISE),
        PyKind::ReturnStatement       => Rename(RETURN),
        PyKind::String                => Rename(STRING),
        PyKind::Subscript             => Rename(SUBSCRIPT),
        PyKind::True                  => Rename(TRUE),
        PyKind::TryStatement          => Rename(TRY),
        PyKind::TypeConversion        => Rename(CAST),
        PyKind::TypedDefaultParameter => Rename(PARAMETER),
        PyKind::TypedParameter        => Rename(PARAMETER),
        PyKind::WhileStatement        => Rename(WHILE),
        PyKind::WithStatement         => Rename(WITH),
        PyKind::Yield                 => Rename(YIELD),

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
