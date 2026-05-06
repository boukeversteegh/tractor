//! PHP post_transform pipeline + helpers.
//!
//! Runs after `walk_transform` to apply chain-inversion pre-passes
//! (member/call slot wrap), chain inversion, conditional collapse,
//! expression-position wrap, `<use>` restructure, role tagging, path
//! flattening, brace strip, list distribution.
//!
//! Moved out of `tractor/src/languages/mod.rs` iter 333 per user
//! direction: per-language transform code belongs with the language
//! module, not in the generic registry.

use xot::{Xot, Node as XotNode};

use crate::languages::{collapse_conditionals, collect_named_elements};

/// PHP post-transform: collapse conditionals + wrap expression
/// positions in `<expression>` hosts (Principle #15) + restructure
/// `<use>` elements into the unified path/alias/marker shape.
pub fn php_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    // PHP's `<member>` and `<call>` use the `->` operator and emit
    // unwrapped slots: receiver as a child with `field="object"`,
    // access name as a bare `<name>` sibling (no `<property>`).
    // Pre-pass wraps these into the canonical input shape, then
    // chain inversion runs.
    php_wrap_member_call_slots(xot, root)?;
    crate::transform::chain_inversion::wrap_flat_call_member(xot, root)?;
    crate::transform::chain_inversion::invert_chains_in_tree(xot, root)?;
    collapse_conditionals(xot, root)?;
    crate::transform::wrap_expression_positions(
        xot,
        root,
        &["value", "condition", "left", "right", "return"],
    )?;
    php_restructure_use(xot, root)?;
    // Wrap bare-name relationship targets in `<type>` BEFORE tagging
    // multi-role children so `("implements", "type")` sees the wrapped
    // shape. Iter 339: PHP `implements A, B` lowers as bare `<name>`
    // children, and the wrap pass needs to run first so the role-tag
    // pass can find the `<type>` siblings.
    crate::transform::wrap_relationship_targets_in_type(xot, root)?;
    crate::transform::tag_multi_same_name_children(xot, root, &["type", "pattern", "string", "import"])?;
    crate::transform::tag_multi_target_expressions(xot, root)?;
    crate::transform::tag_multi_role_children(
        xot, root,
        &[
            // PHP `use Foo\{First, Second};` — `<use[group]>` parent
            // with multiple inner `<use>` siblings (one per imported
            // entity). Tag with `list="uses"` so JSON renders as
            // `uses: [...]` array. Mirrors Rust iter 267.
            ("use", "use"),
            // C-style for header `for ($i=0, $j=10; ...; $i++, $j--)`
            // produces `<for>` with multiple `<assign>` siblings (init
            // sequence) AND multiple `<unary>` siblings (post-update
            // sequence). Both role-uniform per Principle #19. The
            // unary tagging mirrors TypeScript iter 269.
            ("for", "assign"),
            ("for", "unary"),
            // PHP `<string>` parent: interpolated strings have one or
            // more `<interpolation>` chunks; heredoc strings have one
            // or more `<value>` chunks. Bulk-distribute on `"string"`
            // (removed below iter 308) was wrapping single-interp /
            // single-value cases in 1-elem JSON arrays. Targeted role
            // tags here cover both single (lifts as singleton) and
            // multi (proper array) cases.
            ("string", "interpolation"),
            ("string", "value"),
            // PHP namespace path `App\Blueprint` produces multiple
            // `<name>` children of `<namespace>`. Iter 328 dropped
            // `"namespace"` from PHP's bulk distribute (the
            // architectural ROLE_MIXED_PARENTS guard requires it);
            // this targeted tag covers the multi-name path case.
            // Single-name namespaces lift as `name: "App"` singleton.
            ("namespace", "name"),
            // IR-pipeline additions: cover multi-cardinality role
            // children that overflow $children otherwise.
            ("class", "field"),
            ("class", "const"),
            ("class", "method"),
            ("member", "name"),
            ("implements", "type"),
            ("ternary", "string"),
            ("match", "arm"),
            ("pair", "variable"),
            ("enum", "constant"),
            ("call", "argument"),
        ],
    )?;
    crate::transform::flatten_nested_paths(xot, root)?;
    crate::transform::strip_body_braces(xot, root, &["body", "then", "else"])?;
    crate::transform::distribute_member_list_attrs(
        // `"namespace"` removed iter 328 — see targeted role tag above.
        xot, root, &["body", "program", "tuple", "list", "dict", "array", "repetition"],
    )?;
    Ok(())
}

/// Pre-pass for chain inversion: rewrite PHP's `<member>` and
/// `<call>` shapes into the canonical right-deep input.
///
/// PHP emits:
///   `<member><instance/>RECEIVER<name>X</name></member>` — receiver
///   has `field="object"`, name is a bare sibling.
///   `<call><instance/>CALLEE...args</call>` where CALLEE may be
///   `<member>` for method calls or any other expression for direct
///   function calls.
///
/// The chain inverter wants:
///   `<member><object>RECEIVER</object><property><name>X</name></property></member>`
///
/// This pass walks every `<member>` element and:
///   1. Wraps the `field="object"` child in an `<object>` slot.
///   2. Wraps the trailing bare `<name>` in a `<property>` slot.
///
/// `<call>` elements need no rewriting: PHP's tree-sitter places
/// the `<member>` callee as the first non-marker child, matching
/// the canonical `<call><member>...</member>...args</call>` shape
/// once the `<member>` itself is normalised.
fn php_wrap_member_call_slots(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{copy_source_location, get_attr, get_element_name};
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    let mut targets: Vec<XotNode> = Vec::new();
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some() {
            let name = get_element_name(xot, node);
            if matches!(name.as_deref(), Some("member") | Some("call")) {
                out.push(node);
            }
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut targets);
    for node in targets {
        let elem_name = get_element_name(xot, node);
        let is_member = elem_name.as_deref() == Some("member");
        // Find the field=object child (the receiver).
        let receiver = xot.children(node).find(|&c| {
            xot.element(c).is_some()
                && get_attr(xot, c, "field").as_deref() == Some("object")
        });
        // Skip if already canonical (has <object> slot child).
        let has_object_slot = xot.children(node).any(|c| {
            xot.element(c).is_some()
                && get_element_name(xot, c).as_deref() == Some("object")
        });
        if has_object_slot {
            continue;
        }
        let receiver = match receiver {
            Some(r) => r,
            None => continue,
        };

        // Wrap receiver in <object>.
        let object_id = xot.add_name("object");
        let object_slot = xot.new_element(object_id);
        copy_source_location(xot, receiver, object_slot);
        xot.insert_before(receiver, object_slot)?;
        xot.detach(receiver)?;
        xot.append(object_slot, receiver)?;

        // For <member>: also wrap the trailing bare <name> in
        // <property>. For <call>: leave the name bare —
        // `wrap_flat_call_member` will package it under a
        // synthetic <member> callee.
        if is_member {
            let name_node = xot.children(node).find(|&c| {
                xot.element(c).is_some()
                    && get_element_name(xot, c).as_deref() == Some("name")
            });
            if let Some(name_node) = name_node {
                let property_id = xot.add_name("property");
                let property_slot = xot.new_element(property_id);
                copy_source_location(xot, name_node, property_slot);
                xot.insert_before(name_node, property_slot)?;
                xot.detach(name_node)?;
                xot.append(property_slot, name_node)?;
            }
        }
    }
    Ok(())
}

/// Walk every `<use>` element and restructure to the shape:
///   `use App\Base`           → `<use><path><name>App</name></path><name>Base</name></use>`
///   `use App\Foo as Bar`     → `<use[alias]><path><name>App</name></path><name>Foo</name><alias><name>Bar</name></alias></use>`
///   `use App\{First, Second}` → `<use[group]><path><name>App</name></path><use><name>First</name></use><use><name>Second</name></use></use>`
///   `use function App\foo`   → `<use[function]><path><name>App</name></path><name>foo</name></use>`
///
/// Operates on the post-rule tree where children are already mostly
/// `<name>` siblings (qualified_name / namespace_use_clause flattened).
/// Detects markers from text content (`as`, `function`, `const`, `\`,
/// `;`, `,`) and rebuilds the structural slots.
fn php_restructure_use(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{XotWithExt, get_element_name};
    use super::output::TractorNode::{Alias, Group, Function as PhpFunction, Path, Const};

    // Collect `<use>` nodes first to avoid mutating during walk.
    let mut targets: Vec<XotNode> = Vec::new();
    collect_named_elements(xot, root, "use", &mut targets);

    for use_node in targets {
        // 1. Determine flavor from preceding bare-keyword text.
        //    `use function App\foo` → flavor Function; `use const App\BAR` → flavor Const.
        let mut flavor: Option<super::output::TractorNode> = None;
        let mut has_alias_keyword = false;
        for child in xot.children(use_node).collect::<Vec<_>>() {
            let Some(text) = xot.text_str(child) else { continue };
            for tok in text.split_whitespace() {
                match tok {
                    "function" => flavor = Some(PhpFunction),
                    "const" => flavor = Some(Const),
                    "as" => has_alias_keyword = true,
                    _ => {}
                }
            }
        }

        // 2. Detect group form: child is `<body>` containing names.
        let group_body = xot.children(use_node).find(|&c| {
            get_element_name(xot, c).as_deref() == Some("body")
        });

        // 3. Strip ALL noise text leaves on use_node.
        for child in xot.children(use_node).collect::<Vec<_>>() {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            }
        }

        // 4. Collect remaining element children in document order.
        let element_children: Vec<XotNode> = xot.children(use_node)
            .filter(|&c| xot.element(c).is_some())
            .collect();

        // 5. Branch on group vs flat.
        if let Some(body) = group_body {
            // Group form. Element children before <body> = path segments.
            let path_segments: Vec<XotNode> = element_children.iter()
                .copied()
                .take_while(|&c| c != body)
                .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
                .collect();

            // Detach body's noise text leaves; remaining elements are leaf
            // names (the {First, Second} list).
            for child in xot.children(body).collect::<Vec<_>>() {
                if xot.text_str(child).is_some() {
                    xot.detach(child)?;
                }
            }
            let leaf_names: Vec<XotNode> = xot.children(body)
                .filter(|&c| xot.element(c).is_some())
                .collect();

            // Build <path> from path_segments (clone-and-detach each segment
            // into a fresh <path> wrapper).
            let path_node = if !path_segments.is_empty() {
                let path_elt = xot.add_name(Path.as_str());
                let path_node = xot.new_element(path_elt);
                xot.append(use_node, path_node)?;
                for seg in path_segments {
                    xot.detach(seg)?;
                    xot.append(path_node, seg)?;
                }
                Some(path_node)
            } else {
                None
            };
            let _ = path_node;

            // For each leaf name, create a child `<use><name>X</name></use>`.
            for name in leaf_names {
                let inner_use_elt = xot.add_name("use");
                let inner_use = xot.new_element(inner_use_elt);
                xot.append(use_node, inner_use)?;
                xot.detach(name)?;
                xot.append(inner_use, name)?;
            }

            // Detach the now-empty body wrapper.
            xot.detach(body)?;

            // Add [group] marker.
            xot.with_prepended_marker(use_node, Group)?;
        } else {
            // Flat form: handle alias if present.
            // Element children all start as <name>X</name>.
            let names: Vec<XotNode> = element_children.iter()
                .copied()
                .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
                .collect();

            if has_alias_keyword && names.len() >= 2 {
                // Last <name> is the alias; preceding ones are path + leaf.
                let alias_name = *names.last().unwrap();
                let path_and_leaf = &names[..names.len() - 1];

                // path = all but last; leaf = last of path_and_leaf
                let leaf_idx = path_and_leaf.len() - 1;
                let path_segments = &path_and_leaf[..leaf_idx];
                let _leaf = path_and_leaf[leaf_idx];

                // Build <path> wrapping segments.
                if !path_segments.is_empty() {
                    let path_elt = xot.add_name(Path.as_str());
                    let path_node = xot.new_element(path_elt);
                    xot.insert_before(path_and_leaf[0], path_node)?;
                    for &seg in path_segments {
                        xot.detach(seg)?;
                        xot.append(path_node, seg)?;
                    }
                }
                // Wrap alias name in <alias>.
                let alias_elt = xot.add_name("aliased");
                let alias_node = xot.new_element(alias_elt);
                xot.insert_before(alias_name, alias_node)?;
                xot.detach(alias_name)?;
                xot.append(alias_node, alias_name)?;

                xot.with_prepended_marker(use_node, Alias)?;
            } else if names.len() >= 2 {
                // Plain multi-segment: all but last become <path>; last is leaf.
                let leaf_idx = names.len() - 1;
                let path_segments = &names[..leaf_idx];
                let path_elt = xot.add_name(Path.as_str());
                let path_node = xot.new_element(path_elt);
                xot.insert_before(path_segments[0], path_node)?;
                for &seg in path_segments {
                    xot.detach(seg)?;
                    xot.append(path_node, seg)?;
                }
            }
            // names.len() == 1 → bare leaf, leave as-is.
        }

        if let Some(f) = flavor {
            xot.with_prepended_marker(use_node, f)?;
        }
        let _ = has_alias_keyword;

        // Discard the description doc-comment about `Const` only.
        // The const flavor distinguishes from function flavor.
    }

    Ok(())
}
