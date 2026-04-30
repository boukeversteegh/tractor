//! Output element names — tractor's Java XML vocabulary after transform.

use once_cell::sync::Lazy;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, EnumIter, EnumString, IntoStaticStr};

use crate::languages::NodeSpec;
use crate::output::syntax_highlight::SyntaxCategory::{self, *};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, IntoStaticStr, AsRefStr, EnumIter,
)]
#[strum(serialize_all = "snake_case")]
pub enum JavaName {
    // Top-level / declarations (Record dual-use)
    Program, Class, Interface, Enum, Record, Method, Constructor, Field, Variable,
    Declarator, Constant,
    // Members
    Parameter, Generic, Generics, Extends, Implements,
    // Type vocabulary (Type dual-use)
    Type, Path, Returns, Dimensions,
    // Control flow
    Return, If, Else, ElseIf, For, Foreach, While, Try, Catch, Finally, Throw, Throws,
    Switch, Arm, Label, Case, Pattern, Guard, Body,
    // Expressions
    Call, New, Member, Index, Assign, Binary, Unary, Lambda, Ternary, Annotation,
    // Imports (Package dual-use)
    Import, Package,
    // Literals
    String, Int, Float, True, False, Null,
    // Identifiers / comments / op
    Name, Comment, Op,
    // Comment markers
    Trailing, Leading,
    // Access modifiers (markers only)
    Public, Private, Protected,
    // Other modifiers (markers only)
    Static, Final, Abstract, Synchronized, Volatile, Transient, Native, Strictfp,
    // Special markers (This dual-use)
    Void, This, Super, Array, Variadic, Compact,
}

impl JavaName {
    pub fn as_str(self) -> &'static str {
        <&'static str>::from(self)
    }

    /// Per-name metadata. Default for unlisted variants: container with
    /// `Default` syntax.
    ///
    /// Dual-use names set BOTH `marker: true` and `container: true`:
    ///   - Package — structural (package_declaration) + marker (implicit
    ///               access when no access modifier is present).
    ///   - Record  — structural (record_declaration) + marker (record_pattern).
    ///   - Type    — structural (type references) + marker (type_pattern).
    ///   - This    — marker on `<call[this]>` + structural for bare `this`.
    pub fn spec(self) -> NodeSpec {
        let (marker, container, syntax) = match self {
            // ---- Markers only ------------------------------------------------
            Self::Trailing | Self::Leading                                          => (true, false, Default),
            Self::Public | Self::Private | Self::Protected
            | Self::Static | Self::Final | Self::Abstract | Self::Synchronized
            | Self::Volatile | Self::Transient | Self::Native | Self::Strictfp
            | Self::Super                                                           => (true, false, Keyword),
            Self::Array                                                             => (true, false, Type),
            Self::Void | Self::Variadic | Self::Compact                             => (true, false, Default),

            // ---- Dual-use (marker AND container) -----------------------------
            Self::Record                                                            => (true, true, Default),
            Self::Type                                                              => (true, true, Type),
            Self::Package | Self::This                                              => (true, true, Keyword),

            // ---- Containers with non-default syntax --------------------------
            Self::Class | Self::Interface | Self::Enum | Self::Method | Self::Field
            | Self::Parameter | Self::Import
            | Self::Return | Self::If | Self::Else | Self::For | Self::Foreach
            | Self::While | Self::Try | Self::Catch | Self::Finally | Self::Throw
            | Self::Switch | Self::Case | Self::New
            | Self::True | Self::False | Self::Null                                 => (false, true, Keyword),
            Self::Generic                                                           => (false, true, Type),
            Self::Call | Self::Lambda                                               => (false, true, Function),
            Self::Assign | Self::Binary | Self::Unary | Self::Ternary | Self::Op    => (false, true, Operator),
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
    Lazy::new(|| JavaName::iter().map(|n| n.spec()).collect());

pub fn nodes() -> &'static [NodeSpec] {
    NODES_TABLE.as_slice()
}

pub fn spec(name: &str) -> Option<&'static NodeSpec> {
    let parsed: JavaName = name.parse().ok()?;
    let target = parsed.as_str();
    NODES_TABLE.iter().find(|s| s.name == target)
}

pub fn all_names() -> impl Iterator<Item = &'static str> {
    JavaName::iter().map(JavaName::as_str)
}

pub fn is_marker_only(name: &str) -> bool {
    spec(name).map_or(false, |s| s.marker && !s.container)
}

pub fn is_declared(name: &str) -> bool {
    spec(name).is_some()
}

#[allow(dead_code)]
const _SYNTAX_CATEGORY_USED: Option<SyntaxCategory> = None;
