//! Output element names — tractor's C# XML vocabulary after transform.
//! These are the names that appear in tractor's output and that the
//! renderer reads. The tree-sitter kind strings are external vocabulary,
//! surfaced as the typed [`super::input::CsKind`] enum. The kind→output
//! table lives in [`super::rules::rule`].
//!
//! Each variant of [`TractorNode`] is one element name the C# transform can
//! emit. The wire string is the variant's snake_case form (via strum).
//! Per-name metadata (marker / container role, syntax-highlight
//! category) is computed in [`TractorNode::spec`] using a default-valued
//! match — most names are containers with `Default` syntax, and only
//! the exceptions need explicit arms.

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
    // Top-level / structural
    Unit, Namespace, Import, Body,
    // Type declarations
    Class, Struct, Interface, Enum, Record,
    // Members
    Method, Constructor, Property, Field, Comment, Event, Delegate, Destructor, Indexer, Operator,
    // Shared children
    Name, Type, Accessors, Accessor, Attributes, Attribute, Arguments, Argument,
    Parameters, Parameter, Variable, Declarator, Extends, Properties, Element, Section, Arm, Value,
    Label, Filter, When, Where, Path,
    // Statements / control flow.
    // NOTE: `ElseIf` serializes as `else_if` — intentional
    // underscore. See `transform/conditionals.rs` and design.md
    // § 17 (canonical allowed-exception). Do not rename.
    Return, If, Else, ElseIf, For, Foreach, While, Do, Try, Catch, Finally, Throw,
    Using, Break, Continue, Switch, Block, Expression, Range,
    // Statement additions
    Yield, Checked, Fixed, Goto, Lock, Default, With,
    // Expressions
    Call, Member, Object, New, Assign, Binary, Unary, Lambda, Await, Ternary, Index, Is,
    Tuple, Literal, Pattern, NonNull,
    // Expression additions
    Cast, Typeof, Sizeof,
    // Generics (dual-use marker/container)
    Generic,
    // LINQ
    Query, From, Select, Order, Group, Let, Join, Ordering,
    // LINQ addition
    Into,
    // Dual-use container/pattern marker
    Constant, Declaration,
    // Generic-constraint container
    Constraint,
    // Literals / atoms
    String, Interpolation, Int, Float, Bool, Null,
    // Literal addition
    Char,
    // Patterns / leaves
    Subpattern, Discard, Modifier, Op,
    // Marker-only type shape
    Nullable,
    // Comment markers
    Trailing, Leading,
    // Marker-only: member-access / pattern / type shape
    Instance, Optional, Array, Pointer, Function, Ref, Recursive, Relational, Logical,
    Prefix, Lookup,
    // File-scoped namespace marker (`namespace Foo;` form)
    File,
    // Pattern-combinator markers
    And, Or, Negated, List, Var,
    // Creation / memory markers
    Anonymous, Stackalloc,
    // Access modifiers (markers only)
    Public, Private, Protected, Internal,
    // Other modifiers (markers only); CONST is dual-use container/marker.
    Static, Abstract, Virtual, Override, Sealed, Readonly, Const, Partial, Async, Extern, Unsafe, This,
    // Accessor declarations
    Get, Set, Init, Add, Remove,
    // Generic-constraint markers
    Notnull, Unmanaged,
    // Constructor-call target marker — `[base]` on `<call>` for
    // `: base(...)` (the `[this]` form reuses the existing This marker).
    // The `base` keyword appears literally in C# source so the
    // marker name matches it (Principle #1).
    Base,
    // Marker on `<type>` for an enum's underlying integral type
    // (`enum Color : uint`). Distinct from `<extends>` since enums
    // don't inherit; the slot is "storage type", not "parent type".
    Underlying,
    // Chain inversion (iter 245): `[access]` distinguishes
    // member-access chains from object literals.
    Access,
}

impl TractorNode {
    /// Wire string for this name (snake_case via strum).
    pub fn as_str(self) -> &'static str {
        <&'static str>::from(self)
    }

    /// Per-name metadata — `marker`/`container` role + syntax category.
    /// Default for unlisted variants: container with `Default` syntax.
    pub fn spec(self) -> TractorNodeSpec {
        let (marker, container, syntax) = match self {
            // ---- Markers only ------------------------------------------------
            // Modifiers
            Self::Public | Self::Private | Self::Protected | Self::Internal
            | Self::Static | Self::Abstract | Self::Virtual | Self::Override | Self::Sealed
            | Self::Readonly | Self::Partial | Self::Async | Self::Extern | Self::Unsafe
            | Self::Await                                                       => (true, false, Keyword),
            Self::NonNull                                                       => (true, false, Operator),
            // Type-shape / member-access / pattern markers
            Self::Nullable | Self::Array                                        => (true, false, Type),
            Self::Trailing | Self::Leading
            | Self::Instance | Self::Optional | Self::Pointer | Self::Function
            | Self::Ref | Self::Recursive | Self::Relational | Self::Prefix | Self::Lookup
            | Self::Notnull | Self::Unmanaged
            | Self::And | Self::Or | Self::Negated | Self::List | Self::Var
            | Self::Anonymous | Self::Stackalloc | Self::File
            | Self::Base | Self::Underlying
            | Self::Access                                                      => (true, false, Default),

            // ---- Dual-use (marker AND container) -----------------------------
            Self::New | Self::Const                                             => (true, true,  Keyword),
            Self::Generic                                                       => (true, true,  Type),
            Self::Logical                                                       => (true, true,  Operator),
            Self::Tuple | Self::Constant | Self::Declaration                    => (true, true,  Default),

            // Bare-keyword statements: dual-use (empty marker OR
            // container with content). `throw;` (rethrow) and
            // `return;` / `break;` / `continue;` are bare; with
            // value or label they're containers.
            Self::Throw | Self::Return | Self::Break | Self::Continue
            | Self::Yield                                                       => (true, true, Keyword),

            // Class / Struct: dual-use — container for `class Foo {…}` /
            // `struct Foo {…}` declaration; marker for generic
            // constraints `where T : class` / `where T : struct`.
            // Surfaced by the shape-contract `container-has-content`
            // rule (iter 295) when run on `where T : struct`-style
            // constraints; the marker form had been declared
            // ContainerOnly. Same archetype as iter 294's Rust
            // Crate/Super fix.
            Self::Class | Self::Struct                                          => (true, true, Keyword),
            // `This` is dual-use: marker form `[this]` on extension-method
            // receiver parameter (`public static void M(this string s)`),
            // AND a name-like self-reference that legitimately appears as
            // bare text in expressions (`this.Foo`, `==this`, `this`-as-
            // argument). Declaring dual-use exempts the source-text form
            // from the keyword-leak detector.
            Self::This                                                          => (true, true, Keyword),

            // ---- Containers with non-default syntax --------------------------
            Self::Namespace | Self::Import
            | Self::Interface | Self::Enum | Self::Record
            | Self::If | Self::Else | Self::For | Self::Foreach | Self::While
            | Self::Do | Self::Try | Self::Catch | Self::Finally
            | Self::Using
            | Self::Checked | Self::Fixed | Self::Goto | Self::Lock
            | Self::Default | Self::With | Self::Typeof | Self::Sizeof
            | Self::Bool | Self::Null
            | Self::Get | Self::Set | Self::Init | Self::Add | Self::Remove     => (false, true, Keyword),
            Self::Cast                                                          => (false, true, Operator),
            Self::Char                                                          => (false, true, String),
            Self::Comment                                                       => (false, true, Comment),
            Self::Name                                                          => (false, true, Identifier),
            // Type: dual-use — `<type>` container for type references
            // (e.g. `<type><name>string</name></type>`) and `[type]`
            // marker on `<pattern>` (`pattern[type]`).
            Self::Type                                                          => (true, true, Type),
            Self::Attributes | Self::Attribute                                  => (false, true, Type),
            Self::Assign | Self::Binary | Self::Unary | Self::Ternary | Self::Op => (false, true, Operator),
            Self::Lambda                                                        => (false, true, Function),
            Self::String                                                        => (false, true, String),
            Self::Int | Self::Float                                             => (false, true, Number),

            // ---- Default: container with Default syntax ----------------------
            _                                                                   => (false, true, Default),
        };
        TractorNodeSpec { name: self.as_str(), marker, container, syntax }
    }
}

/// Materialised table of every name's `TractorNodeSpec`. Built once on first
/// access via `TractorNode::iter()`. Used by `spec` for `&'static TractorNodeSpec`
/// lookup, and exposed as `NODES` for the catalogue test.
static NODES_TABLE: Lazy<Vec<TractorNodeSpec>> =
    Lazy::new(|| TractorNode::iter().map(|n| n.spec()).collect());

/// Snapshot slice over every declared name's `TractorNodeSpec`. Kept for the
/// `csharp_node_metadata_is_well_formed` invariant test.
pub fn nodes() -> &'static [TractorNodeSpec] {
    NODES_TABLE.as_slice()
}

/// Look up a node spec by name. Returns `None` if `name` is not a
/// declared variant. `&'static TractorNodeSpec` because `NODES_TABLE` lives
/// forever once built.
pub fn spec(name: &str) -> Option<&'static TractorNodeSpec> {
    let parsed: TractorNode = name.parse().ok()?;
    let target = parsed.as_str();
    NODES_TABLE.iter().find(|s| s.name == target)
}

/// Iterate every declared semantic name's wire string.
pub fn all_names() -> impl Iterator<Item = &'static str> {
    TractorNode::iter().map(TractorNode::as_str)
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
