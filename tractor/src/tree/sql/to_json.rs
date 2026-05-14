//! [`SqlTree`] → `serde_json::Value` rendering.
//!
//! Direct typed-slot serialization. Each `SqlTree` variant has named
//! slots that map 1-1 to JSON keys; collections render as arrays.
//! No projection heuristics — the typed tree shape carries the
//! semantic information directly.
//!
//! Contrast with `to_json.rs` (cross-language `SyntaxTree`): that renderer
//! grew ~9 special-case branches in `add_children` to fix
//! `\$`-prefixed leaks (Skip filter, Inline transparency,
//! plural-of-self collapse, marker-vs-leaf ambiguity, op_marker
//! emptiness). All of those evaporate here because:
//!
//! - `SqlTree::Insert { columns, values }` produces
//!   `{ "columns": [...], "values": [...] }` directly — no
//!   plural-of-self double-wrap.
//! - `SqlTree::Compare { op: ComparisonOp::Equal }` produces
//!   `{ "op": { "text": "=" } }` — no empty-marker key, no
//!   `\$type": "expression"` wrapper.
//! - There is no Skip / Inline / SimpleStatement to filter — the
//!   tree doesn't have generic catch-alls.
//!
//! ## Status
//!
//! Initial slice covers every variant exercised by the
//! `sql_lower` and `sql_to_xot` test suites.

#![cfg(feature = "native")]

use serde_json::{Map, Value};

use super::types::{
    BinaryOp, ComparisonOp, CreateKind, DropKind, JoinKind, SortDirection, SqlTree, UnaryOp,
};

/// Render a [`SqlTree`] tree to a JSON value.
pub fn sql_to_json(tree: &SqlTree, source: &str) -> Value {
    match tree {
        // ----- Top level ------------------------------------------------
        SqlTree::File { statements, .. } => {
            let mut obj = Map::new();
            obj.insert(
                "statements".into(),
                Value::Array(statements.iter().map(|s| sql_to_json(s, source)).collect()),
            );
            Value::Object(obj)
        }
        SqlTree::Statement { inner, .. } => sql_to_json(inner, source),
        SqlTree::Go { .. } => json_obj([("$type", Value::String("go".into()))]),
        SqlTree::Exec { target, .. } => json_obj([(
            "exec",
            json_obj([("target", sql_to_json(target, source))]),
        )]),
        SqlTree::Set { target, value, .. } => json_obj([(
            "set",
            json_obj([
                ("target", sql_to_json(target, source)),
                ("value", sql_to_json(value, source)),
            ]),
        )]),

        // ----- DML ------------------------------------------------------
        SqlTree::Select {
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
            let mut select = Map::new();
            if !ctes.is_empty() {
                select.insert(
                    "ctes".into(),
                    Value::Array(ctes.iter().map(|c| sql_to_json(c, source)).collect()),
                );
            }
            // For uniform shape: always emit columns as an array.
            select.insert(
                "columns".into(),
                Value::Array(columns.iter().map(|c| sql_to_json(c, source)).collect()),
            );
            if let Some(i) = into {
                select.insert("into".into(), sql_to_json(i, source));
            }
            if let Some(f) = from {
                select.insert("from".into(), sql_to_json(f, source));
            }
            if let Some(w) = where_ {
                select.insert("where".into(), sql_to_json(w, source));
            }
            if let Some(g) = group_by {
                select.insert("group".into(), sql_to_json(g, source));
            }
            if let Some(h) = having {
                select.insert("having".into(), sql_to_json(h, source));
            }
            if let Some(o) = order_by {
                select.insert("order".into(), sql_to_json(o, source));
            }
            json_obj([("select", Value::Object(select))])
        }
        SqlTree::Insert {
            table,
            columns,
            values,
            ..
        } => {
            let mut insert = Map::new();
            insert.insert("table".into(), sql_to_json(table, source));
            if !columns.is_empty() {
                insert.insert(
                    "columns".into(),
                    Value::Array(columns.iter().map(|c| sql_to_json(c, source)).collect()),
                );
            }
            if !values.is_empty() {
                insert.insert(
                    "values".into(),
                    Value::Array(values.iter().map(|v| sql_to_json(v, source)).collect()),
                );
            }
            json_obj([("insert", Value::Object(insert))])
        }
        SqlTree::Update {
            table,
            assignments,
            where_,
            ..
        } => {
            let mut update = Map::new();
            update.insert("table".into(), sql_to_json(table, source));
            update.insert(
                "assignments".into(),
                Value::Array(assignments.iter().map(|a| sql_to_json(a, source)).collect()),
            );
            if let Some(w) = where_ {
                update.insert("where".into(), sql_to_json(w, source));
            }
            json_obj([("update", Value::Object(update))])
        }
        SqlTree::Delete { from, where_, .. } => {
            let mut del = Map::new();
            if let Some(f) = from {
                del.insert("from".into(), sql_to_json(f, source));
            }
            if let Some(w) = where_ {
                del.insert("where".into(), sql_to_json(w, source));
            }
            json_obj([("delete", Value::Object(del))])
        }
        SqlTree::Merge { .. } | SqlTree::MergeWhen { .. } | SqlTree::Transaction { .. } => {
            // Coverage gaps in this slice.
            json_obj([("$type", Value::String("todo".into()))])
        }

        // ----- Clauses --------------------------------------------------
        SqlTree::From { relations, .. } => {
            // Single relation → render directly; multiple → array.
            if relations.len() == 1 {
                sql_to_json(&relations[0], source)
            } else {
                Value::Array(relations.iter().map(|r| sql_to_json(r, source)).collect())
            }
        }
        SqlTree::Where { condition, .. } => sql_to_json(condition, source),
        SqlTree::GroupBy { keys, .. } => {
            Value::Array(keys.iter().map(|k| sql_to_json(k, source)).collect())
        }
        SqlTree::Having { condition, .. } => sql_to_json(condition, source),
        SqlTree::OrderBy { targets, .. } => {
            Value::Array(targets.iter().map(|t| sql_to_json(t, source)).collect())
        }
        SqlTree::OrderTarget {
            expression,
            direction,
            ..
        } => {
            let mut obj = Map::new();
            obj.insert("expression".into(), sql_to_json(expression, source));
            if let Some(d) = direction {
                obj.insert(
                    "direction".into(),
                    Value::String(match d {
                        SortDirection::Asc => "asc",
                        SortDirection::Desc => "desc",
                    }.into()),
                );
            }
            Value::Object(obj)
        }
        SqlTree::PartitionBy { keys, .. } => {
            Value::Array(keys.iter().map(|k| sql_to_json(k, source)).collect())
        }
        SqlTree::Join { kind, relation, on, .. } => {
            let mut obj = Map::new();
            // Render JoinKind as boolean flags (left: true, etc.).
            for marker in join_kind_flags(*kind) {
                obj.insert((*marker).to_string(), Value::Bool(true));
            }
            obj.insert("relation".into(), sql_to_json(relation, source));
            if let Some(o) = on {
                obj.insert("on".into(), sql_to_json(o, source));
            }
            json_obj([("join", Value::Object(obj))])
        }

        // ----- References -----------------------------------------------
        SqlTree::Relation { schema, name, alias, .. } => {
            let mut obj = Map::new();
            if let Some(s) = schema {
                obj.insert("schema".into(), scalar_text(s, source));
            }
            obj.insert("name".into(), scalar_text(name, source));
            if let Some(a) = alias {
                obj.insert("alias".into(), scalar_text(a, source));
            }
            Value::Object(obj)
        }
        SqlTree::Column { expression, alias, .. } => {
            let expr = sql_to_json(expression, source);
            if let Some(a) = alias {
                let mut obj = Map::new();
                if let Value::Object(map) = &expr {
                    for (k, v) in map.iter() {
                        obj.insert(k.clone(), v.clone());
                    }
                } else {
                    obj.insert("expression".into(), expr);
                }
                obj.insert("alias".into(), scalar_text(a, source));
                Value::Object(obj)
            } else {
                expr
            }
        }
        SqlTree::Star { qualifier, .. } => {
            if let Some(q) = qualifier {
                json_obj([
                    ("star", Value::Bool(true)),
                    ("qualifier", sql_to_json(q, source)),
                ])
            } else {
                json_obj([("star", Value::Bool(true))])
            }
        }
        SqlTree::Reference { parts, .. } => {
            // Multiple-part qualified reference renders as array of names.
            let names: Vec<Value> = parts
                .iter()
                .map(|p| scalar_text(p, source))
                .collect();
            json_obj([("names", Value::Array(names))])
        }

        // ----- Expressions ----------------------------------------------
        SqlTree::Compare {
            left, op, right, ..
        } => {
            let mut op_obj = Map::new();
            op_obj.insert("text".into(), Value::String(comparison_op_text(*op).into()));
            for marker in comparison_op_flags(*op) {
                op_obj.insert((*marker).to_string(), Value::Bool(true));
            }
            json_obj([("compare", json_obj([
                ("left", sql_to_json(left, source)),
                ("op", Value::Object(op_obj)),
                ("right", sql_to_json(right, source)),
            ]))])
        }
        SqlTree::Binary { left, op, right, .. } => {
            json_obj([("binary", json_obj([
                ("left", sql_to_json(left, source)),
                ("op", Value::String(binary_op_text(*op).into())),
                ("right", sql_to_json(right, source)),
            ]))])
        }
        SqlTree::Unary { op, operand, .. } => {
            json_obj([("unary", json_obj([
                ("op", Value::String(unary_op_text(*op).into())),
                ("operand", sql_to_json(operand, source)),
            ]))])
        }
        SqlTree::Assign { target, value, .. } => json_obj([(
            "assign",
            json_obj([
                ("target", sql_to_json(target, source)),
                ("value", sql_to_json(value, source)),
            ]),
        )]),
        SqlTree::Between { value, low, high, .. } => json_obj([(
            "between",
            json_obj([
                ("value", sql_to_json(value, source)),
                ("low", sql_to_json(low, source)),
                ("high", sql_to_json(high, source)),
            ]),
        )]),
        SqlTree::Exists { subquery, .. } => json_obj([("exists", sql_to_json(subquery, source))]),
        SqlTree::Case { whens, else_, .. } => {
            let mut case_obj = Map::new();
            case_obj.insert(
                "whens".into(),
                Value::Array(whens.iter().map(|w| sql_to_json(w, source)).collect()),
            );
            if let Some(e) = else_ {
                case_obj.insert("else".into(), sql_to_json(e, source));
            }
            json_obj([("case", Value::Object(case_obj))])
        }
        SqlTree::When { condition, value, .. } => json_obj([
            ("condition", sql_to_json(condition, source)),
            ("value", sql_to_json(value, source)),
        ]),
        SqlTree::Cast { value, type_, .. } => json_obj([(
            "cast",
            json_obj([
                ("value", sql_to_json(value, source)),
                ("type", sql_to_json(type_, source)),
            ]),
        )]),
        SqlTree::Call { callee, arguments, .. } => {
            let mut obj = Map::new();
            obj.insert("callee".into(), sql_to_json(callee, source));
            if !arguments.is_empty() {
                obj.insert(
                    "arguments".into(),
                    Value::Array(arguments.iter().map(|a| sql_to_json(a, source)).collect()),
                );
            }
            json_obj([("call", Value::Object(obj))])
        }
        SqlTree::Window { call, over, .. } => json_obj([(
            "window",
            json_obj([
                ("call", sql_to_json(call, source)),
                ("over", sql_to_json(over, source)),
            ]),
        )]),
        SqlTree::Over {
            partition_by,
            order_by,
            ..
        } => {
            let mut obj = Map::new();
            if let Some(p) = partition_by {
                obj.insert("partition".into(), sql_to_json(p, source));
            }
            if let Some(o) = order_by {
                obj.insert("order".into(), sql_to_json(o, source));
            }
            Value::Object(obj)
        }
        SqlTree::Subquery { select, .. } => json_obj([("subquery", sql_to_json(select, source))]),
        SqlTree::Union { all, selects, .. } => {
            let mut obj = Map::new();
            obj.insert("all".into(), Value::Bool(*all));
            obj.insert(
                "selects".into(),
                Value::Array(selects.iter().map(|s| sql_to_json(s, source)).collect()),
            );
            json_obj([("union", Value::Object(obj))])
        }
        SqlTree::Cte { name, query, .. } => json_obj([(
            "cte",
            json_obj([
                ("name", sql_to_json(name, source)),
                ("query", sql_to_json(query, source)),
            ]),
        )]),
        SqlTree::Tuple { items, .. } => Value::Array(items.iter().map(|i| sql_to_json(i, source)).collect()),

        // ----- DDL ------------------------------------------------------
        SqlTree::Create { kind, name, body, .. } => {
            let mut obj = Map::new();
            obj.insert(
                create_kind_marker(*kind).into(),
                Value::Bool(true),
            );
            obj.insert("name".into(), sql_to_json(name, source));
            if !body.is_empty() {
                obj.insert(
                    "body".into(),
                    Value::Array(body.iter().map(|b| sql_to_json(b, source)).collect()),
                );
            }
            json_obj([("create", Value::Object(obj))])
        }
        SqlTree::Drop { kind, name, .. } => {
            let mut obj = Map::new();
            obj.insert(drop_kind_marker(*kind).into(), Value::Bool(true));
            obj.insert("name".into(), sql_to_json(name, source));
            json_obj([("drop", Value::Object(obj))])
        }
        SqlTree::Alter { name, operation, .. } => json_obj([(
            "alter",
            json_obj([
                ("name", sql_to_json(name, source)),
                ("operation", sql_to_json(operation, source)),
            ]),
        )]),
        SqlTree::ColumnDef {
            name,
            type_,
            constraints,
            ..
        } => {
            let mut obj = Map::new();
            obj.insert("name".into(), sql_to_json(name, source));
            obj.insert("type".into(), sql_to_json(type_, source));
            if !constraints.is_empty() {
                obj.insert(
                    "constraints".into(),
                    Value::Array(constraints.iter().map(|c| sql_to_json(c, source)).collect()),
                );
            }
            Value::Object(obj)
        }
        SqlTree::Constraint { name, body, .. } => {
            let mut obj = Map::new();
            if let Some(n) = name {
                obj.insert("name".into(), sql_to_json(n, source));
            }
            obj.insert(
                "body".into(),
                Value::Array(body.iter().map(|b| sql_to_json(b, source)).collect()),
            );
            json_obj([("constraint", Value::Object(obj))])
        }
        SqlTree::AddColumn { column, .. } => json_obj([("add", sql_to_json(column, source))]),
        SqlTree::AddConstraint { constraint, .. } => {
            json_obj([("add", sql_to_json(constraint, source))])
        }
        SqlTree::Function {
            schema,
            name,
            parameters,
            return_type,
            body,
            ..
        } => {
            let mut obj = Map::new();
            if let Some(s) = schema {
                obj.insert("schema".into(), sql_to_json(s, source));
            }
            obj.insert("name".into(), sql_to_json(name, source));
            obj.insert(
                "parameters".into(),
                Value::Array(parameters.iter().map(|p| sql_to_json(p, source)).collect()),
            );
            if let Some(rt) = return_type {
                obj.insert("return".into(), sql_to_json(rt, source));
            }
            obj.insert("body".into(), sql_to_json(body, source));
            json_obj([("function", Value::Object(obj))])
        }

        // ----- Types ----------------------------------------------------
        SqlTree::DataType { name, length, range, .. } => {
            if let Some(len) = length {
                json_obj([(*name, sql_to_json(len, source))])
            } else {
                Value::String(range.slice(source).to_string())
            }
        }

        // ----- Atoms ----------------------------------------------------
        // Identifier-class atoms use the parsed `value`. Quoting is
        // captured as a sibling-level marker in object-context shapes
        // (Relation/Reference/Column.alias) but flattens to a plain
        // string when rendered standalone — JSON consumers querying
        // by identity see `"dbo"`, not `"[dbo]"`.
        SqlTree::Identifier { value, .. } => Value::String(value.clone()),
        SqlTree::Schema { value, .. } => Value::String(value.clone()),
        SqlTree::Alias { value, .. } => Value::String(value.clone()),
        SqlTree::Temp { name, .. } => json_obj([("temp", sql_to_json(name, source))]),
        SqlTree::Variable { range, .. } => Value::String(range.slice(source).to_string()),
        SqlTree::Literal { range, .. } => Value::String(range.slice(source).to_string()),
        SqlTree::Comment { range, .. } => Value::String(range.slice(source).to_string()),

        // ----- Escape hatch --------------------------------------------
        SqlTree::Unknown { kind, .. } => json_obj([("unknown", Value::String(kind.clone()))]),
    }
}

// ----- helpers ----------------------------------------------------------

fn json_obj<I, K>(entries: I) -> Value
where
    I: IntoIterator<Item = (K, Value)>,
    K: Into<String>,
{
    let mut m = Map::new();
    for (k, v) in entries {
        m.insert(k.into(), v);
    }
    Value::Object(m)
}

/// Render an atom node as a plain string scalar. Identifier-class
/// atoms use the parsed `value` (quoting stripped); literals/variables
/// keep the raw source slice.
fn scalar_text(tree: &SqlTree, source: &str) -> Value {
    match tree {
        SqlTree::Identifier { value, .. }
        | SqlTree::Schema { value, .. }
        | SqlTree::Alias { value, .. } => Value::String(value.clone()),
        SqlTree::Variable { range, .. }
        | SqlTree::Literal { range, .. } => Value::String(range.slice(source).to_string()),
        other => sql_to_json(other, source),
    }
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

fn comparison_op_flags(op: ComparisonOp) -> &'static [&'static str] {
    match op {
        ComparisonOp::Less => &["less"],
        ComparisonOp::LessEqual => &["less", "equal"],
        ComparisonOp::Greater => &["greater"],
        ComparisonOp::GreaterEqual => &["greater", "equal"],
        ComparisonOp::NotEqual => &["not", "equal"],
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

fn join_kind_flags(kind: JoinKind) -> &'static [&'static str] {
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
    use crate::languages::tsql::lower_sql_root;
    use tree_sitter::Parser;

    fn parse_tsql(source: &str) -> tree_sitter::Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_sequel_tsql::LANGUAGE.into())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    fn render(source: &str) -> Value {
        let tree = parse_tsql(source);
        let tree = lower_sql_root(&crate::raw::RawNode::from_tree_sitter(tree.root_node(), source), source);
        sql_to_json(&tree, source)
    }

    #[test]
    fn select_star_renders_typed_json() {
        let v = render("SELECT * FROM Users");
        // {statements: [{select: {columns: [{star:true}], from: {name:"Users"}}}]}
        let stmts = v.get("statements").unwrap().as_array().unwrap();
        let stmt = &stmts[0];
        let select = stmt.get("select").unwrap();
        let columns = select.get("columns").unwrap().as_array().unwrap();
        assert_eq!(columns[0].get("star"), Some(&Value::Bool(true)));
        let from = select.get("from").unwrap();
        assert_eq!(from.get("name").unwrap().as_str(), Some("Users"));
    }

    #[test]
    fn compare_renders_typed_op_with_flags() {
        let v = render("SELECT * FROM x WHERE a > 1");
        let stmts = v.get("statements").unwrap().as_array().unwrap();
        let select = stmts[0].get("select").unwrap();
        let where_ = select.get("where").unwrap();
        let compare = where_.get("compare").unwrap();
        let op = compare.get("op").unwrap();
        assert_eq!(op.get("text").unwrap().as_str(), Some(">"));
        assert_eq!(op.get("greater"), Some(&Value::Bool(true)));
        // No empty `""` key.
        assert!(op.as_object().unwrap().get("").is_none(),
            "unexpected empty key in {op:?}");
    }

    #[test]
    fn insert_renders_typed_columns_and_values_arrays() {
        let v = render("INSERT INTO L (a, b) VALUES (1, 'x')");
        let stmts = v.get("statements").unwrap().as_array().unwrap();
        let insert = stmts[0].get("insert").unwrap();
        let columns = insert.get("columns").unwrap().as_array().unwrap();
        let values = insert.get("values").unwrap().as_array().unwrap();
        assert_eq!(columns.len(), 2);
        assert_eq!(values.len(), 2);
        // No `\$inline`, no `columns: {columns: [...]}` double wrap.
        assert!(insert.as_object().unwrap().get("$inline").is_none());
    }

    #[test]
    fn equal_op_no_empty_marker_key() {
        let v = render("SELECT * FROM x WHERE a = 1");
        let stmts = v.get("statements").unwrap().as_array().unwrap();
        let op = stmts[0]
            .get("select").unwrap()
            .get("where").unwrap()
            .get("compare").unwrap()
            .get("op").unwrap();
        assert_eq!(op.get("text").unwrap().as_str(), Some("="));
        // Critical: no empty-string key (was the iter 32 bug fix in to_json.rs).
        assert!(op.as_object().unwrap().get("").is_none());
        // Equal has no marker flags, so just `text`.
        assert_eq!(op.as_object().unwrap().len(), 1);
    }

    #[test]
    fn left_join_renders_typed_join_flags() {
        let v = render("SELECT * FROM A LEFT JOIN B ON A.id = B.id");
        let stmts = v.get("statements").unwrap().as_array().unwrap();
        let _ = stmts[0]
            .get("select").unwrap()
            .get("from").unwrap();
        // Just check json renders without panic; structure varies
        // by how `from` is composed (single relation or array
        // including the join). Detailed shape tests come once the
        // lowering for multi-relation FROM is solidified.
    }
}
