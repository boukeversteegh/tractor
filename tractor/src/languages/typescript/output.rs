//! Output element names — tractor's TypeScript/JavaScript XML
//! vocabulary after transform.

use once_cell::sync::Lazy;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, EnumIter, EnumString, IntoStaticStr};

use crate::languages::NodeSpec;
use crate::output::syntax_highlight::SyntaxCategory::{self, *};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, IntoStaticStr, AsRefStr, EnumIter,
)]
#[strum(serialize_all = "snake_case")]
pub enum TsName {
    // Top-level / declarations (Function dual-use)
    Program, Class, Interface, Enum, Function, Method, Property, Constructor, Indexer, Alias,
    Variable, Arrow,
    // Members
    Field, Parameter, Extends, Implements,
    // Type vocabulary
    Type, Generic, Generics, Predicate, Annotation,
    // Control flow
    Block, Return, If, Else, ElseIf, For, While, Try, Catch, Throw, Finally, Switch, Case,
    Break, Continue, Body,
    // Expressions
    Call, New, Member, Assign, Binary, Unary, Ternary, Await, Yield, As, Satisfies, Index, Pattern,
    Spread, Rest,
    // Imports / exports (Export dual-use)
    Import, Export, Imports, Spec, Clause, Namespace,
    // Templates (Template dual-use)
    Template, Interpolation,
    // JSX
    Element, Opening, Closing, Prop, Value, Text,
    // Enum members + object pair
    Constant, Pair,
    // Literals
    String, Number, Bool, Null, Undefined,
    // Keyword expressions
    This, Super, Constraint,
    // Identifiers / comments / op
    Name, Comment, Op,
    // Comment markers
    Trailing, Leading,
    // Switch default — dual-use marker / container
    Default,
    // Accessibility / modifier markers
    Public, Private, Protected, Override, Readonly, Abstract, Optional, Required,
    // Function markers
    Async, Generator, Get, Set,
    // Variable-keyword markers
    Let, Const, Var,
    // Type-shape markers (Array, Object dual-use)
    Union, Intersection, Array, Literal, Tuple, Parenthesized, Object, Conditional, Infer, Lookup,
    Keyof,
}

impl TsName {
    pub fn as_str(self) -> &'static str {
        <&'static str>::from(self)
    }

    /// Per-name metadata. Default for unlisted variants: container with
    /// `Default` syntax.
    ///
    /// Dual-use names set BOTH `marker: true` and `container: true`:
    ///   - Export   — marker via extract_keyword_modifiers + structural
    ///                container from `export_statement`.
    ///   - Default  — marker modifier + structural switch_default container.
    ///   - Function — function_declaration container + marker on type.
    ///   - Template — template_string container + marker on template_type.
    ///   - Array    — marker on `<type>` / `<pattern>` + container literal.
    ///   - Object   — marker on `<type>` / `<pattern>` + container literal.
    pub fn spec(self) -> NodeSpec {
        let (marker, container, syntax) = match self {
            // ---- Markers only ------------------------------------------------
            Self::Trailing | Self::Leading
            | Self::Generator | Self::Get | Self::Set
            | Self::Union | Self::Intersection | Self::Literal | Self::Tuple
            | Self::Parenthesized | Self::Conditional | Self::Infer | Self::Lookup
            | Self::Keyof                                                          => (true, false, Default),
            Self::Public | Self::Private | Self::Protected | Self::Override
            | Self::Readonly | Self::Abstract | Self::Optional | Self::Required
            | Self::Async
            | Self::Let | Self::Const | Self::Var                                  => (true, false, Keyword),

            // ---- Dual-use (marker AND container) -----------------------------
            Self::Function | Self::Export | Self::Default                          => (true, true, Keyword),
            Self::Template | Self::Array | Self::Object                            => (true, true, Default),

            // ---- Containers with non-default syntax --------------------------
            Self::Class | Self::Interface | Self::Enum | Self::Method
            | Self::Alias | Self::Variable | Self::Parameter
            | Self::Return | Self::If | Self::Else | Self::For | Self::While
            | Self::Try | Self::Catch | Self::Throw | Self::Finally | Self::Switch
            | Self::Case | Self::Break | Self::Continue | Self::New | Self::Await
            | Self::Yield | Self::Import
            | Self::Bool | Self::Null | Self::Undefined
            | Self::This | Self::Super                                             => (false, true, Keyword),
            Self::Type | Self::Generic | Self::Generics                            => (false, true, Type),
            Self::Call | Self::Arrow                                               => (false, true, Function),
            Self::Assign | Self::Binary | Self::Unary | Self::Ternary | Self::Op   => (false, true, Operator),
            Self::String                                                           => (false, true, String),
            Self::Number                                                           => (false, true, Number),
            Self::Name                                                             => (false, true, Identifier),
            Self::Comment                                                          => (false, true, Comment),

            // ---- Default: container with Default syntax ----------------------
            _                                                                      => (false, true, Default),
        };
        NodeSpec { name: self.as_str(), marker, container, syntax }
    }
}

static NODES_TABLE: Lazy<Vec<NodeSpec>> =
    Lazy::new(|| TsName::iter().map(|n| n.spec()).collect());

pub fn nodes() -> &'static [NodeSpec] {
    NODES_TABLE.as_slice()
}

pub fn spec(name: &str) -> Option<&'static NodeSpec> {
    let parsed: TsName = name.parse().ok()?;
    let target = parsed.as_str();
    NODES_TABLE.iter().find(|s| s.name == target)
}

pub fn all_names() -> impl Iterator<Item = &'static str> {
    TsName::iter().map(TsName::as_str)
}

pub fn is_marker_only(name: &str) -> bool {
    spec(name).map_or(false, |s| s.marker && !s.container)
}

pub fn is_declared(name: &str) -> bool {
    spec(name).is_some()
}

#[allow(dead_code)]
const _SYNTAX_CATEGORY_USED: Option<SyntaxCategory> = None;
