//! Output element names — tractor's PHP XML vocabulary after transform.

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
    // Top-level / declarations (Function, Constant dual-use)
    Program, Namespace, Use, Class, Interface, Trait, Enum, Method, Function, Field, Const, Constant,
    // Members / parameters
    Parameter, Argument,
    // Inheritance
    Extends, Implements, Types,
    // Statements / control flow
    Return, If, Else, ElseIf, For, Foreach, While, Do, Switch, Case, Try, Catch, Finally, Throw,
    Echo, Continue, Break, Match, Arm, Yield, Require, Print, Exit, Declare, Goto,
    Clone, Unset, Label,
    // Expressions
    Call, Member, Object, Property, Index, New, Cast, Assign, Binary, Unary, Ternary, Array,
    Spread, Expression, Scope, Shell,
    // Types / atoms
    Type, String, Int, Float, Bool, Null, Variable,
    // Misc structural
    Tag, Interpolation, Attribute,
    // Identifiers / comments / pair / op
    Name, Comment, Pair, Op,
    // Comment markers
    Trailing, Leading,
    // Visibility / access modifiers
    Public, Private, Protected,
    // Other modifiers
    Final, Abstract, Readonly,
    // Call/member flavor markers
    Instance,
    // Type-shape markers
    Primitive, Union, Optional, Underlying,
    // Parameter-shape markers
    Variadic,
    // Anonymous / arrow function shape markers
    Anonymous, Arrow,
    // php_tag marker
    Open,
    // Unary-shape marker
    Prefix,
    // Iter 17: PHP-specific markers
    Nullsafe, Bottom, Intersection, Disjunctive, Global, Dynamic, Promoted,
    Heredoc, Nowdoc,
    // Chain inversion (iter 247): `[access]` distinguishes
    // member-access chains from object literals.
    Access,
    // Import-shape (path container, alias dual-use, group marker)
    Path, Alias, Aliased, Group,
    // Dual-use names
    Static, Default,
}

impl TractorNode {
    pub fn as_str(self) -> &'static str {
        <&'static str>::from(self)
    }

    /// Per-name metadata. Default for unlisted variants: container with
    /// `Default` syntax.
    ///
    /// Dual-use names set BOTH `marker: true` and `container: true`:
    ///   - Static   — `static_modifier` keyword marker + scoped-call shape marker.
    ///   - Constant — `enum_case` / `const_element` (container) +
    ///                `class_constant_access_expression` member-shape marker.
    ///   - Default  — `default_statement` (container) + `match_default_expression`
    ///                arm-shape marker.
    ///   - Function — function_definition (container) + anonymous/arrow markers.
    pub fn spec(self) -> TractorNodeSpec {
        let (marker, container, syntax) = match self {
            // ---- Markers only ------------------------------------------------
            Self::Trailing | Self::Leading
            | Self::Instance | Self::Primitive | Self::Union | Self::Optional
            | Self::Underlying
            | Self::Variadic | Self::Anonymous | Self::Arrow | Self::Open
            | Self::Prefix
            | Self::Nullsafe | Self::Bottom | Self::Intersection | Self::Disjunctive
            | Self::Dynamic | Self::Promoted | Self::Heredoc | Self::Nowdoc
            | Self::Access
            | Self::Group                                                                => (true, false, Default),
            Self::Public | Self::Private | Self::Protected
            | Self::Final | Self::Abstract | Self::Readonly | Self::Global               => (true, false, Keyword),

            // ---- Dual-use (marker AND container) -----------------------------
            Self::Function | Self::Constant | Self::Static | Self::Default               => (true, true, Keyword),
            // `Alias` is the `[alias]` marker on `<use>` for
            // `use App\Foo as Bar`. The local-binding wrapper uses
            // `<aliased>` (renamed iter 184) so the marker and
            // wrapper don't collide on the same JSON key. Kept as
            // dual-use (true, true) for safety in case any future
            // path uses `<alias>` as a structural container.
            Self::Alias                                                                  => (true, true, Default),
            Self::Aliased                                                                => (false, true, Default),

            // Bare-keyword statements: dual-use (empty marker OR
            // container). `break;` / `continue;` are bare without
            // depth; `return;` / `throw;` likewise.
            Self::Break | Self::Continue | Self::Return | Self::Throw                    => (true, true, Keyword),

            // ---- Containers with non-default syntax --------------------------
            Self::Namespace | Self::Use | Self::Class | Self::Interface | Self::Trait
            | Self::Enum | Self::Method | Self::Field | Self::Const | Self::Parameter
            | Self::Extends | Self::Implements
            | Self::If | Self::Else | Self::ElseIf | Self::For
            | Self::Foreach | Self::While | Self::Do | Self::Switch | Self::Case
            | Self::Try | Self::Catch | Self::Finally
            | Self::Clone | Self::Unset
            | Self::Bool | Self::Null                                                    => (false, true, Keyword),
            Self::Type                                                                   => (false, true, Type),
            Self::Path                                                                   => (false, true, Default),
            Self::Call | Self::New                                                       => (false, true, Function),
            Self::Cast | Self::Assign | Self::Binary | Self::Unary | Self::Ternary | Self::Op
                                                                                          => (false, true, Operator),
            Self::String                                                                 => (false, true, String),
            Self::Int | Self::Float                                                      => (false, true, Number),
            Self::Name                                                                   => (false, true, Identifier),
            Self::Comment                                                                => (false, true, Comment),

            // ---- Default: container with Default syntax ----------------------
            _                                                                            => (false, true, Default),
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
