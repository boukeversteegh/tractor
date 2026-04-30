//! Output element names — tractor's Python XML vocabulary after transform.

use once_cell::sync::Lazy;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, EnumIter, EnumString, IntoStaticStr};

use crate::languages::NodeSpec;
use crate::output::syntax_highlight::SyntaxCategory::{self, *};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, IntoStaticStr, AsRefStr, EnumIter,
)]
#[strum(serialize_all = "snake_case")]
pub enum PyName {
    // Top-level / declarations
    Module, Class, Function, Decorated, Decorator, Lambda,
    // Members
    Parameter, Argument,
    // Type vocabulary
    Type,
    // Control flow
    Return, If, ElseIf, Else, For, While, Try, Except, Finally, With, Raise, Pass, Break,
    Continue, Match, Arm, Pattern,
    // Imports / names
    Import, From, Assert, Delete, Global, Nonlocal,
    // Expressions
    Call, Member, Subscript, Assign, Binary, Unary, Compare, Logical, Await, Yield, Generator,
    Ternary, Cast, As, Spread, Format, Tuple, Generic, Pair, Interpolation,
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
    // Pattern / type shape markers
    Union, Splat,
}

impl PyName {
    pub fn as_str(self) -> &'static str {
        <&'static str>::from(self)
    }

    /// Per-name metadata. Default for unlisted variants: container with
    /// `Default` syntax.
    ///
    /// Dual-use names (List, Dict, Set) — structural container for a
    /// collection literal, but also emitted as pattern / spread markers.
    pub fn spec(self) -> NodeSpec {
        let (marker, container, syntax) = match self {
            // ---- Markers only ------------------------------------------------
            Self::Trailing | Self::Leading
            | Self::Union | Self::Splat                                          => (true, false, Default),
            Self::Public | Self::Private | Self::Protected
            | Self::Async | Self::Literal | Self::Comprehension                  => (true, false, Keyword),

            // ---- Dual-use (marker AND container) -----------------------------
            Self::List | Self::Dict | Self::Set                                  => (true, true, Default),

            // ---- Containers with non-default syntax --------------------------
            Self::Module | Self::Class | Self::Function | Self::Decorated
            | Self::Decorator | Self::Parameter
            | Self::Return | Self::If | Self::ElseIf | Self::Else | Self::For
            | Self::While | Self::Try | Self::Except | Self::Finally | Self::With
            | Self::Raise | Self::Pass | Self::Break | Self::Continue
            | Self::Import | Self::From
            | Self::Await | Self::Yield | Self::Generator
            | Self::True | Self::False | Self::None                              => (false, true, Keyword),
            Self::Type                                                           => (false, true, Type),
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
        NodeSpec { name: self.as_str(), marker, container, syntax }
    }
}

static NODES_TABLE: Lazy<Vec<NodeSpec>> =
    Lazy::new(|| PyName::iter().map(|n| n.spec()).collect());

pub fn nodes() -> &'static [NodeSpec] {
    NODES_TABLE.as_slice()
}

pub fn spec(name: &str) -> Option<&'static NodeSpec> {
    let parsed: PyName = name.parse().ok()?;
    let target = parsed.as_str();
    NODES_TABLE.iter().find(|s| s.name == target)
}

pub fn all_names() -> impl Iterator<Item = &'static str> {
    PyName::iter().map(PyName::as_str)
}

pub fn is_marker_only(name: &str) -> bool {
    spec(name).map_or(false, |s| s.marker && !s.container)
}

pub fn is_declared(name: &str) -> bool {
    spec(name).is_some()
}

#[allow(dead_code)]
const _SYNTAX_CATEGORY_USED: Option<SyntaxCategory> = None;
