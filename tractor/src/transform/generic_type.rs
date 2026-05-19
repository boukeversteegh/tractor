//! Generic-type rewriter transformer.
//!
//! Rewrites tree-sitter `generic_type` nodes into the canonical
//! `<type><generic/>NAME<type field="arguments">…</type></type>` shape
//! shared across all parametric-type languages (Rust, TS, Java, C#, Go).
//! Languages call [`rewrite_generic_type`] from their type-handling
//! transform branches.

use xot::{Xot, Node as XotNode};

use super::helpers::{
    collect_descendant_text, descendant_text, get_element_name, get_kind,
    prepend_empty_element, rename,
};

/// Rewrite a tree-sitter `generic_type` node into the canonical
/// shape shared across languages:
///
/// ```text
/// <type>
///   <generic/>
///   Name
///   <type field="arguments">Arg1</type>
///   <type field="arguments">Arg2</type>
/// </type>
/// ```
///
/// The `name_kinds` slice is the list of tree-sitter kinds to accept
/// as the simple type name (typically `type_identifier` and
/// `identifier`). Any child whose `kind` matches is collapsed to plain
/// text; a `<name>` field wrapper around a simple name is also
/// collapsed. Non-simple names (qualified identifiers, etc.) are left
/// as a `<name>` sub-element so they remain queryable.
pub fn rewrite_generic_type(
    xot: &mut Xot,
    node: XotNode,
    name_kinds: &[&str],
) -> Result<(), xot::Error> {
    let children: Vec<XotNode> = xot.children(node).collect();
    for child in children {
        let child_name = match get_element_name(xot, child) {
            Some(n) => n,
            None => continue,
        };

        // Case 1: the name is a <name> field-wrapper created by the
        // builder. If it wraps a single simple identifier, normalise
        // it to `<name>TEXT</name>` (collapse any inner identifier
        // element into plain text).
        if child_name == "name" {
            let grandchildren: Vec<XotNode> = xot.children(child).collect();
            let is_simple = !grandchildren.is_empty()
                && grandchildren.iter().all(|&gc| {
                    match get_kind(xot, gc).as_deref() {
                        Some(k) => name_kinds.contains(&k),
                        None => xot.text_str(gc).is_some(),
                    }
                });
            if is_simple {
                let mut buf = String::new();
                collect_descendant_text(xot, child, &mut buf);
                if !buf.is_empty() {
                    // Replace any children with a single text node.
                    for gc in grandchildren {
                        xot.detach(gc)?;
                    }
                    let text_node = xot.new_text(&buf);
                    xot.append(child, text_node)?;
                }
            }
            break;
        }

        // Case 2: the name is a bare identifier child with no field
        // wrapper (happens when the grammar doesn't tag it as a
        // field). Wrap in a `<name>` element.
        //
        // Use descendant_text rather than direct-child text so
        // that scoped paths like `std::collections::HashMap`
        // (whose segments live inside nested `scoped_identifier`
        // elements) carry the full qualified name, not just the
        // first top-level separator.
        if let Some(kind) = get_kind(xot, child) {
            if name_kinds.contains(&kind.as_str()) {
                let text = descendant_text(xot, child);
                if !text.is_empty() {
                    let name_id = xot.add_name("name");
                    let name_el = xot.new_element(name_id);
                    let text_node = xot.new_text(&text);
                    xot.append(name_el, text_node)?;
                    xot.insert_before(child, name_el)?;
                    xot.detach(child)?;
                }
                break;
            }
        }
    }
    prepend_empty_element(xot, node, "generic")?;
    rename(xot, node, "type");
    Ok(())
}
