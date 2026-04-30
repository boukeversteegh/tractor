//! Output element names — tractor's Rust XML vocabulary after transform.

use once_cell::sync::Lazy;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, EnumIter, EnumString, IntoStaticStr};

use crate::languages::NodeSpec;
use crate::output::syntax_highlight::SyntaxCategory::{self, *};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, IntoStaticStr, AsRefStr, EnumIter,
)]
#[strum(serialize_all = "snake_case")]
pub enum RustName {
    // Top-level / declarations (Function, Struct, Trait, Const dual-use)
    File, Function, Impl, Struct, Enum, Trait, Mod, Use, Const, Static, Alias, Signature, Modifiers,
    // Members (Field dual-use)
    Parameter,
    #[strum(serialize = "self")]
    Self_,
    Field, Variant, Lifetime, Attribute,
    // Types / generics (Generic dual-use)
    Type, Generic, Generics, Path, Bounds, Bound, Where,
    // Statements / control flow
    Let, Return, If, Else, ElseIf, For, While, Loop, Match, Arm, Pattern, Break, Continue, Range,
    Send, Label,
    // Expressions (Try, Ref, Tuple dual-use)
    Call, Index, Binary, Unary, Assign, Closure, Await, Try, Macro, Cast, Ref, Tuple, Unsafe,
    Literal, Block,
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
}

impl RustName {
    pub fn as_str(self) -> &'static str {
        <&'static str>::from(self)
    }

    /// Per-name metadata. Default for unlisted variants: container with
    /// `Default` syntax.
    ///
    /// Dual-use names set BOTH `marker: true` and `container: true`:
    ///   - Function — function_item (container) vs function_type (marker)
    ///   - Tuple    — tuple_expression (container) vs tuple_type (marker)
    ///   - Trait    — trait_item (container) vs trait_type (marker)
    ///   - Ref      — reference_expression (container) vs ref_pattern (marker)
    ///   - Field    — field_expression / field_declaration (container) vs
    ///                field_pattern / base_field_initializer (markers)
    ///   - Struct   — struct_item (container) vs struct_pattern (marker)
    ///   - Generic  — generic_type (container) vs generic_function (marker)
    ///   - Const    — const_item (container) vs const_block (marker)
    ///   - Try      — try_expression (container) vs try_block (marker)
    pub fn spec(self) -> NodeSpec {
        let (marker, container, syntax) = match self {
            // ---- Markers only ------------------------------------------------
            Self::Trailing | Self::Leading
            | Self::Raw | Self::Inner | Self::Borrowed
            | Self::Pointer | Self::Never | Self::Unit | Self::Dynamic
            | Self::Abstract | Self::Associated | Self::Bounded | Self::Array | Self::Or
            | Self::Method | Self::Base | Self::Slice                                  => (true, false, Default),
            Self::Private | Self::Crate | Self::Super | Self::Mut | Self::Async        => (true, false, Keyword),

            // ---- Dual-use (marker AND container) -----------------------------
            Self::Function | Self::Struct | Self::Trait | Self::Const                  => (true, true, Keyword),
            Self::Field | Self::Tuple                                                  => (true, true, Default),
            Self::Generic | Self::Ref                                                  => (true, true, Type),
            Self::Try                                                                  => (true, true, Operator),

            // ---- Containers with non-default syntax --------------------------
            Self::Impl | Self::Enum | Self::Mod | Self::Use | Self::Static | Self::Alias
            | Self::Parameter | Self::Self_
            | Self::Let | Self::Return | Self::If | Self::Else | Self::For | Self::While
            | Self::Loop | Self::Match | Self::Arm | Self::Break | Self::Continue
            | Self::Unsafe | Self::Pub
            | Self::Bool                                                               => (false, true, Keyword),
            Self::Type | Self::Path                                                    => (false, true, Type),
            Self::Call | Self::Closure | Self::Macro                                   => (false, true, Function),
            Self::Binary | Self::Unary | Self::Op                                      => (false, true, Operator),
            Self::String                                                               => (false, true, String),
            Self::Int | Self::Float                                                    => (false, true, Number),
            Self::Name                                                                 => (false, true, Identifier),
            Self::Comment                                                              => (false, true, Comment),

            // ---- Default: container with Default syntax ----------------------
            _                                                                          => (false, true, Default),
        };
        NodeSpec { name: self.as_str(), marker, container, syntax }
    }
}

static NODES_TABLE: Lazy<Vec<NodeSpec>> =
    Lazy::new(|| RustName::iter().map(|n| n.spec()).collect());

pub fn nodes() -> &'static [NodeSpec] {
    NODES_TABLE.as_slice()
}

pub fn spec(name: &str) -> Option<&'static NodeSpec> {
    let parsed: RustName = name.parse().ok()?;
    let target = parsed.as_str();
    NODES_TABLE.iter().find(|s| s.name == target)
}

pub fn all_names() -> impl Iterator<Item = &'static str> {
    RustName::iter().map(RustName::as_str)
}

pub fn is_marker_only(name: &str) -> bool {
    spec(name).map_or(false, |s| s.marker && !s.container)
}

pub fn is_declared(name: &str) -> bool {
    spec(name).is_some()
}

#[allow(dead_code)]
const _SYNTAX_CATEGORY_USED: Option<SyntaxCategory> = None;
