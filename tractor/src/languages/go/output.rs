//! Output element names — tractor's Go XML vocabulary after transform.
//! These are the names that appear in tractor's output. The tree-sitter
//! kind strings are external vocabulary, surfaced as the typed
//! [`super::input::GoKind`] enum. The kind→output table lives in
//! [`super::rules::rule`].
//!
//! Each variant of [`TractorNode`] is one element name the Go transform can
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
    File, Package, Import,
    // Declarations (Function, Type are dual-use marker/container)
    Function, Method, Type, Struct, Interface, Const, Var, Alias, Aliased, Variable,
    // Members / parameters
    Field, Parameter, Arguments, Spread, Pair,
    // Types (Slice dual-use after iter 12: type vs. slice-expression marker)
    Pointer, Slice, Map, Chan, Array,
    // Inheritance / interface embedding (cross-language)
    Extends,
    // Statements / control flow.
    // NOTE: `ElseIf` serializes as `else_if` — intentional
    // underscore. See `transform/conditionals.rs` and design.md
    // § 17 (canonical allowed-exception). Do not rename.
    Return, If, Else, ElseIf, For, Range, Switch, Case, Default, Defer, Go, Select,
    Break, Continue, Goto, Labeled, Label, Send, Receive, Assign, Fallthrough,
    // Expressions
    Call, Member, Object, Property, Index, Binary, Unary, Assert, Closure, Literal, Expression,
    // Literals / atoms
    String, Int, Float, Char, True, False, Nil, Iota,
    // Identifiers / comments / op
    Name, Comment, Op,
    // Comment markers
    Trailing, Leading,
    // Import-shape (Path, Alias as containers; Blank, Dot as markers)
    Path, Blank, Dot,
    // Marker-only
    Raw, Short, Exported, Unexported, Approximation, Generic, Implicit,
    // Chain inversion (iter 242): `[access]` marker distinguishes
    // member-access chains (`obj.method()`) from object literals.
    Access,
}

impl TractorNode {
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
    ///                emits `<switch><type/>…>` (marker), and
    ///                `type_conversion_expression` emits `<call><type/>…>`.
    ///   - Slice    — slice_type (container `<slice>`) vs slice_expression
    ///                (marker `<index><slice/>…`).
    pub fn spec(self) -> TractorNodeSpec {
        let (marker, container, syntax) = match self {
            // ---- Markers only ------------------------------------------------
            Self::Trailing | Self::Leading
            | Self::Raw | Self::Short | Self::Approximation
            | Self::Implicit
            | Self::Blank | Self::Dot
            | Self::Access                                                          => (true, false, Default),
            // `Generic` is dual-use: marker `[generic]` on `<type>`
            // for generic-type instantiations AND container `<generic>`
            // for type-parameter declarations.
            Self::Generic                                                           => (true, true, Default),
            Self::Exported | Self::Unexported                                      => (true, false, Keyword),

            // ---- Dual-use (marker AND container) -----------------------------
            Self::Function                                                          => (true, true, Keyword),
            Self::Type | Self::Slice                                                => (true, true, Type),
            // `Alias` is dual-use: marker `[alias]` on `<import>` for
            // `import myio "io"` AND container `<alias><name>myio</name></alias>`
            // for the local-binding wrapper. See
            // specs/.../transformations/imports-grouping.md.
            Self::Alias                                                             => (true, true, Default),

            // Bare-keyword statements: dual-use (empty marker OR
            // container with content). `return` / `break` / `continue`
            // are bare keyword leaves when no value/label, full
            // containers otherwise. Same for `goto label`,
            // `fallthrough`.
            Self::Return | Self::Break | Self::Continue | Self::Goto
            | Self::Fallthrough                                                     => (true, true, Keyword),

            // ---- Containers with non-default syntax --------------------------
            Self::Package | Self::Import
            | Self::Method | Self::Struct | Self::Interface | Self::Const | Self::Var
            | Self::Parameter
            | Self::If | Self::Else | Self::For | Self::Range
            | Self::Case | Self::Default | Self::Defer | Self::Go | Self::Select
            | Self::True | Self::False | Self::Nil                                  => (false, true, Keyword),
            Self::Path                                                              => (false, true, Default),
            Self::Pointer | Self::Map | Self::Chan | Self::Array                    => (false, true, Type),
            Self::Call                                                              => (false, true, Function),
            Self::Binary | Self::Unary | Self::Op                                   => (false, true, Operator),
            Self::String                                                            => (false, true, String),
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

/// Snapshot slice over every declared name's `TractorNodeSpec`. Kept for the
/// `go_node_metadata_is_well_formed` invariant test.
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
