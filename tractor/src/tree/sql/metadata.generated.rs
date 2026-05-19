// DO NOT EDIT — emitted by `tractor/build.rs` on every build.
// Source: SqlTree enum in tractor/src/tree/sql/types.rs.
//
// Variant-blind reflection metadata for the T-SQL tree. Mirrors the
// `tree/syntax/metadata.generated.rs` shape — same codegen functions
// in `tractor/build_codegen.rs`, same accessor signatures (modulo
// `from_json` which is `SyntaxTree`-only).

#![cfg(feature = "native")]
#![allow(clippy::too_many_lines)]

#[allow(unused_imports)]
use super::types::SqlTree;
#[allow(unused_imports)]
use crate::tree::types::{ByteRange, Marker, Span};
#[allow(unused_imports)]
use crate::tree::walker::TreeField;

/// The XML element name for this tree node, or `None` if the
/// node renders no wrapper (`Inline`, `Skip`).
///
/// Borrowed lifetime: most arms return `&'static str` literals or
/// `&'static str` field values, but `Unknown` / `Raw` carry `String`
/// kinds so the return type ties to the tree.
pub fn element_name_of(tree: &SqlTree) -> Option<&str> {
    match tree {
        SqlTree::File { .. } => Some("file"),
        SqlTree::Statement { .. } => Some("statement"),
        SqlTree::Go { .. } => Some("go"),
        SqlTree::Exec { .. } => Some("exec"),
        SqlTree::Set { .. } => Some("set"),
        SqlTree::Select { .. } => Some("select"),
        SqlTree::Insert { .. } => Some("insert"),
        SqlTree::Update { .. } => Some("update"),
        SqlTree::Delete { .. } => Some("delete"),
        SqlTree::Merge { .. } => Some("merge"),
        SqlTree::MergeWhen { .. } => Some("merge_when"),
        SqlTree::Transaction { .. } => Some("transaction"),
        SqlTree::From { .. } => Some("from"),
        SqlTree::Where { .. } => Some("where"),
        SqlTree::GroupBy { .. } => Some("group_by"),
        SqlTree::Having { .. } => Some("having"),
        SqlTree::OrderBy { .. } => Some("order_by"),
        SqlTree::OrderTarget { .. } => Some("order_target"),
        SqlTree::PartitionBy { .. } => Some("partition_by"),
        SqlTree::Join { .. } => Some("join"),
        SqlTree::Relation { .. } => Some("relation"),
        SqlTree::Column { .. } => Some("column"),
        SqlTree::Star { .. } => Some("star"),
        SqlTree::Reference { .. } => Some("reference"),
        SqlTree::Compare { .. } => Some("compare"),
        SqlTree::Binary { .. } => Some("binary"),
        SqlTree::Unary { .. } => Some("unary"),
        SqlTree::Assign { .. } => Some("assign"),
        SqlTree::Between { .. } => Some("between"),
        SqlTree::Exists { .. } => Some("exists"),
        SqlTree::Case { .. } => Some("case"),
        SqlTree::When { .. } => Some("when"),
        SqlTree::Cast { .. } => Some("cast"),
        SqlTree::Call { .. } => Some("call"),
        SqlTree::Window { .. } => Some("window"),
        SqlTree::Over { .. } => Some("over"),
        SqlTree::Subquery { .. } => Some("subquery"),
        SqlTree::Union { .. } => Some("union"),
        SqlTree::Cte { .. } => Some("cte"),
        SqlTree::Tuple { .. } => Some("tuple"),
        SqlTree::Create { .. } => Some("create"),
        SqlTree::Drop { .. } => Some("drop"),
        SqlTree::Alter { .. } => Some("alter"),
        SqlTree::ColumnDef { .. } => Some("column_def"),
        SqlTree::Constraint { .. } => Some("constraint"),
        SqlTree::AddColumn { .. } => Some("add_column"),
        SqlTree::AddConstraint { .. } => Some("add_constraint"),
        SqlTree::Function { .. } => Some("function"),
        SqlTree::DataType { .. } => Some("data_type"),
        SqlTree::Identifier { .. } => Some("identifier"),
        SqlTree::Schema { .. } => Some("schema"),
        SqlTree::Alias { .. } => Some("alias"),
        SqlTree::Temp { .. } => Some("temp"),
        SqlTree::Variable { .. } => Some("variable"),
        SqlTree::Literal { .. } => Some("literal"),
        SqlTree::Comment { .. } => Some("comment"),
        SqlTree::Unknown { .. } => Some("unknown"),
    }
}

/// Empty-element marker children for this tree node. Drawn from
/// `Modifiers::markers_with_spans()`, any `Flag` field (named after
/// the field with trailing `_` stripped), `Vec<Marker>` and
/// `Vec<&'static str>` marker-name fields.
///
/// Each entry's `span` lets the XML renderer emit
/// `line` / `column` attributes at the keyword position when the flag
/// is anchored, falling back to a default for implicit flags.
pub fn flags_of(tree: &SqlTree) -> Vec<Marker> {
    let mut out: Vec<Marker> = Vec::new();
    match tree {
        SqlTree::File { .. } => {}
        SqlTree::Statement { .. } => {}
        SqlTree::Go { .. } => {}
        SqlTree::Exec { .. } => {}
        SqlTree::Set { .. } => {}
        SqlTree::Select { .. } => {}
        SqlTree::Insert { .. } => {}
        SqlTree::Update { .. } => {}
        SqlTree::Delete { .. } => {}
        SqlTree::Merge { .. } => {}
        SqlTree::MergeWhen { .. } => {}
        SqlTree::Transaction { .. } => {}
        SqlTree::From { .. } => {}
        SqlTree::Where { .. } => {}
        SqlTree::GroupBy { .. } => {}
        SqlTree::Having { .. } => {}
        SqlTree::OrderBy { .. } => {}
        SqlTree::OrderTarget { extra_markers, .. } => {
            for m in extra_markers { out.push(*m); }

        }
        SqlTree::PartitionBy { .. } => {}
        SqlTree::Join { extra_markers, .. } => {
            for m in extra_markers { out.push(*m); }

        }
        SqlTree::Relation { .. } => {}
        SqlTree::Column { .. } => {}
        SqlTree::Star { .. } => {}
        SqlTree::Reference { .. } => {}
        SqlTree::Compare { .. } => {}
        SqlTree::Binary { .. } => {}
        SqlTree::Unary { .. } => {}
        SqlTree::Assign { .. } => {}
        SqlTree::Between { .. } => {}
        SqlTree::Exists { .. } => {}
        SqlTree::Case { .. } => {}
        SqlTree::When { .. } => {}
        SqlTree::Cast { .. } => {}
        SqlTree::Call { .. } => {}
        SqlTree::Window { .. } => {}
        SqlTree::Over { .. } => {}
        SqlTree::Subquery { .. } => {}
        SqlTree::Union { .. } => {}
        SqlTree::Cte { .. } => {}
        SqlTree::Tuple { .. } => {}
        SqlTree::Create { .. } => {}
        SqlTree::Drop { .. } => {}
        SqlTree::Alter { .. } => {}
        SqlTree::ColumnDef { .. } => {}
        SqlTree::Constraint { .. } => {}
        SqlTree::AddColumn { .. } => {}
        SqlTree::AddConstraint { .. } => {}
        SqlTree::Function { .. } => {}
        SqlTree::DataType { .. } => {}
        SqlTree::Identifier { .. } => {}
        SqlTree::Schema { .. } => {}
        SqlTree::Alias { .. } => {}
        SqlTree::Temp { .. } => {}
        SqlTree::Variable { .. } => {}
        SqlTree::Literal { .. } => {}
        SqlTree::Comment { .. } => {}
        SqlTree::Unknown { .. } => {}
    }
    out
}

/// Source byte range of this node. Used for verbatim-source
/// recovery (`source[range]`) and for gap-text computation in the
/// renderer. Every variant carries a `range: ByteRange` field.
pub fn range_of(tree: &SqlTree) -> ByteRange {
    match tree {
        SqlTree::File { range, .. } => *range,
        SqlTree::Statement { range, .. } => *range,
        SqlTree::Go { range, .. } => *range,
        SqlTree::Exec { range, .. } => *range,
        SqlTree::Set { range, .. } => *range,
        SqlTree::Select { range, .. } => *range,
        SqlTree::Insert { range, .. } => *range,
        SqlTree::Update { range, .. } => *range,
        SqlTree::Delete { range, .. } => *range,
        SqlTree::Merge { range, .. } => *range,
        SqlTree::MergeWhen { range, .. } => *range,
        SqlTree::Transaction { range, .. } => *range,
        SqlTree::From { range, .. } => *range,
        SqlTree::Where { range, .. } => *range,
        SqlTree::GroupBy { range, .. } => *range,
        SqlTree::Having { range, .. } => *range,
        SqlTree::OrderBy { range, .. } => *range,
        SqlTree::OrderTarget { range, .. } => *range,
        SqlTree::PartitionBy { range, .. } => *range,
        SqlTree::Join { range, .. } => *range,
        SqlTree::Relation { range, .. } => *range,
        SqlTree::Column { range, .. } => *range,
        SqlTree::Star { range, .. } => *range,
        SqlTree::Reference { range, .. } => *range,
        SqlTree::Compare { range, .. } => *range,
        SqlTree::Binary { range, .. } => *range,
        SqlTree::Unary { range, .. } => *range,
        SqlTree::Assign { range, .. } => *range,
        SqlTree::Between { range, .. } => *range,
        SqlTree::Exists { range, .. } => *range,
        SqlTree::Case { range, .. } => *range,
        SqlTree::When { range, .. } => *range,
        SqlTree::Cast { range, .. } => *range,
        SqlTree::Call { range, .. } => *range,
        SqlTree::Window { range, .. } => *range,
        SqlTree::Over { range, .. } => *range,
        SqlTree::Subquery { range, .. } => *range,
        SqlTree::Union { range, .. } => *range,
        SqlTree::Cte { range, .. } => *range,
        SqlTree::Tuple { range, .. } => *range,
        SqlTree::Create { range, .. } => *range,
        SqlTree::Drop { range, .. } => *range,
        SqlTree::Alter { range, .. } => *range,
        SqlTree::ColumnDef { range, .. } => *range,
        SqlTree::Constraint { range, .. } => *range,
        SqlTree::AddColumn { range, .. } => *range,
        SqlTree::AddConstraint { range, .. } => *range,
        SqlTree::Function { range, .. } => *range,
        SqlTree::DataType { range, .. } => *range,
        SqlTree::Identifier { range, .. } => *range,
        SqlTree::Schema { range, .. } => *range,
        SqlTree::Alias { range, .. } => *range,
        SqlTree::Temp { range, .. } => *range,
        SqlTree::Variable { range, .. } => *range,
        SqlTree::Literal { range, .. } => *range,
        SqlTree::Comment { range, .. } => *range,
        SqlTree::Unknown { range, .. } => *range,
    }
}

/// Source-location span of this node. Used for XML attribute
/// emission (`line` / `column` / `end_line` / `end_column` / `id`).
/// Every variant carries a `span: Span` field.
pub fn span_of(tree: &SqlTree) -> Span {
    match tree {
        SqlTree::File { span, .. } => *span,
        SqlTree::Statement { span, .. } => *span,
        SqlTree::Go { span, .. } => *span,
        SqlTree::Exec { span, .. } => *span,
        SqlTree::Set { span, .. } => *span,
        SqlTree::Select { span, .. } => *span,
        SqlTree::Insert { span, .. } => *span,
        SqlTree::Update { span, .. } => *span,
        SqlTree::Delete { span, .. } => *span,
        SqlTree::Merge { span, .. } => *span,
        SqlTree::MergeWhen { span, .. } => *span,
        SqlTree::Transaction { span, .. } => *span,
        SqlTree::From { span, .. } => *span,
        SqlTree::Where { span, .. } => *span,
        SqlTree::GroupBy { span, .. } => *span,
        SqlTree::Having { span, .. } => *span,
        SqlTree::OrderBy { span, .. } => *span,
        SqlTree::OrderTarget { span, .. } => *span,
        SqlTree::PartitionBy { span, .. } => *span,
        SqlTree::Join { span, .. } => *span,
        SqlTree::Relation { span, .. } => *span,
        SqlTree::Column { span, .. } => *span,
        SqlTree::Star { span, .. } => *span,
        SqlTree::Reference { span, .. } => *span,
        SqlTree::Compare { span, .. } => *span,
        SqlTree::Binary { span, .. } => *span,
        SqlTree::Unary { span, .. } => *span,
        SqlTree::Assign { span, .. } => *span,
        SqlTree::Between { span, .. } => *span,
        SqlTree::Exists { span, .. } => *span,
        SqlTree::Case { span, .. } => *span,
        SqlTree::When { span, .. } => *span,
        SqlTree::Cast { span, .. } => *span,
        SqlTree::Call { span, .. } => *span,
        SqlTree::Window { span, .. } => *span,
        SqlTree::Over { span, .. } => *span,
        SqlTree::Subquery { span, .. } => *span,
        SqlTree::Union { span, .. } => *span,
        SqlTree::Cte { span, .. } => *span,
        SqlTree::Tuple { span, .. } => *span,
        SqlTree::Create { span, .. } => *span,
        SqlTree::Drop { span, .. } => *span,
        SqlTree::Alter { span, .. } => *span,
        SqlTree::ColumnDef { span, .. } => *span,
        SqlTree::Constraint { span, .. } => *span,
        SqlTree::AddColumn { span, .. } => *span,
        SqlTree::AddConstraint { span, .. } => *span,
        SqlTree::Function { span, .. } => *span,
        SqlTree::DataType { span, .. } => *span,
        SqlTree::Identifier { span, .. } => *span,
        SqlTree::Schema { span, .. } => *span,
        SqlTree::Alias { span, .. } => *span,
        SqlTree::Temp { span, .. } => *span,
        SqlTree::Variable { span, .. } => *span,
        SqlTree::Literal { span, .. } => *span,
        SqlTree::Comment { span, .. } => *span,
        SqlTree::Unknown { span, .. } => *span,
    }
}

/// Stored text for scalar-leaf variants (`Name`, `Atom`, `Int`,
/// `Float`, `String`, `True`, `False`, `None`, `Null`). Returns
/// `None` for compound variants. The renderer uses this to emit
/// leaf literals without consulting the source string (S13-Z1).
pub fn scalar_text_of(tree: &SqlTree) -> Option<&str> {
    match tree {
        SqlTree::Go { text, .. } => Some(text.as_str()),
        SqlTree::Identifier { value, .. } => Some(value.as_str()),
        SqlTree::Schema { value, .. } => Some(value.as_str()),
        SqlTree::Alias { value, .. } => Some(value.as_str()),
        SqlTree::Variable { text, .. } => Some(text.as_str()),
        SqlTree::Literal { text, .. } => Some(text.as_str()),
        SqlTree::Comment { text, .. } => Some(text.as_str()),
        _ => None,
    }
}

/// Direct tree children of this node, in source order. Excludes
/// synthetic render-time wrappers, modifier markers, and other shape
/// metadata. Used by every variant-blind walker (`to_xot.rs`,
/// `to_json.rs`, …) as the single source of truth for tree traversal.
pub fn children_of(tree: &SqlTree) -> Vec<&SqlTree> {
    let mut v: Vec<&SqlTree> = Vec::new();
    match tree {
        SqlTree::File { statements, .. } => {
            v.extend(statements.iter());
        }
        SqlTree::Statement { inner, .. } => {
            v.push(inner);
        }
        SqlTree::Go { .. } => {}
        SqlTree::Exec { target, .. } => {
            v.push(target);
        }
        SqlTree::Set { target, value, .. } => {
            v.push(target);
            v.push(value);
        }
        SqlTree::Select { ctes, columns, into, from, where_, group_by, having, order_by, .. } => {
            v.extend(ctes.iter());
            v.extend(columns.iter());
            if let Some(__t) = into { v.push(__t); }
            if let Some(__t) = from { v.push(__t); }
            if let Some(__t) = where_ { v.push(__t); }
            if let Some(__t) = group_by { v.push(__t); }
            if let Some(__t) = having { v.push(__t); }
            if let Some(__t) = order_by { v.push(__t); }
        }
        SqlTree::Insert { table, columns, values, .. } => {
            v.push(table);
            v.extend(columns.iter());
            v.extend(values.iter());
        }
        SqlTree::Update { table, assignments, where_, .. } => {
            v.push(table);
            v.extend(assignments.iter());
            if let Some(__t) = where_ { v.push(__t); }
        }
        SqlTree::Delete { from, where_, .. } => {
            if let Some(__t) = from { v.push(__t); }
            if let Some(__t) = where_ { v.push(__t); }
        }
        SqlTree::Merge { target, source, on, whens, .. } => {
            v.push(target);
            v.push(source);
            v.push(on);
            v.extend(whens.iter());
        }
        SqlTree::MergeWhen { action, .. } => {
            v.push(action);
        }
        SqlTree::Transaction { statements, .. } => {
            v.extend(statements.iter());
        }
        SqlTree::From { relations, .. } => {
            v.extend(relations.iter());
        }
        SqlTree::Where { condition, .. } => {
            v.push(condition);
        }
        SqlTree::GroupBy { keys, .. } => {
            v.extend(keys.iter());
        }
        SqlTree::Having { condition, .. } => {
            v.push(condition);
        }
        SqlTree::OrderBy { targets, .. } => {
            v.extend(targets.iter());
        }
        SqlTree::OrderTarget { expression, .. } => {
            v.push(expression);
        }
        SqlTree::PartitionBy { keys, .. } => {
            v.extend(keys.iter());
        }
        SqlTree::Join { relation, on, .. } => {
            v.push(relation);
            if let Some(__t) = on { v.push(__t); }
        }
        SqlTree::Relation { schema, name, alias, .. } => {
            if let Some(__t) = schema { v.push(__t); }
            v.push(name);
            if let Some(__t) = alias { v.push(__t); }
        }
        SqlTree::Column { expression, alias, .. } => {
            v.push(expression);
            if let Some(__t) = alias { v.push(__t); }
        }
        SqlTree::Star { qualifier, .. } => {
            if let Some(__t) = qualifier { v.push(__t); }
        }
        SqlTree::Reference { parts, .. } => {
            v.extend(parts.iter());
        }
        SqlTree::Compare { left, right, .. } => {
            v.push(left);
            v.push(right);
        }
        SqlTree::Binary { left, right, .. } => {
            v.push(left);
            v.push(right);
        }
        SqlTree::Unary { operand, .. } => {
            v.push(operand);
        }
        SqlTree::Assign { target, value, .. } => {
            v.push(target);
            v.push(value);
        }
        SqlTree::Between { value, low, high, .. } => {
            v.push(value);
            v.push(low);
            v.push(high);
        }
        SqlTree::Exists { subquery, .. } => {
            v.push(subquery);
        }
        SqlTree::Case { whens, else_, .. } => {
            v.extend(whens.iter());
            if let Some(__t) = else_ { v.push(__t); }
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
            if let Some(__t) = partition_by { v.push(__t); }
            if let Some(__t) = order_by { v.push(__t); }
        }
        SqlTree::Subquery { select, .. } => {
            v.push(select);
        }
        SqlTree::Union { selects, .. } => {
            v.extend(selects.iter());
        }
        SqlTree::Cte { name, query, .. } => {
            v.push(name);
            v.push(query);
        }
        SqlTree::Tuple { items, .. } => {
            v.extend(items.iter());
        }
        SqlTree::Create { name, body, .. } => {
            v.push(name);
            v.extend(body.iter());
        }
        SqlTree::Drop { name, .. } => {
            v.push(name);
        }
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
            if let Some(__t) = name { v.push(__t); }
            v.extend(body.iter());
        }
        SqlTree::AddColumn { column, .. } => {
            v.push(column);
        }
        SqlTree::AddConstraint { constraint, .. } => {
            v.push(constraint);
        }
        SqlTree::Function { schema, name, parameters, return_type, body, .. } => {
            if let Some(__t) = schema { v.push(__t); }
            v.push(name);
            v.extend(parameters.iter());
            if let Some(__t) = return_type { v.push(__t); }
            v.push(body);
        }
        SqlTree::DataType { length, .. } => {
            if let Some(__t) = length { v.push(__t); }
        }
        SqlTree::Identifier { .. } => {}
        SqlTree::Schema { .. } => {}
        SqlTree::Alias { .. } => {}
        SqlTree::Temp { name, .. } => {
            v.push(name);
        }
        SqlTree::Variable { .. } => {}
        SqlTree::Literal { .. } => {}
        SqlTree::Comment { .. } => {}
        SqlTree::Unknown { .. } => {}
    }
    v.sort_by_key(|c| range_of(c).start);
    v
}

/// Field-projection view of this node — one [`TreeField`] per
/// projected struct field. Only consulted when [`use_field_projection`]
/// returns true. See the walker for projection semantics.
pub fn fields_of(tree: &SqlTree) -> Vec<TreeField<'_, SqlTree>> {
    let mut out: Vec<TreeField<'_, SqlTree>> = Vec::new();
    match tree {
        SqlTree::File { statements, .. } => {
            out.push(TreeField::Many { name: "statements", items: statements.iter().collect() });
        }
        SqlTree::Statement { inner, .. } => {
            out.push(TreeField::Single { name: "inner", value: inner });
        }
        SqlTree::Go { text, .. } => {
            out.push(TreeField::Scalar { name: "text", value: text.as_str() });
        }
        SqlTree::Exec { target, .. } => {
            out.push(TreeField::Single { name: "target", value: target });
        }
        SqlTree::Set { target, value, .. } => {
            out.push(TreeField::Single { name: "target", value: target });
            out.push(TreeField::Single { name: "value", value: value });
        }
        SqlTree::Select { ctes, columns, into, from, where_, group_by, having, order_by, .. } => {
            out.push(TreeField::Many { name: "ctes", items: ctes.iter().collect() });
            out.push(TreeField::Many { name: "columns", items: columns.iter().collect() });
            if let Some(__t) = into { out.push(TreeField::Single { name: "into", value: __t }); }
            if let Some(__t) = from { out.push(TreeField::Single { name: "from", value: __t }); }
            if let Some(__t) = where_ { out.push(TreeField::Single { name: "where", value: __t }); }
            if let Some(__t) = group_by { out.push(TreeField::Single { name: "group_by", value: __t }); }
            if let Some(__t) = having { out.push(TreeField::Single { name: "having", value: __t }); }
            if let Some(__t) = order_by { out.push(TreeField::Single { name: "order_by", value: __t }); }
        }
        SqlTree::Insert { table, columns, values, .. } => {
            out.push(TreeField::Single { name: "table", value: table });
            out.push(TreeField::Many { name: "columns", items: columns.iter().collect() });
            out.push(TreeField::Many { name: "values", items: values.iter().collect() });
        }
        SqlTree::Update { table, assignments, where_, .. } => {
            out.push(TreeField::Single { name: "table", value: table });
            out.push(TreeField::Many { name: "assignments", items: assignments.iter().collect() });
            if let Some(__t) = where_ { out.push(TreeField::Single { name: "where", value: __t }); }
        }
        SqlTree::Delete { from, where_, .. } => {
            if let Some(__t) = from { out.push(TreeField::Single { name: "from", value: __t }); }
            if let Some(__t) = where_ { out.push(TreeField::Single { name: "where", value: __t }); }
        }
        SqlTree::Merge { target, source, on, whens, .. } => {
            out.push(TreeField::Single { name: "target", value: target });
            out.push(TreeField::Single { name: "source", value: source });
            out.push(TreeField::Single { name: "on", value: on });
            out.push(TreeField::Many { name: "whens", items: whens.iter().collect() });
        }
        SqlTree::MergeWhen { action, .. } => {
            out.push(TreeField::Single { name: "action", value: action });
        }
        SqlTree::Transaction { statements, .. } => {
            out.push(TreeField::Many { name: "statements", items: statements.iter().collect() });
        }
        SqlTree::From { relations, .. } => {
            out.push(TreeField::Many { name: "relations", items: relations.iter().collect() });
        }
        SqlTree::Where { condition, .. } => {
            out.push(TreeField::Single { name: "condition", value: condition });
        }
        SqlTree::GroupBy { keys, .. } => {
            out.push(TreeField::Many { name: "keys", items: keys.iter().collect() });
        }
        SqlTree::Having { condition, .. } => {
            out.push(TreeField::Single { name: "condition", value: condition });
        }
        SqlTree::OrderBy { targets, .. } => {
            out.push(TreeField::Many { name: "targets", items: targets.iter().collect() });
        }
        SqlTree::OrderTarget { expression, extra_markers, .. } => {
            out.push(TreeField::Single { name: "expression", value: expression });
            for m in extra_markers { out.push(TreeField::Flag { name: m.name, marker: *m }); }
        }
        SqlTree::PartitionBy { keys, .. } => {
            out.push(TreeField::Many { name: "keys", items: keys.iter().collect() });
        }
        SqlTree::Join { relation, on, extra_markers, .. } => {
            out.push(TreeField::Single { name: "relation", value: relation });
            if let Some(__t) = on { out.push(TreeField::Single { name: "on", value: __t }); }
            for m in extra_markers { out.push(TreeField::Flag { name: m.name, marker: *m }); }
        }
        SqlTree::Relation { schema, name, alias, .. } => {
            if let Some(__t) = schema { out.push(TreeField::Single { name: "schema", value: __t }); }
            out.push(TreeField::Single { name: "name", value: name });
            if let Some(__t) = alias { out.push(TreeField::Single { name: "alias", value: __t }); }
        }
        SqlTree::Column { expression, alias, .. } => {
            out.push(TreeField::Single { name: "expression", value: expression });
            if let Some(__t) = alias { out.push(TreeField::Single { name: "alias", value: __t }); }
        }
        SqlTree::Star { qualifier, .. } => {
            if let Some(__t) = qualifier { out.push(TreeField::Single { name: "qualifier", value: __t }); }
        }
        SqlTree::Reference { parts, .. } => {
            out.push(TreeField::Many { name: "parts", items: parts.iter().collect() });
        }
        SqlTree::Compare { left, op_text, right, .. } => {
            out.push(TreeField::Single { name: "left", value: left });
            out.push(TreeField::Scalar { name: "op_text", value: op_text.as_str() });
            out.push(TreeField::Single { name: "right", value: right });
        }
        SqlTree::Binary { left, op_text, right, .. } => {
            out.push(TreeField::Single { name: "left", value: left });
            out.push(TreeField::Scalar { name: "op_text", value: op_text.as_str() });
            out.push(TreeField::Single { name: "right", value: right });
        }
        SqlTree::Unary { op_text, operand, .. } => {
            out.push(TreeField::Scalar { name: "op_text", value: op_text.as_str() });
            out.push(TreeField::Single { name: "operand", value: operand });
        }
        SqlTree::Assign { target, value, .. } => {
            out.push(TreeField::Single { name: "target", value: target });
            out.push(TreeField::Single { name: "value", value: value });
        }
        SqlTree::Between { value, low, high, .. } => {
            out.push(TreeField::Single { name: "value", value: value });
            out.push(TreeField::Single { name: "low", value: low });
            out.push(TreeField::Single { name: "high", value: high });
        }
        SqlTree::Exists { subquery, .. } => {
            out.push(TreeField::Single { name: "subquery", value: subquery });
        }
        SqlTree::Case { whens, else_, .. } => {
            out.push(TreeField::Many { name: "whens", items: whens.iter().collect() });
            if let Some(__t) = else_ { out.push(TreeField::Single { name: "else", value: __t }); }
        }
        SqlTree::When { condition, value, .. } => {
            out.push(TreeField::Single { name: "condition", value: condition });
            out.push(TreeField::Single { name: "value", value: value });
        }
        SqlTree::Cast { value, type_, .. } => {
            out.push(TreeField::Single { name: "value", value: value });
            out.push(TreeField::Single { name: "type", value: type_ });
        }
        SqlTree::Call { callee, arguments, .. } => {
            out.push(TreeField::Single { name: "callee", value: callee });
            out.push(TreeField::Many { name: "arguments", items: arguments.iter().collect() });
        }
        SqlTree::Window { call, over, .. } => {
            out.push(TreeField::Single { name: "call", value: call });
            out.push(TreeField::Single { name: "over", value: over });
        }
        SqlTree::Over { partition_by, order_by, .. } => {
            if let Some(__t) = partition_by { out.push(TreeField::Single { name: "partition_by", value: __t }); }
            if let Some(__t) = order_by { out.push(TreeField::Single { name: "order_by", value: __t }); }
        }
        SqlTree::Subquery { select, .. } => {
            out.push(TreeField::Single { name: "select", value: select });
        }
        SqlTree::Union { selects, .. } => {
            out.push(TreeField::Many { name: "selects", items: selects.iter().collect() });
        }
        SqlTree::Cte { name, query, .. } => {
            out.push(TreeField::Single { name: "name", value: name });
            out.push(TreeField::Single { name: "query", value: query });
        }
        SqlTree::Tuple { items, .. } => {
            out.push(TreeField::Many { name: "items", items: items.iter().collect() });
        }
        SqlTree::Create { name, body, .. } => {
            out.push(TreeField::Single { name: "name", value: name });
            out.push(TreeField::Many { name: "body", items: body.iter().collect() });
        }
        SqlTree::Drop { name, .. } => {
            out.push(TreeField::Single { name: "name", value: name });
        }
        SqlTree::Alter { name, operation, .. } => {
            out.push(TreeField::Single { name: "name", value: name });
            out.push(TreeField::Single { name: "operation", value: operation });
        }
        SqlTree::ColumnDef { name, type_, constraints, .. } => {
            out.push(TreeField::Single { name: "name", value: name });
            out.push(TreeField::Single { name: "type", value: type_ });
            out.push(TreeField::Many { name: "constraints", items: constraints.iter().collect() });
        }
        SqlTree::Constraint { name, body, .. } => {
            if let Some(__t) = name { out.push(TreeField::Single { name: "name", value: __t }); }
            out.push(TreeField::Many { name: "body", items: body.iter().collect() });
        }
        SqlTree::AddColumn { column, .. } => {
            out.push(TreeField::Single { name: "column", value: column });
        }
        SqlTree::AddConstraint { constraint, .. } => {
            out.push(TreeField::Single { name: "constraint", value: constraint });
        }
        SqlTree::Function { schema, name, parameters, return_type, body, .. } => {
            if let Some(__t) = schema { out.push(TreeField::Single { name: "schema", value: __t }); }
            out.push(TreeField::Single { name: "name", value: name });
            out.push(TreeField::Many { name: "parameters", items: parameters.iter().collect() });
            if let Some(__t) = return_type { out.push(TreeField::Single { name: "return_type", value: __t }); }
            out.push(TreeField::Single { name: "body", value: body });
        }
        SqlTree::DataType { length, .. } => {
            if let Some(__t) = length { out.push(TreeField::Single { name: "length", value: __t }); }
        }
        SqlTree::Identifier { value, .. } => {
            out.push(TreeField::Scalar { name: "value", value: value.as_str() });
        }
        SqlTree::Schema { value, .. } => {
            out.push(TreeField::Scalar { name: "value", value: value.as_str() });
        }
        SqlTree::Alias { value, .. } => {
            out.push(TreeField::Scalar { name: "value", value: value.as_str() });
        }
        SqlTree::Temp { name, .. } => {
            out.push(TreeField::Single { name: "name", value: name });
        }
        SqlTree::Variable { text, .. } => {
            out.push(TreeField::Scalar { name: "text", value: text.as_str() });
        }
        SqlTree::Literal { text, .. } => {
            out.push(TreeField::Scalar { name: "text", value: text.as_str() });
        }
        SqlTree::Comment { text, .. } => {
            out.push(TreeField::Scalar { name: "text", value: text.as_str() });
        }
        SqlTree::Unknown { kind, .. } => {
            out.push(TreeField::Scalar { name: "kind", value: kind.as_str() });
        }
    }
    out
}

/// Per-variant opt-in flag for the field-projection walker
/// path. Returns true for variants whose doc comments carry
/// `@field_projection`; false otherwise.
pub fn use_field_projection(tree: &SqlTree) -> bool {
    match tree {
        _ => false,
    }
}

/// Mutable access to the source-location span. Used by
/// `assign_ids` to stamp `NodeId`s into existing spans without
/// rebuilding nodes. Delegates from `TreeNode::span_mut`.
pub fn span_mut_of(tree: &mut SqlTree) -> &mut Span {
    match tree {
        SqlTree::File { span, .. } => span,
        SqlTree::Statement { span, .. } => span,
        SqlTree::Go { span, .. } => span,
        SqlTree::Exec { span, .. } => span,
        SqlTree::Set { span, .. } => span,
        SqlTree::Select { span, .. } => span,
        SqlTree::Insert { span, .. } => span,
        SqlTree::Update { span, .. } => span,
        SqlTree::Delete { span, .. } => span,
        SqlTree::Merge { span, .. } => span,
        SqlTree::MergeWhen { span, .. } => span,
        SqlTree::Transaction { span, .. } => span,
        SqlTree::From { span, .. } => span,
        SqlTree::Where { span, .. } => span,
        SqlTree::GroupBy { span, .. } => span,
        SqlTree::Having { span, .. } => span,
        SqlTree::OrderBy { span, .. } => span,
        SqlTree::OrderTarget { span, .. } => span,
        SqlTree::PartitionBy { span, .. } => span,
        SqlTree::Join { span, .. } => span,
        SqlTree::Relation { span, .. } => span,
        SqlTree::Column { span, .. } => span,
        SqlTree::Star { span, .. } => span,
        SqlTree::Reference { span, .. } => span,
        SqlTree::Compare { span, .. } => span,
        SqlTree::Binary { span, .. } => span,
        SqlTree::Unary { span, .. } => span,
        SqlTree::Assign { span, .. } => span,
        SqlTree::Between { span, .. } => span,
        SqlTree::Exists { span, .. } => span,
        SqlTree::Case { span, .. } => span,
        SqlTree::When { span, .. } => span,
        SqlTree::Cast { span, .. } => span,
        SqlTree::Call { span, .. } => span,
        SqlTree::Window { span, .. } => span,
        SqlTree::Over { span, .. } => span,
        SqlTree::Subquery { span, .. } => span,
        SqlTree::Union { span, .. } => span,
        SqlTree::Cte { span, .. } => span,
        SqlTree::Tuple { span, .. } => span,
        SqlTree::Create { span, .. } => span,
        SqlTree::Drop { span, .. } => span,
        SqlTree::Alter { span, .. } => span,
        SqlTree::ColumnDef { span, .. } => span,
        SqlTree::Constraint { span, .. } => span,
        SqlTree::AddColumn { span, .. } => span,
        SqlTree::AddConstraint { span, .. } => span,
        SqlTree::Function { span, .. } => span,
        SqlTree::DataType { span, .. } => span,
        SqlTree::Identifier { span, .. } => span,
        SqlTree::Schema { span, .. } => span,
        SqlTree::Alias { span, .. } => span,
        SqlTree::Temp { span, .. } => span,
        SqlTree::Variable { span, .. } => span,
        SqlTree::Literal { span, .. } => span,
        SqlTree::Comment { span, .. } => span,
        SqlTree::Unknown { span, .. } => span,
    }
}

/// Mutable mirror of `children_of`. Source-sorted
/// `Vec<&mut SqlTree>` covering every reachable sub-tree.
pub fn children_mut_of(tree: &mut SqlTree) -> Vec<&mut SqlTree> {
    let mut v: Vec<&mut SqlTree> = Vec::new();
    match tree {
        SqlTree::File { statements, .. } => {
            v.extend(statements.iter_mut());
        }
        SqlTree::Statement { inner, .. } => {
            v.push(inner.as_mut());
        }
        SqlTree::Go { .. } => {}
        SqlTree::Exec { target, .. } => {
            v.push(target.as_mut());
        }
        SqlTree::Set { target, value, .. } => {
            v.push(target.as_mut());
            v.push(value.as_mut());
        }
        SqlTree::Select { ctes, columns, into, from, where_, group_by, having, order_by, .. } => {
            v.extend(ctes.iter_mut());
            v.extend(columns.iter_mut());
            if let Some(__t) = into { v.push(__t.as_mut()); }
            if let Some(__t) = from { v.push(__t.as_mut()); }
            if let Some(__t) = where_ { v.push(__t.as_mut()); }
            if let Some(__t) = group_by { v.push(__t.as_mut()); }
            if let Some(__t) = having { v.push(__t.as_mut()); }
            if let Some(__t) = order_by { v.push(__t.as_mut()); }
        }
        SqlTree::Insert { table, columns, values, .. } => {
            v.push(table.as_mut());
            v.extend(columns.iter_mut());
            v.extend(values.iter_mut());
        }
        SqlTree::Update { table, assignments, where_, .. } => {
            v.push(table.as_mut());
            v.extend(assignments.iter_mut());
            if let Some(__t) = where_ { v.push(__t.as_mut()); }
        }
        SqlTree::Delete { from, where_, .. } => {
            if let Some(__t) = from { v.push(__t.as_mut()); }
            if let Some(__t) = where_ { v.push(__t.as_mut()); }
        }
        SqlTree::Merge { target, source, on, whens, .. } => {
            v.push(target.as_mut());
            v.push(source.as_mut());
            v.push(on.as_mut());
            v.extend(whens.iter_mut());
        }
        SqlTree::MergeWhen { action, .. } => {
            v.push(action.as_mut());
        }
        SqlTree::Transaction { statements, .. } => {
            v.extend(statements.iter_mut());
        }
        SqlTree::From { relations, .. } => {
            v.extend(relations.iter_mut());
        }
        SqlTree::Where { condition, .. } => {
            v.push(condition.as_mut());
        }
        SqlTree::GroupBy { keys, .. } => {
            v.extend(keys.iter_mut());
        }
        SqlTree::Having { condition, .. } => {
            v.push(condition.as_mut());
        }
        SqlTree::OrderBy { targets, .. } => {
            v.extend(targets.iter_mut());
        }
        SqlTree::OrderTarget { expression, .. } => {
            v.push(expression.as_mut());
        }
        SqlTree::PartitionBy { keys, .. } => {
            v.extend(keys.iter_mut());
        }
        SqlTree::Join { relation, on, .. } => {
            v.push(relation.as_mut());
            if let Some(__t) = on { v.push(__t.as_mut()); }
        }
        SqlTree::Relation { schema, name, alias, .. } => {
            if let Some(__t) = schema { v.push(__t.as_mut()); }
            v.push(name.as_mut());
            if let Some(__t) = alias { v.push(__t.as_mut()); }
        }
        SqlTree::Column { expression, alias, .. } => {
            v.push(expression.as_mut());
            if let Some(__t) = alias { v.push(__t.as_mut()); }
        }
        SqlTree::Star { qualifier, .. } => {
            if let Some(__t) = qualifier { v.push(__t.as_mut()); }
        }
        SqlTree::Reference { parts, .. } => {
            v.extend(parts.iter_mut());
        }
        SqlTree::Compare { left, right, .. } => {
            v.push(left.as_mut());
            v.push(right.as_mut());
        }
        SqlTree::Binary { left, right, .. } => {
            v.push(left.as_mut());
            v.push(right.as_mut());
        }
        SqlTree::Unary { operand, .. } => {
            v.push(operand.as_mut());
        }
        SqlTree::Assign { target, value, .. } => {
            v.push(target.as_mut());
            v.push(value.as_mut());
        }
        SqlTree::Between { value, low, high, .. } => {
            v.push(value.as_mut());
            v.push(low.as_mut());
            v.push(high.as_mut());
        }
        SqlTree::Exists { subquery, .. } => {
            v.push(subquery.as_mut());
        }
        SqlTree::Case { whens, else_, .. } => {
            v.extend(whens.iter_mut());
            if let Some(__t) = else_ { v.push(__t.as_mut()); }
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
            if let Some(__t) = partition_by { v.push(__t.as_mut()); }
            if let Some(__t) = order_by { v.push(__t.as_mut()); }
        }
        SqlTree::Subquery { select, .. } => {
            v.push(select.as_mut());
        }
        SqlTree::Union { selects, .. } => {
            v.extend(selects.iter_mut());
        }
        SqlTree::Cte { name, query, .. } => {
            v.push(name.as_mut());
            v.push(query.as_mut());
        }
        SqlTree::Tuple { items, .. } => {
            v.extend(items.iter_mut());
        }
        SqlTree::Create { name, body, .. } => {
            v.push(name.as_mut());
            v.extend(body.iter_mut());
        }
        SqlTree::Drop { name, .. } => {
            v.push(name.as_mut());
        }
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
            if let Some(__t) = name { v.push(__t.as_mut()); }
            v.extend(body.iter_mut());
        }
        SqlTree::AddColumn { column, .. } => {
            v.push(column.as_mut());
        }
        SqlTree::AddConstraint { constraint, .. } => {
            v.push(constraint.as_mut());
        }
        SqlTree::Function { schema, name, parameters, return_type, body, .. } => {
            if let Some(__t) = schema { v.push(__t.as_mut()); }
            v.push(name.as_mut());
            v.extend(parameters.iter_mut());
            if let Some(__t) = return_type { v.push(__t.as_mut()); }
            v.push(body.as_mut());
        }
        SqlTree::DataType { length, .. } => {
            if let Some(__t) = length { v.push(__t.as_mut()); }
        }
        SqlTree::Identifier { .. } => {}
        SqlTree::Schema { .. } => {}
        SqlTree::Alias { .. } => {}
        SqlTree::Temp { name, .. } => {
            v.push(name.as_mut());
        }
        SqlTree::Variable { .. } => {}
        SqlTree::Literal { .. } => {}
        SqlTree::Comment { .. } => {}
        SqlTree::Unknown { .. } => {}
    }
    v.sort_by_key(|c| range_of(c).start);
    v
}
