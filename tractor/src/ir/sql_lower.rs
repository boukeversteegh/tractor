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
use super::sql::{ComparisonOp, SqlIr};

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
