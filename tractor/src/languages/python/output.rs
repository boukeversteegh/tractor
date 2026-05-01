//! Output element names — tractor's Python XML vocabulary after transform.

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
    Module, Class, Function, Decorated, Decorator, Lambda, Alias,
    // Members
    Parameter, Argument,
    // Type vocabulary
    Type,
    // Control flow
    Return, If, ElseIf, Else, For, While, Try, Except, Finally, With, Raise, Pass, Break,
    Continue, Match, Arm, Pattern,
    // Imports / names
    Import, From, Assert, Delete, Global, Nonlocal,
    // Python 2 leftovers (kept as own elements; rare in modern code)
    Exec, Print,
    // Expressions
    Call, Member, Subscript, Assign, Binary, Unary, Compare, Logical, Await, Yield, Generator,
    Ternary, Cast, As, Spread, Format, Tuple, Generic, Pair, Interpolation, Expression,
    // Function-signature separators
    Keyword, Positional,
    // Collection containers (dual-use marker/container)
    List, Dict, Set,
    // Literals
    String, Int, Float, True, False, None,
    // Identifiers / comments / op
    Name, Comment, Op,
    // Comment markers
    Trailing, Leading,
    // Visibility (markers only)
    Public, Private, Protected,
    // Function flags
    Async,
    // Collection-construction markers
    Literal, Comprehension,
    // Pattern / type / import / string shape markers
    Union, Splat, Kwsplat, Constrained, Complex, Group, Future, Wildcard, Concatenated, Escape,
    // Import-shape (Path container; Relative marker for `from . import`)
    Path, Relative,
}

impl TractorNode {
    pub fn as_str(self) -> &'static str {
        <&'static str>::from(self)
    }

    /// Per-name metadata. Default for unlisted variants: container with
    /// `Default` syntax.
    ///
    /// Dual-use names (List, Dict, Set) — structural container for a
    /// collection literal, but also emitted as pattern / spread markers.
    pub fn spec(self) -> TractorNodeSpec {
        let (marker, container, syntax) = match self {
            // ---- Markers only ------------------------------------------------
            Self::Trailing | Self::Leading
            | Self::Union | Self::Splat | Self::Kwsplat
            | Self::Constrained | Self::Complex | Self::Group | Self::Future
            | Self::Wildcard | Self::Concatenated | Self::Escape
            | Self::Relative                                                      => (true, false, Default),
            Self::Public | Self::Private | Self::Protected
            | Self::Async | Self::Literal | Self::Comprehension
            | Self::Await                                                        => (true, false, Keyword),

            // ---- Dual-use (marker AND container) -----------------------------
            Self::List | Self::Dict | Self::Set                                  => (true, true, Default),
            // Alias dual-use: `[alias]` marker on `<import>` for
            // `import x as y` AND `<alias><name>y</name></alias>` for
            // the local-binding wrapper (matches Go/Rust/PHP/TS shape).
            Self::Alias                                                          => (true, true, Default),
            // Generic: container for `Foo[T, U]` (with type children) +
            // marker for empty / wildcard generics.
            // Tuple: container for `(a, b)` literal + marker for empty `()`.
            Self::Generic | Self::Tuple                                          => (true, true, Default),
            // Class: dual-use — container for `class Foo:` declaration;
            // marker for class-pattern matches (`case Point(x=0, y=y):`).
            Self::Class                                                          => (true, true, Keyword),

            // Bare-keyword statements: dual-use (empty marker OR
            // container). `pass` is always bare; `break`/`continue`/
            // `return` are bare without value.
            Self::Pass | Self::Break | Self::Continue | Self::Return
            | Self::Yield                                                        => (true, true, Keyword),

            // ---- Containers with non-default syntax --------------------------
            Self::Module | Self::Function | Self::Decorated
            | Self::Decorator | Self::Parameter
            | Self::If | Self::ElseIf | Self::Else | Self::For
            | Self::While | Self::Try | Self::Except | Self::Finally | Self::With
            | Self::Raise
            | Self::Import | Self::From
            | Self::Exec | Self::Print
            | Self::Generator
            | Self::True | Self::False | Self::None                              => (false, true, Keyword),
            Self::Type                                                           => (false, true, Type),
            Self::Path                                                           => (false, true, Default),
            Self::Lambda | Self::Call                                            => (false, true, Function),
            Self::Assign | Self::Binary | Self::Unary | Self::Compare
            | Self::Logical | Self::Ternary | Self::Op                           => (false, true, Operator),
            Self::String                                                         => (false, true, String),
            Self::Int | Self::Float                                              => (false, true, Number),
            Self::Name                                                           => (false, true, Identifier),
            Self::Comment                                                        => (false, true, Comment),

            // ---- Default: container with Default syntax ----------------------
            _                                                                    => (false, true, Default),
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
