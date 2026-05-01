//! Output element names — tractor's Rust XML vocabulary after transform.

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
    // Top-level / declarations (Function, Struct, Trait, Const, Macro dual-use)
    File, Function, Impl, Struct, Enum, Trait, Mod, Use, Const, Static, Alias, Signature, Modifiers,
    Union,
    // Members (Field dual-use)
    Parameter,
    #[strum(serialize = "self")]
    Self_,
    Field, Variant, Lifetime, Attribute,
    // Types / generics (Generic dual-use)
    Type, Generic, Generics, Path, Bounds, Bound, Where,
    // Statements / control flow
    Let, Return, If, Else, ElseIf, For, While, Loop, Match, Arm, Pattern, Break, Continue, Range,
    Send, Label, Yield,
    // Expressions (Ref, Tuple, Array dual-use; Await/Try are markers)
    Call, Index, Binary, Unary, Assign, Closure, Await, Try, Macro, Cast, Ref, Tuple, Unsafe,
    Literal, Block, Expression,
    // Macro grammar
    Repetition, Fragment,
    // Visibility
    Pub, In,
    // Literals / atoms
    String, Int, Float, Bool, Char,
    // Identifiers / comments / op
    Name, Comment, Op,
    // Comment markers
    Trailing, Leading,
    // Marker-only
    Raw, Inner, Borrowed, Private, Crate, Super, Mut, Async, Pointer, Never, Unit, Dynamic,
    Abstract, Associated, Bounded, Array, Or, Method, Base, Slice,
    // Iter 16: pattern / macro / item / type / expression markers
    Capture, Rest, Binding, Definition, Extern, Foreign, Gen, Higher, Optional, Turbofish, Variadic,
    Negative,
    // Import-shape markers (Group, Wildcard, Reexport on `<use>`)
    Group, Wildcard, Reexport,
}

impl TractorNode {
    pub fn as_str(self) -> &'static str {
        <&'static str>::from(self)
    }

    /// Per-name metadata. Default for unlisted variants: container with
    /// `Default` syntax.
    ///
    /// Dual-use names set BOTH `marker: true` and `container: true`:
    ///   - Function — function_item (container) vs function_type (marker)
    ///   - Tuple    — tuple_expression (container) vs tuple_pattern + tuple_type (markers)
    ///   - Trait    — trait_item (container) vs trait_type (marker)
    ///   - Ref      — reference_expression (container) vs reference_pattern + ref_pattern (markers)
    ///   - Field    — field_expression / field_declaration (container) vs
    ///                field_pattern / base_field_initializer (markers)
    ///   - Struct   — struct_item (container) vs struct_pattern (marker)
    ///   - Generic  — generic_type (container) vs generic_function + generic_pattern (markers)
    ///   - Const    — const_item (container) vs const_block + const_parameter (markers)
    ///   - Macro    — macro_definition / macro_invocation (container) — Definition
    ///                marker distinguishes the two when needed.
    ///   - Array    — array_expression (container) vs array_type + slice_pattern (markers).
    ///
    /// Marker-only (under stable expression hosts, principle #15):
    ///   - Try   — try_block + try_expression both attach as `<try/>` marker.
    ///   - Await — `await_expression` attaches as `<await/>` marker.
    pub fn spec(self) -> TractorNodeSpec {
        let (marker, container, syntax) = match self {
            // ---- Markers only ------------------------------------------------
            Self::Trailing | Self::Leading
            | Self::Raw | Self::Inner | Self::Borrowed
            | Self::Pointer | Self::Never | Self::Unit | Self::Dynamic
            | Self::Abstract | Self::Associated | Self::Bounded | Self::Or
            | Self::Method | Self::Base | Self::Slice                                  => (true, false, Default),
            Self::Capture | Self::Rest | Self::Binding | Self::Definition
            | Self::Foreign | Self::Higher | Self::Optional | Self::Turbofish
            | Self::Variadic | Self::Negative
            | Self::Group | Self::Wildcard | Self::Reexport                            => (true, false, Default),
            Self::Private | Self::Crate | Self::Super | Self::Mut | Self::Async
            | Self::Await | Self::Extern | Self::Gen                                   => (true, false, Keyword),
            Self::Try                                                                  => (true, false, Operator),

            // ---- Dual-use (marker AND container) -----------------------------
            Self::Function | Self::Struct | Self::Trait | Self::Const | Self::Macro    => (true, true, Keyword),
            Self::Field | Self::Tuple                                                  => (true, true, Default),
            Self::Generic | Self::Ref | Self::Array                                    => (true, true, Type),
            // `<self>` keyword container + `[self]` marker on `<parameter>`
            // for the receiver parameter (`fn f(&self) {}` →
            // `parameter[self]/self = "self"` — same shape as C#
            // `parameter[this]/this = "this"`).
            Self::Self_                                                                => (true, true, Keyword),
            // Bare-keyword statements + modifier markers: dual-use because
            // they can appear as either a structural container (with
            // content / restriction) or as an empty marker (bare keyword).
            //   <pub/> (default-access marker) vs <pub><crate/></pub>
            //   <unsafe/> (function modifier) vs <unsafe>{...}</unsafe>
            //   <break/> (bare break) vs <break>'label</break>
            //   <return/> (bare return) vs <return>expr</return>
            Self::Pub | Self::Unsafe | Self::Break | Self::Continue | Self::Return
            | Self::Yield                                                              => (true, true, Keyword),

            // `Alias` dual-use: `[alias]` marker on `<use>` for
            // `use std::X as Y` AND `<alias><name>Y</name></alias>`
            // child for the local-binding wrapper. Also covers Rust's
            // `type Color = int` `<alias>` container shape.
            Self::Alias                                                                => (true, true, Keyword),

            // ---- Containers with non-default syntax --------------------------
            Self::Impl | Self::Enum | Self::Mod | Self::Use | Self::Static
            | Self::Union | Self::Parameter
            | Self::Let | Self::If | Self::Else | Self::For | Self::While
            | Self::Loop | Self::Match | Self::Arm
            | Self::Bool                                                               => (false, true, Keyword),
            Self::Type | Self::Path                                                    => (false, true, Type),
            Self::Call | Self::Closure                                                 => (false, true, Function),
            Self::Binary | Self::Unary | Self::Op                                      => (false, true, Operator),
            Self::String                                                               => (false, true, String),
            Self::Int | Self::Float                                                    => (false, true, Number),
            Self::Name                                                                 => (false, true, Identifier),
            Self::Comment                                                              => (false, true, Comment),

            // ---- Default: container with Default syntax ----------------------
            _                                                                          => (false, true, Default),
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
