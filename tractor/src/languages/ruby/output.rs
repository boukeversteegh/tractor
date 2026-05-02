//! Output element names — tractor's Ruby XML vocabulary after transform.

use once_cell::sync::Lazy;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, EnumIter, EnumString, IntoStaticStr};

use crate::languages::TractorNodeSpec;
use crate::output::syntax_highlight::SyntaxCategory::{self, *};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, IntoStaticStr, AsRefStr, EnumIter,
)]
#[strum(serialize_all = "snake_case")]
pub enum TractorNode {
    // Top-level / declarations
    Program, Module, Class, Method,
    // Statements / control flow (Begin dual-use)
    If, Unless, Else, ElseIf, Case, Then, While, Until, For, Begin, Rescue, Ensure, Break, Continue,
    // Members / parameters
    Parameter, Argument, Variable,
    // Expressions
    Call, Assign, Binary, Unary, Ternary, Range, Lambda, Yield, Spread, Left, Right, Expression,
    Index, Member, Match,
    // Pattern-matching
    When, In, Pattern,
    // Control-flow keyword leaves
    Next, Redo, Retry, Return,
    // Rescue / class header metadata
    Exceptions, Extends,
    // Collections / atoms (Array, Hash, String, Symbol dual-use after iter 15)
    Array, Hash, Pair, String, Interpolation, Symbol, Int, Float, Regex,
    // Literal atoms (Nil dual-use after iter 15: container + `<spread[nil]>` marker)
    True, False, Nil,
    #[strum(serialize = "self")]
    Self_,
    // Identifiers
    Name, Constant, Comment, Type,
    // Comment markers
    Trailing, Leading,
    // Spread-shape markers
    List, Dict,
    // Parameter-shape markers
    Keyword, Default, Splat, Kwsplat,
    // Block-shape / dual-use marker
    Do,
    // Symbol-shape marker
    Delimited,
    // Class / method singleton marker
    Singleton,
    // Dual-use (block container + `<parameter><block/>` marker, plus `<block[end]>` for END {...})
    Block,
    // Pattern / argument / parameter / string shape markers (iter 15)
    Alternative, As, Find, Test, Forward, Destructured, Concatenated, Static, End,
    // Safe-navigation marker on `<call>` / `<member>` for `a&.b`.
    Optional,
    // Range role wrappers (`(1..9)` / `(1...9)`) and bound-style markers.
    From, To, Inclusive, Exclusive,
}

impl TractorNode {
    pub fn as_str(self) -> &'static str {
        <&'static str>::from(self)
    }

    /// Per-name metadata. Default for unlisted variants: container with
    /// `Default` syntax.
    ///
    /// Dual-use names set BOTH `marker: true` and `container: true`:
    ///   - String — `<string>` literal container + `<array><string/>` shape marker.
    ///   - Symbol — `<symbol>` literal container + `<array><symbol/>` shape marker.
    ///   - Block  — `<block>` container (do/begin blocks) +
    ///              `<parameter><block/>` shape marker.
    ///   - Begin  — `<begin>` container + `<block><begin/>` marker.
    ///   - Do     — `<block><do/>` marker + structural `do` container
    ///              (body of while/until/for loops).
    pub fn spec(self) -> TractorNodeSpec {
        let (marker, container, syntax) = match self {
            // ---- Markers only ------------------------------------------------
            Self::Trailing | Self::Leading
            | Self::List | Self::Dict | Self::Delimited | Self::Singleton           => (true, false, Default),
            Self::Keyword | Self::Default | Self::Splat | Self::Kwsplat             => (true, false, Default),
            Self::Alternative | Self::As | Self::Find | Self::Test | Self::Forward
            | Self::Destructured | Self::Concatenated | Self::Static
            | Self::Optional
            | Self::Inclusive | Self::Exclusive                                     => (true, false, Default),
            // `End` is dual-use: marker form `<block[end]>` for the
            // `END { ... }` at-exit hook, AND a syntactic block-closer
            // keyword that legitimately appears as bare text inside
            // every Ruby compound element (`<module>...end</module>`).
            // Declaring dual-use prevents the leak detector from
            // flagging the source-text-preservation form.
            Self::End                                                               => (true, true, Keyword),

            // ---- Dual-use (marker AND container) -----------------------------
            Self::Begin | Self::Do                                                  => (true, true, Keyword),
            Self::String | Self::Symbol                                             => (true, true, String),
            Self::Block                                                             => (true, true, Default),
            // Iter 15: Array/Hash/Variable/Nil/Expression also surface as markers
            // on <pattern> (`<pattern[array]>`, `<pattern[hash]>`, `<pattern[variable]>`,
            // `<pattern[expression]>`); Nil also appears as `<spread[nil]>`.
            Self::Array | Self::Hash                                                => (true, true, Type),
            Self::Variable                                                          => (true, true, Default),
            Self::Nil                                                               => (true, true, Keyword),
            Self::Expression                                                        => (true, true, Default),

            // Bare-keyword statements: dual-use because they appear
            // as either an empty marker (bare keyword) or a container
            // (with optional content like a label/value).
            //   <break/> bare vs <break>n</break> with arg
            //   <next/> bare vs <next>label</next>
            //   <redo/>, <retry/>, <return/>, <yield/> all bare-or-content
            Self::Break | Self::Next | Self::Redo | Self::Retry | Self::Return
            | Self::Yield                                                           => (true, true, Keyword),

            // ---- Containers with non-default syntax --------------------------
            Self::Class | Self::Method
            | Self::If | Self::Unless | Self::Else | Self::ElseIf | Self::Case
            | Self::Match
            | Self::While | Self::Until | Self::For | Self::Rescue | Self::Ensure
            | Self::When
            | Self::True | Self::False | Self::Self_                                => (false, true, Keyword),
            Self::Call | Self::Lambda                                               => (false, true, Function),
            Self::Assign | Self::Binary | Self::Unary                               => (false, true, Operator),
            Self::Int | Self::Float                                                 => (false, true, Number),
            Self::Name                                                              => (false, true, Identifier),
            Self::Comment                                                           => (false, true, Comment),

            // ---- Default: container with Default syntax ----------------------
            _                                                                       => (false, true, Default),
        };
        TractorNodeSpec { name: self.as_str(), marker, container, syntax }
    }
}

static NODES_TABLE: Lazy<Vec<TractorNodeSpec>> =
    Lazy::new(|| TractorNode::iter().map(|n| n.spec()).collect());

pub fn nodes() -> &'static [TractorNodeSpec] {
    NODES_TABLE.as_slice()
}

pub fn spec(name: &str) -> Option<&'static TractorNodeSpec> {
    let parsed: TractorNode = name.parse().ok()?;
    let target = parsed.as_str();
    NODES_TABLE.iter().find(|s| s.name == target)
}

pub fn all_names() -> impl Iterator<Item = &'static str> {
    TractorNode::iter().map(TractorNode::as_str)
}

pub fn is_marker_only(name: &str) -> bool {
    spec(name).map_or(false, |s| s.marker && !s.container)
}

pub fn is_declared(name: &str) -> bool {
    spec(name).is_some()
}

#[allow(dead_code)]
const _SYNTAX_CATEGORY_USED: Option<SyntaxCategory> = None;
