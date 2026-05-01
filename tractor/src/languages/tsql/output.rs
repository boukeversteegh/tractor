//! Output element names — tractor's T-SQL XML vocabulary after transform.

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
    // Top-level
    File, Statement,
    // DML statements
    Select, Insert, Delete, Update,
    // Clauses
    From, Where, Order, Target, Group, Having, Join, Direction,
    // References and columns
    Relation, Ref, Column, Star,
    // Literals and values
    Literal, List,
    // Functions / calls
    Call, Body, Arg,
    // Subqueries, CTEs, set operations
    Subquery, Cte, Union, Exists,
    // Window functions
    Window, Over, Partition,
    // CASE expression
    Case, When,
    // CAST
    Cast,
    // DDL
    Create, Columns, Definition,
    // MERGE
    Merge,
    // Transactions
    Transaction,
    // SET variable
    Set,
    // CREATE FUNCTION
    Function,
    // GO batch separator
    Go,
    // EXEC
    Exec,
    // Data types
    Int, Varchar, Nvarchar, Datetime,
    // Expressions
    Compare, Between, Assign,
    // DDL — generic containers
    Alter, Drop, Constraint,
    // Control flow
    While,
    // Window
    Frame, Filter,
    // Variable declarations
    Declare,
    // Storage / table options (catch-all for dialect-specific clauses)
    Option,
    // RESET statement
    Reset,
    // Identifiers and their variants
    Name, Alias, Schema, Var, Temp, Comment,
    // Operator child
    Op,
}

impl TractorNode {
    pub fn as_str(self) -> &'static str {
        <&'static str>::from(self)
    }

    /// Per-name metadata. Default for unlisted variants: container with
    /// `Default` syntax.
    pub fn spec(self) -> TractorNodeSpec {
        let (marker, container, syntax) = match self {
            // ---- Dual-use (marker AND container) -----------------------------
            // `INT` / `VARCHAR(n)` etc. — type elements that may carry
            // a length argument as a container OR be bare empty
            // markers when the type has no parameters.
            Self::Int | Self::Varchar | Self::Nvarchar | Self::Datetime         => (true, true, Type),
            // Sort direction `ASC` / `DESC` — bare keyword marker on
            // `<order>`.
            Self::Direction                                                     => (true, true, Keyword),
            // `DELETE` may be either a DML statement (container) or
            // an empty marker on `ALTER TABLE ... DROP CONSTRAINT`.
            Self::Delete                                                        => (true, true, Keyword),
            // `<literal>` for SQL string/numeric literals — usually
            // has a text child, but can be empty as a placeholder.
            Self::Literal                                                       => (true, true, String),

            // ---- Containers with non-default syntax --------------------------
            Self::Statement | Self::Select | Self::Insert | Self::Update
            | Self::From | Self::Where | Self::Having | Self::Join
            | Self::Star | Self::Cte | Self::Union | Self::Exists
            | Self::Case | Self::When
            | Self::Merge | Self::Transaction | Self::Set | Self::Go | Self::Exec
            | Self::Alter | Self::Drop | Self::While | Self::Filter | Self::Declare
            | Self::Reset                                                        => (false, true, Keyword),
            Self::Ref                                                           => (false, true, Type),
            Self::Call | Self::Window | Self::Cast                              => (false, true, Function),
            Self::Compare | Self::Between | Self::Assign | Self::Op             => (false, true, Operator),
            Self::Column | Self::Name | Self::Alias | Self::Schema | Self::Var | Self::Temp
                                                                                => (false, true, Identifier),
            Self::Comment                                                       => (false, true, Comment),

            // ---- Default: container with Default syntax ----------------------
            _                                                                   => (false, true, Default),
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
