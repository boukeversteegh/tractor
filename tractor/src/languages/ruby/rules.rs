//! Per-kind transformation rules for Ruby: the `RubyKind` → `Rule` table.
//!
//! Read this file to find the rule for a specific kind. Read
//! [`super::transformations`] for the body of any `Rule::Custom`
//! handler. Read [`super::semantic`] for the output vocabulary.
//!
//! Exhaustive over `RubyKind` — the compiler enforces coverage.

use crate::languages::rule::Rule;

use super::input::RubyKind;
use super::output::TractorNode::{self, *};
use super::transformations;

pub fn rule(k: RubyKind) -> Rule<TractorNode> {
    use Rule::*;
    match k {
        // ---- RenameWithMarker ------------------------------------------
        RubyKind::BeginBlock         => RenameWithMarker(Block, Begin),
        RubyKind::BlockParameter     => RenameWithMarker(Parameter, Block),
        RubyKind::DelimitedSymbol    => RenameWithMarker(Symbol, Delimited),
        RubyKind::DoBlock            => RenameWithMarker(Block, Do),
        RubyKind::HashSplatArgument  => RenameWithMarker(Spread, Dict),
        RubyKind::HashSplatParameter => RenameWithMarker(Spread, Dict),
        RubyKind::KeywordParameter   => RenameWithMarker(Parameter, Keyword),
        RubyKind::OptionalParameter  => RenameWithMarker(Parameter, Default),
        RubyKind::SingletonClass     => RenameWithMarker(Class, Singleton),
        RubyKind::SingletonMethod    => RenameWithMarker(Method, Singleton),
        RubyKind::SplatArgument      => RenameWithMarker(Spread, List),
        RubyKind::SplatParameter     => RenameWithMarker(Spread, List),
        RubyKind::StringArray        => RenameWithMarker(Array, String),
        RubyKind::SymbolArray        => RenameWithMarker(Array, Symbol),

        // ---- Flatten with field distribution ---------------------------
        RubyKind::ArgumentList     => Flatten { distribute_field: Some("arguments") },
        RubyKind::MethodParameters => Flatten { distribute_field: Some("parameters") },

        // ---- Pure Flatten ----------------------------------------------
        RubyKind::BareString
        | RubyKind::BareSymbol
        | RubyKind::BlockBody
        | RubyKind::BlockParameters
        | RubyKind::BodyStatement
        | RubyKind::EscapeSequence
        | RubyKind::HashKeySymbol
        | RubyKind::HeredocBeginning
        | RubyKind::HeredocBody
        | RubyKind::HeredocContent
        | RubyKind::HeredocEnd
        | RubyKind::LambdaParameters
        | RubyKind::ParenthesizedStatements
        | RubyKind::SimpleSymbol
        | RubyKind::StringContent => Flatten { distribute_field: None },

        // ---- Custom (language-specific logic in transformations.rs) ---
        RubyKind::Comment => Custom(transformations::comment),

        // ---- Pure Rename -----------------------------------------------
        RubyKind::Array               => Rename(Array),
        RubyKind::Assignment          => Rename(Assign),
        RubyKind::Begin               => Rename(Begin),
        RubyKind::Binary              => ExtractOpThenRename(Binary),
        RubyKind::Unary               => ExtractOpThenRename(Unary),
        RubyKind::Call                => Rename(Call),
        RubyKind::Case                => Rename(Case),
        RubyKind::Class               => Rename(Class),
        RubyKind::ClassVariable       => Rename(Name),
        RubyKind::Else                => Rename(Else),
        RubyKind::Elsif               => Rename(ElseIf),
        RubyKind::Ensure              => Rename(Ensure),
        RubyKind::ExceptionVariable   => Rename(Variable),
        RubyKind::For                 => Rename(For),
        RubyKind::GlobalVariable      => Rename(Name),
        RubyKind::Hash                => Rename(Hash),
        RubyKind::Identifier          => Rename(Name),
        RubyKind::If                  => Rename(If),
        RubyKind::IfModifier          => Rename(If),
        RubyKind::InstanceVariable    => Rename(Name),
        RubyKind::Integer             => Rename(Int),
        RubyKind::LeftAssignmentList  => Rename(Left),
        RubyKind::Method              => Rename(Method),
        RubyKind::Module              => Rename(Module),
        RubyKind::OperatorAssignment  => Rename(Assign),
        RubyKind::Program             => Rename(Program),
        RubyKind::Rescue              => Rename(Rescue),
        RubyKind::RestAssignment      => Rename(Spread),
        RubyKind::String              => Rename(String),
        RubyKind::Unless              => Rename(Unless),
        RubyKind::UnlessModifier      => Rename(Unless),
        RubyKind::Until               => Rename(Until),
        RubyKind::UntilModifier       => Rename(Until),
        RubyKind::While               => Rename(While),
        RubyKind::WhileModifier       => Rename(While),

        // ---- Passthrough — kind name already matches the vocabulary,
        //      OR the kind is unhandled and survives as raw kind name.

        // Already matches our vocabulary.
        RubyKind::Block
        | RubyKind::Break
        | RubyKind::Conditional
        | RubyKind::Constant
        | RubyKind::Do
        | RubyKind::Exceptions
        | RubyKind::False
        | RubyKind::In
        | RubyKind::Interpolation
        | RubyKind::Lambda
        | RubyKind::Next
        | RubyKind::Nil
        | RubyKind::Operator
        | RubyKind::Pair
        | RubyKind::Pattern
        | RubyKind::Range
        | RubyKind::Redo
        | RubyKind::Regex
        | RubyKind::Retry
        | RubyKind::Self_
        | RubyKind::Superclass
        | RubyKind::Then
        | RubyKind::True
        | RubyKind::When
        | RubyKind::Yield => Passthrough,

        // ---- Unhandled in the previous dispatcher — survive as raw
        //      kind names. Most are TODO candidates.

        // TODO: Ruby pattern-matching (3.0+) variants. Likely each
        // renames to PATTERN with a marker:
        // Pattern-matching family — `case x in ...` shapes. Variants
        // attach as markers on the shared `<pattern>` host, so
        // `//pattern` is the broad path and `[array]` / `[hash]` /
        // `[alternative]` etc. narrow.
        RubyKind::AlternativePattern         => RenameWithMarker(Pattern, Alternative),
        RubyKind::ArrayPattern               => RenameWithMarker(Pattern, Array),
        RubyKind::AsPattern                  => RenameWithMarker(Pattern, As),
        RubyKind::ExpressionReferencePattern => RenameWithMarker(Pattern, Expression),
        RubyKind::FindPattern                => RenameWithMarker(Pattern, Find),
        RubyKind::HashPattern                => RenameWithMarker(Pattern, Hash),
        RubyKind::KeywordPattern             => RenameWithMarker(Pattern, Keyword),
        RubyKind::MatchPattern               => RenameWithMarker(Pattern, Match),
        RubyKind::TestPattern                => RenameWithMarker(Pattern, Test),
        RubyKind::VariableReferencePattern   => RenameWithMarker(Pattern, Variable),
        // Pure grammar grouping; flatten so the inner pattern bubbles up.
        RubyKind::ParenthesizedPattern       => Flatten { distribute_field: None },
        // case/in shapes: `case_match` is the construct, `in_clause`
        // is the `in pattern` arm body, guards are postfix predicates.
        RubyKind::CaseMatch                  => Rename(Match),
        RubyKind::InClause                   => Rename(In),
        RubyKind::IfGuard                    => Rename(If),
        RubyKind::UnlessGuard                => Rename(Unless),

        // TODO: alias / undef declarations.
        //   alias  → Rename(ALIAS)? own semantic?
        //   undef  → Rename(UNDEF)?
        RubyKind::Alias
        | RubyKind::Undef => Passthrough,

        // Argument / parameter shape variants. Cross-language same-
        // concept naming: `<argument[block]>` / `<parameter[forward]>`
        // / `<spread[nil]>` parallel Python's `<parameter[splat]>` etc.
        RubyKind::BlockArgument             => RenameWithMarker(Argument, Block),
        RubyKind::ForwardArgument           => RenameWithMarker(Argument, Forward),
        RubyKind::ForwardParameter          => RenameWithMarker(Parameter, Forward),
        RubyKind::DestructuredParameter     => RenameWithMarker(Parameter, Destructured),
        RubyKind::DestructuredLeftAssignment => RenameWithMarker(Left, Destructured),
        RubyKind::HashSplatNil              => RenameWithMarker(Spread, Nil),

        // Adjacent string literals `"a" "b"` join under <string[concatenated]>
        // (matches Python's iter-13 shape for `"a" "b"`).
        RubyKind::ChainedString => RenameWithMarker(String, Concatenated),

        // TODO: literal / numeric kinds without underscored names.
        //   character / complex / rational — single-word, fine as-is
        //   for the underscore gate; consider proper Rename later.
        RubyKind::Character
        | RubyKind::Complex
        | RubyKind::Float
        | RubyKind::Rational => Passthrough,

        // Control-flow / structural odds and ends with underscored names.
        RubyKind::ElementReference     => Rename(Index),       // `arr[i]`
        RubyKind::EmptyStatement       => Flatten { distribute_field: None },
        RubyKind::EndBlock             => RenameWithMarker(Block, End),  // `END { ... }`
        RubyKind::RescueModifier       => Rename(Rescue),      // `expr rescue fallback`
        RubyKind::RightAssignmentList  => Rename(Right),       // `(a, b) = ...` RHS list
        RubyKind::ScopeResolution      => RenameWithMarker(Member, Static),  // `Foo::Bar`

        // TODO: remaining single-word passthroughs (no underscore violation):
        //   encoding/file/line — `__ENCODING__`/`__FILE__`/`__LINE__`
        //   return / setter / subshell / super / uninterpreted
        RubyKind::Encoding
        | RubyKind::File
        | RubyKind::Line
        | RubyKind::Return
        | RubyKind::Setter
        | RubyKind::Subshell
        | RubyKind::Super
        | RubyKind::Uninterpreted => Passthrough,
    }
}
