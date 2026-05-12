//! SQL-language IR — a fully typed representation of T-SQL (and
//! eventually MySQL / PostgreSQL / SQLite) constructs. Parallel to
//! [`crate::tree::SyntaxTree`] (programming languages) and [`crate::tree::data::DataTree`]
//! (data languages).
//!
//! ## Why a separate IR
//!
//! Forcing SQL into the cross-language [`SyntaxTree`] enum either:
//! - bloats `SyntaxTree` with SQL-specific variants (Insert, Select, Where,
//!   Subquery, Cte, …) that no programming language uses, OR
//! - forces SQL into the generic `SyntaxTree::SimpleStatement` catch-all,
//!   which loses typed-slot semantics — the source of the JSON
//!   projection heuristics in iters 29-36.
//!
//! Per user direction (2026-05-07): "there is absolutely no rule
//! that says that all languages should use the same IR".
//!
//! `SqlTree` gives every SQL construct a typed shape. Both XML and
//! JSON output read typed slots directly — no projection rules
//! needed.
//!
//! ## Status
//!
//! Type definitions only. Lowering (`tsql.rs`), XML rendering
//! (`sql_to_xot.rs`), JSON rendering (`sql_to_json.rs`), and
//! parser wiring land in subsequent iters.
//!
//! ## Invariants
//!
//! 1. **Round-trip identity** — `to_source(sql_tree, source) == source`.
//! 2. **XPath text recovery** — `string(rendered_root) == source`.
//! 3. **No silent drops** — un-handled CST kinds fall through to
//!    [`SqlTree::Unknown`].
//! 4. **Canonical reconstruction without source** —
//!    `parse(render_sql(tree, None)) == parse(s)` where
//!    `tree = lower_sql_root(parse(s), &s)`. The IR carries every
//!    semantic distinction needed to regenerate equivalent source
//!    from scratch, without consulting source byte ranges.
//!    Identifier quoting style (`[name]` / `"name"` / `` `name` ``)
//!    is captured by [`QuoteStyle`] on the atom variant; `value` is
//!    the parsed unquoted text. Implemented as the canonical-mode
//!    arm of [`crate::tree::source::render_sql`] in `source/sql.rs`,
//!    fitting the existing per-language source-rendering convention.

#![cfg(feature = "native")]

use super::types::{ByteRange, Span};

/// Typed SQL IR.
#[derive(Debug, Clone)]
pub enum SqlTree {
    // ----- Top level -----------------------------------------------------

    /// `<file>` — top-level container of statements.
    File {
        statements: Vec<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<statement>` — a generic wrapper for a statement node when
    /// the inner kind is one of the typed variants below. Renders
    /// as `<statement>{inner_render}</statement>`. The inner is
    /// any other `SqlTree` variant.
    Statement {
        inner: Box<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<go>` — T-SQL batch separator.
    Go { range: ByteRange, span: Span },

    /// `<exec>` — `EXEC sp_helpdb`.
    Exec {
        target: Box<SqlTree>,           // SqlTree::Identifier or SqlTree::Call
        range: ByteRange,
        span: Span,
    },

    /// `<set>` — `SET @var = expr`.
    Set {
        target: Box<SqlTree>,           // SqlTree::Variable
        value: Box<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    // ----- DML statements ------------------------------------------------

    /// `<select>` — `SELECT cols FROM ... WHERE ... GROUP BY ... HAVING ... ORDER BY ...`.
    /// Each clause is its own typed slot.
    Select {
        ctes: Vec<SqlTree>,             // WITH ... AS (...) CTE clauses (each SqlTree::Cte)
        columns: Vec<SqlTree>,          // each is SqlTree::Column or SqlTree::Star
        into: Option<Box<SqlTree>>,     // SELECT ... INTO #temp
        from: Option<Box<SqlTree>>,     // SqlTree::From
        where_: Option<Box<SqlTree>>,   // SqlTree::Where
        group_by: Option<Box<SqlTree>>, // SqlTree::GroupBy
        having: Option<Box<SqlTree>>,   // SqlTree::Having
        order_by: Option<Box<SqlTree>>, // SqlTree::OrderBy
        range: ByteRange,
        span: Span,
    },

    /// `<insert>` — `INSERT INTO table (cols) VALUES (vals)`.
    Insert {
        table: Box<SqlTree>,            // SqlTree::Relation
        columns: Vec<SqlTree>,          // empty when columns omitted
        values: Vec<SqlTree>,           // each row is a SqlTree::Tuple
        range: ByteRange,
        span: Span,
    },

    /// `<update>` — `UPDATE table SET col=val,... WHERE ...`.
    Update {
        table: Box<SqlTree>,
        assignments: Vec<SqlTree>,      // each is SqlTree::Assign
        where_: Option<Box<SqlTree>>,
        range: ByteRange,
        span: Span,
    },

    /// `<delete>` — `DELETE FROM table WHERE ...`.
    Delete {
        from: Option<Box<SqlTree>>,
        where_: Option<Box<SqlTree>>,
        range: ByteRange,
        span: Span,
    },

    /// `<merge>` — `MERGE INTO target USING source ON ... WHEN ...`.
    Merge {
        target: Box<SqlTree>,           // SqlTree::Relation
        source: Box<SqlTree>,           // SqlTree::Relation or SqlTree::Subquery
        on: Box<SqlTree>,               // join condition
        whens: Vec<SqlTree>,            // each is SqlTree::MergeWhen
        range: ByteRange,
        span: Span,
    },

    /// `<when>` inside MERGE — `WHEN MATCHED THEN UPDATE ...` / `WHEN NOT MATCHED THEN INSERT ...`.
    MergeWhen {
        matched: bool,                 // MATCHED vs NOT MATCHED
        action: Box<SqlTree>,            // SqlTree::Update / SqlTree::Insert / SqlTree::Delete
        range: ByteRange,
        span: Span,
    },

    /// `<transaction>` — `BEGIN TRANSACTION ... COMMIT`.
    Transaction {
        statements: Vec<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    // ----- Clauses (children of DML statements) --------------------------

    /// `<from>` — `FROM relation [JOIN ...]`.
    From {
        relations: Vec<SqlTree>,         // first is base; subsequent are joins
        range: ByteRange,
        span: Span,
    },

    /// `<where>` — `WHERE condition`.
    Where { condition: Box<SqlTree>, range: ByteRange, span: Span },

    /// `<group>` — `GROUP BY col, col, ...`.
    GroupBy { keys: Vec<SqlTree>, range: ByteRange, span: Span },

    /// `<having>` — `HAVING condition`.
    Having { condition: Box<SqlTree>, range: ByteRange, span: Span },

    /// `<order>` — `ORDER BY target [ASC|DESC], ...`.
    OrderBy { targets: Vec<SqlTree>, range: ByteRange, span: Span },

    /// `<target>` — one item in ORDER BY: `expr [ASC|DESC]`.
    OrderTarget {
        expression: Box<SqlTree>,
        direction: Option<SortDirection>,
        range: ByteRange,
        span: Span,
    },

    /// `<partition>` — `PARTITION BY col, col, ...` inside OVER().
    PartitionBy { keys: Vec<SqlTree>, range: ByteRange, span: Span },

    /// `<join>` — one JOIN clause: `[LEFT|RIGHT|FULL|INNER|CROSS] JOIN relation ON cond`.
    Join {
        kind: JoinKind,
        relation: Box<SqlTree>,
        on: Option<Box<SqlTree>>,        // None for CROSS JOIN
        range: ByteRange,
        span: Span,
    },

    // ----- References and columns ----------------------------------------

    /// `<relation>` — a table reference, optionally aliased and schema-qualified.
    Relation {
        schema: Option<Box<SqlTree>>,    // SqlTree::Identifier
        name: Box<SqlTree>,              // SqlTree::Identifier
        alias: Option<Box<SqlTree>>,     // SqlTree::Identifier
        range: ByteRange,
        span: Span,
    },

    /// `<column>` in SELECT clause — `expr [AS alias]`.
    Column {
        expression: Box<SqlTree>,
        alias: Option<Box<SqlTree>>,
        range: ByteRange,
        span: Span,
    },

    /// `<star>` — `*` (unqualified or `relation.*`).
    Star {
        qualifier: Option<Box<SqlTree>>, // for `t.*`
        range: ByteRange,
        span: Span,
    },

    /// `<reference>` — column reference, possibly qualified:
    /// `name`, `t.name`, `dbo.t.name`, etc.
    Reference {
        parts: Vec<SqlTree>,             // each is SqlTree::Identifier
        range: ByteRange,
        span: Span,
    },

    // ----- Expressions ---------------------------------------------------

    /// `<compare>` — binary comparison: `a = b`, `a > b`, `a IN (...)`, etc.
    Compare {
        left: Box<SqlTree>,
        op: ComparisonOp,
        right: Box<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<binary>` — arithmetic or logical: `a + b`, `a AND b`.
    Binary {
        left: Box<SqlTree>,
        op: BinaryOp,
        right: Box<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<unary>` — `NOT expr`, `-expr`.
    Unary {
        op: UnaryOp,
        operand: Box<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<assign>` — `SET col = expr` (in UPDATE) or `SET @var = expr`.
    Assign {
        target: Box<SqlTree>,
        value: Box<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<between>` — `expr BETWEEN low AND high`.
    Between {
        value: Box<SqlTree>,
        low: Box<SqlTree>,
        high: Box<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<exists>` — `EXISTS (subquery)`.
    Exists {
        subquery: Box<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<case>` — `CASE WHEN ... THEN ... [ELSE ...] END`.
    Case {
        whens: Vec<SqlTree>,             // each is SqlTree::When
        else_: Option<Box<SqlTree>>,
        range: ByteRange,
        span: Span,
    },

    /// `<when>` — one `WHEN cond THEN value` arm in a CASE.
    When {
        condition: Box<SqlTree>,
        value: Box<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<cast>` — `CAST(expr AS type)`.
    Cast {
        value: Box<SqlTree>,
        type_: Box<SqlTree>,             // SqlTree::DataType
        range: ByteRange,
        span: Span,
    },

    /// `<call>` — function invocation: `LOWER(x)`, `COUNT(*)`, etc.
    Call {
        callee: Box<SqlTree>,            // SqlTree::Identifier
        arguments: Vec<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<window>` — windowed aggregate: `func() OVER (...)`.
    Window {
        call: Box<SqlTree>,              // SqlTree::Call
        over: Box<SqlTree>,              // SqlTree::Over
        range: ByteRange,
        span: Span,
    },

    /// `<over>` — `OVER (PARTITION BY ... ORDER BY ...)`.
    Over {
        partition_by: Option<Box<SqlTree>>,
        order_by: Option<Box<SqlTree>>,
        range: ByteRange,
        span: Span,
    },

    /// `<subquery>` — parenthesized SELECT.
    Subquery {
        select: Box<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<union>` — `SELECT ... UNION [ALL] SELECT ...`.
    Union {
        all: bool,
        selects: Vec<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<cte>` — `WITH name AS (SELECT ...)`.
    Cte {
        name: Box<SqlTree>,
        query: Box<SqlTree>,             // SqlTree::Select
        range: ByteRange,
        span: Span,
    },

    /// `<tuple>` — `(a, b, c)` value list (used in INSERT VALUES rows
    /// and IN lists).
    Tuple {
        items: Vec<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    // ----- DDL ------------------------------------------------------------

    /// `<create>` — `CREATE TABLE / VIEW / INDEX name ...`.
    Create {
        kind: CreateKind,
        name: Box<SqlTree>,              // SqlTree::Identifier
        body: Vec<SqlTree>,              // columns / select / index_fields
        range: ByteRange,
        span: Span,
    },

    /// `<drop>` — `DROP TABLE / INDEX name`.
    Drop {
        kind: DropKind,
        name: Box<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<alter>` — `ALTER TABLE name operation`.
    Alter {
        name: Box<SqlTree>,
        operation: Box<SqlTree>,         // SqlTree::AddColumn / SqlTree::AddConstraint / ...
        range: ByteRange,
        span: Span,
    },

    /// `<column>` (DDL) — column definition in CREATE TABLE.
    ColumnDef {
        name: Box<SqlTree>,
        type_: Box<SqlTree>,
        constraints: Vec<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<constraint>` — table constraint (PRIMARY KEY, FOREIGN KEY, …).
    Constraint {
        name: Option<Box<SqlTree>>,
        body: Vec<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<add>` — `ADD COLUMN col TYPE` operation in ALTER TABLE.
    AddColumn {
        column: Box<SqlTree>,            // SqlTree::ColumnDef
        range: ByteRange,
        span: Span,
    },

    /// `<add>` for constraints — `ADD CONSTRAINT name ...`.
    AddConstraint {
        constraint: Box<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<function>` — `CREATE FUNCTION ... AS BEGIN ... END`.
    Function {
        schema: Option<Box<SqlTree>>,
        name: Box<SqlTree>,
        parameters: Vec<SqlTree>,
        return_type: Option<Box<SqlTree>>,
        body: Box<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    // ----- Type expressions ----------------------------------------------

    /// `<int>` / `<varchar>` / `<nvarchar>` / `<datetime>` etc. —
    /// SQL data type. `length` is `(n)` for VARCHAR(n) or absent for
    /// fixed-width types.
    DataType {
        name: &'static str,            // "int", "varchar", "datetime", ...
        length: Option<Box<SqlTree>>,    // SqlTree::Integer
        range: ByteRange,
        span: Span,
    },

    // ----- Atoms ---------------------------------------------------------

    /// `<name>` — identifier text leaf. Always rendered as a text-only
    /// `<name>` element (per the cross-language `name-is-text-leaf`
    /// shape contract). Quoting style (T-SQL `[name]`, ANSI `"name"`,
    /// MySQL `` `name` ``) is captured in `quoting` so the IR carries
    /// the syntactic distinction without relying on the source range.
    /// The `value` field carries the parsed (unquoted) identifier text;
    /// renderers read this directly rather than slicing source.
    Identifier { value: String, quoting: QuoteStyle, range: ByteRange, span: Span },

    /// `<schema>` — schema qualifier in `dbo.Table`. Renders as a
    /// container holding an inner `<name>` leaf and any quoting
    /// markers (so that markers stay off the text-only `<name>`).
    Schema { value: String, quoting: QuoteStyle, range: ByteRange, span: Span },

    /// `<alias>` — alias position identifier (`t` in `Users t`).
    /// Renders as a container holding an inner `<name>` leaf.
    Alias { value: String, quoting: QuoteStyle, range: ByteRange, span: Span },

    /// `<temp>` — temp-table qualifier `#name` / `##name`.
    Temp { name: Box<SqlTree>, range: ByteRange, span: Span },

    /// `<var>` — `@variable` reference.
    Variable { range: ByteRange, span: Span },

    /// `<literal>` — string / numeric / hex literal. Text is verbatim
    /// from the source range.
    Literal { range: ByteRange, span: Span },

    /// `<comment>` — `-- line` or `/* block */`.
    Comment { range: ByteRange, span: Span },

    // ----- Escape hatches ------------------------------------------------

    /// `<unknown kind="…">` — last-resort fallback for un-handled
    /// CST kinds. Visible in output so coverage gaps show up.
    Unknown {
        kind: String,
        range: ByteRange,
        span: Span,
    },
}

/// Sort direction for `ORDER BY` and within `OVER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

/// Quoting style for identifier-class atoms (`Identifier`, `Schema`,
/// `Alias`). Captures the syntactic distinction explicitly in the IR
/// so that a fully semantically equivalent source can be reconstructed
/// from the IR alone — without relying on byte ranges into the
/// original source string. (See module-level invariant 4.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteStyle {
    /// Bare identifier — `Users`, `dbo`. No quoting.
    None,
    /// T-SQL bracket quoting — `[Users]`. Allows reserved words and
    /// special characters in identifier names.
    Brackets,
    /// ANSI / Postgres double quotes — `"Users"`. Same role as
    /// brackets; T-SQL accepts both via `QUOTED_IDENTIFIER` setting.
    DoubleQuote,
    /// MySQL backticks — `` `Users` ``. Same role; included for
    /// future cross-dialect coverage.
    Backtick,
}

impl QuoteStyle {
    /// Marker element name to emit on the wrapping element when
    /// rendering this quoting style — empty for `None`.
    pub const fn marker_name(self) -> Option<&'static str> {
        match self {
            QuoteStyle::None => None,
            QuoteStyle::Brackets => Some("bracketed"),
            QuoteStyle::DoubleQuote => Some("quoted"),
            QuoteStyle::Backtick => Some("backticked"),
        }
    }

    /// Wrap a parsed identifier value in this quoting style for
    /// canonical-source reconstruction. `None` returns the value
    /// unchanged.
    pub fn wrap(self, value: &str) -> String {
        match self {
            QuoteStyle::None => value.to_string(),
            QuoteStyle::Brackets => format!("[{value}]"),
            QuoteStyle::DoubleQuote => format!("\"{value}\""),
            QuoteStyle::Backtick => format!("`{value}`"),
        }
    }
}

/// Comparison operator for `<compare>`. `Op` is the canonical
/// classification; the source text is recoverable via the operand
/// ranges flanking the op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOp {
    Equal,         // =
    NotEqual,      // <> / !=
    Less,          // <
    LessEqual,     // <=
    Greater,       // >
    GreaterEqual,  // >=
    Like,
    In,
    Is,            // IS NULL / IS NOT NULL
    IsNot,
    And,           // logical AND
    Or,            // logical OR
}

/// Arithmetic / bitwise / logical binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
    Concat,        // `||` (some dialects)
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
}

/// Unary operator: `NOT expr`, `-expr`, `+expr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Negate,
    Positive,
}

/// JOIN direction marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinKind {
    Inner,         // default — no marker
    Left,
    Right,
    Full,
    LeftOuter,
    RightOuter,
    FullOuter,
    Cross,
}

/// CREATE statement variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateKind {
    Table,
    View,
    Index,
}

/// DROP statement variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropKind {
    Table,
    Index,
}

impl SqlTree {
    /// Source byte range covered by this node.
    pub fn range(&self) -> ByteRange {
        match self {
            SqlTree::File { range, .. }
            | SqlTree::Statement { range, .. }
            | SqlTree::Go { range, .. }
            | SqlTree::Exec { range, .. }
            | SqlTree::Set { range, .. }
            | SqlTree::Select { range, .. }
            | SqlTree::Insert { range, .. }
            | SqlTree::Update { range, .. }
            | SqlTree::Delete { range, .. }
            | SqlTree::Merge { range, .. }
            | SqlTree::MergeWhen { range, .. }
            | SqlTree::Transaction { range, .. }
            | SqlTree::From { range, .. }
            | SqlTree::Where { range, .. }
            | SqlTree::GroupBy { range, .. }
            | SqlTree::Having { range, .. }
            | SqlTree::OrderBy { range, .. }
            | SqlTree::OrderTarget { range, .. }
            | SqlTree::PartitionBy { range, .. }
            | SqlTree::Join { range, .. }
            | SqlTree::Relation { range, .. }
            | SqlTree::Column { range, .. }
            | SqlTree::Star { range, .. }
            | SqlTree::Reference { range, .. }
            | SqlTree::Compare { range, .. }
            | SqlTree::Binary { range, .. }
            | SqlTree::Unary { range, .. }
            | SqlTree::Assign { range, .. }
            | SqlTree::Between { range, .. }
            | SqlTree::Exists { range, .. }
            | SqlTree::Case { range, .. }
            | SqlTree::When { range, .. }
            | SqlTree::Cast { range, .. }
            | SqlTree::Call { range, .. }
            | SqlTree::Window { range, .. }
            | SqlTree::Over { range, .. }
            | SqlTree::Subquery { range, .. }
            | SqlTree::Union { range, .. }
            | SqlTree::Cte { range, .. }
            | SqlTree::Tuple { range, .. }
            | SqlTree::Create { range, .. }
            | SqlTree::Drop { range, .. }
            | SqlTree::Alter { range, .. }
            | SqlTree::ColumnDef { range, .. }
            | SqlTree::Constraint { range, .. }
            | SqlTree::AddColumn { range, .. }
            | SqlTree::AddConstraint { range, .. }
            | SqlTree::Function { range, .. }
            | SqlTree::DataType { range, .. }
            | SqlTree::Identifier { range, .. }
            | SqlTree::Schema { range, .. }
            | SqlTree::Alias { range, .. }
            | SqlTree::Temp { range, .. }
            | SqlTree::Variable { range, .. }
            | SqlTree::Literal { range, .. }
            | SqlTree::Comment { range, .. }
            | SqlTree::Unknown { range, .. } => *range,
        }
    }

    /// Source-location span (line / column).
    pub fn span(&self) -> Span {
        match self {
            SqlTree::File { span, .. }
            | SqlTree::Statement { span, .. }
            | SqlTree::Go { span, .. }
            | SqlTree::Exec { span, .. }
            | SqlTree::Set { span, .. }
            | SqlTree::Select { span, .. }
            | SqlTree::Insert { span, .. }
            | SqlTree::Update { span, .. }
            | SqlTree::Delete { span, .. }
            | SqlTree::Merge { span, .. }
            | SqlTree::MergeWhen { span, .. }
            | SqlTree::Transaction { span, .. }
            | SqlTree::From { span, .. }
            | SqlTree::Where { span, .. }
            | SqlTree::GroupBy { span, .. }
            | SqlTree::Having { span, .. }
            | SqlTree::OrderBy { span, .. }
            | SqlTree::OrderTarget { span, .. }
            | SqlTree::PartitionBy { span, .. }
            | SqlTree::Join { span, .. }
            | SqlTree::Relation { span, .. }
            | SqlTree::Column { span, .. }
            | SqlTree::Star { span, .. }
            | SqlTree::Reference { span, .. }
            | SqlTree::Compare { span, .. }
            | SqlTree::Binary { span, .. }
            | SqlTree::Unary { span, .. }
            | SqlTree::Assign { span, .. }
            | SqlTree::Between { span, .. }
            | SqlTree::Exists { span, .. }
            | SqlTree::Case { span, .. }
            | SqlTree::When { span, .. }
            | SqlTree::Cast { span, .. }
            | SqlTree::Call { span, .. }
            | SqlTree::Window { span, .. }
            | SqlTree::Over { span, .. }
            | SqlTree::Subquery { span, .. }
            | SqlTree::Union { span, .. }
            | SqlTree::Cte { span, .. }
            | SqlTree::Tuple { span, .. }
            | SqlTree::Create { span, .. }
            | SqlTree::Drop { span, .. }
            | SqlTree::Alter { span, .. }
            | SqlTree::ColumnDef { span, .. }
            | SqlTree::Constraint { span, .. }
            | SqlTree::AddColumn { span, .. }
            | SqlTree::AddConstraint { span, .. }
            | SqlTree::Function { span, .. }
            | SqlTree::DataType { span, .. }
            | SqlTree::Identifier { span, .. }
            | SqlTree::Schema { span, .. }
            | SqlTree::Alias { span, .. }
            | SqlTree::Temp { span, .. }
            | SqlTree::Variable { span, .. }
            | SqlTree::Literal { span, .. }
            | SqlTree::Comment { span, .. }
            | SqlTree::Unknown { span, .. } => *span,
        }
    }

    /// Round-trip helper: the original source slice covered by this
    /// node. Equivalent to `self.range().slice(source)`.
    pub fn to_source<'a>(&self, source: &'a str) -> &'a str {
        self.range().slice(source)
    }
}
