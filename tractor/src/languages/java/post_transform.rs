//! Java post_transform pipeline + helpers.
//!
//! Runs after `walk_transform` to apply chain inversion (with flat-
//! call pre-pass), conditional collapse, expression-position wrap,
//! multi-target tagging, role tagging, type-in-path unwrap, brace
//! strip, declarator flatten, list distribution.
//!
//! Moved out of `tractor/src/languages/mod.rs` iter 331 per user
//! direction: per-language transform code belongs with the language
//! module, not in the generic registry.

use xot::{Xot, Node as XotNode};

use crate::languages::{collapse_conditionals, collect_named_elements};

/// Java post-transform: collapse conditionals + wrap expression
/// positions in `<expression>` hosts (Principle #15).
pub fn java_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    // Normalise Java's flat call shape to the canonical input.
    // Java emits `<call><object/>NAME...args</call>` where the
    // method name is a bare `<name>` sibling of `<object>`. The
    // chain inverter expects `<call><member><object/><property/></member>...args</call>`,
    // so pre-wrap the `<object>`+`<name>` pair into a synthetic
    // `<member>` first.
    crate::transform::chain_inversion::wrap_flat_call_member(xot, root)?;
    crate::transform::chain_inversion::invert_chains_in_tree(xot, root)?;
    collapse_conditionals(xot, root)?;
    crate::transform::wrap_expression_positions(
        xot,
        root,
        &["value", "condition", "left", "right", "return"],
    )?;
    crate::transform::tag_multi_target_expressions(xot, root)?;
    crate::transform::tag_multi_same_name_children(xot, root, &["type", "pattern", "string", "import"])?;
    // Java method reference: `String::valueOf` produces `<reference>`
    // with two `<name>` siblings (class + method). Tag both with
    // `list="name"` so the JSON name array is uniform; cardinality
    // discriminator (>=2) keeps singleton uses untouched.
    crate::transform::tag_multi_role_children(
        xot, root,
        &[
            ("reference", "name"),
            // Multi-declarator (`int x = 1, y = 2`) keeps
            // `<declarator>` wrappers (per iter 263). Tag with
            // `list="declarators"` so JSON renders them as an
            // array; single-declarator is flattened by the
            // post-pass below and doesn't reach this tag.
            ("variable", "declarator"),
            ("field", "declarator"),
        ],
    )?;
    java_unwrap_type_in_path(xot, root)?;
    crate::transform::flatten_nested_paths(xot, root)?;
    crate::transform::strip_body_braces(
        xot, root,
        &["body", "block", "then", "else", "call"],
    )?;
    // Single-declarator fields and locals lose their <declarator>
    // wrapper (`int x = 1;` → `field/{type, name, value}`).
    // Multi-declarator (`int a, b = 5`) keeps wrappers — each is a
    // role-mixed name+value group whose pairing depends on the
    // wrapper. See cold-read backlog iter 233.
    crate::transform::flatten_single_declarator_children(xot, root, &["field", "variable"])?;
    crate::transform::distribute_member_list_attrs(
        xot, root, &["body", "block", "program", "tuple", "list", "dict", "array", "hash", "repetition"],
    )?;
    Ok(())
}

/// Inside `<path>`, tree-sitter Java's `scoped_type_identifier` produces
/// `<type><name>X</name></type>` segments. The path is a namespace
/// identifier path; the segments are *names*, not types (Principle
/// #14). Walk every `<path>` and collapse `<type>` segment wrappers to
/// bare `<name>` children.
fn java_unwrap_type_in_path(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::get_element_name;
    let mut paths: Vec<XotNode> = Vec::new();
    collect_named_elements(xot, root, "path", &mut paths);
    for path in paths {
        for child in xot.children(path).collect::<Vec<_>>() {
            if get_element_name(xot, child).as_deref() != Some("type") {
                continue;
            }
            // Replace each <type><name>X</name></type> with <name>X</name>.
            let inner_names: Vec<XotNode> = xot.children(child)
                .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
                .collect();
            if inner_names.len() != 1 {
                continue;
            }
            let inner_name = inner_names[0];
            xot.detach(inner_name)?;
            xot.insert_before(child, inner_name)?;
            xot.detach(child)?;
        }
    }
    Ok(())
}
