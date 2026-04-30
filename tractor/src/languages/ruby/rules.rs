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
        RubyKind::Binary              => Rename(Binary),
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
        | RubyKind::Unary
        | RubyKind::When
        | RubyKind::Yield => Custom(transformations::passthrough),

        // ---- Unhandled in the previous dispatcher — survive as raw
        //      kind names. Most are TODO candidates.

        // TODO: Ruby pattern-matching (3.0+) variants. Likely each
        // renames to PATTERN with a marker:
        //   alternative_pattern   → RenameWithMarker(PATTERN, OR)?
        //   array_pattern         → RenameWithMarker(PATTERN, ARRAY)
        //   as_pattern            → RenameWithMarker(PATTERN, AS)?
        //   case_match            → Rename(MATCH)
        //   expression_reference_pattern / find_pattern / hash_pattern
        //   keyword_pattern / match_pattern / parenthesized_pattern
        //   test_pattern / variable_reference_pattern
        //   if_guard / unless_guard / in_clause
        RubyKind::AlternativePattern
        | RubyKind::ArrayPattern
        | RubyKind::AsPattern
        | RubyKind::CaseMatch
        | RubyKind::ExpressionReferencePattern
        | RubyKind::FindPattern
        | RubyKind::HashPattern
        | RubyKind::IfGuard
        | RubyKind::InClause
        | RubyKind::KeywordPattern
        | RubyKind::MatchPattern
        | RubyKind::ParenthesizedPattern
        | RubyKind::TestPattern
        | RubyKind::UnlessGuard
        | RubyKind::VariableReferencePattern => Custom(transformations::passthrough),

        // TODO: alias / undef declarations.
        //   alias  → Rename(ALIAS)? own semantic?
        //   undef  → Rename(UNDEF)?
        RubyKind::Alias
        | RubyKind::Undef => Custom(transformations::passthrough),

        // TODO: special argument / parameter variants.
        //   block_argument           → marker
        //   forward_argument         → marker
        //   forward_parameter        → marker
        //   destructured_left_assignment / destructured_parameter
        //   hash_splat_nil           → marker for `**nil`
        RubyKind::BlockArgument
        | RubyKind::DestructuredLeftAssignment
        | RubyKind::DestructuredParameter
        | RubyKind::ForwardArgument
        | RubyKind::ForwardParameter
        | RubyKind::HashSplatNil => Custom(transformations::passthrough),

        // TODO: literal / numeric kinds.
        //   character        → Rename(CHAR)?
        //   chained_string   → adjacent literals; concat-like
        //   complex          → Rename(COMPLEX)? (e.g. `1i`)
        //   float            → Rename(FLOAT)
        //   rational         → Rename(RATIONAL)? (e.g. `1r`)
        RubyKind::ChainedString
        | RubyKind::Character
        | RubyKind::Complex
        | RubyKind::Float
        | RubyKind::Rational => Custom(transformations::passthrough),

        // TODO: control-flow / structural odds and ends.
        //   element_reference (`a[i]`) → Rename(INDEX)?
        //   empty_statement   → Flatten?
        //   end_block         → BeginBlock sibling: `END { ... }`
        //   rescue_modifier   → Rename(RESCUE)?
        //   right_assignment_list → Rename(RIGHT)?
        //   return            → Rename(RETURN)
        //   scope_resolution  → Rename(PATH)?
        //   setter            → method-like (`def x=(v)`)
        //   subshell          → backtick command
        //   uninterpreted     → grammar marker for unparsed text
        //   __FILE__ / __LINE__ / __ENCODING__ → marker (file / line / encoding)
        RubyKind::ElementReference
        | RubyKind::EmptyStatement
        | RubyKind::Encoding
        | RubyKind::EndBlock
        | RubyKind::File
        | RubyKind::Line
        | RubyKind::RescueModifier
        | RubyKind::RightAssignmentList
        | RubyKind::Return
        | RubyKind::ScopeResolution
        | RubyKind::Setter
        | RubyKind::Subshell
        | RubyKind::Super
        | RubyKind::Uninterpreted => Custom(transformations::passthrough),
    }
}
