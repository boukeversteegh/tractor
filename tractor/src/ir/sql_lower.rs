//! TSQL CST → [`SqlIr`] lowering.
//!
//! Parallel to `tsql.rs` (which emits the cross-language `Ir`).
//! This module emits the typed [`SqlIr`] enum — every TSQL
//! construct gets a typed variant with named slots, eliminating
//! the generic-`SimpleStatement` ambiguity that drove the iter
//! 29-36 JSON projection heuristics.
//!
//! Status: under construction. Production parser still routes
//! through `tsql.rs`. Coverage extends per slice; once parity
//! across the TSQL blueprint is reached, the parser flips and
//! `tsql.rs` retires.

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::lower_helpers::{range_of, span_of};
use super::sql::{ComparisonOp, CreateKind, DropKind, SqlIr};

/// Lower a T-SQL `program` CST root to [`SqlIr::File`].
pub fn lower_sql_root(root: TsNode<'_>, source: &str) -> SqlIr {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "program" => {
            let mut cur = root.walk();
            let statements: Vec<SqlIr> = root
                .named_children(&mut cur)
                .map(|c| lower_node(c, source))
                .collect();
            SqlIr::File { statements, range, span }
        }
        other => SqlIr::Unknown { kind: other.to_string(), range, span },
    }
}

fn lower_node(node: TsNode<'_>, source: &str) -> SqlIr {
    let span = span_of(node);
    let range = range_of(node);
    let kind = node.kind();

    match kind {
        // ----- Top-level wrapping ------------------------------------
        "statement" => lower_statement(node, source),
        "go_statement" => SqlIr::Go { range, span },

        // ----- Atoms --------------------------------------------------
        "identifier" => {
            let text = range.slice(source);
            if text.starts_with('@') {
                SqlIr::Variable { range, span }
            } else {
                SqlIr::Identifier { range, span }
            }
        }
        "literal" => SqlIr::Literal { range, span },
        "string" | "national_string" => SqlIr::Literal { range, span },
        "int" => SqlIr::Literal { range, span },
        "comment" | "line_comment" | "block_comment" => SqlIr::Comment { range, span },

        // ----- DML statements ----------------------------------------
        "select" => lower_select(node, source),
        "insert" => lower_insert(node, source),
        "update" => lower_update(node, source),
        "delete" => lower_delete(node, source),
        "subquery" => lower_subquery(node, source),

        // ----- Clauses (when they appear as children of select) ------
        "from" => lower_from(node, source),
        "where" => lower_where(node, source),

        // ----- References / columns ----------------------------------
        "object_reference" => lower_object_reference(node, source),
        "column_reference" | "field" => lower_column_reference(node, source),
        "all_fields" => SqlIr::Star { qualifier: None, range, span },
        "term" => lower_term(node, source),
        "relation" => lower_relation(node, source),

        // ----- Expressions -------------------------------------------
        "binary_expression" => lower_binary_or_compare(node, source),
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
        "int" => SqlIr::DataType { name: "int", length: None, range, span },
        "varchar" => lower_data_type(node, "varchar", source),
        "nvarchar" => lower_data_type(node, "nvarchar", source),
        "datetime" => SqlIr::DataType { name: "datetime", length: None, range, span },

        // ----- Fallback ----------------------------------------------
        other => SqlIr::Unknown {
            kind: other.to_string(),
            range,
            span,
        },
    }
}

/// `statement` CST → typed statement wrapped in `SqlIr::Statement`.
///
/// The TSQL grammar puts the `<select>` clause and its companion
/// clauses (`<from>`, `<where>`, etc.) as SIBLINGS under the
/// `<statement>` node. `lower_statement` looks at all children to
/// determine the statement kind and gathers the clauses into a
/// single typed variant.
fn lower_statement(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut cur = node.walk();
    let kinds: Vec<&str> = node
        .named_children(&mut cur)
        .map(|c| c.kind())
        .collect();

    // SELECT statement: has a `select` clause (and possibly select_expression
    // when grammar uses it directly).
    if kinds.iter().any(|k| *k == "select" || *k == "select_expression") {
        let inner = aggregate_select(node, source);
        return SqlIr::Statement { inner: Box::new(inner), range, span };
    }
    // UPDATE statement: aggregate `update` + `from` (where lives nested).
    if kinds.iter().any(|k| *k == "update") {
        let inner = aggregate_update(node, source);
        return SqlIr::Statement { inner: Box::new(inner), range, span };
    }
    // DELETE statement: aggregate `delete` + `from` (where nested).
    if kinds.iter().any(|k| *k == "delete") {
        let inner = aggregate_delete(node, source);
        return SqlIr::Statement { inner: Box::new(inner), range, span };
    }
    // INSERT statement: a single `insert` child does the work.
    if kinds.iter().any(|k| *k == "insert") {
        if let Some(insert_node) = node
            .named_children(&mut node.walk())
            .find(|c| c.kind() == "insert")
        {
            let inner = lower_insert(insert_node, source);
            return SqlIr::Statement { inner: Box::new(inner), range, span };
        }
    }
    // Fallback: lower the first named child as the statement body.
    let mut cur2 = node.walk();
    let first_named = node.named_children(&mut cur2).next();
    let inner = first_named
        .map(|c| lower_node(c, source))
        .unwrap_or_else(|| SqlIr::Unknown {
            kind: "empty_statement".into(),
            range,
            span,
        });
    SqlIr::Statement { inner: Box::new(inner), range, span }
}

/// Aggregate `update` + nested `from`/`where` clauses into a
/// `SqlIr::Update`. The TSQL grammar emits `update` as a sibling
/// of `from` under `<statement>`, with `from` carrying `where` etc.
fn aggregate_update(stmt: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(stmt);
    let span = span_of(stmt);
    let mut table: Option<SqlIr> = None;
    let mut assignments: Vec<SqlIr> = Vec::new();
    let mut where_: Option<Box<SqlIr>> = None;

    let mut cur = stmt.walk();
    for c in stmt.named_children(&mut cur) {
        match c.kind() {
            "update" => {
                // Children: keyword_update, relation, keyword_set,
                // assignment, [comma assignment]…, where.
                // The TSQL grammar puts `where` INSIDE `update` here
                // (unlike SELECT, where it's nested in `from`).
                let mut sub = c.walk();
                for inner in c.named_children(&mut sub) {
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
                let mut sub = c.walk();
                for inner in c.named_children(&mut sub) {
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

    let table = Box::new(table.unwrap_or(SqlIr::Unknown {
        kind: "missing_update_table".into(),
        range,
        span,
    }));
    SqlIr::Update {
        table,
        assignments,
        where_,
        range,
        span,
    }
}

/// Aggregate `delete` + nested `from`/`where` into `SqlIr::Delete`.
fn aggregate_delete(stmt: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(stmt);
    let span = span_of(stmt);
    let mut from: Option<Box<SqlIr>> = None;
    let mut where_: Option<Box<SqlIr>> = None;

    let mut cur = stmt.walk();
    for c in stmt.named_children(&mut cur) {
        match c.kind() {
            "delete" => {
                // Usually empty body — DELETE just sits as a marker.
            }
            "from" => {
                let mut sub = c.walk();
                let mut relation_nodes: Vec<TsNode<'_>> = Vec::new();
                for inner in c.named_children(&mut sub) {
                    let ik = inner.kind();
                    if ik.starts_with("keyword_") || ik.starts_with("op_") {
                        continue;
                    }
                    match ik {
                        "where" => where_ = Some(Box::new(lower_where(inner, source))),
                        _ => relation_nodes.push(inner),
                    }
                }
                let relations: Vec<SqlIr> = relation_nodes
                    .into_iter()
                    .map(|n| lower_node(n, source))
                    .collect();
                from = Some(Box::new(SqlIr::From {
                    relations,
                    range: range_of(c),
                    span: span_of(c),
                }));
            }
            "where" => where_ = Some(Box::new(lower_where(c, source))),
            _ => {}
        }
    }

    SqlIr::Delete {
        from,
        where_,
        range,
        span,
    }
}

/// `assignment` CST → `SqlIr::Assign { target, value }`. Used in
/// UPDATE SET and SET @var = val.
fn lower_assignment(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut cur = node.walk();
    let operands: Vec<TsNode<'_>> = node
        .named_children(&mut cur)
        .filter(|c| {
            !c.kind().starts_with("keyword_") && !c.kind().starts_with("op_")
        })
        .collect();
    let target = operands
        .first()
        .map(|c| lower_node(*c, source))
        .unwrap_or(SqlIr::Unknown {
            kind: "missing_assign_target".into(),
            range,
            span,
        });
    let value = operands
        .get(1)
        .map(|c| lower_node(*c, source))
        .unwrap_or(SqlIr::Unknown {
            kind: "missing_assign_value".into(),
            range,
            span,
        });
    SqlIr::Assign {
        target: Box::new(target),
        value: Box::new(value),
        range,
        span,
    }
}

/// `update` CST as a direct lowering target — fallback when called
/// outside `aggregate_update`'s context.
fn lower_update(node: TsNode<'_>, source: &str) -> SqlIr {
    aggregate_update(node, source)
}

/// `delete` CST — fallback.
fn lower_delete(node: TsNode<'_>, source: &str) -> SqlIr {
    aggregate_delete(node, source)
}

/// `subquery` CST → `SqlIr::Subquery { select }`.
fn lower_subquery(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    // A subquery's body is a Select. The CST may have it directly
    // or wrapped in another statement node — we recurse.
    let mut cur = node.walk();
    let inner = node
        .named_children(&mut cur)
        .next()
        .map(|c| {
            if c.kind() == "select" || c.kind() == "select_expression" {
                aggregate_select(node, source)
            } else {
                lower_node(c, source)
            }
        })
        .unwrap_or(SqlIr::Unknown {
            kind: "empty_subquery".into(),
            range,
            span,
        });
    SqlIr::Subquery {
        select: Box::new(inner),
        range,
        span,
    }
}

/// Aggregate `select`/`from`/`where`/`group_by`/`having`/`order_by`
/// children of a `statement` node into a single `SqlIr::Select`.
fn aggregate_select(stmt: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(stmt);
    let span = span_of(stmt);
    let mut columns: Vec<SqlIr> = Vec::new();
    let into: Option<Box<SqlIr>> = None;
    let mut from: Option<Box<SqlIr>> = None;
    let mut where_: Option<Box<SqlIr>> = None;
    let mut group_by: Option<Box<SqlIr>> = None;
    let mut having: Option<Box<SqlIr>> = None;
    let mut order_by: Option<Box<SqlIr>> = None;

    let mut cur = stmt.walk();
    for c in stmt.named_children(&mut cur) {
        match c.kind() {
            "select" => {
                // Walk select's named children — `keyword_select` and
                // `select_expression`. Filter the keyword; dive into
                // select_expression to collect column items.
                let mut sub = c.walk();
                for inner in c.named_children(&mut sub) {
                    let ik = inner.kind();
                    if ik.starts_with("keyword_") || ik.starts_with("op_") {
                        continue;
                    }
                    match ik {
                        "select_expression" => {
                            let mut sc = inner.walk();
                            for item in inner.named_children(&mut sc) {
                                columns.push(lower_column_item(item, source));
                            }
                        }
                        "all_fields" => {
                            columns.push(SqlIr::Star {
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
                let mut sub = c.walk();
                let mut relation_nodes: Vec<TsNode<'_>> = Vec::new();
                for inner in c.named_children(&mut sub) {
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
                let relations: Vec<SqlIr> = relation_nodes
                    .into_iter()
                    .map(|n| lower_node(n, source))
                    .collect();
                from = Some(Box::new(SqlIr::From {
                    relations,
                    range: range_of(c),
                    span: span_of(c),
                }));
            }
            "where" => where_ = Some(Box::new(lower_where(c, source))),
            "group_by" => group_by = Some(Box::new(lower_node(c, source))),
            "having" => having = Some(Box::new(lower_node(c, source))),
            "order_by" => order_by = Some(Box::new(lower_node(c, source))),
            _ => {}
        }
    }

    SqlIr::Select {
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

/// `select` CST → `SqlIr::Select`. Walks named children and assigns
/// each to its typed slot based on CST kind. Anonymous tokens
/// (commas) are ignored — they aren't needed in `SqlIr`.
fn lower_select(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut columns: Vec<SqlIr> = Vec::new();
    let mut into: Option<Box<SqlIr>> = None;
    let mut from: Option<Box<SqlIr>> = None;
    let mut where_: Option<Box<SqlIr>> = None;
    let mut group_by: Option<Box<SqlIr>> = None;
    let mut having: Option<Box<SqlIr>> = None;
    let mut order_by: Option<Box<SqlIr>> = None;

    let mut cur = node.walk();
    for c in node.named_children(&mut cur) {
        match c.kind() {
            "select_expression" => {
                // The list of selected items.
                let mut sub_cur = c.walk();
                for item in c.named_children(&mut sub_cur) {
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

    SqlIr::Select {
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

/// `insert` CST → `SqlIr::Insert`. The CST is flat: `keyword_insert`,
/// `keyword_into`, `object_reference` (table), `list` (columns,
/// before VALUES), `keyword_values`, `list` (values, after VALUES).
fn lower_insert(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut table: Option<SqlIr> = None;
    let mut columns: Vec<SqlIr> = Vec::new();
    let mut values: Vec<SqlIr> = Vec::new();
    let mut seen_values_kw = false;

    let mut cur = node.walk();
    for c in node.children(&mut cur) {
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
                let mut sub_cur = c.walk();
                for item in c.named_children(&mut sub_cur) {
                    columns.push(lower_node(item, source));
                }
            }
            "list" if seen_values_kw => {
                let mut sub_cur = c.walk();
                for item in c.named_children(&mut sub_cur) {
                    values.push(lower_node(item, source));
                }
            }
            _ => {}
        }
    }

    let table = Box::new(table.unwrap_or_else(|| SqlIr::Unknown {
        kind: "missing_table".into(),
        range,
        span,
    }));
    SqlIr::Insert { table, columns, values, range, span }
}

/// `from` CST → `SqlIr::From`. Children are relations; the first is
/// the base, subsequent ones may be JOINs.
fn lower_from(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut relations: Vec<SqlIr> = Vec::new();
    let mut cur = node.walk();
    for c in node.named_children(&mut cur) {
        relations.push(lower_node(c, source));
    }
    SqlIr::From { relations, range, span }
}

/// `where` CST → `SqlIr::Where`. The first non-keyword named child
/// is the condition expression.
fn lower_where(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut cur = node.walk();
    let condition = node
        .named_children(&mut cur)
        .filter(|c| !c.kind().starts_with("keyword_") && !c.kind().starts_with("op_"))
        .next()
        .map(|c| lower_node(c, source))
        .unwrap_or_else(|| SqlIr::Unknown {
            kind: "missing_where_condition".into(),
            range,
            span,
        });
    SqlIr::Where { condition: Box::new(condition), range, span }
}

/// `object_reference` (in a relation context) becomes a
/// `SqlIr::Relation` with a single name and no schema/alias.
/// In other contexts it becomes a `SqlIr::Reference`.
fn lower_object_reference(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut cur = node.walk();
    let parts: Vec<SqlIr> = node
        .named_children(&mut cur)
        .map(|c| lower_node(c, source))
        .collect();
    if parts.len() == 1 {
        // Single identifier — return it directly.
        parts.into_iter().next().unwrap_or(SqlIr::Unknown {
            kind: "empty_object_reference".into(),
            range,
            span,
        })
    } else {
        SqlIr::Reference { parts, range, span }
    }
}

/// `column_reference` / `field` CST → `SqlIr::Reference` or a single
/// `Identifier`/`Variable` when the chain has one segment.
fn lower_column_reference(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let text = range.slice(source);
    if text.starts_with('@') {
        return SqlIr::Variable { range, span };
    }
    let mut cur = node.walk();
    let named: Vec<TsNode<'_>> = node.named_children(&mut cur).collect();
    if named.len() == 1 && named[0].kind() == "identifier" {
        return SqlIr::Identifier { range, span };
    }
    let parts: Vec<SqlIr> = named.into_iter().map(|c| lower_node(c, source)).collect();
    SqlIr::Reference { parts, range, span }
}

/// Lower a single SELECT column item. The CST puts each item under
/// a `term` wrapper, but for `*` we want a bare `SqlIr::Star`
/// rather than `Column { expression: Star }`.
fn lower_column_item(node: TsNode<'_>, source: &str) -> SqlIr {
    if node.kind() == "term" {
        let mut sub = node.walk();
        let named: Vec<TsNode<'_>> = node.named_children(&mut sub).collect();
        if named.len() == 1 && named[0].kind() == "all_fields" {
            return SqlIr::Star {
                qualifier: None,
                range: range_of(named[0]),
                span: span_of(named[0]),
            };
        }
    }
    lower_node(node, source)
}

/// `term` (a SELECT column item) → `SqlIr::Column`. May contain an
/// alias (trailing identifier after AS) and an expression.
fn lower_term(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut cur = node.walk();
    let named: Vec<TsNode<'_>> = node.named_children(&mut cur).collect();
    let mut alias: Option<Box<SqlIr>> = None;
    let mut expression: Option<SqlIr> = None;
    if named.len() >= 2 {
        // Last child as alias, first as expression. Crude but
        // matches the common case; fancier alias detection lands
        // when needed.
        let last = named.last().unwrap();
        if last.kind() == "identifier" {
            alias = Some(Box::new(SqlIr::Alias {
                range: range_of(*last),
                span: span_of(*last),
            }));
            expression = Some(lower_node(named[0], source));
        }
    }
    if expression.is_none() {
        // No alias detected — first named child is the expression.
        expression = named.first().map(|c| lower_node(*c, source));
    }
    let expression = Box::new(expression.unwrap_or(SqlIr::Unknown {
        kind: "empty_term".into(),
        range,
        span,
    }));
    SqlIr::Column { expression, alias, range, span }
}

/// `relation` CST → `SqlIr::Relation`. Captures schema, name, alias.
fn lower_relation(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut cur = node.walk();
    let named: Vec<TsNode<'_>> = node.named_children(&mut cur).collect();
    // A relation is typically `[schema.]name [alias]`.
    let mut schema: Option<Box<SqlIr>> = None;
    let mut alias: Option<Box<SqlIr>> = None;

    let name_idx = match named.len() {
        0 => return SqlIr::Unknown { kind: "empty_relation".into(), range, span },
        1 => 0,
        2 => {
            // Either `schema.name` or `name alias`. Heuristic: if the
            // first node is `object_reference` with multiple parts,
            // it's a qualified name; the second is alias.
            let first = named[0];
            if first.kind() == "object_reference" {
                let mut fc = first.walk();
                let parts = first.named_children(&mut fc).count();
                if parts > 1 {
                    // Qualified name; second is alias.
                    alias = Some(Box::new(SqlIr::Alias {
                        range: range_of(named[1]),
                        span: span_of(named[1]),
                    }));
                    0
                } else {
                    // Bare name; second is alias.
                    alias = Some(Box::new(SqlIr::Alias {
                        range: range_of(named[1]),
                        span: span_of(named[1]),
                    }));
                    0
                }
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
        let mut nc = name_node.walk();
        let parts: Vec<TsNode<'_>> = name_node.named_children(&mut nc).collect();
        if parts.len() >= 2 {
            schema = Some(Box::new(SqlIr::Schema {
                range: range_of(parts[0]),
                span: span_of(parts[0]),
            }));
            Box::new(SqlIr::Identifier {
                range: range_of(parts[1]),
                span: span_of(parts[1]),
            })
        } else if let Some(only) = parts.first() {
            Box::new(SqlIr::Identifier {
                range: range_of(*only),
                span: span_of(*only),
            })
        } else {
            Box::new(SqlIr::Identifier {
                range: range_of(name_node),
                span: span_of(name_node),
            })
        }
    } else {
        Box::new(SqlIr::Identifier {
            range: range_of(name_node),
            span: span_of(name_node),
        })
    };

    SqlIr::Relation { schema, name, alias, range, span }
}

/// `binary_expression` CST → `SqlIr::Compare` (when the operator is
/// a comparison) or `SqlIr::Binary` (arithmetic / logical / bitwise).
fn lower_binary_or_compare(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut cur = node.walk();
    let named: Vec<TsNode<'_>> = node.named_children(&mut cur).collect();

    // Find the operator child by scanning all (named + unnamed)
    // children for the first non-keyword anonymous token.
    let op_text = {
        let mut walk = node.walk();
        let mut found: Option<&str> = None;
        for c in node.children(&mut walk) {
            let kind = c.kind();
            if !c.is_named() {
                let t = range_of(c).slice(source).trim();
                if !t.is_empty() {
                    found = Some(match t {
                        "=" => "=",
                        "<" => "<",
                        ">" => ">",
                        "<=" => "<=",
                        ">=" => ">=",
                        "<>" => "<>",
                        "!=" => "!=",
                        "+" => "+",
                        "-" => "-",
                        "*" => "*",
                        "/" => "/",
                        _ => "?",
                    });
                    break;
                }
            } else if kind.starts_with("keyword_") {
                let t = range_of(c).slice(source).to_uppercase();
                let mapped = match t.as_str() {
                    "AND" => Some("AND"),
                    "OR" => Some("OR"),
                    "LIKE" => Some("LIKE"),
                    "IN" => Some("IN"),
                    "IS" => Some("IS"),
                    _ => None,
                };
                if let Some(m) = mapped { found = Some(m); break; }
            }
        }
        found.unwrap_or("?")
    };

    let operands: Vec<&TsNode<'_>> = named.iter().filter(|c| {
        !c.kind().starts_with("keyword_") && !c.kind().starts_with("op_")
    }).collect();
    let left = operands
        .first()
        .map(|c| lower_node(**c, source))
        .unwrap_or(SqlIr::Unknown {
            kind: "missing_left".into(),
            range,
            span,
        });
    let right = operands
        .get(1)
        .map(|c| lower_node(**c, source))
        .unwrap_or(SqlIr::Unknown {
            kind: "missing_right".into(),
            range,
            span,
        });

    if let Some(cmp_op) = comparison_op_from_text(op_text) {
        return SqlIr::Compare {
            left: Box::new(left),
            op: cmp_op,
            right: Box::new(right),
            range,
            span,
        };
    }
    // Otherwise fall through to Binary — once BinaryOp parsing is
    // implemented. For now, treat as Unknown.
    let _ = (left, right);
    SqlIr::Unknown {
        kind: format!("binary_op_unhandled:{}", op_text),
        range,
        span,
    }
}

/// `between_expression` CST → `SqlIr::Between { value, low, high }`.
fn lower_between(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut cur = node.walk();
    let operands: Vec<TsNode<'_>> = node
        .named_children(&mut cur)
        .filter(|c| {
            !c.kind().starts_with("keyword_") && !c.kind().starts_with("op_")
        })
        .collect();
    let value = operands
        .first()
        .map(|c| lower_node(*c, source))
        .unwrap_or(SqlIr::Unknown { kind: "missing_between_value".into(), range, span });
    let low = operands
        .get(1)
        .map(|c| lower_node(*c, source))
        .unwrap_or(SqlIr::Unknown { kind: "missing_between_low".into(), range, span });
    let high = operands
        .get(2)
        .map(|c| lower_node(*c, source))
        .unwrap_or(SqlIr::Unknown { kind: "missing_between_high".into(), range, span });
    SqlIr::Between {
        value: Box::new(value),
        low: Box::new(low),
        high: Box::new(high),
        range,
        span,
    }
}

/// `case` CST → `SqlIr::Case { whens, else_ }`. The flat CST
/// (keyword_when, cond, keyword_then, value, [keyword_when ...
/// keyword_else], elseval, keyword_end) is grouped into typed
/// When { condition, value } arms.
fn lower_case(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut whens: Vec<SqlIr> = Vec::new();
    let mut else_: Option<Box<SqlIr>> = None;

    #[derive(Copy, Clone, PartialEq)]
    enum State { Before, AfterWhen, AfterThen, AfterElse }
    let mut state = State::Before;
    let mut cond_buffer: Option<SqlIr> = None;
    let mut when_start: super::types::ByteRange =
        super::types::ByteRange::empty_at(range.start);

    let mut cur = node.walk();
    for c in node.named_children(&mut cur) {
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
                let cond = cond_buffer.take().unwrap_or(SqlIr::Unknown {
                    kind: "missing_when_condition".into(),
                    range,
                    span,
                });
                let value = lower_node(c, source);
                let v_end = value.range().end;
                whens.push(SqlIr::When {
                    condition: Box::new(cond),
                    value: Box::new(value),
                    range: super::types::ByteRange::new(when_start.start, v_end),
                    span,
                });
            }
            State::AfterElse => {
                else_ = Some(Box::new(lower_node(c, source)));
            }
            State::Before => {}
        }
    }

    SqlIr::Case { whens, else_, range, span }
}

/// `exists` CST → `SqlIr::Exists { subquery }`.
fn lower_exists(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut cur = node.walk();
    let inner = node
        .named_children(&mut cur)
        .filter(|c| !c.kind().starts_with("keyword_"))
        .next()
        .map(|c| lower_node(c, source))
        .unwrap_or(SqlIr::Unknown { kind: "empty_exists".into(), range, span });
    SqlIr::Exists { subquery: Box::new(inner), range, span }
}

/// `cast` CST → `SqlIr::Cast { value, type_ }`.
fn lower_cast(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut cur = node.walk();
    let named: Vec<TsNode<'_>> = node
        .named_children(&mut cur)
        .filter(|c| !c.kind().starts_with("keyword_"))
        .collect();
    let value = named.first().map(|c| lower_node(*c, source)).unwrap_or(
        SqlIr::Unknown { kind: "missing_cast_value".into(), range, span },
    );
    let type_ = named.get(1).map(|c| lower_node(*c, source)).unwrap_or(
        SqlIr::Unknown { kind: "missing_cast_type".into(), range, span },
    );
    SqlIr::Cast {
        value: Box::new(value),
        type_: Box::new(type_),
        range,
        span,
    }
}

/// `invocation` CST → `SqlIr::Call { callee, arguments }`.
fn lower_call(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut cur = node.walk();
    let named: Vec<TsNode<'_>> = node
        .named_children(&mut cur)
        .filter(|c| !c.kind().starts_with("keyword_"))
        .collect();
    let callee = named.first().map(|c| lower_node(*c, source)).unwrap_or(
        SqlIr::Unknown { kind: "missing_call_callee".into(), range, span },
    );
    let arguments: Vec<SqlIr> = named.iter().skip(1).map(|c| lower_node(*c, source)).collect();
    SqlIr::Call {
        callee: Box::new(callee),
        arguments,
        range,
        span,
    }
}

/// `create_table` / `create_view` / `create_index` → `SqlIr::Create`.
fn lower_create(node: TsNode<'_>, kind: CreateKind, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut cur = node.walk();
    let named: Vec<TsNode<'_>> = node
        .named_children(&mut cur)
        .filter(|c| !c.kind().starts_with("keyword_"))
        .collect();
    let name = named.first().map(|c| {
        // Usually object_reference/identifier — pull the inner identifier.
        let mut nc = c.walk();
        let inner: Vec<TsNode<'_>> = c.named_children(&mut nc).collect();
        if let Some(only) = inner.first() {
            SqlIr::Identifier {
                range: range_of(*only),
                span: span_of(*only),
            }
        } else {
            SqlIr::Identifier {
                range: range_of(*c),
                span: span_of(*c),
            }
        }
    }).unwrap_or(SqlIr::Unknown { kind: "missing_create_name".into(), range, span });
    // Body collects the substantive children. For CREATE TABLE the
    // grammar wraps column definitions in `column_definitions`; we
    // inline its children so `body` is a flat list of `ColumnDef`.
    let mut body: Vec<SqlIr> = Vec::new();
    for c in named.iter().skip(1) {
        if c.kind() == "column_definitions" {
            let mut sub = c.walk();
            for inner in c.named_children(&mut sub) {
                if !inner.kind().starts_with("keyword_") {
                    body.push(lower_node(inner, source));
                }
            }
        } else {
            body.push(lower_node(*c, source));
        }
    }
    SqlIr::Create {
        kind,
        name: Box::new(name),
        body,
        range,
        span,
    }
}

/// `drop_table` / `drop_index` → `SqlIr::Drop`.
fn lower_drop(node: TsNode<'_>, kind: DropKind, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut cur = node.walk();
    let named: Vec<TsNode<'_>> = node
        .named_children(&mut cur)
        .filter(|c| !c.kind().starts_with("keyword_"))
        .collect();
    let name = named.first().map(|c| {
        let mut nc = c.walk();
        let inner: Vec<TsNode<'_>> = c.named_children(&mut nc).collect();
        if let Some(only) = inner.first() {
            SqlIr::Identifier { range: range_of(*only), span: span_of(*only) }
        } else {
            SqlIr::Identifier { range: range_of(*c), span: span_of(*c) }
        }
    }).unwrap_or(SqlIr::Unknown { kind: "missing_drop_name".into(), range, span });
    let _ = source;
    SqlIr::Drop {
        kind,
        name: Box::new(name),
        range,
        span,
    }
}

/// `alter_table` → `SqlIr::Alter { name, operation }`.
fn lower_alter(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut cur = node.walk();
    let named: Vec<TsNode<'_>> = node
        .named_children(&mut cur)
        .filter(|c| !c.kind().starts_with("keyword_"))
        .collect();
    let name = named.first().map(|c| {
        let mut nc = c.walk();
        let inner: Vec<TsNode<'_>> = c.named_children(&mut nc).collect();
        if let Some(only) = inner.first() {
            SqlIr::Identifier { range: range_of(*only), span: span_of(*only) }
        } else {
            SqlIr::Identifier { range: range_of(*c), span: span_of(*c) }
        }
    }).unwrap_or(SqlIr::Unknown { kind: "missing_alter_name".into(), range, span });
    let operation = named.get(1).map(|c| lower_node(*c, source)).unwrap_or(
        SqlIr::Unknown { kind: "missing_alter_operation".into(), range, span },
    );
    SqlIr::Alter {
        name: Box::new(name),
        operation: Box::new(operation),
        range,
        span,
    }
}

/// `add_column` CST → `SqlIr::AddColumn { column }`.
fn lower_add_column(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut cur = node.walk();
    let column = node
        .named_children(&mut cur)
        .filter(|c| !c.kind().starts_with("keyword_"))
        .next()
        .map(|c| lower_node(c, source))
        .unwrap_or(SqlIr::Unknown { kind: "missing_add_column".into(), range, span });
    SqlIr::AddColumn {
        column: Box::new(column),
        range,
        span,
    }
}

/// `column_definition` CST → `SqlIr::ColumnDef { name, type_, constraints }`.
fn lower_column_def(node: TsNode<'_>, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut cur = node.walk();
    let named: Vec<TsNode<'_>> = node
        .named_children(&mut cur)
        .filter(|c| !c.kind().starts_with("keyword_"))
        .collect();
    let name = named.first().map(|c| SqlIr::Identifier {
        range: range_of(*c),
        span: span_of(*c),
    }).unwrap_or(SqlIr::Unknown { kind: "missing_column_name".into(), range, span });
    let type_ = named.get(1).map(|c| lower_node(*c, source)).unwrap_or(
        SqlIr::Unknown { kind: "missing_column_type".into(), range, span },
    );
    let constraints: Vec<SqlIr> = named.iter().skip(2).map(|c| lower_node(*c, source)).collect();
    SqlIr::ColumnDef {
        name: Box::new(name),
        type_: Box::new(type_),
        constraints,
        range,
        span,
    }
}

/// `varchar` / `nvarchar` etc. with optional length — `VARCHAR(100)`.
fn lower_data_type(node: TsNode<'_>, name: &'static str, source: &str) -> SqlIr {
    let range = range_of(node);
    let span = span_of(node);
    let mut cur = node.walk();
    let length = node
        .named_children(&mut cur)
        .filter(|c| !c.kind().starts_with("keyword_"))
        .next()
        .map(|c| Box::new(lower_node(c, source)));
    SqlIr::DataType {
        name,
        length,
        range,
        span,
    }
}

fn comparison_op_from_text(text: &str) -> Option<ComparisonOp> {
    Some(match text {
        "=" => ComparisonOp::Equal,
        "<>" | "!=" => ComparisonOp::NotEqual,
        "<" => ComparisonOp::Less,
        "<=" => ComparisonOp::LessEqual,
        ">" => ComparisonOp::Greater,
        ">=" => ComparisonOp::GreaterEqual,
        "LIKE" => ComparisonOp::Like,
        "IN" => ComparisonOp::In,
        "IS" => ComparisonOp::Is,
        "AND" => ComparisonOp::And,
        "OR" => ComparisonOp::Or,
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
        let ir = lower_sql_root(tree.root_node(), source);
        let SqlIr::File { statements, .. } = ir else {
            panic!("expected File, got {ir:?}");
        };
        assert_eq!(statements.len(), 1, "{statements:?}");
        let SqlIr::Statement { inner, .. } = &statements[0] else {
            panic!("expected Statement");
        };
        let SqlIr::Select { columns, from, .. } = inner.as_ref() else {
            panic!("expected Select, got {inner:?}");
        };
        assert!(matches!(columns.first(), Some(SqlIr::Column { .. }) | Some(SqlIr::Star { .. })));
        assert!(from.is_some());
    }

    #[test]
    fn insert_lowers_with_typed_columns_and_values() {
        let source = "INSERT INTO L (a, b) VALUES (1, 'x')";
        let tree = parse_tsql(source);
        let ir = lower_sql_root(tree.root_node(), source);
        let SqlIr::File { statements, .. } = ir else { panic!(); };
        let SqlIr::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlIr::Insert { table, columns, values, .. } = inner.as_ref() else {
            panic!("expected Insert, got {inner:?}");
        };
        assert!(matches!(table.as_ref(), SqlIr::Relation { .. }));
        assert_eq!(columns.len(), 2, "columns: {columns:?}");
        assert_eq!(values.len(), 2, "values: {values:?}");
    }

    #[test]
    fn update_with_set_and_where_lowers_to_typed_update() {
        let source = "UPDATE Users SET Active = 0 WHERE ID = 1";
        let tree = parse_tsql(source);
        let ir = lower_sql_root(tree.root_node(), source);
        let SqlIr::File { statements, .. } = ir else { panic!(); };
        let SqlIr::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlIr::Update { table, assignments, where_, .. } = inner.as_ref() else {
            panic!("expected Update, got {inner:?}");
        };
        assert!(matches!(table.as_ref(), SqlIr::Relation { .. }));
        assert_eq!(assignments.len(), 1);
        assert!(matches!(&assignments[0], SqlIr::Assign { .. }));
        assert!(where_.is_some());
    }

    #[test]
    fn create_table_lowers_to_typed_create_with_column_defs() {
        let source = "CREATE TABLE T (id INT, name VARCHAR(100))";
        let tree = parse_tsql(source);
        let ir = lower_sql_root(tree.root_node(), source);
        let SqlIr::File { statements, .. } = ir else { panic!(); };
        let SqlIr::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlIr::Create { kind, body, .. } = inner.as_ref() else {
            panic!("expected Create, got {inner:?}");
        };
        assert_eq!(*kind, super::super::sql::CreateKind::Table);
        assert_eq!(body.len(), 2);
        assert!(body.iter().all(|c| matches!(c, SqlIr::ColumnDef { .. })));
    }

    #[test]
    fn drop_table_lowers_to_typed_drop() {
        let source = "DROP TABLE T";
        let tree = parse_tsql(source);
        let ir = lower_sql_root(tree.root_node(), source);
        let SqlIr::File { statements, .. } = ir else { panic!(); };
        let SqlIr::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlIr::Drop { kind, .. } = inner.as_ref() else {
            panic!("expected Drop, got {inner:?}");
        };
        assert_eq!(*kind, super::super::sql::DropKind::Table);
    }

    #[test]
    fn between_expression_lowers_to_typed_between() {
        let source = "SELECT a BETWEEN 1 AND 10 FROM x";
        let tree = parse_tsql(source);
        let ir = lower_sql_root(tree.root_node(), source);
        let SqlIr::File { statements, .. } = ir else { panic!(); };
        let SqlIr::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlIr::Select { columns, .. } = inner.as_ref() else { panic!(); };
        // The first column item should be a Column wrapping a Between
        // (or Between directly if term unwrapping is added later).
        match columns.first() {
            Some(SqlIr::Column { expression, .. }) => {
                assert!(matches!(expression.as_ref(), SqlIr::Between { .. }),
                    "column expression: {expression:?}");
            }
            Some(SqlIr::Between { .. }) => {}
            other => panic!("unexpected column: {other:?}"),
        }
    }

    #[test]
    fn case_when_then_else_lowers_to_typed_case() {
        let source = "SELECT CASE WHEN a > 0 THEN 'P' ELSE 'N' END FROM x";
        let tree = parse_tsql(source);
        let ir = lower_sql_root(tree.root_node(), source);
        let SqlIr::File { statements, .. } = ir else { panic!(); };
        let SqlIr::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlIr::Select { columns, .. } = inner.as_ref() else { panic!(); };
        let case_ir = match columns.first() {
            Some(SqlIr::Column { expression, .. }) => expression.as_ref(),
            Some(other) => other,
            None => panic!("no columns"),
        };
        let SqlIr::Case { whens, else_, .. } = case_ir else {
            panic!("expected Case, got {case_ir:?}");
        };
        assert_eq!(whens.len(), 1);
        assert!(else_.is_some());
    }

    #[test]
    fn delete_with_where_lowers_to_typed_delete() {
        let source = "DELETE FROM Old WHERE Created < '2020'";
        let tree = parse_tsql(source);
        let ir = lower_sql_root(tree.root_node(), source);
        let SqlIr::File { statements, .. } = ir else { panic!(); };
        let SqlIr::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlIr::Delete { from, where_, .. } = inner.as_ref() else {
            panic!("expected Delete, got {inner:?}");
        };
        assert!(from.is_some(), "from missing");
        assert!(where_.is_some(), "where missing");
    }

    #[test]
    fn where_with_compare_lowers_to_typed_compare() {
        let source = "SELECT * FROM U WHERE Active = 1";
        let tree = parse_tsql(source);
        let ir = lower_sql_root(tree.root_node(), source);
        let SqlIr::File { statements, .. } = ir else { panic!(); };
        let SqlIr::Statement { inner, .. } = &statements[0] else { panic!(); };
        let SqlIr::Select { where_, .. } = inner.as_ref() else { panic!(); };
        let where_ = where_.as_ref().expect("where present");
        let SqlIr::Where { condition, .. } = where_.as_ref() else { panic!(); };
        let SqlIr::Compare { op, .. } = condition.as_ref() else {
            panic!("expected Compare, got {condition:?}");
        };
        assert_eq!(*op, ComparisonOp::Equal);
    }
}
