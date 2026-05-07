//! [`SqlIr`] → Xot rendering. Produces the queryable XML surface
//! for SQL inputs.
//!
//! Mirrors the existing `<select>/<from>/<where>/<compare>/<column>/
//! <relation>` vocabulary that XPath queries are written against.
//!
//! ## Status
//!
//! Initial slice covers the SELECT / DML / DDL paths exercised by
//! the unit tests in `sql_lower.rs`. Each SqlIr variant has one
//! deterministic XML shape — no heuristics, no post-passes.
//!
//! ## Source-text recovery
//!
//! For now this renderer emits structural XML only (no gap-fill).
//! Text-recovery via `string()` is added once the parser flips
//! TSQL to this pipeline; until then, `tsql.rs` (cross-language
//! `Ir`) handles XPath queries.

#![cfg(feature = "native")]

use xot::{Node as XotNode, Xot};

use super::sql::{
    BinaryOp, ComparisonOp, CreateKind, DropKind, JoinKind, SortDirection, SqlIr, UnaryOp,
};

/// Render a [`SqlIr`] tree as a child of `parent` in `xot`.
pub fn render_sql_to_xot(
    xot: &mut Xot,
    parent: XotNode,
    ir: &SqlIr,
    source: &str,
) -> Result<XotNode, xot::Error> {
    match ir {
        // ----- Top level ------------------------------------------------
        SqlIr::File { statements, .. } => {
            let node = element(xot, "file")?;
            xot.append(parent, node)?;
            for s in statements {
                render_sql_to_xot(xot, node, s, source)?;
            }
            Ok(node)
        }
        SqlIr::Statement { inner, .. } => {
            let node = element(xot, "statement")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, inner, source)?;
            Ok(node)
        }
        SqlIr::Go { .. } => leaf(xot, parent, "go", "GO"),
        SqlIr::Exec { target, .. } => {
            let node = element(xot, "exec")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, target, source)?;
            Ok(node)
        }
        SqlIr::Set { target, value, .. } => {
            let node = element(xot, "set")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, target, source)?;
            render_sql_to_xot(xot, node, value, source)?;
            Ok(node)
        }

        // ----- DML ------------------------------------------------------
        SqlIr::Select {
            ctes,
            columns,
            into,
            from,
            where_,
            group_by,
            having,
            order_by,
            ..
        } => {
            // CTEs render as siblings BEFORE the <select> element
            // under the parent statement.
            for c in ctes {
                render_sql_to_xot(xot, parent, c, source)?;
            }
            let select_node = element(xot, "select")?;
            xot.append(parent, select_node)?;
            for c in columns {
                render_sql_to_xot(xot, select_node, c, source)?;
            }
            // Render clauses as siblings under the parent statement,
            // matching the existing TSQL XML shape (where `<select>`
            // is one element and `<from>`/`<where>` are siblings).
            // We attach to `parent` so the wrapping statement holds
            // them flat.
            if let Some(i) = into {
                let into_node = element(xot, "into")?;
                xot.append(parent, into_node)?;
                render_sql_to_xot(xot, into_node, i, source)?;
            }
            if let Some(f) = from {
                render_sql_to_xot(xot, parent, f, source)?;
            }
            if let Some(w) = where_ {
                render_sql_to_xot(xot, parent, w, source)?;
            }
            if let Some(g) = group_by {
                render_sql_to_xot(xot, parent, g, source)?;
            }
            if let Some(h) = having {
                render_sql_to_xot(xot, parent, h, source)?;
            }
            if let Some(o) = order_by {
                render_sql_to_xot(xot, parent, o, source)?;
            }
            Ok(select_node)
        }
        SqlIr::Insert {
            table,
            columns,
            values,
            ..
        } => {
            let node = element(xot, "insert")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, table, source)?;
            if !columns.is_empty() {
                let cols = element(xot, "columns")?;
                xot.append(node, cols)?;
                for c in columns {
                    render_sql_to_xot(xot, cols, c, source)?;
                }
            }
            if !values.is_empty() {
                let vals = element(xot, "values")?;
                xot.append(node, vals)?;
                for v in values {
                    render_sql_to_xot(xot, vals, v, source)?;
                }
            }
            Ok(node)
        }
        SqlIr::Update {
            table,
            assignments,
            where_,
            ..
        } => {
            let node = element(xot, "update")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, table, source)?;
            for a in assignments {
                render_sql_to_xot(xot, node, a, source)?;
            }
            if let Some(w) = where_ {
                render_sql_to_xot(xot, node, w, source)?;
            }
            Ok(node)
        }
        SqlIr::Delete { from, where_, .. } => {
            let node = element(xot, "delete")?;
            xot.append(parent, node)?;
            if let Some(f) = from {
                render_sql_to_xot(xot, parent, f, source)?;
            }
            if let Some(w) = where_ {
                render_sql_to_xot(xot, parent, w, source)?;
            }
            Ok(node)
        }
        SqlIr::Merge { target, source: src, on, whens, .. } => {
            let node = element(xot, "merge")?;
            xot.append(parent, node)?;
            // Render target/source relations as <relation> children.
            render_sql_to_xot(xot, node, target, source)?;
            render_sql_to_xot(xot, node, src, source)?;
            // ON condition.
            render_sql_to_xot(xot, node, on, source)?;
            // WHEN clauses.
            for w in whens {
                render_sql_to_xot(xot, node, w, source)?;
            }
            Ok(node)
        }
        SqlIr::MergeWhen { matched, action, .. } => {
            let node = element(xot, "when")?;
            xot.append(parent, node)?;
            // Markers — `<matched/>` always, `<not/>` for the NOT MATCHED
            // case. Per Principle: no underscore in element names; split
            // compound words into separate markers.
            if !*matched {
                let n = element(xot, "not")?;
                xot.append(node, n)?;
            }
            let m = element(xot, "matched")?;
            xot.append(node, m)?;
            render_sql_to_xot(xot, node, action, source)?;
            Ok(node)
        }
        SqlIr::Transaction { statements, .. } => {
            let node = element(xot, "transaction")?;
            xot.append(parent, node)?;
            for s in statements {
                render_sql_to_xot(xot, node, s, source)?;
            }
            Ok(node)
        }

        // ----- Clauses --------------------------------------------------
        SqlIr::From { relations, .. } => {
            let node = element(xot, "from")?;
            xot.append(parent, node)?;
            for r in relations {
                render_sql_to_xot(xot, node, r, source)?;
            }
            Ok(node)
        }
        SqlIr::Where { condition, .. } => {
            let node = element(xot, "where")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, condition, source)?;
            Ok(node)
        }
        SqlIr::GroupBy { keys, .. } => {
            let node = element(xot, "group")?;
            xot.append(parent, node)?;
            for k in keys {
                render_sql_to_xot(xot, node, k, source)?;
            }
            Ok(node)
        }
        SqlIr::Having { condition, .. } => {
            let node = element(xot, "having")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, condition, source)?;
            Ok(node)
        }
        SqlIr::OrderBy { targets, .. } => {
            let node = element(xot, "order")?;
            xot.append(parent, node)?;
            for t in targets {
                render_sql_to_xot(xot, node, t, source)?;
            }
            Ok(node)
        }
        SqlIr::OrderTarget {
            expression,
            direction,
            ..
        } => {
            let node = element(xot, "target")?;
            xot.append(parent, node)?;
            if let Some(d) = direction {
                let m = element(
                    xot,
                    match d {
                        SortDirection::Asc => "asc",
                        SortDirection::Desc => "desc",
                    },
                )?;
                xot.append(node, m)?;
            }
            render_sql_to_xot(xot, node, expression, source)?;
            Ok(node)
        }
        SqlIr::PartitionBy { keys, .. } => {
            let node = element(xot, "partition")?;
            xot.append(parent, node)?;
            for k in keys {
                render_sql_to_xot(xot, node, k, source)?;
            }
            Ok(node)
        }
        SqlIr::Join { kind, relation, on, .. } => {
            let node = element(xot, "join")?;
            xot.append(parent, node)?;
            for marker in join_markers(*kind) {
                let m = element(xot, marker)?;
                xot.append(node, m)?;
            }
            render_sql_to_xot(xot, node, relation, source)?;
            if let Some(o) = on {
                render_sql_to_xot(xot, node, o, source)?;
            }
            Ok(node)
        }

        // ----- References -----------------------------------------------
        SqlIr::Relation { schema, name, alias, .. } => {
            let node = element(xot, "relation")?;
            xot.append(parent, node)?;
            // Schema (when present): <schema><bracketed/><name>dbo</name></schema>
            if let Some(s) = schema {
                if let SqlIr::Schema { value, quoting, .. } = s.as_ref() {
                    let sn = element(xot, "schema")?;
                    xot.append(node, sn)?;
                    if let Some(marker) = quoting.marker_name() {
                        let m = element(xot, marker)?;
                        xot.append(sn, m)?;
                    }
                    let nn = element(xot, "name")?;
                    xot.append(sn, nn)?;
                    let t = xot.new_text(value);
                    xot.append(nn, t)?;
                }
            }
            // Name (always): <part><bracketed?/><name>Users</name></part>
            // — wrapping in <part> so quoting markers can attach
            // without violating name-is-text-leaf.
            if let SqlIr::Identifier { value, quoting, .. } = name.as_ref() {
                let pn = element(xot, "part")?;
                xot.append(node, pn)?;
                if let Some(marker) = quoting.marker_name() {
                    let m = element(xot, marker)?;
                    xot.append(pn, m)?;
                }
                let nn = element(xot, "name")?;
                xot.append(pn, nn)?;
                let t = xot.new_text(value);
                xot.append(nn, t)?;
            } else {
                render_sql_to_xot(xot, node, name, source)?;
            }
            // Alias: <alias><bracketed?/><name>u</name></alias>
            if let Some(a) = alias {
                if let SqlIr::Alias { value, quoting, .. } = a.as_ref() {
                    let an = element(xot, "alias")?;
                    xot.append(node, an)?;
                    if let Some(marker) = quoting.marker_name() {
                        let m = element(xot, marker)?;
                        xot.append(an, m)?;
                    }
                    let nn = element(xot, "name")?;
                    xot.append(an, nn)?;
                    let t = xot.new_text(value);
                    xot.append(nn, t)?;
                }
            }
            Ok(node)
        }
        SqlIr::Column { expression, alias, .. } => {
            let node = element(xot, "column")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, expression, source)?;
            if let Some(a) = alias {
                if let SqlIr::Alias { value, quoting, .. } = a.as_ref() {
                    let an = element(xot, "alias")?;
                    xot.append(node, an)?;
                    if let Some(marker) = quoting.marker_name() {
                        let m = element(xot, marker)?;
                        xot.append(an, m)?;
                    }
                    let nn = element(xot, "name")?;
                    xot.append(an, nn)?;
                    let t = xot.new_text(value);
                    xot.append(nn, t)?;
                }
            }
            Ok(node)
        }
        SqlIr::Star { qualifier, .. } => {
            let node = element(xot, "star")?;
            xot.append(parent, node)?;
            if let Some(q) = qualifier {
                render_sql_to_xot(xot, node, q, source)?;
            }
            Ok(node)
        }
        SqlIr::Reference { parts, .. } => {
            // Render each part as <part><bracketed?/><name>text</name></part>
            // — uniform wrapper so quoting markers attach without
            // violating name-is-text-leaf. Roles aren't determinable
            // from syntax for qualified column refs (e.g.
            // dbo.SomeTable.SomeColumn could be schema.table.column
            // OR alias.table.column), so parts stay anonymous.
            for p in parts {
                if let SqlIr::Identifier { value, quoting, .. } = p {
                    let pn = element(xot, "part")?;
                    xot.append(parent, pn)?;
                    if let Some(marker) = quoting.marker_name() {
                        let m = element(xot, marker)?;
                        xot.append(pn, m)?;
                    }
                    let nn = element(xot, "name")?;
                    xot.append(pn, nn)?;
                    let t = xot.new_text(value);
                    xot.append(nn, t)?;
                } else {
                    render_sql_to_xot(xot, parent, p, source)?;
                }
            }
            Ok(parent)
        }

        // ----- Expressions ----------------------------------------------
        SqlIr::Compare { left, op, right, .. } => {
            let node = element(xot, "compare")?;
            xot.append(parent, node)?;
            let l = element(xot, "left")?;
            xot.append(node, l)?;
            render_sql_to_xot(xot, l, left, source)?;
            let op_n = element(xot, "op")?;
            xot.append(node, op_n)?;
            let op_text = comparison_op_text(*op);
            let op_text_n = xot.new_text(op_text);
            xot.append(op_n, op_text_n)?;
            for marker in comparison_op_markers(*op) {
                let m = element(xot, marker)?;
                xot.append(op_n, m)?;
            }
            let r = element(xot, "right")?;
            xot.append(node, r)?;
            render_sql_to_xot(xot, r, right, source)?;
            Ok(node)
        }
        SqlIr::Binary { left, op, right, .. } => {
            let node = element(xot, "binary")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, left, source)?;
            let op_n = element(xot, "op")?;
            xot.append(node, op_n)?;
            let op_text_n = xot.new_text(binary_op_text(*op));
            xot.append(op_n, op_text_n)?;
            render_sql_to_xot(xot, node, right, source)?;
            Ok(node)
        }
        SqlIr::Unary { op, operand, .. } => {
            let node = element(xot, "unary")?;
            xot.append(parent, node)?;
            let op_n = element(xot, "op")?;
            xot.append(node, op_n)?;
            let op_text_n = xot.new_text(unary_op_text(*op));
            xot.append(op_n, op_text_n)?;
            render_sql_to_xot(xot, node, operand, source)?;
            Ok(node)
        }
        SqlIr::Assign { target, value, .. } => {
            let node = element(xot, "assign")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, target, source)?;
            value_render(xot, node, value, source)?;
            Ok(node)
        }
        SqlIr::Between { value, low, high, .. } => {
            let node = element(xot, "between")?;
            xot.append(parent, node)?;
            wrap_render(xot, node, "value", value, source)?;
            wrap_render(xot, node, "low", low, source)?;
            wrap_render(xot, node, "high", high, source)?;
            Ok(node)
        }
        SqlIr::Exists { subquery, .. } => {
            let node = element(xot, "exists")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, subquery, source)?;
            Ok(node)
        }
        SqlIr::Case { whens, else_, .. } => {
            let node = element(xot, "case")?;
            xot.append(parent, node)?;
            for w in whens {
                render_sql_to_xot(xot, node, w, source)?;
            }
            if let Some(e) = else_ {
                let en = element(xot, "else")?;
                xot.append(node, en)?;
                render_sql_to_xot(xot, en, e, source)?;
            }
            Ok(node)
        }
        SqlIr::When { condition, value, .. } => {
            let node = element(xot, "when")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, condition, source)?;
            render_sql_to_xot(xot, node, value, source)?;
            Ok(node)
        }
        SqlIr::Cast { value, type_, .. } => {
            let node = element(xot, "cast")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, value, source)?;
            render_sql_to_xot(xot, node, type_, source)?;
            Ok(node)
        }
        SqlIr::Call { callee, arguments, .. } => {
            let node = element(xot, "call")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, callee, source)?;
            for a in arguments {
                render_sql_to_xot(xot, node, a, source)?;
            }
            Ok(node)
        }
        SqlIr::Window { call, over, .. } => {
            let node = element(xot, "window")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, call, source)?;
            render_sql_to_xot(xot, node, over, source)?;
            Ok(node)
        }
        SqlIr::Over {
            partition_by,
            order_by,
            ..
        } => {
            let node = element(xot, "over")?;
            xot.append(parent, node)?;
            if let Some(p) = partition_by {
                render_sql_to_xot(xot, node, p, source)?;
            }
            if let Some(o) = order_by {
                render_sql_to_xot(xot, node, o, source)?;
            }
            Ok(node)
        }
        SqlIr::Subquery { select, .. } => {
            let node = element(xot, "subquery")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, select, source)?;
            Ok(node)
        }
        SqlIr::Union { selects, .. } => {
            let node = element(xot, "union")?;
            xot.append(parent, node)?;
            for s in selects {
                render_sql_to_xot(xot, node, s, source)?;
            }
            Ok(node)
        }
        SqlIr::Cte { name, query, .. } => {
            let node = element(xot, "cte")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, name, source)?;
            render_sql_to_xot(xot, node, query, source)?;
            Ok(node)
        }
        SqlIr::Tuple { items, .. } => {
            let node = element(xot, "tuple")?;
            xot.append(parent, node)?;
            for i in items {
                render_sql_to_xot(xot, node, i, source)?;
            }
            Ok(node)
        }

        // ----- DDL ------------------------------------------------------
        SqlIr::Create { kind, name, body, .. } => {
            let node = element(xot, "create")?;
            xot.append(parent, node)?;
            let m = element(xot, create_kind_marker(*kind))?;
            xot.append(node, m)?;
            render_sql_to_xot(xot, node, name, source)?;
            for b in body {
                render_sql_to_xot(xot, node, b, source)?;
            }
            Ok(node)
        }
        SqlIr::Drop { kind, name, .. } => {
            let node = element(xot, "drop")?;
            xot.append(parent, node)?;
            let m = element(xot, drop_kind_marker(*kind))?;
            xot.append(node, m)?;
            render_sql_to_xot(xot, node, name, source)?;
            Ok(node)
        }
        SqlIr::Alter { name, operation, .. } => {
            let node = element(xot, "alter")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, name, source)?;
            render_sql_to_xot(xot, node, operation, source)?;
            Ok(node)
        }
        SqlIr::ColumnDef {
            name,
            type_,
            constraints,
            ..
        } => {
            let node = element(xot, "column")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, name, source)?;
            render_sql_to_xot(xot, node, type_, source)?;
            for c in constraints {
                render_sql_to_xot(xot, node, c, source)?;
            }
            Ok(node)
        }
        SqlIr::Constraint { name, body, .. } => {
            let node = element(xot, "constraint")?;
            xot.append(parent, node)?;
            if let Some(n) = name {
                render_sql_to_xot(xot, node, n, source)?;
            }
            for b in body {
                render_sql_to_xot(xot, node, b, source)?;
            }
            Ok(node)
        }
        SqlIr::AddColumn { column, .. } => {
            let node = element(xot, "add")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, column, source)?;
            Ok(node)
        }
        SqlIr::AddConstraint { constraint, .. } => {
            let node = element(xot, "add")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, constraint, source)?;
            Ok(node)
        }
        SqlIr::Function {
            schema,
            name,
            parameters,
            return_type,
            body,
            ..
        } => {
            let node = element(xot, "function")?;
            xot.append(parent, node)?;
            if let Some(s) = schema {
                render_sql_to_xot(xot, node, s, source)?;
            }
            render_sql_to_xot(xot, node, name, source)?;
            for p in parameters {
                render_sql_to_xot(xot, node, p, source)?;
            }
            if let Some(rt) = return_type {
                render_sql_to_xot(xot, node, rt, source)?;
            }
            render_sql_to_xot(xot, node, body, source)?;
            Ok(node)
        }

        // ----- Types ----------------------------------------------------
        SqlIr::DataType { name, length, range, .. } => {
            let node = element(xot, name)?;
            xot.append(parent, node)?;
            if let Some(len) = length {
                render_sql_to_xot(xot, node, len, source)?;
            } else {
                // Type with no length — emit the source text (e.g.
                // "INT"). Lets queries match on the keyword text.
                let text = xot.new_text(range.slice(source));
                xot.append(node, text)?;
            }
            Ok(node)
        }

        // ----- Atoms ----------------------------------------------------
        // Standalone atom rendering — when an Identifier/Schema/Alias
        // is rendered without a wrapping context (Relation / Reference /
        // Column.alias). Wrap with the role-named element to hold any
        // quoting markers; <name> always stays a text-only leaf.
        SqlIr::Identifier { value, quoting, .. } => {
            // Bare identifier — emit <name>value</name>; if quoted,
            // wrap in <part><bracketed/>...</part> to give markers a
            // home. Standalone bare identifier with no quoting is a
            // pure leaf (no part wrapper needed).
            if let Some(marker) = quoting.marker_name() {
                let pn = element(xot, "part")?;
                xot.append(parent, pn)?;
                let m = element(xot, marker)?;
                xot.append(pn, m)?;
                let nn = element(xot, "name")?;
                xot.append(pn, nn)?;
                let t = xot.new_text(value);
                xot.append(nn, t)?;
                Ok(pn)
            } else {
                leaf(xot, parent, "name", value)
            }
        }
        SqlIr::Schema { value, quoting, .. } => {
            let sn = element(xot, "schema")?;
            xot.append(parent, sn)?;
            if let Some(marker) = quoting.marker_name() {
                let m = element(xot, marker)?;
                xot.append(sn, m)?;
            }
            let nn = element(xot, "name")?;
            xot.append(sn, nn)?;
            let t = xot.new_text(value);
            xot.append(nn, t)?;
            Ok(sn)
        }
        SqlIr::Alias { value, quoting, .. } => {
            let an = element(xot, "alias")?;
            xot.append(parent, an)?;
            if let Some(marker) = quoting.marker_name() {
                let m = element(xot, marker)?;
                xot.append(an, m)?;
            }
            let nn = element(xot, "name")?;
            xot.append(an, nn)?;
            let t = xot.new_text(value);
            xot.append(nn, t)?;
            Ok(an)
        }
        SqlIr::Temp { name, .. } => {
            let node = element(xot, "temp")?;
            xot.append(parent, node)?;
            render_sql_to_xot(xot, node, name, source)?;
            Ok(node)
        }
        SqlIr::Variable { range, .. } => leaf(xot, parent, "var", range.slice(source)),
        SqlIr::Literal { range, .. } => leaf(xot, parent, "literal", range.slice(source)),
        SqlIr::Comment { range, .. } => leaf(xot, parent, "comment", range.slice(source)),

        // ----- Escape hatch --------------------------------------------
        SqlIr::Unknown { kind, .. } => unknown(xot, parent, kind, source),
    }
}

// ----- helpers ----------------------------------------------------------

fn element(xot: &mut Xot, name: &str) -> Result<XotNode, xot::Error> {
    let n = xot.add_name(name);
    Ok(xot.new_element(n))
}

fn leaf(xot: &mut Xot, parent: XotNode, name: &str, text: &str) -> Result<XotNode, xot::Error> {
    let n = element(xot, name)?;
    xot.append(parent, n)?;
    if !text.is_empty() {
        let t = xot.new_text(text);
        xot.append(n, t)?;
    }
    Ok(n)
}

fn unknown(
    xot: &mut Xot,
    parent: XotNode,
    kind: &str,
    _source: &str,
) -> Result<XotNode, xot::Error> {
    let n = element(xot, "unknown")?;
    xot.append(parent, n)?;
    let t = xot.new_text(kind);
    xot.append(n, t)?;
    Ok(n)
}

fn wrap_render(
    xot: &mut Xot,
    parent: XotNode,
    wrap_name: &str,
    inner: &SqlIr,
    source: &str,
) -> Result<XotNode, xot::Error> {
    let n = element(xot, wrap_name)?;
    xot.append(parent, n)?;
    render_sql_to_xot(xot, n, inner, source)?;
    Ok(n)
}

fn value_render(
    xot: &mut Xot,
    parent: XotNode,
    inner: &SqlIr,
    source: &str,
) -> Result<XotNode, xot::Error> {
    wrap_render(xot, parent, "value", inner, source)
}

fn comparison_op_text(op: ComparisonOp) -> &'static str {
    match op {
        ComparisonOp::Equal => "=",
        ComparisonOp::NotEqual => "<>",
        ComparisonOp::Less => "<",
        ComparisonOp::LessEqual => "<=",
        ComparisonOp::Greater => ">",
        ComparisonOp::GreaterEqual => ">=",
        ComparisonOp::Like => "LIKE",
        ComparisonOp::In => "IN",
        ComparisonOp::Is => "IS",
        ComparisonOp::IsNot => "IS NOT",
        ComparisonOp::And => "AND",
        ComparisonOp::Or => "OR",
    }
}

fn comparison_op_markers(op: ComparisonOp) -> &'static [&'static str] {
    match op {
        ComparisonOp::Less => &["compare", "less"],
        ComparisonOp::LessEqual => &["compare", "less", "equal"],
        ComparisonOp::Greater => &["compare", "greater"],
        ComparisonOp::GreaterEqual => &["compare", "greater", "equal"],
        ComparisonOp::NotEqual => &["compare", "not", "equal"],
        // `=`, IS, IN, LIKE, AND, OR have no marker chips.
        _ => &[],
    }
}

fn binary_op_text(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Plus => "+",
        BinaryOp::Minus => "-",
        BinaryOp::Multiply => "*",
        BinaryOp::Divide => "/",
        BinaryOp::Modulo => "%",
        BinaryOp::Concat => "||",
        BinaryOp::BitwiseAnd => "&",
        BinaryOp::BitwiseOr => "|",
        BinaryOp::BitwiseXor => "^",
    }
}

fn unary_op_text(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Not => "NOT",
        UnaryOp::Negate => "-",
        UnaryOp::Positive => "+",
    }
}

fn join_markers(kind: JoinKind) -> &'static [&'static str] {
    match kind {
        JoinKind::Inner => &[],
        JoinKind::Left => &["left"],
        JoinKind::Right => &["right"],
        JoinKind::Full => &["full"],
        JoinKind::LeftOuter => &["left", "outer"],
        JoinKind::RightOuter => &["right", "outer"],
        JoinKind::FullOuter => &["full", "outer"],
        JoinKind::Cross => &["cross"],
    }
}

fn create_kind_marker(kind: CreateKind) -> &'static str {
    match kind {
        CreateKind::Table => "table",
        CreateKind::View => "view",
        CreateKind::Index => "index",
    }
}

fn drop_kind_marker(kind: DropKind) -> &'static str {
    match kind {
        DropKind::Table => "table",
        DropKind::Index => "index",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::sql_lower::lower_sql_root;
    use tree_sitter::Parser;

    fn parse_tsql(source: &str) -> tree_sitter::Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_sequel_tsql::LANGUAGE.into())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    fn render(source: &str) -> String {
        let tree = parse_tsql(source);
        let ir = lower_sql_root(tree.root_node(), source);
        let mut xot = Xot::new();
        let root_name = xot.add_name("root");
        let root_el = xot.new_element(root_name);
        let doc = xot.new_document_with_element(root_el).unwrap();
        render_sql_to_xot(&mut xot, root_el, &ir, source).unwrap();
        xot.to_string(doc).unwrap()
    }

    #[test]
    fn select_star_renders_typed_xml() {
        let xml = render("SELECT * FROM Users");
        assert!(xml.contains("<file>"));
        assert!(xml.contains("<select>"));
        assert!(xml.contains("<star/>"));
        assert!(xml.contains("<from>"));
        assert!(xml.contains("<relation>"));
        assert!(xml.contains("<name>Users</name>"));
    }

    #[test]
    fn where_compare_renders_typed_xml() {
        let xml = render("SELECT * FROM U WHERE Active = 1");
        assert!(xml.contains("<where>"));
        assert!(xml.contains("<compare>"));
        assert!(xml.contains("<left>"));
        assert!(xml.contains("<op>="));
        assert!(xml.contains("<right>"));
        assert!(xml.contains("<literal>1</literal>"));
    }

    #[test]
    fn insert_renders_typed_columns_and_values() {
        let xml = render("INSERT INTO L (a, b) VALUES (1, 'x')");
        assert!(xml.contains("<insert>"));
        assert!(xml.contains("<columns>"));
        assert!(xml.contains("<values>"));
        assert!(xml.contains("<literal>1</literal>"));
    }

    #[test]
    fn create_table_renders_typed_xml() {
        let xml = render("CREATE TABLE T (id INT, name VARCHAR(100))");
        assert!(xml.contains("<create>"), "xml: {xml}");
        assert!(xml.contains("<table/>"), "xml: {xml}");
        assert!(xml.contains("<column>"), "xml: {xml}");
        assert!(xml.contains("<int>"), "xml: {xml}");
        assert!(xml.contains("<varchar>"), "xml: {xml}");
    }
}
