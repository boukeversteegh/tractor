//! Per-kind transformation rules for Ruby: the `RubyKind` → `Rule` table.
//!
//! Read this file to find the rule for a specific kind. Read
//! [`super::transformations`] for the body of any `Rule::Custom`
//! handler. Read [`super::semantic`] for the output vocabulary.
//!
//! Exhaustive over `RubyKind` — the compiler enforces coverage.

use crate::languages::rule::Rule;

use super::input::RubyKind;
use super::output::*;
use super::transformations;

pub fn rule(k: RubyKind) -> Rule {
    use Rule::*;
    match k {
        // ---- RenameWithMarker ------------------------------------------
        RubyKind::BeginBlock         => RenameWithMarker(BLOCK, BEGIN),
        RubyKind::BlockParameter     => RenameWithMarker(PARAMETER, BLOCK),
        RubyKind::DelimitedSymbol    => RenameWithMarker(SYMBOL, DELIMITED),
        RubyKind::DoBlock            => RenameWithMarker(BLOCK, DO),
        RubyKind::HashSplatArgument  => RenameWithMarker(SPREAD, DICT),
        RubyKind::HashSplatParameter => RenameWithMarker(SPREAD, DICT),
        RubyKind::KeywordParameter   => RenameWithMarker(PARAMETER, KEYWORD),
        RubyKind::OptionalParameter  => RenameWithMarker(PARAMETER, DEFAULT),
        RubyKind::SingletonClass     => RenameWithMarker(CLASS, SINGLETON),
        RubyKind::SingletonMethod    => RenameWithMarker(METHOD, SINGLETON),
        RubyKind::SplatArgument      => RenameWithMarker(SPREAD, LIST),
        RubyKind::SplatParameter     => RenameWithMarker(SPREAD, LIST),
        RubyKind::StringArray        => RenameWithMarker(ARRAY, STRING),
        RubyKind::SymbolArray        => RenameWithMarker(ARRAY, SYMBOL),

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
        RubyKind::Array               => Rename(ARRAY),
        RubyKind::Assignment          => Rename(ASSIGN),
        RubyKind::Begin               => Rename(BEGIN),
        RubyKind::Binary              => Rename(BINARY),
        RubyKind::Call                => Rename(CALL),
        RubyKind::Case                => Rename(CASE),
        RubyKind::Class               => Rename(CLASS),
        RubyKind::ClassVariable       => Rename(NAME),
        RubyKind::Else                => Rename(ELSE),
        RubyKind::Elsif               => Rename(ELSE_IF),
        RubyKind::Ensure              => Rename(ENSURE),
        RubyKind::ExceptionVariable   => Rename(VARIABLE),
        RubyKind::For                 => Rename(FOR),
        RubyKind::GlobalVariable      => Rename(NAME),
        RubyKind::Hash                => Rename(HASH),
        RubyKind::Identifier          => Rename(NAME),
        RubyKind::If                  => Rename(IF),
        RubyKind::IfModifier          => Rename(IF),
        RubyKind::InstanceVariable    => Rename(NAME),
        RubyKind::Integer             => Rename(INT),
        RubyKind::LeftAssignmentList  => Rename(LEFT),
        RubyKind::Method              => Rename(METHOD),
        RubyKind::Module              => Rename(MODULE),
        RubyKind::OperatorAssignment  => Rename(ASSIGN),
        RubyKind::Program             => Rename(PROGRAM),
        RubyKind::Rescue              => Rename(RESCUE),
        RubyKind::RestAssignment      => Rename(SPREAD),
        RubyKind::String              => Rename(STRING),
        RubyKind::Unless              => Rename(UNLESS),
        RubyKind::UnlessModifier      => Rename(UNLESS),
        RubyKind::Until               => Rename(UNTIL),
        RubyKind::UntilModifier       => Rename(UNTIL),
        RubyKind::While               => Rename(WHILE),
        RubyKind::WhileModifier       => Rename(WHILE),

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
