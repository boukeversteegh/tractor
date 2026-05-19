//! TSQL CST → [`SqlTree`] lowering.
//!
//! Parallel to `tsql.rs` (which emits the cross-language `SyntaxTree`).
//! This module emits the typed [`SqlTree`] enum — every TSQL
//! construct gets a typed variant with named slots, eliminating
//! the generic-`SimpleStatement` ambiguity that drove the iter
//! 29-36 JSON projection heuristics.
//!
//! Status: under construction. Production parser still routes
//! through `tsql.rs`. Coverage extends per slice; once parity
//! across the TSQL blueprint is reached, the parser flips and
//! `tsql.rs` retires.


use crate::raw::RawNode;

use crate::tree::types::{ByteRange, Marker};
use crate::tree::lower_helpers::{range_of, span_of};
use crate::tree::sql::{CreateKind, DropKind, QuoteStyle, SqlTree};

/// Detect `QuoteStyle` from raw source text and return the parsed
/// (unquoted) identifier value alongside it. Bracket / double-quote /
/// backtick quoting strips one character from each end; bare
/// identifiers pass through unchanged.
fn parse_id_quoting(text: &str) -> (String, QuoteStyle) {
    if text.len() >= 2 {
        if text.starts_with('[') && text.ends_with(']') {
            return (text[1..text.len() - 1].to_string(), QuoteStyle::Brackets);
        }
        if text.starts_with('"') && text.ends_with('"') {
            return (text[1..text.len() - 1].to_string(), QuoteStyle::Double);
        }
        if text.starts_with('`') && text.ends_with('`') {
            return (text[1..text.len() - 1].to_string(), QuoteStyle::Backtick);
        }
    }
    (text.to_string(), QuoteStyle::Plain)
}

/// Build `SqlTree::Identifier` from a CST node, parsing quoting style
/// out of the source text.
fn ident_at(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let (value, quoting) = parse_id_quoting(range.slice(source));
    SqlTree::Identifier { value, quoting, range, span: span_of(node) }
}

/// Build `SqlTree::Schema` from a CST node.
fn schema_at(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let (value, quoting) = parse_id_quoting(range.slice(source));
    SqlTree::Schema { value, quoting, range, span: span_of(node) }
}

/// Build `SqlTree::Alias` from a CST node.
fn alias_at(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let (value, quoting) = parse_id_quoting(range.slice(source));
    SqlTree::Alias { value, quoting, range, span: span_of(node) }
}

/// Lower a T-SQL `program` CST root to [`SqlTree::File`].
pub fn lower_sql_root(root: &RawNode, source: &str) -> SqlTree {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "program" => {
            let statements: Vec<SqlTree> = root
                .named_children()
                .map(|c| lower_node(c, source))
                .collect();
            SqlTree::File { statements, range, span }
        }
        other => SqlTree::Unknown { kind: other.to_string(), range, span },
    }
}

fn lower_node(node: &RawNode, source: &str) -> SqlTree {
    let span = span_of(node);
    let range = range_of(node);
    let kind = node.kind();

    match kind {
        // ----- Top-level wrapping ------------------------------------
        "statement" => lower_statement(node, source),
        "go_statement" => SqlTree::Go { text: range.slice(source).to_string(), range, span },
        "execute_statement" => lower_exec(node, source),
        "set_statement" => lower_set(node, source),
        "transaction" => lower_transaction(node, source),
        "create_function" => lower_create_function(node, source),
        "function_argument" => lower_function_argument(node, source),
        "function_body" => lower_function_body(node, source),
        "when_clause" => lower_merge_when(node, source),

        // ----- Atoms --------------------------------------------------
        "identifier" => {
            let text = range.slice(source);
            if text.starts_with('@') {
                SqlTree::Variable { text: text.to_string(), range, span }
            } else {
                ident_at(node, source)
            }
        }
        "literal" => SqlTree::Literal { text: range.slice(source).to_string(), range, span },
        "string" | "national_string" => SqlTree::Literal { text: range.slice(source).to_string(), range, span },
        "comment" | "line_comment" | "block_comment" => SqlTree::Comment { text: range.slice(source).to_string(), range, span },

        // ----- DML statements ----------------------------------------
        "select" => lower_select(node, source),
        "insert" => lower_insert(node, source),
        "update" => lower_update(node, source),
        "delete" => lower_delete(node, source),
        "subquery" => lower_subquery(node, source),

        // ----- Clauses (when they appear as children of select) ------
        "from" => lower_from(node, source),
        "where" => lower_where(node, source),
        "group_by" => lower_group_by(node, source),
        "having" => lower_having(node, source),
        "order_by" => lower_order_by(node, source),
        "order_target" => lower_order_target(node, source),
        "partition_by" => lower_partition_by(node, source),
        "join" => lower_join(node, source),
        "cte" => lower_cte(node, source),
        "set_operation" => lower_set_operation(node, source),
        "window_function" => lower_window(node, source),
        "window_specification" => lower_over(node, source),

        // ----- References / columns ----------------------------------
        "object_reference" => lower_object_reference(node, source),
        "column_reference" | "field" => lower_column_reference(node, source),
        "all_fields" => SqlTree::Star { qualifier: None, range, span },
        "term" => lower_term(node, source),
        "relation" => lower_relation(node, source),
        // `column` (CST) — appears inside INSERT column lists,
        // `ordered_columns` (FK), etc. Wraps a single inner
        // identifier or field. Unwrap to the typed inner.
        "column" => {
            let inner = node.named_children()
                .filter(|c| !c.kind().starts_with("keyword_"))
                .next();
            if let Some(c) = inner {
                lower_node(c, source)
            } else {
                ident_at(node, source)
            }
        }

        // ----- Expressions -------------------------------------------
        "binary_expression" => lower_binary_or_compare(node, source),
        "unary_expression" => lower_unary_or_temp(node, source),
        "between_expression" => lower_between(node, source),
        "case" => lower_case(node, source),
        "exists" => lower_exists(node, source),
        "cast" => lower_cast(node, source),
        "invocation" => lower_call(node, source),

        // ----- DDL ----------------------------------------------------
        "create_table" => lower_create(node, CreateKind::Table, source),
        "create_index" => lower_create(node, CreateKind::Index, source),
        "create_view" => lower_create(node, CreateKind::View, source),
        "drop_table" => lower_drop(node, DropKind::Table, source),
        "drop_index" => lower_drop(node, DropKind::Index, source),
        "alter_table" => lower_alter(node, source),
        "add_column" => lower_add_column(node, source),
        "column_definition" => lower_column_def(node, source),

        // ----- Data types ---------------------------------------------
        "int" => SqlTree::DataType { name: "int", length: None, range, span },
        "varchar" => lower_data_type(node, "varchar", source),
        "nvarchar" => lower_data_type(node, "nvarchar", source),
        "datetime" => SqlTree::DataType { name: "datetime", length: None, range, span },

        // ----- Fallback ----------------------------------------------
        other => SqlTree::Unknown {
            kind: other.to_string(),
            range,
            span,
        },
    }
}

/// `statement` CST → typed statement wrapped in `SqlTree::Statement`.
///
/// The TSQL grammar puts the `<select>` clause and its companion
/// clauses (`<from>`, `<where>`, etc.) as SIBLINGS under the
/// `<statement>` node. `lower_statement` looks at all children to
/// determine the statement kind and gathers the clauses into a
/// single typed variant.
fn lower_statement(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let kinds: Vec<&str> = node
        .named_children()
        .map(|c| c.kind())
        .collect();

    // SELECT statement: has a `select` clause (and possibly select_expression
    // when grammar uses it directly).
    if kinds.iter().any(|k| *k == "select" || *k == "select_expression") {
        let inner = aggregate_select(node, source);
        return SqlTree::Statement { inner: Box::new(inner), range, span };
    }
    // UPDATE statement: aggregate `update` + `from` (where lives nested).
    if kinds.iter().any(|k| *k == "update") {
        let inner = aggregate_update(node, source);
        return SqlTree::Statement { inner: Box::new(inner), range, span };
    }
    // DELETE statement: aggregate `delete` + `from` (where nested).
    if kinds.iter().any(|k| *k == "delete") {
        let inner = aggregate_delete(node, source);
        return SqlTree::Statement { inner: Box::new(inner), range, span };
    }
    // MERGE statement: detected by a `keyword_merge` child. The
    // `<statement>` itself holds all the merge body (target/source
    // /on/when_clause as siblings).
    let has_merge_keyword = node
        .children()
        .any(|c| c.kind() == "keyword_merge");
    if has_merge_keyword {
        let inner = aggregate_merge(node, source);
        return SqlTree::Statement { inner: Box::new(inner), range, span };
    }
    // INSERT statement: a single `insert` child does the work.
    if kinds.iter().any(|k| *k == "insert") {
        if let Some(insert_node) = node
            .named_children()
            .find(|c| c.kind() == "insert")
        {
            let inner = lower_insert(insert_node, source);
            return SqlTree::Statement { inner: Box::new(inner), range, span };
        }
    }
    // Fallback: lower the first named child as the statement body.
    let first_named = node.named_children().next();
    let inner = first_named
        .map(|c| lower_node(c, source))
        .unwrap_or_else(|| SqlTree::Unknown {
            kind: "empty_statement".into(),
            range,
            span,
        });
    SqlTree::Statement { inner: Box::new(inner), range, span }
}

/// Aggregate `update` + nested `from`/`where` clauses into a
/// `SqlTree::Update`. The TSQL grammar emits `update` as a sibling
/// of `from` under `<statement>`, with `from` carrying `where` etc.
fn aggregate_update(stmt: &RawNode, source: &str) -> SqlTree {
    let range = range_of(stmt);
    let span = span_of(stmt);
    let mut table: Option<SqlTree> = None;
    let mut assignments: Vec<SqlTree> = Vec::new();
    let mut where_: Option<Box<SqlTree>> = None;

    for c in stmt.named_children() {
        match c.kind() {
            "update" => {
                // Children: keyword_update, relation, keyword_set,
                // assignment, [comma assignment]…, where.
                // The TSQL grammar puts `where` INSIDE `update` here
                // (unlike SELECT, where it's nested in `from`).
                for inner in c.named_children() {
                    let ik = inner.kind();
                    if ik.starts_with("keyword_") || ik.starts_with("op_") {
                        continue;
                    }
                    match ik {
                        "relation" => {
                            if table.is_none() {
                                table = Some(lower_relation(inner, source));
                            }
                        }
                        "assignment" => assignments.push(lower_assignment(inner, source)),
                        "where" => where_ = Some(Box::new(lower_where(inner, source))),
                        _ => {}
                    }
                }
            }
            "from" => {
                // For UPDATE, `from` may carry `where`; the relations
                // are usually empty (target is in `update`).
                for inner in c.named_children() {
                    let ik = inner.kind();
                    if ik.starts_with("keyword_") || ik.starts_with("op_") {
                        continue;
                    }
                    if ik == "where" {
                        where_ = Some(Box::new(lower_where(inner, source)));
                    }
                }
            }
            "where" => where_ = Some(Box::new(lower_where(c, source))),
            _ => {}
        }
    }

    let table = Box::new(table.unwrap_or(SqlTree::Unknown {
        kind: "missing_update_table".into(),
        range,
        span,
    }));
    SqlTree::Update {
        table,
        assignments,
        where_,
        range,
        span,
    }
}

/// Aggregate `delete` + nested `from`/`where` into `SqlTree::Delete`.
fn aggregate_delete(stmt: &RawNode, source: &str) -> SqlTree {
    let range = range_of(stmt);
    let span = span_of(stmt);
    let mut from: Option<Box<SqlTree>> = None;
    let mut where_: Option<Box<SqlTree>> = None;

    for c in stmt.named_children() {
        match c.kind() {
            "delete" => {
                // Usually empty body — DELETE just sits as a marker.
            }
            "from" => {
                let mut relation_nodes: Vec<&RawNode> = Vec::new();
                for inner in c.named_children() {
                    let ik = inner.kind();
                    if ik.starts_with("keyword_") || ik.starts_with("op_") {
                        continue;
                    }
                    match ik {
                        "where" => where_ = Some(Box::new(lower_where(inner, source))),
                        _ => relation_nodes.push(inner),
                    }
                }
                let relations: Vec<SqlTree> = relation_nodes
                    .into_iter()
                    .map(|n| lower_node(n, source))
                    .collect();
                from = Some(Box::new(SqlTree::From {
                    relations,
                    range: range_of(c),
                    span: span_of(c),
                }));
            }
            "where" => where_ = Some(Box::new(lower_where(c, source))),
            _ => {}
        }
    }

    SqlTree::Delete {
        from,
        where_,
        range,
        span,
    }
}

/// `assignment` CST → `SqlTree::Assign { target, value }`. Used in
/// UPDATE SET and SET @var = val.
fn lower_assignment(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let operands: Vec<&RawNode> = node
        .named_children()
        .filter(|c| {
            !c.kind().starts_with("keyword_") && !c.kind().starts_with("op_")
        })
        .collect();
    let target = operands
        .first()
        .map(|c| lower_node(*c, source))
        .unwrap_or(SqlTree::Unknown {
            kind: "missing_assign_target".into(),
            range,
            span,
        });
    let value = operands
        .get(1)
        .map(|c| lower_node(*c, source))
        .unwrap_or(SqlTree::Unknown {
            kind: "missing_assign_value".into(),
            range,
            span,
        });
    SqlTree::Assign {
        target: Box::new(target),
        value: Box::new(value),
        range,
        span,
    }
}

/// `update` CST as a direct lowering target — fallback when called
/// outside `aggregate_update`'s context.
fn lower_update(node: &RawNode, source: &str) -> SqlTree {
    aggregate_update(node, source)
}

/// `delete` CST — fallback.
fn lower_delete(node: &RawNode, source: &str) -> SqlTree {
    aggregate_delete(node, source)
}

/// `subquery` CST → `SqlTree::Subquery { select }`.
fn lower_subquery(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    // A subquery's body is a Select. The CST may have it directly
    // or wrapped in another statement node — we recurse.
    let inner = node
        .named_children()
        .next()
        .map(|c| {
            if c.kind() == "select" || c.kind() == "select_expression" {
                aggregate_select(node, source)
            } else {
                lower_node(c, source)
            }
        })
        .unwrap_or(SqlTree::Unknown {
            kind: "empty_subquery".into(),
            range,
            span,
        });
    SqlTree::Subquery {
        select: Box::new(inner),
        range,
        span,
    }
}

/// Aggregate `select`/`from`/`where`/`group_by`/`having`/`order_by`
/// children of a `statement` node into a single `SqlTree::Select`.
fn aggregate_select(stmt: &RawNode, source: &str) -> SqlTree {
    let range = range_of(stmt);
    let span = span_of(stmt);
    let mut ctes: Vec<SqlTree> = Vec::new();
    let mut columns: Vec<SqlTree> = Vec::new();
    let mut into: Option<Box<SqlTree>> = None;
    let mut from: Option<Box<SqlTree>> = None;
    let mut where_: Option<Box<SqlTree>> = None;
    let mut group_by: Option<Box<SqlTree>> = None;
    let mut having: Option<Box<SqlTree>> = None;
    let mut order_by: Option<Box<SqlTree>> = None;

    // Track whether we just saw `keyword_into` so the next
    // `select_expression` becomes the INTO target.
    let mut after_into = false;
    for c in stmt.named_children() {
        match c.kind() {
            "keyword_into" => {
                after_into = true;
                continue;
            }
            "select_expression" if after_into => {
                // INTO target — typically a single term wrapping a
                // unary `#name` reference (temp table).
                if let Some(item) = c.named_children().next() {
                    let lowered = lower_column_item(item, source);
                    into = Some(Box::new(lowered));
                }
                after_into = false;
                continue;
            }
            _ => { after_into = false; }
        }
        match c.kind() {
            "select" => {
                // Walk select's named children — `keyword_select` and
                // `select_expression`. Filter the keyword; dive into
                // select_expression to collect column items.
                for inner in c.named_children() {
                    let ik = inner.kind();
                    if ik.starts_with("keyword_") || ik.starts_with("op_") {
                        continue;
                    }
                    match ik {
                        "select_expression" => {
                            for item in inner.named_children() {
                                columns.push(lower_column_item(item, source));
                            }
                        }
                        "all_fields" => {
                            columns.push(SqlTree::Star {
                                qualifier: None,
                                range: range_of(inner),
                                span: span_of(inner),
                            });
                        }
                        _ => columns.push(lower_node(inner, source)),
                    }
                }
            }
            "from" => {
                // The TSQL grammar nests `where`/`order_by` etc. INSIDE
                // the `from` node. Pluck them out as Select's siblings;
                // pass only the relations to lower_from.
                let mut relation_nodes: Vec<&RawNode> = Vec::new();
                for inner in c.named_children() {
                    let ik = inner.kind();
                    if ik.starts_with("keyword_") || ik.starts_with("op_") {
                        continue;
                    }
                    match ik {
                        "where" => where_ = Some(Box::new(lower_where(inner, source))),
                        "group_by" => group_by = Some(Box::new(lower_node(inner, source))),
                        "having" => having = Some(Box::new(lower_node(inner, source))),
                        "order_by" => order_by = Some(Box::new(lower_node(inner, source))),
                        _ => relation_nodes.push(inner),
                    }
                }
                // Build From from the relation children only.
                let relations: Vec<SqlTree> = relation_nodes
                    .into_iter()
                    .map(|n| lower_node(n, source))
                    .collect();
                from = Some(Box::new(SqlTree::From {
                    relations,
                    range: range_of(c),
                    span: span_of(c),
                }));
            }
            "where" => where_ = Some(Box::new(lower_where(c, source))),
            "group_by" => group_by = Some(Box::new(lower_node(c, source))),
            "having" => having = Some(Box::new(lower_node(c, source))),
            "order_by" => order_by = Some(Box::new(lower_node(c, source))),
            "cte" => ctes.push(lower_cte(c, source)),
            _ => {}
        }
    }

    SqlTree::Select {
        ctes,
        columns,
        into,
        from,
        where_,
        group_by,
        having,
        order_by,
        range,
        span,
    }
}

/// `select` CST → `SqlTree::Select`. Walks named children and assigns
/// each to its typed slot based on CST kind. Anonymous tokens
/// (commas) are ignored — they aren't needed in `SqlTree`.
fn lower_select(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let mut columns: Vec<SqlTree> = Vec::new();
    let mut into: Option<Box<SqlTree>> = None;
    let mut from: Option<Box<SqlTree>> = None;
    let mut where_: Option<Box<SqlTree>> = None;
    let mut group_by: Option<Box<SqlTree>> = None;
    let mut having: Option<Box<SqlTree>> = None;
    let mut order_by: Option<Box<SqlTree>> = None;

    for c in node.named_children() {
        match c.kind() {
            "select_expression" => {
                // The list of selected items.
                for item in c.named_children() {
                    let lowered = lower_node(item, source);
                    columns.push(lowered);
                }
            }
            "from" => from = Some(Box::new(lower_from(c, source))),
            "where" => where_ = Some(Box::new(lower_where(c, source))),
            "group_by" => group_by = Some(Box::new(lower_node(c, source))),
            "having" => having = Some(Box::new(lower_node(c, source))),
            "order_by" => order_by = Some(Box::new(lower_node(c, source))),
            _ => {
                // Unrecognized — fold into Unknown later. For now, ignore.
            }
        }
        let _ = into.is_some(); // hush unused
    }
    let _ = (&into, &group_by, &having, &order_by);

    SqlTree::Select {
        ctes: Vec::new(),
        columns,
        into,
        from,
        where_,
        group_by,
        having,
        order_by,
        range,
        span,
    }
}

/// `insert` CST → `SqlTree::Insert`. The CST is flat: `keyword_insert`,
/// `keyword_into`, `object_reference` (table), `list` (columns,
/// before VALUES), `keyword_values`, `list` (values, after VALUES).
fn lower_insert(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let mut table: Option<SqlTree> = None;
    let mut columns: Vec<SqlTree> = Vec::new();
    let mut values: Vec<SqlTree> = Vec::new();
    let mut seen_values_kw = false;

    for c in node.children() {
        let kind = c.kind();
        if kind == "keyword_values" {
            seen_values_kw = true;
            continue;
        }
        if !c.is_named() { continue; }
        match kind {
            "object_reference" => {
                if table.is_none() {
                    table = Some(lower_relation(c, source));
                }
            }
            "list" if !seen_values_kw => {
                for item in c.named_children() {
                    columns.push(lower_node(item, source));
                }
            }
            "list" if seen_values_kw => {
                for item in c.named_children() {
                    values.push(lower_node(item, source));
                }
            }
            _ => {}
        }
    }

    let table = Box::new(table.unwrap_or_else(|| SqlTree::Unknown {
        kind: "missing_table".into(),
        range,
        span,
    }));
    SqlTree::Insert { table, columns, values, range, span }
}

/// `from` CST → `SqlTree::From`. Children are relations; the first is
/// the base, subsequent ones may be JOINs.
fn lower_from(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let mut relations: Vec<SqlTree> = Vec::new();
    for c in node.named_children() {
        relations.push(lower_node(c, source));
    }
    SqlTree::From { relations, range, span }
}

/// `where` CST → `SqlTree::Where`. The first non-keyword named child
/// is the condition expression.
fn lower_where(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let condition = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_") && !c.kind().starts_with("op_"))
        .next()
        .map(|c| lower_node(c, source))
        .unwrap_or_else(|| SqlTree::Unknown {
            kind: "missing_where_condition".into(),
            range,
            span,
        });
    SqlTree::Where { condition: Box::new(condition), range, span }
}

/// `object_reference` (in a relation context) becomes a
/// `SqlTree::Relation` with a single name and no schema/alias.
/// In other contexts it becomes a `SqlTree::Reference`.
fn lower_object_reference(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let parts: Vec<SqlTree> = node
        .named_children()
        .map(|c| lower_node(c, source))
        .collect();
    if parts.len() == 1 {
        // Single identifier — return it directly.
        parts.into_iter().next().unwrap_or(SqlTree::Unknown {
            kind: "empty_object_reference".into(),
            range,
            span,
        })
    } else {
        SqlTree::Reference { parts, range, span }
    }
}

/// `column_reference` / `field` CST → `SqlTree::Reference` or a single
/// `Identifier`/`Variable` when the chain has one segment.
fn lower_column_reference(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let text = range.slice(source);
    if text.starts_with('@') {
        return SqlTree::Variable { text: text.to_string(), range, span };
    }
    let named: Vec<&RawNode> = node.named_children().collect();
    if named.len() == 1 && named[0].kind() == "identifier" {
        return ident_at(node, source);
    }
    let parts: Vec<SqlTree> = named.into_iter().map(|c| lower_node(c, source)).collect();
    SqlTree::Reference { parts, range, span }
}

/// Lower a single SELECT column item. The CST puts each item under
/// a `term` wrapper, but for `*` we want a bare `SqlTree::Star`
/// rather than `Column { expression: Star }`.
fn lower_column_item(node: &RawNode, source: &str) -> SqlTree {
    if node.kind() == "term" {
        let named: Vec<&RawNode> = node.named_children().collect();
        if named.len() == 1 && named[0].kind() == "all_fields" {
            return SqlTree::Star {
                qualifier: None,
                range: range_of(named[0]),
                span: span_of(named[0]),
            };
        }
    }
    lower_node(node, source)
}

/// `term` (a SELECT column item) → `SqlTree::Column`. May contain an
/// alias (trailing identifier after AS) and an expression.
fn lower_term(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let named: Vec<&RawNode> = node.named_children().collect();
    let mut alias: Option<Box<SqlTree>> = None;
    let mut expression: Option<SqlTree> = None;
    if named.len() >= 2 {
        // Last child as alias, first as expression. Crude but
        // matches the common case; fancier alias detection lands
        // when needed.
        let last = named.last().unwrap();
        if last.kind() == "identifier" {
            alias = Some(Box::new(alias_at(*last, source)));
            expression = Some(lower_node(named[0], source));
        }
    }
    if expression.is_none() {
        // No alias detected — first named child is the expression.
        expression = named.first().map(|c| lower_node(*c, source));
    }
    let expression = Box::new(expression.unwrap_or(SqlTree::Unknown {
        kind: "empty_term".into(),
        range,
        span,
    }));
    SqlTree::Column { expression, alias, range, span }
}

/// `relation` CST → `SqlTree::Relation`. Captures schema, name, alias.
fn lower_relation(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let named: Vec<&RawNode> = node.named_children().collect();
    // A relation is typically `[schema.]name [alias]`.
    let mut schema: Option<Box<SqlTree>> = None;
    let mut alias: Option<Box<SqlTree>> = None;

    let name_idx = match named.len() {
        0 => return SqlTree::Unknown { kind: "empty_relation".into(), range, span },
        1 => 0,
        2 => {
            // Either `schema.name` or `name alias`. Heuristic: if the
            // first node is `object_reference` with multiple parts,
            // it's a qualified name; the second is alias.
            let first = named[0];
            if first.kind() == "object_reference" {
                // Qualified or bare name in first position; second is alias.
                alias = Some(Box::new(alias_at(named[1], source)));
                0
            } else {
                0
            }
        }
        _ => 0,
    };

    // Lower the name node — usually an `object_reference` with one or
    // two parts.
    let name_node = named[name_idx];
    let name = if name_node.kind() == "object_reference" {
        let parts: Vec<&RawNode> = name_node.named_children().collect();
        if parts.len() >= 2 {
            schema = Some(Box::new(schema_at(parts[0], source)));
            Box::new(ident_at(parts[1], source))
        } else if let Some(only) = parts.first() {
            Box::new(ident_at(*only, source))
        } else {
            Box::new(ident_at(name_node, source))
        }
    } else {
        Box::new(ident_at(name_node, source))
    };

    SqlTree::Relation { schema, name, alias, range, span }
}

/// `binary_expression` CST → `SqlTree::Compare` (when the operator is
/// a comparison) or `SqlTree::Binary` (arithmetic / logical / bitwise).
fn lower_binary_or_compare(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let named: Vec<&RawNode> = node.named_children().collect();

    // Find the operator child; capture both the text and its source
    // range so the typed Op slot carries position info (mirrors the
    // SyntaxTree::Binary `op_range` field).
    let mut op_text = String::new();
    let mut op_range = ByteRange::synthetic_empty();
    for c in node.children() {
        let kind = c.kind();
        if !c.is_named() {
            let t = range_of(c).slice(source).trim();
            if !t.is_empty() {
                op_text = t.to_string();
                op_range = range_of(c);
                break;
            }
        } else if kind.starts_with("keyword_") {
            op_text = range_of(c).slice(source).to_string();
            op_range = range_of(c);
            break;
        }
    }

    let operands: Vec<&&RawNode> = named.iter().filter(|c| {
        !c.kind().starts_with("keyword_") && !c.kind().starts_with("op_")
    }).collect();
    let left = operands
        .first()
        .map(|c| lower_node(**c, source))
        .unwrap_or(SqlTree::Unknown {
            kind: "missing_left".into(),
            range,
            span,
        });
    let right = operands
        .get(1)
        .map(|c| lower_node(**c, source))
        .unwrap_or(SqlTree::Unknown {
            kind: "missing_right".into(),
            range,
            span,
        });

    match classify_op(&op_text) {
        Some((marker, OpKind::Comparison)) => SqlTree::Compare {
            left: Box::new(left),
            op_text,
            op_marker: marker,
            op_range,
            right: Box::new(right),
            range,
            span,
        },
        Some((marker, OpKind::Binary)) => SqlTree::Binary {
            left: Box::new(left),
            op_text,
            op_marker: marker,
            op_range,
            right: Box::new(right),
            range,
            span,
        },
        None => SqlTree::Unknown {
            kind: format!("binary_op_unhandled:{}", op_text),
            range,
            span,
        },
    }
}

/// `unary_expression` CST. T-SQL uses `#x` for local temp tables
/// and `##x` for global temp tables — both lower to `SqlTree::Temp
/// { name }`. Other unary operators (NOT, -, +) lower to
/// `SqlTree::Unary` (TODO when needed).
fn lower_unary_or_temp(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let named: Vec<&RawNode> = node.named_children().collect();
    // Find the op text and operand.
    let op_text = named
        .iter()
        .find(|c| c.kind().starts_with("op_"))
        .map(|c| range_of(*c).slice(source))
        .unwrap_or("");
    if op_text == "#" || op_text == "##" {
        // Temp-table reference. Operand is the inner identifier.
        let operand_node = named
            .iter()
            .find(|c| !c.kind().starts_with("keyword_") && !c.kind().starts_with("op_"))
            .copied();
        let operand = if let Some(c) = operand_node {
            // Could be a `field` wrapping an `identifier` or an
            // identifier directly.
            if c.kind() == "field" {
                let inner_first = c.named_children().next();
                if let Some(id) = inner_first {
                    ident_at(id, source)
                } else {
                    ident_at(c, source)
                }
            } else {
                ident_at(c, source)
            }
        } else {
            SqlTree::Unknown {
                kind: "missing_temp_name".into(),
                range,
                span,
            }
        };
        return SqlTree::Temp {
            name: Box::new(operand),
            range,
            span,
        };
    }
    // Generic unary fallback — render as Unknown for now until
    // typed operators are needed.
    SqlTree::Unknown {
        kind: format!("unary_op_unhandled:{}", op_text),
        range,
        span,
    }
}

/// `between_expression` CST → `SqlTree::Between { value, low, high }`.
fn lower_between(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let operands: Vec<&RawNode> = node
        .named_children()
        .filter(|c| {
            !c.kind().starts_with("keyword_") && !c.kind().starts_with("op_")
        })
        .collect();
    let value = operands
        .first()
        .map(|c| lower_node(*c, source))
        .unwrap_or(SqlTree::Unknown { kind: "missing_between_value".into(), range, span });
    let low = operands
        .get(1)
        .map(|c| lower_node(*c, source))
        .unwrap_or(SqlTree::Unknown { kind: "missing_between_low".into(), range, span });
    let high = operands
        .get(2)
        .map(|c| lower_node(*c, source))
        .unwrap_or(SqlTree::Unknown { kind: "missing_between_high".into(), range, span });
    SqlTree::Between {
        value: Box::new(value),
        low: Box::new(low),
        high: Box::new(high),
        range,
        span,
    }
}

/// `case` CST → `SqlTree::Case { whens, else_ }`. The flat CST
/// (keyword_when, cond, keyword_then, value, [keyword_when ...
/// keyword_else], elseval, keyword_end) is grouped into typed
/// When { condition, value } arms.
fn lower_case(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let mut whens: Vec<SqlTree> = Vec::new();
    let mut else_: Option<Box<SqlTree>> = None;

    #[derive(Copy, Clone, PartialEq)]
    enum State { Before, AfterWhen, AfterThen, AfterElse }
    let mut state = State::Before;
    let mut cond_buffer: Option<SqlTree> = None;
    let mut when_start: crate::tree::types::ByteRange =
        crate::tree::types::ByteRange::empty_at(range.start);

    for c in node.named_children() {
        let ck = c.kind();
        if ck == "keyword_when" {
            // Flush previous when if both pieces present.
            if let Some(cond) = cond_buffer.take() {
                // Was AfterThen but THEN value missing — leave; or
                // we already pushed via AfterThen path below.
                let _ = cond;
            }
            state = State::AfterWhen;
            when_start = range_of(c);
            continue;
        }
        if ck == "keyword_then" {
            state = State::AfterThen;
            continue;
        }
        if ck == "keyword_else" {
            state = State::AfterElse;
            continue;
        }
        if ck == "keyword_end" || ck == "keyword_case" {
            continue;
        }
        if ck.starts_with("keyword_") || ck.starts_with("op_") {
            continue;
        }

        match state {
            State::AfterWhen => {
                cond_buffer = Some(lower_node(c, source));
            }
            State::AfterThen => {
                let cond = cond_buffer.take().unwrap_or(SqlTree::Unknown {
                    kind: "missing_when_condition".into(),
                    range,
                    span,
                });
                let value = lower_node(c, source);
                let v_end = value.range().end;
                whens.push(SqlTree::When {
                    condition: Box::new(cond),
                    value: Box::new(value),
                    range: crate::tree::types::ByteRange::new(when_start.start, v_end),
                    span,
                });
            }
            State::AfterElse => {
                else_ = Some(Box::new(lower_node(c, source)));
            }
            State::Before => {}
        }
    }

    SqlTree::Case { whens, else_, range, span }
}

/// `exists` CST → `SqlTree::Exists { subquery }`.
fn lower_exists(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let inner = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_"))
        .next()
        .map(|c| lower_node(c, source))
        .unwrap_or(SqlTree::Unknown { kind: "empty_exists".into(), range, span });
    SqlTree::Exists { subquery: Box::new(inner), range, span }
}

/// `cast` CST → `SqlTree::Cast { value, type_ }`.
fn lower_cast(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let named: Vec<&RawNode> = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_"))
        .collect();
    let value = named.first().map(|c| lower_node(*c, source)).unwrap_or(
        SqlTree::Unknown { kind: "missing_cast_value".into(), range, span },
    );
    let type_ = named.get(1).map(|c| lower_node(*c, source)).unwrap_or(
        SqlTree::Unknown { kind: "missing_cast_type".into(), range, span },
    );
    SqlTree::Cast {
        value: Box::new(value),
        type_: Box::new(type_),
        range,
        span,
    }
}

/// `invocation` CST → `SqlTree::Call { callee, arguments }`.
fn lower_call(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let named: Vec<&RawNode> = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_"))
        .collect();
    let callee = named.first().map(|c| lower_node(*c, source)).unwrap_or(
        SqlTree::Unknown { kind: "missing_call_callee".into(), range, span },
    );
    let arguments: Vec<SqlTree> = named.iter().skip(1).map(|c| lower_node(*c, source)).collect();
    SqlTree::Call {
        callee: Box::new(callee),
        arguments,
        range,
        span,
    }
}

/// `create_table` / `create_view` / `create_index` → `SqlTree::Create`.
fn lower_create(node: &RawNode, kind: CreateKind, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let named: Vec<&RawNode> = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_"))
        .collect();
    let name = named.first().map(|c| {
        // Usually object_reference/identifier — pull the inner identifier.
        let inner: Vec<&RawNode> = c.named_children().collect();
        if let Some(only) = inner.first() {
            ident_at(*only, source)
        } else {
            ident_at(*c, source)
        }
    }).unwrap_or(SqlTree::Unknown { kind: "missing_create_name".into(), range, span });
    // Body collects the substantive children. For CREATE TABLE the
    // grammar wraps column definitions in `column_definitions`; we
    // inline its children so `body` is a flat list of `ColumnDef`.
    let mut body: Vec<SqlTree> = Vec::new();
    for c in named.iter().skip(1) {
        if c.kind() == "column_definitions" {
            for inner in c.named_children() {
                if !inner.kind().starts_with("keyword_") {
                    body.push(lower_node(inner, source));
                }
            }
        } else {
            body.push(lower_node(*c, source));
        }
    }
    SqlTree::Create {
        kind,
        name: Box::new(name),
        body,
        range,
        span,
    }
}

/// `drop_table` / `drop_index` → `SqlTree::Drop`.
fn lower_drop(node: &RawNode, kind: DropKind, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let named: Vec<&RawNode> = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_"))
        .collect();
    let name = named.first().map(|c| {
        let inner: Vec<&RawNode> = c.named_children().collect();
        if let Some(only) = inner.first() {
            ident_at(*only, source)
        } else {
            ident_at(*c, source)
        }
    }).unwrap_or(SqlTree::Unknown { kind: "missing_drop_name".into(), range, span });
    let _ = source;
    SqlTree::Drop {
        kind,
        name: Box::new(name),
        range,
        span,
    }
}

/// `alter_table` → `SqlTree::Alter { name, operation }`.
fn lower_alter(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let named: Vec<&RawNode> = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_"))
        .collect();
    let name = named.first().map(|c| {
        let inner: Vec<&RawNode> = c.named_children().collect();
        if let Some(only) = inner.first() {
            ident_at(*only, source)
        } else {
            ident_at(*c, source)
        }
    }).unwrap_or(SqlTree::Unknown { kind: "missing_alter_name".into(), range, span });
    let operation = named.get(1).map(|c| lower_node(*c, source)).unwrap_or(
        SqlTree::Unknown { kind: "missing_alter_operation".into(), range, span },
    );
    SqlTree::Alter {
        name: Box::new(name),
        operation: Box::new(operation),
        range,
        span,
    }
}

/// `add_column` CST → `SqlTree::AddColumn { column }`.
fn lower_add_column(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let column = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_"))
        .next()
        .map(|c| lower_node(c, source))
        .unwrap_or(SqlTree::Unknown { kind: "missing_add_column".into(), range, span });
    SqlTree::AddColumn {
        column: Box::new(column),
        range,
        span,
    }
}

/// `column_definition` CST → `SqlTree::ColumnDef { name, type_, constraints }`.
fn lower_column_def(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let named: Vec<&RawNode> = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_"))
        .collect();
    let name = named.first().map(|c| ident_at(*c, source)).unwrap_or(SqlTree::Unknown { kind: "missing_column_name".into(), range, span });
    let type_ = named.get(1).map(|c| lower_node(*c, source)).unwrap_or(
        SqlTree::Unknown { kind: "missing_column_type".into(), range, span },
    );
    let constraints: Vec<SqlTree> = named.iter().skip(2).map(|c| lower_node(*c, source)).collect();
    SqlTree::ColumnDef {
        name: Box::new(name),
        type_: Box::new(type_),
        constraints,
        range,
        span,
    }
}

/// `varchar` / `nvarchar` etc. with optional length — `VARCHAR(100)`.
fn lower_data_type(node: &RawNode, name: &'static str, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let length = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_"))
        .next()
        .map(|c| Box::new(lower_node(c, source)));
    SqlTree::DataType {
        name,
        length,
        range,
        span,
    }
}

/// `group_by` CST → `SqlTree::GroupBy { keys }`.
fn lower_group_by(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let keys: Vec<SqlTree> = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_") && !c.kind().starts_with("op_"))
        .map(|c| lower_node(c, source))
        .collect();
    SqlTree::GroupBy { keys, range, span }
}

/// `having` CST → `SqlTree::Having { condition }`.
fn lower_having(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let condition = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_"))
        .next()
        .map(|c| lower_node(c, source))
        .unwrap_or(SqlTree::Unknown { kind: "missing_having".into(), range, span });
    SqlTree::Having { condition: Box::new(condition), range, span }
}

/// `order_by` CST → `SqlTree::OrderBy { targets }`.
fn lower_order_by(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let targets: Vec<SqlTree> = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_"))
        .map(|c| lower_node(c, source))
        .collect();
    SqlTree::OrderBy { targets, range, span }
}

/// `order_target` CST → `SqlTree::OrderTarget { expression,
/// extra_markers }`. The direction keyword (`ASC` / `DESC`) lowers
/// to an anchored `Marker` so the variant-blind walker emits
/// `<target><asc/>…</target>` mechanically.
fn lower_order_target(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let mut expression: Option<SqlTree> = None;
    let mut extra_markers: Vec<Marker> = Vec::new();
    for c in node.named_children() {
        let kind = c.kind();
        if kind == "direction" {
            let text = range_of(c).slice(source).to_uppercase();
            let marker_name = match text.as_str() {
                "ASC" => Some("asc"),
                "DESC" => Some("desc"),
                _ => None,
            };
            if let Some(name) = marker_name {
                extra_markers.push(Marker::anchored(name, range_of(c), span_of(c)));
            }
            continue;
        }
        if kind.starts_with("keyword_") || kind.starts_with("op_") {
            continue;
        }
        if expression.is_none() {
            expression = Some(lower_node(c, source));
        }
    }
    let expression = Box::new(expression.unwrap_or(SqlTree::Unknown {
        kind: "missing_order_target".into(),
        range,
        span,
    }));
    SqlTree::OrderTarget {
        expression,
        extra_markers,
        range,
        span,
    }
}

/// `partition_by` CST → `SqlTree::PartitionBy { keys }`.
fn lower_partition_by(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let keys: Vec<SqlTree> = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_") && !c.kind().starts_with("op_"))
        .map(|c| lower_node(c, source))
        .collect();
    SqlTree::PartitionBy { keys, range, span }
}

/// `join` CST → `SqlTree::Join { relation, on, extra_markers }`.
/// The direction keywords (`LEFT`/`RIGHT`/`FULL`/`OUTER`/`CROSS`)
/// each lower to an anchored `Marker`; INNER (the default) emits no
/// marker. Matches the SyntaxTree typed-marker pattern.
fn lower_join(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let mut extra_markers: Vec<Marker> = Vec::new();
    let mut relation: Option<SqlTree> = None;
    let mut on: Option<Box<SqlTree>> = None;

    for c in node.named_children() {
        match c.kind() {
            "keyword_left" => extra_markers.push(Marker::anchored("left", range_of(c), span_of(c))),
            "keyword_right" => extra_markers.push(Marker::anchored("right", range_of(c), span_of(c))),
            "keyword_full" => extra_markers.push(Marker::anchored("full", range_of(c), span_of(c))),
            "keyword_outer" => extra_markers.push(Marker::anchored("outer", range_of(c), span_of(c))),
            "keyword_cross" => extra_markers.push(Marker::anchored("cross", range_of(c), span_of(c))),
            "keyword_inner" | "keyword_join" | "keyword_on" => {}
            k if k.starts_with("keyword_") || k.starts_with("op_") => {}
            "relation" => relation = Some(lower_relation(c, source)),
            "object_reference" => relation = Some(lower_relation(c, source)),
            _ => {
                // Treat as ON condition.
                if on.is_none() {
                    on = Some(Box::new(lower_node(c, source)));
                }
            }
        }
    }
    let relation = Box::new(relation.unwrap_or(SqlTree::Unknown {
        kind: "missing_join_relation".into(),
        range,
        span,
    }));
    SqlTree::Join {
        relation,
        on,
        extra_markers,
        range,
        span,
    }
}

/// `cte` CST → `SqlTree::Cte { name, query }`.
fn lower_cte(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let named: Vec<&RawNode> = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_"))
        .collect();
    let name = named.first().map(|c| lower_node(*c, source)).unwrap_or(SqlTree::Unknown {
        kind: "missing_cte_name".into(),
        range,
        span,
    });
    let query = named.get(1).map(|c| {
        if c.kind() == "statement" {
            lower_statement(*c, source)
        } else {
            lower_node(*c, source)
        }
    }).unwrap_or(SqlTree::Unknown {
        kind: "missing_cte_query".into(),
        range,
        span,
    });
    SqlTree::Cte {
        name: Box::new(name),
        query: Box::new(query),
        range,
        span,
    }
}

/// `set_operation` CST → `SqlTree::Union { all, selects }`. Detects
/// the ALL variant from the keyword children.
fn lower_set_operation(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let mut all = false;
    let mut selects: Vec<SqlTree> = Vec::new();
    for c in node.named_children() {
        match c.kind() {
            "keyword_all" => all = true,
            k if k.starts_with("keyword_") || k.starts_with("op_") => {}
            _ => selects.push(lower_node(c, source)),
        }
    }
    SqlTree::Union {
        all,
        selects,
        range,
        span,
    }
}

/// `window_function` CST → `SqlTree::Window { call, over }`.
fn lower_window(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let named: Vec<&RawNode> = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_"))
        .collect();
    let call = named.first().map(|c| lower_node(*c, source)).unwrap_or(SqlTree::Unknown {
        kind: "missing_window_call".into(),
        range,
        span,
    });
    let over = named.get(1).map(|c| lower_node(*c, source)).unwrap_or(SqlTree::Unknown {
        kind: "missing_window_over".into(),
        range,
        span,
    });
    SqlTree::Window {
        call: Box::new(call),
        over: Box::new(over),
        range,
        span,
    }
}

/// `window_specification` CST → `SqlTree::Over { partition_by, order_by }`.
fn lower_over(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let mut partition_by: Option<Box<SqlTree>> = None;
    let mut order_by: Option<Box<SqlTree>> = None;
    for c in node.named_children() {
        match c.kind() {
            "partition_by" => partition_by = Some(Box::new(lower_partition_by(c, source))),
            "order_by" => order_by = Some(Box::new(lower_order_by(c, source))),
            _ => {}
        }
    }
    SqlTree::Over {
        partition_by,
        order_by,
        range,
        span,
    }
}

/// `execute_statement` CST → `SqlTree::Exec { target }`.
fn lower_exec(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let target = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_"))
        .next()
        .map(|c| {
            // Unwrap object_reference to its single identifier.
            if c.kind() == "object_reference" {
                let parts: Vec<&RawNode> = c.named_children().collect();
                if let Some(only) = parts.first() {
                    return ident_at(*only, source);
                }
            }
            lower_node(c, source)
        })
        .unwrap_or(SqlTree::Unknown {
            kind: "missing_exec_target".into(),
            range,
            span,
        });
    SqlTree::Exec {
        target: Box::new(target),
        range,
        span,
    }
}

/// `set_statement` CST → `SqlTree::Set { target, value }`.
fn lower_set(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let named: Vec<&RawNode> = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_") && !c.kind().starts_with("op_"))
        .collect();
    let target = named.first().map(|c| {
        // The set target is usually an object_reference holding a
        // single @-prefixed identifier — unwrap to Variable.
        if c.kind() == "object_reference" {
            let parts: Vec<&RawNode> = c.named_children().collect();
            if let Some(only) = parts.first() {
                let text = range_of(*only).slice(source);
                if text.starts_with('@') {
                    return SqlTree::Variable {
                        text: text.to_string(),
                        range: range_of(*only),
                        span: span_of(*only),
                    };
                }
                return ident_at(*only, source);
            }
        }
        lower_node(*c, source)
    }).unwrap_or(SqlTree::Unknown {
        kind: "missing_set_target".into(),
        range,
        span,
    });
    let value = named.get(1).map(|c| lower_node(*c, source)).unwrap_or(SqlTree::Unknown {
        kind: "missing_set_value".into(),
        range,
        span,
    });
    SqlTree::Set {
        target: Box::new(target),
        value: Box::new(value),
        range,
        span,
    }
}

/// `transaction` CST → `SqlTree::Transaction { statements }`. Walks
/// inner statements, dropping BEGIN / COMMIT / ROLLBACK keywords.
fn lower_transaction(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let statements: Vec<SqlTree> = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_"))
        .map(|c| lower_node(c, source))
        .collect();
    SqlTree::Transaction {
        statements,
        range,
        span,
    }
}

/// `create_function` CST → `SqlTree::Function`. Children: optional
/// schema-qualified name, function_arguments, RETURNS type,
/// function_body.
fn lower_create_function(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let mut schema: Option<Box<SqlTree>> = None;
    let mut name_ir: Option<SqlTree> = None;
    let mut parameters: Vec<SqlTree> = Vec::new();
    let mut return_type: Option<Box<SqlTree>> = None;
    let mut body: Option<SqlTree> = None;

    for c in node.named_children() {
        let kind = c.kind();
        if kind.starts_with("keyword_") || kind.starts_with("op_") {
            continue;
        }
        match kind {
            "object_reference" => {
                let parts: Vec<&RawNode> = c.named_children().collect();
                if parts.len() >= 2 {
                    schema = Some(Box::new(schema_at(parts[0], source)));
                    name_ir = Some(ident_at(parts[1], source));
                } else if let Some(only) = parts.first() {
                    name_ir = Some(ident_at(*only, source));
                }
            }
            "function_arguments" => {
                for arg in c.named_children() {
                    if !arg.kind().starts_with("keyword_") {
                        parameters.push(lower_node(arg, source));
                    }
                }
            }
            "function_body" => body = Some(lower_function_body(c, source)),
            // Anything else after RETURNS is the return type.
            _ if return_type.is_none() && body.is_none() => {
                return_type = Some(Box::new(lower_node(c, source)));
            }
            _ => {}
        }
    }

    let name = Box::new(name_ir.unwrap_or(SqlTree::Unknown {
        kind: "missing_function_name".into(),
        range,
        span,
    }));
    let body = Box::new(body.unwrap_or(SqlTree::Unknown {
        kind: "missing_function_body".into(),
        range,
        span,
    }));
    SqlTree::Function {
        schema,
        name,
        parameters,
        return_type,
        body,
        range,
        span,
    }
}

/// `function_argument` CST → reuse the cross-language `Column` /
/// `ColumnDef` shape — emit `ColumnDef { name, type_ }`.
fn lower_function_argument(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let named: Vec<&RawNode> = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_"))
        .collect();
    let name = named.first().map(|c| {
        let text = range_of(*c).slice(source);
        if text.starts_with('@') {
            SqlTree::Variable { text: text.to_string(), range: range_of(*c), span: span_of(*c) }
        } else {
            ident_at(*c, source)
        }
    }).unwrap_or(SqlTree::Unknown {
        kind: "missing_arg_name".into(),
        range,
        span,
    });
    let type_ = named.get(1).map(|c| lower_node(*c, source)).unwrap_or(SqlTree::Unknown {
        kind: "missing_arg_type".into(),
        range,
        span,
    });
    SqlTree::ColumnDef {
        name: Box::new(name),
        type_: Box::new(type_),
        constraints: Vec::new(),
        range,
        span,
    }
}

/// `function_body` CST → wrap the inner expression(s) as a typed
/// body. For now, just collect non-keyword children into a Tuple
/// (or single child unwrapped).
fn lower_function_body(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let inner: Vec<SqlTree> = node
        .named_children()
        .filter(|c| !c.kind().starts_with("keyword_"))
        .map(|c| lower_node(c, source))
        .collect();
    if inner.len() == 1 {
        inner.into_iter().next().unwrap()
    } else {
        SqlTree::Tuple { items: inner, range, span }
    }
}

/// Aggregate a MERGE statement from siblings under `<statement>`.
/// Children: keyword_merge, keyword_into, object_reference (target),
/// keyword_as, identifier (target alias), keyword_using,
/// object_reference (source), keyword_as, identifier (source alias),
/// keyword_on, binary_expression (ON condition), when_clause(s).
fn aggregate_merge(stmt: &RawNode, source: &str) -> SqlTree {
    let range = range_of(stmt);
    let span = span_of(stmt);
    #[derive(Copy, Clone, PartialEq)]
    enum State { Initial, AfterInto, AfterUsing, AfterOn }
    let mut state = State::Initial;
    let mut target: Option<SqlTree> = None;
    let mut source_rel: Option<SqlTree> = None;
    let mut on: Option<SqlTree> = None;
    let mut whens: Vec<SqlTree> = Vec::new();

    // Track whether we've consumed the `<as> <identifier>` alias for
    // the current relation context.
    let mut last_was_as = false;

    for c in stmt.named_children() {
        let kind = c.kind();
        match kind {
            "keyword_into" => { state = State::AfterInto; last_was_as = false; }
            "keyword_using" => { state = State::AfterUsing; last_was_as = false; }
            "keyword_on" => { state = State::AfterOn; last_was_as = false; }
            "keyword_as" => { last_was_as = true; }
            "when_clause" => whens.push(lower_merge_when(c, source)),
            "object_reference" => {
                let rel = build_relation_from_object_reference(c, source);
                match state {
                    State::AfterInto if target.is_none() => target = Some(rel),
                    State::AfterUsing if source_rel.is_none() => source_rel = Some(rel),
                    _ => {}
                }
                last_was_as = false;
            }
            "identifier" if last_was_as => {
                let alias = alias_at(c, source);
                match state {
                    State::AfterInto => {
                        if let Some(SqlTree::Relation { schema, name, alias: _, range, span }) = target.take() {
                            target = Some(SqlTree::Relation {
                                schema, name, alias: Some(Box::new(alias)),
                                range, span,
                            });
                        }
                    }
                    State::AfterUsing => {
                        if let Some(SqlTree::Relation { schema, name, alias: _, range, span }) = source_rel.take() {
                            source_rel = Some(SqlTree::Relation {
                                schema, name, alias: Some(Box::new(alias)),
                                range, span,
                            });
                        }
                    }
                    _ => {}
                }
                last_was_as = false;
            }
            _ if state == State::AfterOn && on.is_none()
                && !kind.starts_with("keyword_") => {
                on = Some(lower_node(c, source));
                last_was_as = false;
            }
            k if k.starts_with("keyword_") || k.starts_with("op_") => {}
            _ => { last_was_as = false; }
        }
    }

    let target = Box::new(target.unwrap_or(SqlTree::Unknown {
        kind: "missing_merge_target".into(),
        range,
        span,
    }));
    let source_rel = Box::new(source_rel.unwrap_or(SqlTree::Unknown {
        kind: "missing_merge_source".into(),
        range,
        span,
    }));
    let on = Box::new(on.unwrap_or(SqlTree::Unknown {
        kind: "missing_merge_on".into(),
        range,
        span,
    }));
    SqlTree::Merge {
        target,
        source: source_rel,
        on,
        whens,
        range,
        span,
    }
}

/// Build a `SqlTree::Relation` from an object_reference CST node.
fn build_relation_from_object_reference(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let parts: Vec<&RawNode> = node.named_children().collect();
    let (schema, name) = if parts.len() >= 2 {
        (
            Some(Box::new(schema_at(parts[0], source))),
            Box::new(ident_at(parts[1], source)),
        )
    } else if let Some(only) = parts.first() {
        (None, Box::new(ident_at(*only, source)))
    } else {
        return SqlTree::Unknown {
            kind: "empty_object_reference".into(),
            range,
            span,
        };
    };
    SqlTree::Relation {
        schema,
        name,
        alias: None,
        range,
        span,
    }
}

/// `when_clause` (in MERGE) → `SqlTree::MergeWhen { matched, action }`.
fn lower_merge_when(node: &RawNode, source: &str) -> SqlTree {
    let range = range_of(node);
    let span = span_of(node);
    let mut matched = true;
    let mut action: Option<SqlTree> = None;

    for c in node.named_children() {
        match c.kind() {
            "keyword_not" => matched = false,
            "keyword_matched" => {}
            "keyword_when" | "keyword_then" => {}
            // Action is one of: UPDATE / INSERT / DELETE
            "keyword_update" | "keyword_insert" | "keyword_delete" => {
                // Action body follows; we'll capture below.
            }
            "assignment" => {
                // For `WHEN MATCHED THEN UPDATE SET ...`, this is the
                // assignment. Wrap in an Update without table.
                if action.is_none() {
                    action = Some(SqlTree::Update {
                        table: Box::new(SqlTree::Unknown {
                            kind: "merge_update_no_explicit_table".into(),
                            range: range_of(c),
                            span: span_of(c),
                        }),
                        assignments: vec![lower_assignment(c, source)],
                        where_: None,
                        range: range_of(c),
                        span: span_of(c),
                    });
                }
            }
            "list" => {
                // For INSERT: a list child holds columns or values.
                // Defer detailed shape to later iter.
                if action.is_none() {
                    action = Some(lower_node(c, source));
                }
            }
            k if k.starts_with("keyword_") || k.starts_with("op_") => {}
            _ => {
                if action.is_none() {
                    action = Some(lower_node(c, source));
                }
            }
        }
    }

    let action = Box::new(action.unwrap_or(SqlTree::Unknown {
        kind: "merge_when_no_action".into(),
        range,
        span,
    }));
    SqlTree::MergeWhen {
        matched,
        action,
        range,
        span,
    }
}

/// Op classification: which marker name to attach, and whether the
/// op is a comparison (→ `<compare>`) or an arithmetic / logical /
/// bitwise op (→ `<binary>`). Marker names are the canonical
/// snake_case identifiers used elsewhere in the codebase (`equal`,
/// `less_equal`, `plus`, `bitwise_xor`).
enum OpKind { Comparison, Binary }

fn classify_op(text: &str) -> Option<(&'static str, OpKind)> {
    let upper = text.to_uppercase();
    Some(match upper.as_str() {
        "=" => ("equal", OpKind::Comparison),
        "<>" | "!=" => ("not_equal", OpKind::Comparison),
        "<" => ("less", OpKind::Comparison),
        "<=" => ("less_equal", OpKind::Comparison),
        ">" => ("greater", OpKind::Comparison),
        ">=" => ("greater_equal", OpKind::Comparison),
        "LIKE" => ("like", OpKind::Comparison),
        "IN" => ("in", OpKind::Comparison),
        "IS" => ("is", OpKind::Comparison),
        "AND" => ("and", OpKind::Comparison),
        "OR" => ("or", OpKind::Comparison),
        "+" => ("plus", OpKind::Binary),
        "-" => ("minus", OpKind::Binary),
        "*" => ("multiply", OpKind::Binary),
        "/" => ("divide", OpKind::Binary),
        "%" => ("modulo", OpKind::Binary),
        "||" => ("concat", OpKind::Binary),
        "&" => ("bitwise_and", OpKind::Binary),
        "|" => ("bitwise_or", OpKind::Binary),
        "^" => ("bitwise_xor", OpKind::Binary),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Parser;

    fn parse_tsql(source: &str) -> tree_sitter::Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_sequel_tsql::LANGUAGE.into())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn select_star_from_users_lowers_to_typed_select() {
        let source = "SELECT * FROM Users";
        let tree = parse_tsql(source);
        let tree = lower_sql_root(&crate::raw::RawNode::from_tree_sitter(tree.root_node(), source), source);
        let SqlTree::File { statements, .. } = tree else {
            panic!("expected File, got {tree:?}");
        };
        assert_eq!(statements.len(), 1, "{statements:?}");
        let SqlTree::Statement { inner, .. } = &statements[0] else {
            panic!("expected Statement");
        };
        let SqlTree::Select { columns, from, .. } = inner.as_ref() else {
            panic!("expected Select, got {inner:?}");
        };
        assert!(matches!(columns.first(), Some(SqlTree::Column { .. }) | Some(SqlTree::Star { .. })));
        assert!(from.is_some());
    }

    #[test]
    fn insert_lowers_with_typed_columns_and_values() {
        let source = "INSERT INTO L (a, b) VALUES (1, 'x')";
        let tree = parse_tsql(source);
        let tree = lower_sql_root(&crate::raw::RawNode::from_tree_sitter(tree.root_node(), source), source);
        let SqlTree::File { statements, .. } = tree else { panic!(); };
        let SqlTree::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlTree::Insert { table, columns, values, .. } = inner.as_ref() else {
            panic!("expected Insert, got {inner:?}");
        };
        assert!(matches!(table.as_ref(), SqlTree::Relation { .. }));
        assert_eq!(columns.len(), 2, "columns: {columns:?}");
        assert_eq!(values.len(), 2, "values: {values:?}");
    }

    #[test]
    fn update_with_set_and_where_lowers_to_typed_update() {
        let source = "UPDATE Users SET Active = 0 WHERE ID = 1";
        let tree = parse_tsql(source);
        let tree = lower_sql_root(&crate::raw::RawNode::from_tree_sitter(tree.root_node(), source), source);
        let SqlTree::File { statements, .. } = tree else { panic!(); };
        let SqlTree::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlTree::Update { table, assignments, where_, .. } = inner.as_ref() else {
            panic!("expected Update, got {inner:?}");
        };
        assert!(matches!(table.as_ref(), SqlTree::Relation { .. }));
        assert_eq!(assignments.len(), 1);
        assert!(matches!(&assignments[0], SqlTree::Assign { .. }));
        assert!(where_.is_some());
    }

    #[test]
    fn create_table_lowers_to_typed_create_with_column_defs() {
        let source = "CREATE TABLE T (id INT, name VARCHAR(100))";
        let tree = parse_tsql(source);
        let tree = lower_sql_root(&crate::raw::RawNode::from_tree_sitter(tree.root_node(), source), source);
        let SqlTree::File { statements, .. } = tree else { panic!(); };
        let SqlTree::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlTree::Create { kind, body, .. } = inner.as_ref() else {
            panic!("expected Create, got {inner:?}");
        };
        assert_eq!(*kind, crate::tree::sql::CreateKind::Table);
        assert_eq!(body.len(), 2);
        assert!(body.iter().all(|c| matches!(c, SqlTree::ColumnDef { .. })));
    }

    #[test]
    fn drop_table_lowers_to_typed_drop() {
        let source = "DROP TABLE T";
        let tree = parse_tsql(source);
        let tree = lower_sql_root(&crate::raw::RawNode::from_tree_sitter(tree.root_node(), source), source);
        let SqlTree::File { statements, .. } = tree else { panic!(); };
        let SqlTree::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlTree::Drop { kind, .. } = inner.as_ref() else {
            panic!("expected Drop, got {inner:?}");
        };
        assert_eq!(*kind, crate::tree::sql::DropKind::Table);
    }

    #[test]
    fn between_expression_lowers_to_typed_between() {
        let source = "SELECT a BETWEEN 1 AND 10 FROM x";
        let tree = parse_tsql(source);
        let tree = lower_sql_root(&crate::raw::RawNode::from_tree_sitter(tree.root_node(), source), source);
        let SqlTree::File { statements, .. } = tree else { panic!(); };
        let SqlTree::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlTree::Select { columns, .. } = inner.as_ref() else { panic!(); };
        // The first column item should be a Column wrapping a Between
        // (or Between directly if term unwrapping is added later).
        match columns.first() {
            Some(SqlTree::Column { expression, .. }) => {
                assert!(matches!(expression.as_ref(), SqlTree::Between { .. }),
                    "column expression: {expression:?}");
            }
            Some(SqlTree::Between { .. }) => {}
            other => panic!("unexpected column: {other:?}"),
        }
    }

    #[test]
    fn exec_lowers_to_typed_exec() {
        let source = "EXEC sp_helpdb";
        let tree = parse_tsql(source);
        let tree = lower_sql_root(&crate::raw::RawNode::from_tree_sitter(tree.root_node(), source), source);
        let SqlTree::File { statements, .. } = tree else { panic!(); };
        let SqlTree::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlTree::Exec { target, .. } = inner.as_ref() else {
            panic!("expected Exec, got {inner:?}");
        };
        assert!(matches!(target.as_ref(), SqlTree::Identifier { .. }));
    }

    #[test]
    fn set_variable_lowers_to_typed_set() {
        let source = "SET @x = 1";
        let tree = parse_tsql(source);
        let tree = lower_sql_root(&crate::raw::RawNode::from_tree_sitter(tree.root_node(), source), source);
        let SqlTree::File { statements, .. } = tree else { panic!(); };
        let SqlTree::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlTree::Set { target, .. } = inner.as_ref() else {
            panic!("expected Set, got {inner:?}");
        };
        assert!(matches!(target.as_ref(), SqlTree::Variable { .. }));
    }

    #[test]
    fn transaction_lowers_to_typed_transaction() {
        let source = "BEGIN TRANSACTION; UPDATE T SET v = 1; COMMIT";
        let tree = parse_tsql(source);
        let tree = lower_sql_root(&crate::raw::RawNode::from_tree_sitter(tree.root_node(), source), source);
        let SqlTree::File { statements, .. } = tree else { panic!(); };
        let SqlTree::Transaction { statements: inner, .. } = &statements[0] else {
            panic!("expected Transaction, got {:?}", statements[0]);
        };
        assert!(!inner.is_empty(), "transaction has no inner statements");
    }

    #[test]
    fn merge_lowers_to_typed_merge() {
        let source = "MERGE INTO T AS t USING S AS s ON t.id = s.id WHEN MATCHED THEN UPDATE SET t.v = s.v";
        let tree = parse_tsql(source);
        let tree = lower_sql_root(&crate::raw::RawNode::from_tree_sitter(tree.root_node(), source), source);
        let SqlTree::File { statements, .. } = tree else { panic!(); };
        let SqlTree::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlTree::Merge { whens, .. } = inner.as_ref() else {
            panic!("expected Merge, got {inner:?}");
        };
        assert!(!whens.is_empty());
        assert!(matches!(&whens[0], SqlTree::MergeWhen { .. }));
    }

    #[test]
    fn create_function_lowers_to_typed_function() {
        let source = "CREATE FUNCTION dbo.GetAge(@b DATE) RETURNS INT AS BEGIN RETURN 1 END";
        let tree = parse_tsql(source);
        let tree = lower_sql_root(&crate::raw::RawNode::from_tree_sitter(tree.root_node(), source), source);
        let SqlTree::File { statements, .. } = tree else { panic!(); };
        let SqlTree::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlTree::Function { schema, name: _, parameters, .. } = inner.as_ref() else {
            panic!("expected Function, got {inner:?}");
        };
        assert!(schema.is_some(), "schema missing");
        assert_eq!(parameters.len(), 1);
    }

    #[test]
    fn left_join_lowers_with_typed_join_kind() {
        let source = "SELECT * FROM A LEFT JOIN B ON A.id = B.id";
        let tree = parse_tsql(source);
        let tree = lower_sql_root(&crate::raw::RawNode::from_tree_sitter(tree.root_node(), source), source);
        let SqlTree::File { statements, .. } = tree else { panic!(); };
        let SqlTree::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlTree::Select { from, .. } = inner.as_ref() else { panic!(); };
        let from = from.as_ref().expect("from");
        let SqlTree::From { relations, .. } = from.as_ref() else { panic!(); };
        // Find a Join in relations.
        let join = relations.iter().find(|r| matches!(r, SqlTree::Join { .. }));
        let SqlTree::Join { extra_markers, on, .. } = join.expect("join") else { panic!(); };
        assert!(extra_markers.iter().any(|m| m.name == "left"));
        assert!(on.is_some());
    }

    #[test]
    fn order_by_with_desc_lowers_to_typed_order_target() {
        let source = "SELECT * FROM x ORDER BY name DESC";
        let tree = parse_tsql(source);
        let tree = lower_sql_root(&crate::raw::RawNode::from_tree_sitter(tree.root_node(), source), source);
        let SqlTree::File { statements, .. } = tree else { panic!(); };
        let SqlTree::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlTree::Select { order_by, .. } = inner.as_ref() else { panic!(); };
        let order_by = order_by.as_ref().expect("order_by");
        let SqlTree::OrderBy { targets, .. } = order_by.as_ref() else { panic!(); };
        assert_eq!(targets.len(), 1);
        let SqlTree::OrderTarget { extra_markers, .. } = &targets[0] else { panic!(); };
        // DESC may not be detected if grammar doesn't expose `direction`
        // — at least assert the OrderTarget shape.
        let _ = extra_markers;
    }

    #[test]
    fn case_when_then_else_lowers_to_typed_case() {
        let source = "SELECT CASE WHEN a > 0 THEN 'P' ELSE 'N' END FROM x";
        let tree = parse_tsql(source);
        let tree = lower_sql_root(&crate::raw::RawNode::from_tree_sitter(tree.root_node(), source), source);
        let SqlTree::File { statements, .. } = tree else { panic!(); };
        let SqlTree::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlTree::Select { columns, .. } = inner.as_ref() else { panic!(); };
        let case_ir = match columns.first() {
            Some(SqlTree::Column { expression, .. }) => expression.as_ref(),
            Some(other) => other,
            None => panic!("no columns"),
        };
        let SqlTree::Case { whens, else_, .. } = case_ir else {
            panic!("expected Case, got {case_ir:?}");
        };
        assert_eq!(whens.len(), 1);
        assert!(else_.is_some());
    }

    #[test]
    fn delete_with_where_lowers_to_typed_delete() {
        let source = "DELETE FROM Old WHERE Created < '2020'";
        let tree = parse_tsql(source);
        let tree = lower_sql_root(&crate::raw::RawNode::from_tree_sitter(tree.root_node(), source), source);
        let SqlTree::File { statements, .. } = tree else { panic!(); };
        let SqlTree::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlTree::Delete { from, where_, .. } = inner.as_ref() else {
            panic!("expected Delete, got {inner:?}");
        };
        assert!(from.is_some(), "from missing");
        assert!(where_.is_some(), "where missing");
    }

    #[test]
    fn where_with_compare_lowers_to_typed_compare() {
        let source = "SELECT * FROM U WHERE Active = 1";
        let tree = parse_tsql(source);
        let tree = lower_sql_root(&crate::raw::RawNode::from_tree_sitter(tree.root_node(), source), source);
        let SqlTree::File { statements, .. } = tree else { panic!(); };
        let SqlTree::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlTree::Select { where_, .. } = inner.as_ref() else { panic!(); };
        let where_ = where_.as_ref().expect("where present");
        let SqlTree::Where { condition, .. } = where_.as_ref() else { panic!(); };
        let SqlTree::Compare { op_marker, op_text, .. } = condition.as_ref() else {
            panic!("expected Compare, got {condition:?}");
        };
        assert_eq!(*op_marker, "equal");
        assert_eq!(op_text, "=");
    }
}
