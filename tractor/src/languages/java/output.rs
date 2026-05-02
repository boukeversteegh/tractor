//! Output element names — tractor's Java XML vocabulary after transform.

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
    // Control flow additions
    Assert, Block, Break, Continue, Do, Instanceof, Yield,
    // Expressions
    Call, New, Member, Object, Property, Index, Assign, Binary, Unary, Lambda, Ternary,
    Annotation, Expression,
    // Expression additions
    Cast, Reference,
    // Imports (Package dual-use)
    Import, Package,
    // Module (Module + Directive container; Exports/Opens/Provides/Requires/Uses markers)
    Directive, Module,
    Exports, Opens, Provides, Requires, Uses,
    // Literals
    String, Int, Float, True, False, Null, Char,
    // String interpolation (Template dual-use; Interpolation container)
    Template, Interpolation,
    // Array / annotation containers
    Pair,
    // Identifiers / comments / op
    Name, Comment, Op,
    // Comment markers
    Trailing, Leading,
    // Access modifiers (markers only)
    Public, Private, Protected,
    // Other modifiers (markers only — Synchronized dual-use: marker on method + container for stmt)
    Static, Final, Abstract, Synchronized, Volatile, Transient, Native, Strictfp,
    // Special markers (This dual-use; Array dual-use; Class dual-use for class_literal)
    Void, This, Super, Array, Variadic, Compact,
    // New markers
    Receiver, Resource, Wildcard,
    // Switch-label marker — `[default]` on `<label>` for `default ->`
    // (vs the regular `case X:` form which uses content).
    Default,
    // Unary-shape marker
    Prefix,
}

impl TractorNode {
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
    pub fn spec(self) -> TractorNodeSpec {
        let (marker, container, syntax) = match self {
            // ---- Markers only ------------------------------------------------
            Self::Trailing | Self::Leading                                          => (true, false, Default),
            Self::Public | Self::Private | Self::Protected
            | Self::Static | Self::Final | Self::Abstract
            | Self::Volatile | Self::Transient | Self::Native | Self::Strictfp
            | Self::Super                                                           => (true, false, Keyword),
            Self::Void | Self::Variadic | Self::Compact | Self::Prefix
            | Self::Receiver | Self::Resource
            | Self::Exports | Self::Opens | Self::Provides | Self::Requires | Self::Uses => (true, false, Default),

            // ---- Dual-use (marker AND container) -----------------------------
            Self::Record                                                            => (true, true, Default),
            Self::Type                                                              => (true, true, Type),
            Self::Package | Self::This                                              => (true, true, Keyword),
            // Class: container for class_declaration + marker for class_literal
            // Synchronized: marker on method + container for synchronized_statement
            Self::Class | Self::Synchronized                                        => (true, true, Keyword),
            // Wildcard: marker for `underscore_pattern` (`case _ ->`) AND
            // for bare `<?>` generic; container for bounded
            // `<? extends T>` / `<? super T>` (keeps the bound).
            Self::Wildcard                                                          => (true, true, Default),
            // `Default` is dual-use: marker on `<label>` for `default ->`
            // arms AND container for the (deprecated) Java
            // `default` switch label arm-body shape.
            Self::Default                                                           => (true, true, Keyword),
            // Array: marker on <type> / <pattern> + container for array_initializer
            Self::Array                                                             => (true, true, Type),
            // Template: container for template_expression + marker (dual-use like TS)
            Self::Template                                                          => (true, true, Default),
            // Annotation: container for `@Annotation(args...)` (with arg
            // children) + marker for bare `@Override` (no args).
            // Generic: container for `<T extends Foo>` (with type
            // children) + marker for wildcard `<?>` / empty type
            // parameter list.
            Self::Annotation | Self::Generic                                        => (true, true, Type),

            // Bare-keyword statements: dual-use (empty marker OR
            // container).
            Self::Break | Self::Continue | Self::Return | Self::Throw
            | Self::Yield                                                           => (true, true, Keyword),

            // ---- Containers with non-default syntax --------------------------
            Self::Interface | Self::Enum | Self::Method | Self::Field
            | Self::Parameter | Self::Import
            | Self::If | Self::Else | Self::For | Self::Foreach
            | Self::While | Self::Try | Self::Catch | Self::Finally
            | Self::Switch | Self::Case | Self::New
            | Self::True | Self::False | Self::Null
            | Self::Assert | Self::Do
            | Self::Instanceof | Self::Module                                       => (false, true, Keyword),
            Self::Call | Self::Lambda                                               => (false, true, Function),
            Self::Assign | Self::Binary | Self::Unary | Self::Ternary | Self::Op
            | Self::Cast                                                            => (false, true, Operator),
            Self::String | Self::Char                                               => (false, true, String),
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
