//! Output element names — tractor's Go XML vocabulary after transform.
//! These are the names that appear in tractor's output. The tree-sitter
//! kind strings are external vocabulary, surfaced as the typed
//! [`super::input::GoKind`] enum. The kind→output table lives in
//! [`super::rules::rule`].
//!
//! Each variant of [`GoName`] is one element name the Go transform can
//! emit. The wire string is the variant's snake_case form (via strum).
//! Per-name metadata (marker / container role, syntax-highlight
//! category) is computed in [`GoName::spec`] using a default-valued
//! match — most names are containers with `Default` syntax, and only
//! the exceptions need explicit arms.

use once_cell::sync::Lazy;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, EnumIter, EnumString, IntoStaticStr};

use crate::languages::NodeSpec;
use crate::output::syntax_highlight::SyntaxCategory::{self, *};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, IntoStaticStr, AsRefStr, EnumIter,
)]
#[strum(serialize_all = "snake_case")]
pub enum GoName {
    // Top-level / structural
    File, Package, Import,
    // Declarations (Function, Type are dual-use marker/container)
    Function, Method, Type, Struct, Interface, Const, Var, Alias, Variable,
    // Members / parameters
    Field, Parameter, Arguments,
    // Types
    Pointer, Slice, Map, Chan,
    // Statements / control flow
    Return, If, Else, ElseIf, For, Range, Switch, Case, Default, Defer, Go, Select,
    Break, Continue, Goto, Labeled, Label, Send, Receive, Assign,
    // Expressions
    Call, Member, Index, Binary, Unary, Assert, Closure, Literal,
    // Literals / atoms
    String, Int, Float, Char, True, False, Nil, Iota,
    // Identifiers / comments / op
    Name, Comment, Op,
    // Comment markers
    Trailing, Leading,
    // Marker-only
    Raw, Short, Exported, Unexported, Negated, Generic,
}

impl GoName {
    pub fn as_str(self) -> &'static str {
        <&'static str>::from(self)
    }

    /// Per-name metadata — `marker`/`container` role + syntax category.
    /// Default for unlisted variants: container with `Default` syntax.
    ///
    /// Dual-use names set BOTH `marker: true` and `container: true`:
    ///   - Function — function_declaration (container) vs function_type
    ///                (marker on `<type>`).
    ///   - Type     — type wrapper (container) vs type_switch_statement
    ///                emits `<switch><type/>…>` (marker).
    pub fn spec(self) -> NodeSpec {
        let (marker, container, syntax) = match self {
            // ---- Markers only ------------------------------------------------
            Self::Trailing | Self::Leading
            | Self::Raw | Self::Short | Self::Negated | Self::Generic              => (true, false, Default),
            Self::Exported | Self::Unexported                                      => (true, false, Keyword),

            // ---- Dual-use (marker AND container) -----------------------------
            Self::Function                                                          => (true, true, Keyword),
            Self::Type                                                              => (true, true, Type),

            // ---- Containers with non-default syntax --------------------------
            Self::Package | Self::Import
            | Self::Method | Self::Struct | Self::Interface | Self::Const | Self::Var
            | Self::Parameter
            | Self::Return | Self::If | Self::Else | Self::For | Self::Range
            | Self::Case | Self::Default | Self::Defer | Self::Go | Self::Select
            | Self::Break | Self::Continue | Self::Goto
            | Self::True | Self::False | Self::Nil                                  => (false, true, Keyword),
            Self::Pointer | Self::Slice | Self::Map | Self::Chan                    => (false, true, Type),
            Self::Call                                                              => (false, true, Function),
            Self::Binary | Self::Unary | Self::Op                                   => (false, true, Operator),
            Self::String                                                            => (false, true, String),
            Self::Int | Self::Float                                                 => (false, true, Number),
            Self::Name                                                              => (false, true, Identifier),
            Self::Comment                                                           => (false, true, Comment),

            // ---- Default: container with Default syntax ----------------------
            _                                                                       => (false, true, Default),
        };
        NodeSpec { name: self.as_str(), marker, container, syntax }
    }
}

static NODES_TABLE: Lazy<Vec<NodeSpec>> =
    Lazy::new(|| GoName::iter().map(|n| n.spec()).collect());

/// Snapshot slice over every declared name's `NodeSpec`. Kept for the
/// `go_node_metadata_is_well_formed` invariant test.
pub fn nodes() -> &'static [NodeSpec] {
    NODES_TABLE.as_slice()
}

pub fn spec(name: &str) -> Option<&'static NodeSpec> {
    let parsed: GoName = name.parse().ok()?;
    let target = parsed.as_str();
    NODES_TABLE.iter().find(|s| s.name == target)
}

pub fn all_names() -> impl Iterator<Item = &'static str> {
    GoName::iter().map(GoName::as_str)
}

pub fn is_marker_only(name: &str) -> bool {
    spec(name).map_or(false, |s| s.marker && !s.container)
}

pub fn is_declared(name: &str) -> bool {
    spec(name).is_some()
}

#[allow(dead_code)]
const _SYNTAX_CATEGORY_USED: Option<SyntaxCategory> = None;
