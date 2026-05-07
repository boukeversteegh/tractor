//! SQL-language IR — a fully typed representation of T-SQL (and
//! eventually MySQL / PostgreSQL / SQLite) constructs. Parallel to
//! [`crate::ir::Ir`] (programming languages) and [`crate::ir::data::DataIr`]
//! (data languages).
//!
//! ## Why a separate IR
//!
//! Forcing SQL into the cross-language [`Ir`] enum either:
//! - bloats `Ir` with SQL-specific variants (Insert, Select, Where,
//!   Subquery, Cte, …) that no programming language uses, OR
//! - forces SQL into the generic `Ir::SimpleStatement` catch-all,
//!   which loses typed-slot semantics — the source of the JSON
//!   projection heuristics in iters 29-36.
//!
//! Per user direction (2026-05-07): "there is absolutely no rule
//! that says that all languages should use the same IR".
//!
//! `SqlIr` gives every SQL construct a typed shape. Both XML and
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
//! Same three as `Ir` and `DataIr`:
//! 1. **Round-trip identity** — `to_source(sql_ir, source) == source`.
//! 2. **XPath text recovery** — `string(rendered_root) == source`.
//! 3. **No silent drops** — un-handled CST kinds fall through to
//!    [`SqlIr::Unknown`].

#![cfg(feature = "native")]

use super::types::{ByteRange, Span};

/// Typed SQL IR.
#[derive(Debug, Clone)]
pub enum SqlIr {
    // ----- Top level -----------------------------------------------------

    /// `<file>` — top-level container of statements.
    File {
        statements: Vec<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    /// `<statement>` — a generic wrapper for a statement node when
    /// the inner kind is one of the typed variants below. Renders
    /// as `<statement>{inner_render}</statement>`. The inner is
    /// any other `SqlIr` variant.
    Statement {
        inner: Box<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    /// `<go>` — T-SQL batch separator.
    Go { range: ByteRange, span: Span },

    /// `<exec>` — `EXEC sp_helpdb`.
    Exec {
        target: Box<SqlIr>,           // SqlIr::Identifier or SqlIr::Call
        range: ByteRange,
        span: Span,
    },

    /// `<set>` — `SET @var = expr`.
    Set {
        target: Box<SqlIr>,           // SqlIr::Variable
        value: Box<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    // ----- DML statements ------------------------------------------------

    /// `<select>` — `SELECT cols FROM ... WHERE ... GROUP BY ... HAVING ... ORDER BY ...`.
    /// Each clause is its own typed slot.
    Select {
        ctes: Vec<SqlIr>,             // WITH ... AS (...) CTE clauses (each SqlIr::Cte)
        columns: Vec<SqlIr>,          // each is SqlIr::Column or SqlIr::Star
        into: Option<Box<SqlIr>>,     // SELECT ... INTO #temp
        from: Option<Box<SqlIr>>,     // SqlIr::From
        where_: Option<Box<SqlIr>>,   // SqlIr::Where
        group_by: Option<Box<SqlIr>>, // SqlIr::GroupBy
        having: Option<Box<SqlIr>>,   // SqlIr::Having
        order_by: Option<Box<SqlIr>>, // SqlIr::OrderBy
        range: ByteRange,
        span: Span,
    },

    /// `<insert>` — `INSERT INTO table (cols) VALUES (vals)`.
    Insert {
        table: Box<SqlIr>,            // SqlIr::Relation
        columns: Vec<SqlIr>,          // empty when columns omitted
        values: Vec<SqlIr>,           // each row is a SqlIr::Tuple
        range: ByteRange,
        span: Span,
    },

    /// `<update>` — `UPDATE table SET col=val,... WHERE ...`.
    Update {
        table: Box<SqlIr>,
        assignments: Vec<SqlIr>,      // each is SqlIr::Assign
        where_: Option<Box<SqlIr>>,
        range: ByteRange,
        span: Span,
    },

    /// `<delete>` — `DELETE FROM table WHERE ...`.
    Delete {
        from: Option<Box<SqlIr>>,
        where_: Option<Box<SqlIr>>,
        range: ByteRange,
        span: Span,
    },

    /// `<merge>` — `MERGE INTO target USING source ON ... WHEN ...`.
    Merge {
        target: Box<SqlIr>,           // SqlIr::Relation
        source: Box<SqlIr>,           // SqlIr::Relation or SqlIr::Subquery
        on: Box<SqlIr>,               // join condition
        whens: Vec<SqlIr>,            // each is SqlIr::MergeWhen
        range: ByteRange,
        span: Span,
    },

    /// `<when>` inside MERGE — `WHEN MATCHED THEN UPDATE ...` / `WHEN NOT MATCHED THEN INSERT ...`.
    MergeWhen {
        matched: bool,                 // MATCHED vs NOT MATCHED
        action: Box<SqlIr>,            // SqlIr::Update / SqlIr::Insert / SqlIr::Delete
        range: ByteRange,
        span: Span,
    },

    /// `<transaction>` — `BEGIN TRANSACTION ... COMMIT`.
    Transaction {
        statements: Vec<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    // ----- Clauses (children of DML statements) --------------------------

    /// `<from>` — `FROM relation [JOIN ...]`.
    From {
        relations: Vec<SqlIr>,         // first is base; subsequent are joins
        range: ByteRange,
        span: Span,
    },

    /// `<where>` — `WHERE condition`.
    Where { condition: Box<SqlIr>, range: ByteRange, span: Span },

    /// `<group>` — `GROUP BY col, col, ...`.
    GroupBy { keys: Vec<SqlIr>, range: ByteRange, span: Span },

    /// `<having>` — `HAVING condition`.
    Having { condition: Box<SqlIr>, range: ByteRange, span: Span },

    /// `<order>` — `ORDER BY target [ASC|DESC], ...`.
    OrderBy { targets: Vec<SqlIr>, range: ByteRange, span: Span },

    /// `<target>` — one item in ORDER BY: `expr [ASC|DESC]`.
    OrderTarget {
        expression: Box<SqlIr>,
        direction: Option<SortDirection>,
        range: ByteRange,
        span: Span,
    },

    /// `<partition>` — `PARTITION BY col, col, ...` inside OVER().
    PartitionBy { keys: Vec<SqlIr>, range: ByteRange, span: Span },

    /// `<join>` — one JOIN clause: `[LEFT|RIGHT|FULL|INNER|CROSS] JOIN relation ON cond`.
    Join {
        kind: JoinKind,
        relation: Box<SqlIr>,
        on: Option<Box<SqlIr>>,        // None for CROSS JOIN
        range: ByteRange,
        span: Span,
    },

    // ----- References and columns ----------------------------------------

    /// `<relation>` — a table reference, optionally aliased and schema-qualified.
    Relation {
        schema: Option<Box<SqlIr>>,    // SqlIr::Identifier
        name: Box<SqlIr>,              // SqlIr::Identifier
        alias: Option<Box<SqlIr>>,     // SqlIr::Identifier
        range: ByteRange,
        span: Span,
    },

    /// `<column>` in SELECT clause — `expr [AS alias]`.
    Column {
        expression: Box<SqlIr>,
        alias: Option<Box<SqlIr>>,
        range: ByteRange,
        span: Span,
    },

    /// `<star>` — `*` (unqualified or `relation.*`).
    Star {
        qualifier: Option<Box<SqlIr>>, // for `t.*`
        range: ByteRange,
        span: Span,
    },

    /// `<reference>` — column reference, possibly qualified:
    /// `name`, `t.name`, `dbo.t.name`, etc.
    Reference {
        parts: Vec<SqlIr>,             // each is SqlIr::Identifier
        range: ByteRange,
        span: Span,
    },

    // ----- Expressions ---------------------------------------------------

    /// `<compare>` — binary comparison: `a = b`, `a > b`, `a IN (...)`, etc.
    Compare {
        left: Box<SqlIr>,
        op: ComparisonOp,
        right: Box<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    /// `<binary>` — arithmetic or logical: `a + b`, `a AND b`.
    Binary {
        left: Box<SqlIr>,
        op: BinaryOp,
        right: Box<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    /// `<unary>` — `NOT expr`, `-expr`.
    Unary {
        op: UnaryOp,
        operand: Box<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    /// `<assign>` — `SET col = expr` (in UPDATE) or `SET @var = expr`.
    Assign {
        target: Box<SqlIr>,
        value: Box<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    /// `<between>` — `expr BETWEEN low AND high`.
    Between {
        value: Box<SqlIr>,
        low: Box<SqlIr>,
        high: Box<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    /// `<exists>` — `EXISTS (subquery)`.
    Exists {
        subquery: Box<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    /// `<case>` — `CASE WHEN ... THEN ... [ELSE ...] END`.
    Case {
        whens: Vec<SqlIr>,             // each is SqlIr::When
        else_: Option<Box<SqlIr>>,
        range: ByteRange,
        span: Span,
    },

    /// `<when>` — one `WHEN cond THEN value` arm in a CASE.
    When {
        condition: Box<SqlIr>,
        value: Box<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    /// `<cast>` — `CAST(expr AS type)`.
    Cast {
        value: Box<SqlIr>,
        type_: Box<SqlIr>,             // SqlIr::DataType
        range: ByteRange,
        span: Span,
    },

    /// `<call>` — function invocation: `LOWER(x)`, `COUNT(*)`, etc.
    Call {
        callee: Box<SqlIr>,            // SqlIr::Identifier
        arguments: Vec<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    /// `<window>` — windowed aggregate: `func() OVER (...)`.
    Window {
        call: Box<SqlIr>,              // SqlIr::Call
        over: Box<SqlIr>,              // SqlIr::Over
        range: ByteRange,
        span: Span,
    },

    /// `<over>` — `OVER (PARTITION BY ... ORDER BY ...)`.
    Over {
        partition_by: Option<Box<SqlIr>>,
        order_by: Option<Box<SqlIr>>,
        range: ByteRange,
        span: Span,
    },

    /// `<subquery>` — parenthesized SELECT.
    Subquery {
        select: Box<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    /// `<union>` — `SELECT ... UNION [ALL] SELECT ...`.
    Union {
        all: bool,
        selects: Vec<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    /// `<cte>` — `WITH name AS (SELECT ...)`.
    Cte {
        name: Box<SqlIr>,
        query: Box<SqlIr>,             // SqlIr::Select
        range: ByteRange,
        span: Span,
    },

    /// `<tuple>` — `(a, b, c)` value list (used in INSERT VALUES rows
    /// and IN lists).
    Tuple {
        items: Vec<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    // ----- DDL ------------------------------------------------------------

    /// `<create>` — `CREATE TABLE / VIEW / INDEX name ...`.
    Create {
        kind: CreateKind,
        name: Box<SqlIr>,              // SqlIr::Identifier
        body: Vec<SqlIr>,              // columns / select / index_fields
        range: ByteRange,
        span: Span,
    },

    /// `<drop>` — `DROP TABLE / INDEX name`.
    Drop {
        kind: DropKind,
        name: Box<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    /// `<alter>` — `ALTER TABLE name operation`.
    Alter {
        name: Box<SqlIr>,
        operation: Box<SqlIr>,         // SqlIr::AddColumn / SqlIr::AddConstraint / ...
        range: ByteRange,
        span: Span,
    },

    /// `<column>` (DDL) — column definition in CREATE TABLE.
    ColumnDef {
        name: Box<SqlIr>,
        type_: Box<SqlIr>,
        constraints: Vec<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    /// `<constraint>` — table constraint (PRIMARY KEY, FOREIGN KEY, …).
    Constraint {
        name: Option<Box<SqlIr>>,
        body: Vec<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    /// `<add>` — `ADD COLUMN col TYPE` operation in ALTER TABLE.
    AddColumn {
        column: Box<SqlIr>,            // SqlIr::ColumnDef
        range: ByteRange,
        span: Span,
    },

    /// `<add>` for constraints — `ADD CONSTRAINT name ...`.
    AddConstraint {
        constraint: Box<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    /// `<function>` — `CREATE FUNCTION ... AS BEGIN ... END`.
    Function {
        schema: Option<Box<SqlIr>>,
        name: Box<SqlIr>,
        parameters: Vec<SqlIr>,
        return_type: Option<Box<SqlIr>>,
        body: Box<SqlIr>,
        range: ByteRange,
        span: Span,
    },

    // ----- Type expressions ----------------------------------------------

    /// `<int>` / `<varchar>` / `<nvarchar>` / `<datetime>` etc. —
    /// SQL data type. `length` is `(n)` for VARCHAR(n) or absent for
    /// fixed-width types.
    DataType {
        name: &'static str,            // "int", "varchar", "datetime", ...
        length: Option<Box<SqlIr>>,    // SqlIr::Integer
        range: ByteRange,
        span: Span,
    },

    // ----- Atoms ---------------------------------------------------------

    /// `<name>` — bare identifier text.
    Identifier { range: ByteRange, span: Span },

    /// `<schema>` — schema qualifier in `dbo.Table`.
    Schema { range: ByteRange, span: Span },

    /// `<alias>` — alias position identifier (`t` in `Users t`).
    Alias { range: ByteRange, span: Span },

    /// `<temp>` — temp-table qualifier `#name` / `##name`.
    Temp { name: Box<SqlIr>, range: ByteRange, span: Span },

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

impl SqlIr {
    /// Source byte range covered by this node.
    pub fn range(&self) -> ByteRange {
        match self {
            SqlIr::File { range, .. }
            | SqlIr::Statement { range, .. }
            | SqlIr::Go { range, .. }
            | SqlIr::Exec { range, .. }
            | SqlIr::Set { range, .. }
            | SqlIr::Select { range, .. }
            | SqlIr::Insert { range, .. }
            | SqlIr::Update { range, .. }
            | SqlIr::Delete { range, .. }
            | SqlIr::Merge { range, .. }
            | SqlIr::MergeWhen { range, .. }
            | SqlIr::Transaction { range, .. }
            | SqlIr::From { range, .. }
            | SqlIr::Where { range, .. }
            | SqlIr::GroupBy { range, .. }
            | SqlIr::Having { range, .. }
            | SqlIr::OrderBy { range, .. }
            | SqlIr::OrderTarget { range, .. }
            | SqlIr::PartitionBy { range, .. }
            | SqlIr::Join { range, .. }
            | SqlIr::Relation { range, .. }
            | SqlIr::Column { range, .. }
            | SqlIr::Star { range, .. }
            | SqlIr::Reference { range, .. }
            | SqlIr::Compare { range, .. }
            | SqlIr::Binary { range, .. }
            | SqlIr::Unary { range, .. }
            | SqlIr::Assign { range, .. }
            | SqlIr::Between { range, .. }
            | SqlIr::Exists { range, .. }
            | SqlIr::Case { range, .. }
            | SqlIr::When { range, .. }
            | SqlIr::Cast { range, .. }
            | SqlIr::Call { range, .. }
            | SqlIr::Window { range, .. }
            | SqlIr::Over { range, .. }
            | SqlIr::Subquery { range, .. }
            | SqlIr::Union { range, .. }
            | SqlIr::Cte { range, .. }
            | SqlIr::Tuple { range, .. }
            | SqlIr::Create { range, .. }
            | SqlIr::Drop { range, .. }
            | SqlIr::Alter { range, .. }
            | SqlIr::ColumnDef { range, .. }
            | SqlIr::Constraint { range, .. }
            | SqlIr::AddColumn { range, .. }
            | SqlIr::AddConstraint { range, .. }
            | SqlIr::Function { range, .. }
            | SqlIr::DataType { range, .. }
            | SqlIr::Identifier { range, .. }
            | SqlIr::Schema { range, .. }
            | SqlIr::Alias { range, .. }
            | SqlIr::Temp { range, .. }
            | SqlIr::Variable { range, .. }
            | SqlIr::Literal { range, .. }
            | SqlIr::Comment { range, .. }
            | SqlIr::Unknown { range, .. } => *range,
        }
    }

    /// Source-location span (line / column).
    pub fn span(&self) -> Span {
        match self {
            SqlIr::File { span, .. }
            | SqlIr::Statement { span, .. }
            | SqlIr::Go { span, .. }
            | SqlIr::Exec { span, .. }
            | SqlIr::Set { span, .. }
            | SqlIr::Select { span, .. }
            | SqlIr::Insert { span, .. }
            | SqlIr::Update { span, .. }
            | SqlIr::Delete { span, .. }
            | SqlIr::Merge { span, .. }
            | SqlIr::MergeWhen { span, .. }
            | SqlIr::Transaction { span, .. }
            | SqlIr::From { span, .. }
            | SqlIr::Where { span, .. }
            | SqlIr::GroupBy { span, .. }
            | SqlIr::Having { span, .. }
            | SqlIr::OrderBy { span, .. }
            | SqlIr::OrderTarget { span, .. }
            | SqlIr::PartitionBy { span, .. }
            | SqlIr::Join { span, .. }
            | SqlIr::Relation { span, .. }
            | SqlIr::Column { span, .. }
            | SqlIr::Star { span, .. }
            | SqlIr::Reference { span, .. }
            | SqlIr::Compare { span, .. }
            | SqlIr::Binary { span, .. }
            | SqlIr::Unary { span, .. }
            | SqlIr::Assign { span, .. }
            | SqlIr::Between { span, .. }
            | SqlIr::Exists { span, .. }
            | SqlIr::Case { span, .. }
            | SqlIr::When { span, .. }
            | SqlIr::Cast { span, .. }
            | SqlIr::Call { span, .. }
            | SqlIr::Window { span, .. }
            | SqlIr::Over { span, .. }
            | SqlIr::Subquery { span, .. }
            | SqlIr::Union { span, .. }
            | SqlIr::Cte { span, .. }
            | SqlIr::Tuple { span, .. }
            | SqlIr::Create { span, .. }
            | SqlIr::Drop { span, .. }
            | SqlIr::Alter { span, .. }
            | SqlIr::ColumnDef { span, .. }
            | SqlIr::Constraint { span, .. }
            | SqlIr::AddColumn { span, .. }
            | SqlIr::AddConstraint { span, .. }
            | SqlIr::Function { span, .. }
            | SqlIr::DataType { span, .. }
            | SqlIr::Identifier { span, .. }
            | SqlIr::Schema { span, .. }
            | SqlIr::Alias { span, .. }
            | SqlIr::Temp { span, .. }
            | SqlIr::Variable { span, .. }
            | SqlIr::Literal { span, .. }
            | SqlIr::Comment { span, .. }
            | SqlIr::Unknown { span, .. } => *span,
        }
    }

    /// Round-trip helper: the original source slice covered by this
    /// node. Equivalent to `self.range().slice(source)`.
    pub fn to_source<'a>(&self, source: &'a str) -> &'a str {
        self.range().slice(source)
    }
}
