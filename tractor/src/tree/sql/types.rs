//! SQL-language tree — a fully typed representation of T-SQL (and
//! eventually MySQL / PostgreSQL / SQLite) constructs. Parallel to
//! [`crate::tree::SyntaxTree`] (programming languages) and [`crate::tree::data::DataTree`]
//! (data languages).
//!
//! ## Why a separate tree
//!
//! Forcing SQL into the cross-language [`SyntaxTree`] enum either:
//! - bloats `SyntaxTree` with SQL-specific variants (Insert, Select, Where,
//!   Subquery, Cte, …) that no programming language uses, OR
//! - forces SQL into the generic `SyntaxTree::SimpleStatement` catch-all,
//!   which loses typed-slot semantics — the source of the JSON
//!   projection heuristics in iters 29-36.
//!
//! Per user direction (2026-05-07): "there is absolutely no rule
//! that says that all languages should use the same tree".
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
//!    `tree = lower_sql_root(parse(s), &s)`. The tree carries every
//!    semantic distinction needed to regenerate equivalent source
//!    from scratch, without consulting source byte ranges.
//!    Identifier quoting style (`[name]` / `"name"` / `` `name` ``)
//!    is captured by [`QuoteStyle`] on the atom variant; `value` is
//!    the parsed unquoted text. Implemented as the canonical-mode
//!    arm of [`crate::tree::render::render_sql`] in `source/sql.rs`,
//!    fitting the existing per-language source-rendering convention.

use crate::tree::types::{ByteRange, Marker, QuoteStyle, Span, TreeNode};

/// Typed SQL tree.
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

    /// `<go>` — T-SQL batch separator. `text` is the verbatim keyword
    /// (`GO` / `go` — case preserved from source). Mirrors the
    /// `text: String` shape on `SyntaxTree` scalar leaves so the
    /// mechanical walker can emit leaf content without consulting
    /// source. Synthetic constructions (round-trip, mutation) set
    /// `text` to `"GO"`.
    Go { text: String, range: ByteRange, span: Span },

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
    /// Direction is carried as `extra_markers` (either `<asc/>` or
    /// `<desc/>` — none when omitted, defaulting to ASC). Matches the
    /// SyntaxTree pattern: typed `Marker` wrapper rather than an enum
    /// field, so the mechanical walker emits markers without any
    /// per-variant codegen rule.
    OrderTarget {
        expression: Box<SqlTree>,
        extra_markers: Vec<Marker>,
        range: ByteRange,
        span: Span,
    },

    /// `<partition>` — `PARTITION BY col, col, ...` inside OVER().
    PartitionBy { keys: Vec<SqlTree>, range: ByteRange, span: Span },

    /// `<join>` — one JOIN clause: `[LEFT|RIGHT|FULL|INNER|CROSS] JOIN relation ON cond`.
    /// Direction / outer-ness / cross is carried as `extra_markers`
    /// — `<left/>`, `<right/>`, `<full/>`, `<outer/>`, `<cross/>` —
    /// matching the SyntaxTree pattern of typed Marker rather than
    /// an enum field. The mechanical Vec<Marker> codegen rule emits
    /// each marker; no JoinKind enum needed.
    Join {
        relation: Box<SqlTree>,
        on: Option<Box<SqlTree>>,        // None for CROSS JOIN
        extra_markers: Vec<Marker>,
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

    /// `<compare>` — binary comparison: `a = b`, `a > b`, `a IN (...)`,
    /// etc. Shape mirrors [`SyntaxTree::Binary`] exactly so the
    /// mechanical walker treats it the same way. `op_text` is the
    /// verbatim source operator (`=`, `<>`, `LIKE`), `op_marker` is the
    /// canonical marker name (`equal`, `not_equal`, `like`), and
    /// `op_range` covers the operator token(s). Op-as-marker emission
    /// is shared with SyntaxTree::Binary and lands together with
    /// parity-track Pass 3 (typed `Op` slot variant).
    Compare {
        left: Box<SqlTree>,
        op_text: String,
        op_marker: &'static str,
        op_range: ByteRange,
        right: Box<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<binary>` — arithmetic or logical: `a + b`, `a AND b`. Same
    /// shape as [`SyntaxTree::Binary`] — see `Compare` for the op*
    /// field semantics.
    Binary {
        left: Box<SqlTree>,
        op_text: String,
        op_marker: &'static str,
        op_range: ByteRange,
        right: Box<SqlTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<unary>` — `NOT expr`, `-expr`. Same op* shape as `Binary`
    /// / [`SyntaxTree::Unary`].
    Unary {
        op_text: String,
        op_marker: &'static str,
        op_range: ByteRange,
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
    /// MySQL `` `name` ``) is captured in `quoting` so the tree carries
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

    /// `<var>` — `@variable` reference. `text` is the verbatim source
    /// (`@name`, `@@name`, etc.). Mirrors `SyntaxTree::Name` shape so
    /// `scalar_text_of` reads from the tree, not from source.
    Variable { text: String, range: ByteRange, span: Span },

    /// `<literal>` — string / numeric / hex literal. `text` is verbatim
    /// from the source range (quotes included for string literals).
    Literal { text: String, range: ByteRange, span: Span },

    /// `<comment>` — `-- line` or `/* block */`. `text` is the verbatim
    /// source including the delimiter.
    Comment { text: String, range: ByteRange, span: Span },

    // ----- Escape hatches ------------------------------------------------

    /// `<unknown kind="…">` — last-resort fallback for un-handled
    /// CST kinds. Visible in output so coverage gaps show up.
    Unknown {
        kind: String,
        range: ByteRange,
        span: Span,
    },
}

// Sort direction (ASC / DESC) carried via `OrderTarget.extra_markers`
// rather than an enum field — same shape as the SyntaxTree pattern.
// Marker names: "asc" / "desc". None ⇒ default (typically ASC).

// `QuoteStyle` is the shared [`crate::tree::types::QuoteStyle`]; SqlTree
// used to carry its own local copy with `{None, Brackets, DoubleQuote,
// Backtick}`. The shared enum covers all four (mapping `None →
// Plain`, `DoubleQuote → Double`) plus the variants other tree types
// need. Migration: SqlTree atoms now reference the shared type via the
// re-export in `tree::sql::mod`.

// Op classification is carried as the `op_marker: &'static str`
// field on `Binary` / `Compare` / `Unary` — matching SyntaxTree
// exactly. Canonical marker names: `equal`, `not_equal`, `less`,
// `less_equal`, `greater`, `greater_equal`, `like`, `in`, `is`,
// `is_not`, `and`, `or` (comparisons); `plus`, `minus`, `multiply`,
// `divide`, `modulo`, `concat`, `bitwise_and`, `bitwise_or`,
// `bitwise_xor` (binary arith); `not`, `negate`, `positive` (unary).
// The strings are interned (`&'static str`) because the set is closed
// — codegen-time enforcement happens through the lowering's match
// arms over CST kinds.

// JOIN direction carried via `Join.extra_markers`. Marker names:
// "left", "right", "full", "outer", "cross". No marker ⇒ INNER
// (the default). LEFT OUTER = ["left", "outer"]; RIGHT OUTER =
// ["right", "outer"]; FULL OUTER = ["full", "outer"]. Same shape
// pattern as SyntaxTree's `<for[async]>` / `<unary[prefix]>`.

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

impl TreeNode for SqlTree {
    fn span(&self) -> Span { self.span() }
    fn range(&self) -> ByteRange { self.range() }

    fn span_mut(&mut self) -> &mut Span {
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
            | SqlTree::Unknown { span, .. } => span,
        }
    }

    fn children(&self) -> Vec<&Self> {
        let mut v: Vec<&SqlTree> = Vec::new();
        match self {
            SqlTree::File { statements, .. }
            | SqlTree::Transaction { statements, .. } => v.extend(statements.iter()),
            SqlTree::Statement { inner, .. } => v.push(inner),
            SqlTree::Go { .. } => {}
            SqlTree::Exec { target, .. } => v.push(target),
            SqlTree::Set { target, value, .. }
            | SqlTree::Assign { target, value, .. } => {
                v.push(target);
                v.push(value);
            }
            SqlTree::Select { ctes, columns, into, from, where_, group_by, having, order_by, .. } => {
                v.extend(ctes.iter());
                v.extend(columns.iter());
                if let Some(i) = into { v.push(i); }
                if let Some(f) = from { v.push(f); }
                if let Some(w) = where_ { v.push(w); }
                if let Some(g) = group_by { v.push(g); }
                if let Some(h) = having { v.push(h); }
                if let Some(o) = order_by { v.push(o); }
            }
            SqlTree::Insert { table, columns, values, .. } => {
                v.push(table);
                v.extend(columns.iter());
                v.extend(values.iter());
            }
            SqlTree::Update { table, assignments, where_, .. } => {
                v.push(table);
                v.extend(assignments.iter());
                if let Some(w) = where_ { v.push(w); }
            }
            SqlTree::Delete { from, where_, .. } => {
                if let Some(f) = from { v.push(f); }
                if let Some(w) = where_ { v.push(w); }
            }
            SqlTree::Merge { target, source, on, whens, .. } => {
                v.push(target);
                v.push(source);
                v.push(on);
                v.extend(whens.iter());
            }
            SqlTree::MergeWhen { action, .. } => v.push(action),
            SqlTree::From { relations, .. } => v.extend(relations.iter()),
            SqlTree::Where { condition, .. }
            | SqlTree::Having { condition, .. } => v.push(condition),
            SqlTree::GroupBy { keys, .. }
            | SqlTree::PartitionBy { keys, .. } => v.extend(keys.iter()),
            SqlTree::OrderBy { targets, .. } => v.extend(targets.iter()),
            SqlTree::OrderTarget { expression, .. } => v.push(expression),
            SqlTree::Join { relation, on, .. } => {
                v.push(relation);
                if let Some(o) = on { v.push(o); }
            }
            SqlTree::Relation { schema, name, alias, .. } => {
                if let Some(s) = schema { v.push(s); }
                v.push(name);
                if let Some(a) = alias { v.push(a); }
            }
            SqlTree::Column { expression, alias, .. } => {
                v.push(expression);
                if let Some(a) = alias { v.push(a); }
            }
            SqlTree::Star { qualifier, .. } => {
                if let Some(q) = qualifier { v.push(q); }
            }
            SqlTree::Reference { parts, .. } => v.extend(parts.iter()),
            SqlTree::Compare { left, right, .. }
            | SqlTree::Binary { left, right, .. } => {
                v.push(left);
                v.push(right);
            }
            SqlTree::Unary { operand, .. } => v.push(operand),
            SqlTree::Between { value, low, high, .. } => {
                v.push(value);
                v.push(low);
                v.push(high);
            }
            SqlTree::Exists { subquery, .. } => v.push(subquery),
            SqlTree::Case { whens, else_, .. } => {
                v.extend(whens.iter());
                if let Some(e) = else_ { v.push(e); }
            }
            SqlTree::When { condition, value, .. } => {
                v.push(condition);
                v.push(value);
            }
            SqlTree::Cast { value, type_, .. } => {
                v.push(value);
                v.push(type_);
            }
            SqlTree::Call { callee, arguments, .. } => {
                v.push(callee);
                v.extend(arguments.iter());
            }
            SqlTree::Window { call, over, .. } => {
                v.push(call);
                v.push(over);
            }
            SqlTree::Over { partition_by, order_by, .. } => {
                if let Some(p) = partition_by { v.push(p); }
                if let Some(o) = order_by { v.push(o); }
            }
            SqlTree::Subquery { select, .. } => v.push(select),
            SqlTree::Union { selects, .. } => v.extend(selects.iter()),
            SqlTree::Cte { name, query, .. } => {
                v.push(name);
                v.push(query);
            }
            SqlTree::Tuple { items, .. } => v.extend(items.iter()),
            SqlTree::Create { name, body, .. } => {
                v.push(name);
                v.extend(body.iter());
            }
            SqlTree::Drop { name, .. } => v.push(name),
            SqlTree::Alter { name, operation, .. } => {
                v.push(name);
                v.push(operation);
            }
            SqlTree::ColumnDef { name, type_, constraints, .. } => {
                v.push(name);
                v.push(type_);
                v.extend(constraints.iter());
            }
            SqlTree::Constraint { name, body, .. } => {
                if let Some(n) = name { v.push(n); }
                v.extend(body.iter());
            }
            SqlTree::AddColumn { column, .. } => v.push(column),
            SqlTree::AddConstraint { constraint, .. } => v.push(constraint),
            SqlTree::Function { schema, name, parameters, return_type, body, .. } => {
                if let Some(s) = schema { v.push(s); }
                v.push(name);
                v.extend(parameters.iter());
                if let Some(r) = return_type { v.push(r); }
                v.push(body);
            }
            SqlTree::DataType { length, .. } => {
                if let Some(l) = length { v.push(l); }
            }
            SqlTree::Temp { name, .. } => v.push(name),
            // Leaves — no SqlTree children.
            SqlTree::Identifier { .. }
            | SqlTree::Schema { .. }
            | SqlTree::Alias { .. }
            | SqlTree::Variable { .. }
            | SqlTree::Literal { .. }
            | SqlTree::Comment { .. }
            | SqlTree::Unknown { .. } => {}
        }
        v.sort_by_key(|c| c.range().start);
        v
    }

    fn children_mut(&mut self) -> Vec<&mut Self> {
        let mut v: Vec<&mut SqlTree> = Vec::new();
        match self {
            SqlTree::File { statements, .. }
            | SqlTree::Transaction { statements, .. } => v.extend(statements.iter_mut()),
            SqlTree::Statement { inner, .. } => v.push(inner.as_mut()),
            SqlTree::Go { .. } => {}
            SqlTree::Exec { target, .. } => v.push(target.as_mut()),
            SqlTree::Set { target, value, .. }
            | SqlTree::Assign { target, value, .. } => {
                v.push(target.as_mut());
                v.push(value.as_mut());
            }
            SqlTree::Select { ctes, columns, into, from, where_, group_by, having, order_by, .. } => {
                v.extend(ctes.iter_mut());
                v.extend(columns.iter_mut());
                if let Some(i) = into { v.push(i.as_mut()); }
                if let Some(f) = from { v.push(f.as_mut()); }
                if let Some(w) = where_ { v.push(w.as_mut()); }
                if let Some(g) = group_by { v.push(g.as_mut()); }
                if let Some(h) = having { v.push(h.as_mut()); }
                if let Some(o) = order_by { v.push(o.as_mut()); }
            }
            SqlTree::Insert { table, columns, values, .. } => {
                v.push(table.as_mut());
                v.extend(columns.iter_mut());
                v.extend(values.iter_mut());
            }
            SqlTree::Update { table, assignments, where_, .. } => {
                v.push(table.as_mut());
                v.extend(assignments.iter_mut());
                if let Some(w) = where_ { v.push(w.as_mut()); }
            }
            SqlTree::Delete { from, where_, .. } => {
                if let Some(f) = from { v.push(f.as_mut()); }
                if let Some(w) = where_ { v.push(w.as_mut()); }
            }
            SqlTree::Merge { target, source, on, whens, .. } => {
                v.push(target.as_mut());
                v.push(source.as_mut());
                v.push(on.as_mut());
                v.extend(whens.iter_mut());
            }
            SqlTree::MergeWhen { action, .. } => v.push(action.as_mut()),
            SqlTree::From { relations, .. } => v.extend(relations.iter_mut()),
            SqlTree::Where { condition, .. }
            | SqlTree::Having { condition, .. } => v.push(condition.as_mut()),
            SqlTree::GroupBy { keys, .. }
            | SqlTree::PartitionBy { keys, .. } => v.extend(keys.iter_mut()),
            SqlTree::OrderBy { targets, .. } => v.extend(targets.iter_mut()),
            SqlTree::OrderTarget { expression, .. } => v.push(expression.as_mut()),
            SqlTree::Join { relation, on, .. } => {
                v.push(relation.as_mut());
                if let Some(o) = on { v.push(o.as_mut()); }
            }
            SqlTree::Relation { schema, name, alias, .. } => {
                if let Some(s) = schema { v.push(s.as_mut()); }
                v.push(name.as_mut());
                if let Some(a) = alias { v.push(a.as_mut()); }
            }
            SqlTree::Column { expression, alias, .. } => {
                v.push(expression.as_mut());
                if let Some(a) = alias { v.push(a.as_mut()); }
            }
            SqlTree::Star { qualifier, .. } => {
                if let Some(q) = qualifier { v.push(q.as_mut()); }
            }
            SqlTree::Reference { parts, .. } => v.extend(parts.iter_mut()),
            SqlTree::Compare { left, right, .. }
            | SqlTree::Binary { left, right, .. } => {
                v.push(left.as_mut());
                v.push(right.as_mut());
            }
            SqlTree::Unary { operand, .. } => v.push(operand.as_mut()),
            SqlTree::Between { value, low, high, .. } => {
                v.push(value.as_mut());
                v.push(low.as_mut());
                v.push(high.as_mut());
            }
            SqlTree::Exists { subquery, .. } => v.push(subquery.as_mut()),
            SqlTree::Case { whens, else_, .. } => {
                v.extend(whens.iter_mut());
                if let Some(e) = else_ { v.push(e.as_mut()); }
            }
            SqlTree::When { condition, value, .. } => {
                v.push(condition.as_mut());
                v.push(value.as_mut());
            }
            SqlTree::Cast { value, type_, .. } => {
                v.push(value.as_mut());
                v.push(type_.as_mut());
            }
            SqlTree::Call { callee, arguments, .. } => {
                v.push(callee.as_mut());
                v.extend(arguments.iter_mut());
            }
            SqlTree::Window { call, over, .. } => {
                v.push(call.as_mut());
                v.push(over.as_mut());
            }
            SqlTree::Over { partition_by, order_by, .. } => {
                if let Some(p) = partition_by { v.push(p.as_mut()); }
                if let Some(o) = order_by { v.push(o.as_mut()); }
            }
            SqlTree::Subquery { select, .. } => v.push(select.as_mut()),
            SqlTree::Union { selects, .. } => v.extend(selects.iter_mut()),
            SqlTree::Cte { name, query, .. } => {
                v.push(name.as_mut());
                v.push(query.as_mut());
            }
            SqlTree::Tuple { items, .. } => v.extend(items.iter_mut()),
            SqlTree::Create { name, body, .. } => {
                v.push(name.as_mut());
                v.extend(body.iter_mut());
            }
            SqlTree::Drop { name, .. } => v.push(name.as_mut()),
            SqlTree::Alter { name, operation, .. } => {
                v.push(name.as_mut());
                v.push(operation.as_mut());
            }
            SqlTree::ColumnDef { name, type_, constraints, .. } => {
                v.push(name.as_mut());
                v.push(type_.as_mut());
                v.extend(constraints.iter_mut());
            }
            SqlTree::Constraint { name, body, .. } => {
                if let Some(n) = name { v.push(n.as_mut()); }
                v.extend(body.iter_mut());
            }
            SqlTree::AddColumn { column, .. } => v.push(column.as_mut()),
            SqlTree::AddConstraint { constraint, .. } => v.push(constraint.as_mut()),
            SqlTree::Function { schema, name, parameters, return_type, body, .. } => {
                if let Some(s) = schema { v.push(s.as_mut()); }
                v.push(name.as_mut());
                v.extend(parameters.iter_mut());
                if let Some(r) = return_type { v.push(r.as_mut()); }
                v.push(body.as_mut());
            }
            SqlTree::DataType { length, .. } => {
                if let Some(l) = length { v.push(l.as_mut()); }
            }
            SqlTree::Temp { name, .. } => v.push(name.as_mut()),
            // Leaves — no SqlTree children.
            SqlTree::Identifier { .. }
            | SqlTree::Schema { .. }
            | SqlTree::Alias { .. }
            | SqlTree::Variable { .. }
            | SqlTree::Literal { .. }
            | SqlTree::Comment { .. }
            | SqlTree::Unknown { .. } => {}
        }
        v.sort_by_key(|c| c.range().start);
        v
    }
}
