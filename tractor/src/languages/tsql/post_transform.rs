//! TSQL post_transform pipeline + helpers.
//!
//! Runs after `walk_transform` to apply binary-operand wrapping
//! (re-creating `<left>`/`<right>` slots from raw `field=` attributes
//! that the dispatcher Skipped) and list-tag distribution for the
//! role-uniform containers.
//!
//! Moved out of `tractor/src/languages/mod.rs` iter 333 per user
//! direction: per-language transform code belongs with the language
//! module, not in the generic registry.

use xot::{Xot, Node as XotNode};

/// TSQL post-transform: pre-iter-182 had `post_transform: None`,
/// which left every container with multiple uniform-role children
/// overflowing into the anonymous `children: [...]` JSON array.
/// Adds `distribute_member_list_attrs` for the role-uniform
/// containers (every direct element child shares a role): file
/// scripts, transaction blocks, union arms, explicit value lists,
/// columns lists, statement bodies (DDL/DML body containers).
///
/// Role-MIXED containers (`<select>`, `<insert>`, `<from>`,
/// `<call>`, `<case>`, `<compare>`, `<between>`, `<assign>`) need
/// targeted handlers that tag only the multi-instance child role —
/// out of scope for this iter.
pub fn tsql_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    tsql_wrap_binary_operands(xot, root)?;
    crate::transform::distribute_member_list_attrs(
        xot,
        root,
        &[
            "file", "transaction", "union", "columns", "list",
            "select", "insert", "from", "call", "case", "constraint",
        ],
    )?;
    tsql_tag_select_columns(xot, root)?;
    Ok(())
}

/// Wrap `<compare>` / `<assign>` / `<between>` operand children in
/// role-named `<left>` / `<right>` slots based on their `field=`
/// attribute. TSQL's transform dispatcher (`tsql/transform.rs:29`)
/// intentionally Skip-routes builder-inserted `<left>` / `<right>`
/// wrappers, so this post-pass re-wraps the operands that retained
/// their `field="left"` / `field="right"` attributes from the raw
/// tree-sitter input.
///
/// Closes the iter-185 deferred mystery (root cause SOLVED iter-197
/// review).
fn tsql_wrap_binary_operands(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{get_attr, get_element_name, XotWithExt};
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    let mut parents: Vec<XotNode> = Vec::new();
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some() {
            if let Some(name) = get_element_name(xot, node) {
                if matches!(name.as_str(), "compare" | "assign" | "between") {
                    out.push(node);
                }
            }
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut parents);

    for parent in parents {
        let elem_children: Vec<XotNode> = xot.children(parent)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        for child in elem_children {
            let field = get_attr(xot, child, "field");
            // BETWEEN's low/high → <low>/<high>; binary operands → <left>/<right>.
            let wrapper = match field.as_deref() {
                Some("left") => "left",
                Some("right") => "right",
                Some("low") => "low",
                Some("high") => "high",
                _ => continue,
            };
            let wrapper_id = xot.add_name(wrapper);
            let wrapper_node = xot.new_element(wrapper_id);
            xot.with_source_location_from(wrapper_node, child)
                .with_wrap_child(child, wrapper_node)?;
        }
    }
    Ok(())
}

/// Tag `<column>` children of `<select>` / `<insert>` with
/// `list="column"` so JSON `select.column: [...]` becomes a uniform
/// array (was: first column lifted as singleton, rest in
/// `children` overflow). Targeted (not bulk via
/// `distribute_member_list_attrs`) because select/insert have
/// role-MIXED children: column lists + singleton clauses
/// (`<from>`, `<where>`, `<order>`, `<alias>`).
fn tsql_tag_select_columns(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{get_attr, get_element_name, XotWithExt};
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    fn collect(xot: &Xot, node: XotNode, names: &[&str], out: &mut Vec<XotNode>) {
        if xot.element(node).is_some() {
            if let Some(name) = get_element_name(xot, node) {
                if names.contains(&name.as_str()) {
                    out.push(node);
                }
            }
        }
        for c in xot.children(node) {
            collect(xot, c, names, out);
        }
    }
    let mut parents: Vec<XotNode> = Vec::new();
    collect(xot, root, &["select", "insert"], &mut parents);
    for parent in parents {
        let columns: Vec<XotNode> = xot.children(parent)
            .filter(|&c| {
                xot.element(c).is_some()
                    && get_element_name(xot, c).as_deref() == Some("column")
            })
            .collect();
        for col in columns {
            if get_attr(xot, col, "list").is_none() {
                xot.with_attr(col, "list", "columns");
            }
        }
    }
    Ok(())
}
