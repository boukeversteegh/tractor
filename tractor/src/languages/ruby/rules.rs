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
        // `do_block` — re-tag `<body>` as `<value>` for single-statement
        // bodies (closure archetype iter 161/162/167/168) and prepend
        // the `[do]` marker.
        RubyKind::DoBlock            => Custom(transformations::do_block),
        RubyKind::HashSplatArgument  => RenameWithMarker(Spread, Dict),
        // `**kwargs` parameter — wrap in `<parameter[kwsplat]>` so
        // cross-language `//parameter` finds it (Principle #5;
        // matches Python iter 50).
        RubyKind::HashSplatParameter => RenameWithMarker(Parameter, Kwsplat),
        RubyKind::KeywordParameter   => RenameWithMarker(Parameter, Keyword),
        RubyKind::OptionalParameter  => RenameWithMarker(Parameter, Default),
        RubyKind::SingletonClass     => RenameWithMarker(Class, Singleton),
        RubyKind::SingletonMethod    => RenameWithMarker(Method, Singleton),
        RubyKind::SplatArgument      => RenameWithMarker(Spread, List),
        // `*args` parameter — wrap in `<parameter[splat]>` so
        // cross-language `//parameter` finds it (Principle #5;
        // matches Python iter 50).
        RubyKind::SplatParameter     => RenameWithMarker(Parameter, Splat),
        RubyKind::StringArray        => RenameWithMarker(Array, String),
        RubyKind::SymbolArray        => RenameWithMarker(Array, Symbol),

        // ---- Flatten with field distribution ---------------------------
        RubyKind::ArgumentList     => Flatten { distribute_list: Some("arguments") },
        RubyKind::MethodParameters => Custom(transformations::method_parameters),
        RubyKind::BlockParameters
        | RubyKind::LambdaParameters => Custom(transformations::block_parameters),

        // ---- Pure Flatten ----------------------------------------------
        RubyKind::BareString
        | RubyKind::BareSymbol
        | RubyKind::BlockBody
        | RubyKind::BodyStatement
        | RubyKind::EscapeSequence
        | RubyKind::HashKeySymbol
        | RubyKind::HeredocBeginning
        | RubyKind::HeredocBody
        | RubyKind::HeredocContent
        | RubyKind::HeredocEnd
        | RubyKind::ParenthesizedStatements
        | RubyKind::StringContent => Flatten { distribute_list: None },

        // `:name` simple symbol — produces `<symbol>:name</symbol>`
        // (with the leading `:` preserved). Matches `<symbol>` shape
        // used elsewhere (DelimitedSymbol, HashSplatNil) and lets
        // queries find symbols structurally rather than as bare text
        // leaves inside calls.
        RubyKind::SimpleSymbol => Rename(Symbol),

        // ---- Custom (language-specific logic in transformations.rs) ---
        RubyKind::Comment => Custom(transformations::comment),

        // ---- Pure Rename -----------------------------------------------
        RubyKind::Array               => Rename(Array),
        RubyKind::Assignment          => Rename(Assign),
        RubyKind::Begin               => Rename(Begin),
        RubyKind::Binary              => ExtractOpThenRename(Binary),
        RubyKind::Unary               => ExtractOpThenRename(Unary),
        // `Call` covers both `obj.method` and `obj&.method` (safe-nav).
        // Custom inspects text for `&.` to add `<optional/>` marker so
        // cross-language `//call[optional]` / `//member[optional]`
        // queries find Ruby safe-navigation (matches C# `?.` shape from
        // iter 57; closes the residual item from todo/37).
        RubyKind::Call                => Custom(transformations::call_expression),
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

        // Bare keyword statements — strip the keyword text leaf so the
        // element is empty (Principle #2 / #13).
        RubyKind::Break  => RenameStripKeyword(Break, "break"),
        RubyKind::Next   => RenameStripKeyword(Next, "next"),
        RubyKind::Redo   => RenameStripKeyword(Redo, "redo"),
        RubyKind::Retry  => RenameStripKeyword(Retry, "retry"),
        RubyKind::Yield  => RenameStripKeyword(Yield, "yield"),

        // Capitalized identifiers — Ruby's grammar distinguishes
        // `constant` lexically (uppercase first letter), but other
        // languages use `<name>` for value-namespace identifiers
        // regardless of casing. Collapse to `<name>` so cross-language
        // `//name` queries find Ruby constants too (Principle #5);
        // the capitalization is preserved in the text content for
        // anyone needing the lexical distinction.
        RubyKind::Constant => Rename(Name),

        // `for j in 1..3` — tree-sitter wraps the iterable in an `<in>`
        // kind. The `in` is just a keyword separating the loop var
        // from the collection; flatten so the iterable is a direct
        // child of `<value>` (or wherever the field places it).
        // The case/in clause uses `InClause` → `<in>` instead.
        RubyKind::In => Flatten { distribute_list: None },

        // `block` (`{ |x| ... }`) — re-tag `<body>` as `<value>` for
        // single-statement bodies so cross-language `//block/value/expression`
        // works (closure archetype iter 161/162/167/168).
        RubyKind::Block => Custom(transformations::block),

        // Already matches our vocabulary (no text leak in current snapshots).
        RubyKind::Conditional
        | RubyKind::Do
        | RubyKind::Exceptions
        | RubyKind::False
        | RubyKind::Interpolation
        | RubyKind::Lambda
        | RubyKind::Nil
        | RubyKind::Operator
        | RubyKind::Pair
        | RubyKind::Pattern
        | RubyKind::Range
        | RubyKind::Regex
        | RubyKind::Self_
        | RubyKind::Then
        | RubyKind::True
        | RubyKind::When => Passthrough,

        // ---- Unhandled in the previous dispatcher — survive as raw
        //      kind names. Pending decisions tracked in
        //      todo/36-rule-todo-followups.md.

        // Pattern-matching family — `case x in ...` shapes (Ruby 3.0+).
        // Variants attach as markers on the shared `<pattern>` host,
        // so `//pattern` is the broad path and `[array]` / `[hash]` /
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
        RubyKind::ParenthesizedPattern       => Flatten { distribute_list: None },
        // case/in shapes: `case_match` is the construct, `in_clause`
        // is the `in pattern` arm body, guards are postfix predicates.
        RubyKind::CaseMatch                  => Rename(Match),
        RubyKind::InClause                   => Rename(In),
        RubyKind::IfGuard                    => Rename(If),
        RubyKind::UnlessGuard                => Rename(Unless),

        // alias / undef declarations — pending Ruby-specific shape
        // decision (own semantic vs. shared with import-like). Tracked
        // in todo/36-rule-todo-followups.md.
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

        // `class Foo < Base` — Ruby's `<` is the inheritance operator
        // (no keyword in source). Per Principle #18, name after the
        // most cross-language idiomatic operator: `<extends>`.
        // Always single (Ruby has single inheritance), but adds
        // `list="extends"` for JSON-array consistency
        // (Principle #12).
        RubyKind::Superclass => Custom(transformations::superclass),

        // Numeric / literal kinds — single-word grammar names, fine
        // as raw passthrough. Could pick up proper Rename targets if
        // a query-language audience needs them; tracked in
        // todo/36-rule-todo-followups.md.
        RubyKind::Character
        | RubyKind::Complex
        | RubyKind::Float
        | RubyKind::Rational => Passthrough,

        // Control-flow / structural odds and ends with underscored names.
        RubyKind::ElementReference     => Rename(Index),       // `arr[i]`
        RubyKind::EmptyStatement       => Flatten { distribute_list: None },
        RubyKind::EndBlock             => RenameWithMarker(Block, End),  // `END { ... }`
        RubyKind::RescueModifier       => Rename(Rescue),      // `expr rescue fallback`
        RubyKind::RightAssignmentList  => Rename(Right),       // `(a, b) = ...` RHS list
        RubyKind::ScopeResolution      => RenameWithMarker(Member, Static),  // `Foo::Bar`

        // Ruby `return` strips its keyword leaf the same way Break / Next do.
        RubyKind::Return => RenameStripKeyword(Return, "return"),

        // Single-word passthroughs (no underscore violation):
        //   encoding/file/line — `__ENCODING__`/`__FILE__`/`__LINE__`
        //   setter / subshell / super / uninterpreted
        // Tracked in todo/36-rule-todo-followups.md if these need
        // semantic upgrades.
        RubyKind::Encoding
        | RubyKind::File
        | RubyKind::Line
        | RubyKind::Setter
        | RubyKind::Subshell
        | RubyKind::Super
        | RubyKind::Uninterpreted => Passthrough,
    }
}
