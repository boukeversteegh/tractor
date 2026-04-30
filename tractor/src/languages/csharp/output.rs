//! Output element names — tractor's C# XML vocabulary after transform.
//! These are the names that appear in tractor's output and that the
//! renderer reads. The tree-sitter kind strings are external vocabulary,
//! surfaced as the typed [`super::input::CsKind`] enum. The kind→output
//! table lives in [`super::rules::rule`].
//!
//! Each variant of [`CsName`] is one element name the C# transform can
//! emit. The wire string is the variant's snake_case form (via strum).
//! Per-name metadata (marker / container role, syntax-highlight
//! category) is computed in [`CsName::spec`] using a default-valued
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
pub enum CsName {
    // Top-level / structural
    Unit, Namespace, Import, Body,
    // Type declarations
    Class, Struct, Interface, Enum, Record,
    // Members
    Method, Constructor, Property, Field, Comment, Event, Delegate, Destructor, Indexer, Operator,
    // Shared children
    Name, Type, Accessors, Accessor, Attributes, Attribute, Arguments, Argument,
    Parameters, Parameter, Variable, Declarator, Extends, Properties, Element, Section, Arm,
    Label, Chain, Filter, When, Where,
    // Statements / control flow
    Return, If, Else, ElseIf, For, Foreach, While, Do, Try, Catch, Finally, Throw,
    Using, Break, Continue, Switch, Block, Expression, Range,
    // Expressions
    Call, Member, New, Assign, Binary, Unary, Lambda, Await, Ternary, Index, Is,
    Tuple, Literal, Pattern,
    // Generics (dual-use marker/container)
    Generic,
    // LINQ
    Query, From, Select, Order, Group, Let, Join, Ordering,
    // Dual-use container/pattern marker
    Constant, Declaration,
    // Literals / atoms
    String, Interpolation, Int, Float, Bool, Null,
    // Patterns / leaves
    Subpattern, Discard, Modifier, Op,
    // Marker-only type shape
    Nullable,
    // Comment markers
    Trailing, Leading,
    // Marker-only: member-access / pattern / type shape
    Instance, Conditional, Array, Pointer, Function, Ref, Recursive, Relational, Logical,
    Prefix, Lookup,
    // Access modifiers (markers only)
    Public, Private, Protected, Internal,
    // Other modifiers (markers only); CONST is dual-use container/marker.
    Static, Abstract, Virtual, Override, Sealed, Readonly, Const, Partial, Async, Extern, Unsafe, This,
    // Accessor declarations
    Get, Set, Init, Add, Remove,
    // Generic-constraint markers
    Notnull, Unmanaged,
}

impl CsName {
    /// Wire string for this name (snake_case via strum).
    pub fn as_str(self) -> &'static str {
        <&'static str>::from(self)
    }

    /// Per-name metadata — `marker`/`container` role + syntax category.
    /// Default for unlisted variants: container with `Default` syntax.
    pub fn spec(self) -> NodeSpec {
        let (marker, container, syntax) = match self {
            // ---- Markers only ------------------------------------------------
            // Modifiers
            Self::Public | Self::Private | Self::Protected | Self::Internal
            | Self::Static | Self::Abstract | Self::Virtual | Self::Override | Self::Sealed
            | Self::Readonly | Self::Partial | Self::Async | Self::Extern | Self::Unsafe
            | Self::This                                                        => (true, false, Keyword),
            // Type-shape / member-access / pattern markers
            Self::Nullable | Self::Array                                        => (true, false, Type),
            Self::Trailing | Self::Leading
            | Self::Instance | Self::Conditional | Self::Pointer | Self::Function
            | Self::Ref | Self::Recursive | Self::Relational | Self::Prefix | Self::Lookup
            | Self::Notnull | Self::Unmanaged                                   => (true, false, Default),

            // ---- Dual-use (marker AND container) -----------------------------
            Self::New | Self::Const                                             => (true, true,  Keyword),
            Self::Generic                                                       => (true, true,  Type),
            Self::Logical                                                       => (true, true,  Operator),
            Self::Tuple | Self::Constant | Self::Declaration                    => (true, true,  Default),

            // ---- Containers with non-default syntax --------------------------
            Self::Namespace | Self::Import
            | Self::Class | Self::Struct | Self::Interface | Self::Enum | Self::Record
            | Self::Return | Self::If | Self::Else | Self::For | Self::Foreach | Self::While
            | Self::Do | Self::Try | Self::Catch | Self::Finally | Self::Throw
            | Self::Using | Self::Break | Self::Continue | Self::Await
            | Self::Bool | Self::Null
            | Self::Get | Self::Set | Self::Init | Self::Add | Self::Remove     => (false, true, Keyword),
            Self::Comment                                                       => (false, true, Comment),
            Self::Name                                                          => (false, true, Identifier),
            Self::Type | Self::Attributes | Self::Attribute                     => (false, true, Type),
            Self::Assign | Self::Binary | Self::Unary | Self::Ternary | Self::Op => (false, true, Operator),
            Self::Lambda                                                        => (false, true, Function),
            Self::String                                                        => (false, true, String),
            Self::Int | Self::Float                                             => (false, true, Number),

            // ---- Default: container with Default syntax ----------------------
            _                                                                   => (false, true, Default),
        };
        NodeSpec { name: self.as_str(), marker, container, syntax }
    }
}

/// Materialised table of every name's `NodeSpec`. Built once on first
/// access via `CsName::iter()`. Used by `spec` for `&'static NodeSpec`
/// lookup, and exposed as `NODES` for the catalogue test.
static NODES_TABLE: Lazy<Vec<NodeSpec>> =
    Lazy::new(|| CsName::iter().map(|n| n.spec()).collect());

/// Snapshot slice over every declared name's `NodeSpec`. Kept for the
/// `csharp_node_metadata_is_well_formed` invariant test.
pub fn nodes() -> &'static [NodeSpec] {
    NODES_TABLE.as_slice()
}

/// Look up a node spec by name. Returns `None` if `name` is not a
/// declared variant. `&'static NodeSpec` because `NODES_TABLE` lives
/// forever once built.
pub fn spec(name: &str) -> Option<&'static NodeSpec> {
    let parsed: CsName = name.parse().ok()?;
    let target = parsed.as_str();
    NODES_TABLE.iter().find(|s| s.name == target)
}

/// Iterate every declared semantic name's wire string.
pub fn all_names() -> impl Iterator<Item = &'static str> {
    CsName::iter().map(CsName::as_str)
}

/// True iff `name` is declared as a pure marker (never a container).
pub fn is_marker_only(name: &str) -> bool {
    spec(name).map_or(false, |s| s.marker && !s.container)
}

/// True iff `name` is declared in this language's vocabulary.
pub fn is_declared(name: &str) -> bool {
    spec(name).is_some()
}

// Silence unused-import warnings when the only use of `SyntaxCategory`
// is via the `*` glob import above.
#[allow(dead_code)]
const _SYNTAX_CATEGORY_USED: Option<SyntaxCategory> = None;
