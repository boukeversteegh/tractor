//! Rust-specific `post_transform` pipeline + helpers.
//!
//! Runs after `walk_transform` to apply Rust-specific structural
//! rewrites (chain inversion pre-pass, `use` restructure, lifetime
//! name normalization) and the shared cross-language passes
//! (chain inversion, conditional collapse, expression-position
//! wrap, list distribution).
//!
//! Moved out of `tractor/src/languages/mod.rs` iter 329 per user
//! direction: per-language transform code belongs with the language
//! module, not in the generic registry.

use xot::{Xot, Node as XotNode};

use crate::languages::{collapse_conditionals, collect_named_elements};

/// Rust-specific `post_transform` orchestrator. Runs after the
/// generic `walk_transform` to apply chain inversion, conditional
/// collapse, `use` restructure, expression-position wrap, multi-
/// target tagging, list distribution, and Rust's specific
/// pre/post-passes (`field_expression` normalize, lifetime name
/// normalize).
///
/// The expression-position pass runs after `collapse_conditionals` so
/// the `then`/`else` slots produced by the conditional collapse get
/// hosts too.
pub fn rust_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    // Rust's `field_expression` (`obj.foo`) renames to `<field>`
    // alongside FieldDeclaration / FieldInitializer / etc. The
    // chain inverter expects the canonical `<member>` shape, so
    // pre-pass converts the field-expression flavor to canonical
    // first.
    rust_normalize_field_expression(xot, root)?;
    crate::transform::chain_inversion::invert_chains_in_tree(xot, root)?;
    collapse_conditionals(xot, root)?;
    crate::transform::wrap_expression_positions(
        xot,
        root,
        // Slot wrappers that contain a single expression operand.
        // `then`/`else` are block bodies (statement sequences) and
        // must not be wrapped — their children carry their own
        // statement-level hosts via `expression_statement`.
        // `return` holds the optional return value as its first
        // element child; wrap so `<return>/<expression>/...` is the
        // uniform shape (no value -> no host, the wrap pass is a
        // no-op for empty returns).
        &["value", "condition", "left", "right", "return"],
    )?;
    rust_restructure_use(xot, root)?;
    crate::transform::tag_multi_target_expressions(xot, root)?;
    crate::transform::tag_multi_same_name_children(xot, root, &["type", "pattern", "string", "import"])?;
    // Rust `if let ... && let ...` chains produce `<condition>` with
    // multiple `<expression>` siblings (one per let-clause).
    crate::transform::tag_multi_role_children(
        xot, root,
        &[
            ("condition", "expression"),
            // Rust `use std::fmt::{Display, Write as IoWrite}` —
            // `<use[group]>` parent with multiple inner `<use>`
            // siblings (one per imported entity). Tag with
            // `list="uses"` so JSON renders as `uses: [...]` array
            // rather than colliding on the singleton `use` key
            // and overflowing into `children`.
            ("use", "use"),
            // Tuple/slice patterns `(a, b, c)` and `[a, b, c]` produce
            // `<pattern[tuple]>` / `<pattern[slice]>` with multiple
            // `<name>` binding siblings. Per Principle #19 they're
            // role-uniform — each is a positional binding.
            ("pattern", "name"),
            // Or-patterns `Shape::Dot | Shape::Square` mix `<path>`,
            // `<pattern>`, and `<int>` siblings. Tag each kind that
            // can appear so JSON renders consistently.
            ("pattern", "int"),
            ("pattern", "string"),
            ("pattern", "path"),
            // Rust macro invocations `vec![1, 2, 3]`, `println!("hi
            // {}", x)`, etc. — `<macro>` parent contains the macro
            // name plus heterogeneous args. Targeted role tags
            // replace the bulk distribute on `"macro"` (removed iter
            // 310 — was wrapping single-name / single-int / single-
            // string cases in 1-elem JSON arrays).
            ("macro", "name"),
            ("macro", "int"),
            ("macro", "string"),
            // `matches!(x, A | B)` and similar pattern-matching
            // macros expand into multiple `<arm>` children.
            ("macro", "arm"),
            // Rust array literals `[1, 2, 3]` and
            // `[counter, counter * 2, counter * 3]`. Iter 327 dropped
            // `"array"` from the bulk distribute (was wrapping the
            // singleton `<name>` in `[counter, counter*2, counter*3]`
            // in a 1-elem JSON array). Targeted role tags here cover
            // multi-cardinality element types in the blueprint.
            ("array", "int"),
            ("array", "binary"),
            // IR-pipeline additions for Rust (cover multi-cardinality
            // children that overflow $children otherwise):
            ("attribute", "name"),
            ("const", "name"),
            ("static", "name"),
            ("extends", "type"),
            ("enum", "variant"),
            ("type", "name"),
            ("object", "call"),
            ("call", "int"),
            ("call", "name"),
            ("for", "name"),
            ("expression", "name"),
            ("parameter", "name"),
        ],
    )?;
    crate::transform::flatten_nested_paths(xot, root)?;
    crate::transform::strip_body_braces(xot, root, &["body", "block"])?;
    rust_normalize_lifetime_names(xot, root)?;
    crate::transform::distribute_member_list_attrs(
        // `"array"` removed iter 327 — see targeted role tags above.
        xot, root, &["body", "block", "file", "tuple", "list", "dict", "repetition"],
    )?;
    Ok(())
}

/// Rust use-position lifetimes `&'a str` produce `<lifetime>` elements
/// whose inner identifier text retains the `'` sigil — render
/// declaration-position lifetimes (`<'a>` in generics) the same way.
/// Strip a leading `'` from any `<name>` text whose parent is a
/// `<lifetime>`. Idempotent.
fn rust_normalize_lifetime_names(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::*;
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    let mut targets: Vec<XotNode> = Vec::new();
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some()
            && get_element_name(xot, node).as_deref() == Some("lifetime")
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut targets);
    for lifetime in targets {
        let name_children: Vec<XotNode> = xot.children(lifetime)
            .filter(|&c| {
                xot.element(c).is_some()
                    && get_element_name(xot, c).as_deref() == Some("name")
            })
            .collect();
        for name in name_children {
            if let Some(text) = get_text_content(xot, name) {
                let trimmed = text.trim_start_matches('\'');
                if trimmed.len() != text.len() {
                    xot.with_only_text(name, trimmed)?;
                }
            }
        }
    }
    Ok(())
}

/// Pre-pass for chain inversion: convert Rust `field_expression`-derived
/// `<field>` elements to canonical `<member>`/`<object>`/`<property>`.
///
/// Rust emits:
///   `<field><value><expression>RECEIVER</expression></value><name>X</name></field>`
///
/// The chain inverter wants:
///   `<member><object>RECEIVER</object><property><name>X</name></property></member>`
///
/// Identifies the field-expression flavor by tree-sitter `kind` attribute
/// (Rust's other `<field>` uses — declarations, initializers — share
/// the element name but have different kinds). Skips non-matching
/// `<field>` elements. Idempotent.
fn rust_normalize_field_expression(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{copy_source_location, get_attr, get_element_name, rename};
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    let mut fields: Vec<XotNode> = Vec::new();
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some()
            && get_element_name(xot, node).as_deref() == Some("field")
            // Discriminate by tree-sitter kind. `<field>` is shared
            // by FieldDeclaration / FieldExpression /
            // FieldInitializer / ShorthandFieldInitializer (all
            // renamed to Field in rules.rs). Only the expression
            // flavour participates in member-access chains.
            && get_attr(xot, node, "kind").as_deref() == Some("field_expression")
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut fields);
    for field in fields {
        // Field-expression always has a <value> slot (the receiver).
        let value_slot = xot.children(field).find(|&c| {
            xot.element(c).is_some()
                && get_element_name(xot, c).as_deref() == Some("value")
        });
        let value_slot = match value_slot {
            Some(v) => v,
            None => continue,
        };
        // Rename element: <field> → <member>.
        rename(xot, field, "member");
        // Rename slot: <value> → <object>.
        rename(xot, value_slot, "object");
        // Unwrap the inner <expression> host so <object>RECV</object>
        // is direct (matches canonical shape — Python/Go don't have an
        // <expression> host inside <object>).
        let expr_inner = xot.children(value_slot).find(|&c| {
            xot.element(c).is_some()
                && get_element_name(xot, c).as_deref() == Some("expression")
        });
        if let Some(expr) = expr_inner {
            // Lift expression's children up into <object>.
            let children: Vec<XotNode> = xot.children(expr).collect();
            for c in children {
                xot.detach(c)?;
                xot.insert_before(expr, c)?;
            }
            xot.detach(expr)?;
        }
        // Wrap bare <name> in <property>.
        let name_node = xot.children(field).find(|&c| {
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
    Ok(())
}

/// Restructure every Rust `<use>` element into the unified shape
/// (per `imports-grouping.md`):
///
///   use std::collections::HashMap                  → <use><path><name>std</name><name>collections</name></path><name>HashMap</name></use>
///   use std::collections::HashSet as Set           → <use[alias]>...<name>HashSet</name><alias><name>Set</name></alias></use>
///   use std::collections::{HashMap, HashSet}       → <use[group]>...<use><name>HashMap</name></use><use><name>HashSet</name></use></use>
///   use std::fmt::self                             → <use[self]>...</use>
///   use std::fmt::*                                → <use[wildcard]>...</use>
///   pub use foo::bar                               → <use[reexport][pub]>...</use>
fn rust_restructure_use(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{XotWithExt, get_element_name};
    use super::output::TractorNode::{Alias, Group, Reexport, Self_, Wildcard};

    let mut targets: Vec<XotNode> = Vec::new();
    collect_named_elements(xot, root, "use", &mut targets);

    for use_node in targets {
        // Skip inner <use> elements that are children of a grouped <use>
        // (we may be re-walking after restructuring an outer one).
        if xot.parent(use_node)
            .and_then(|p| get_element_name(xot, p))
            .as_deref() == Some("use")
        {
            continue;
        }

        // 1. Inspect text leaves for keywords / sigils.
        let mut has_as = false;
        let mut has_wildcard = false;
        let mut has_reexport_keyword = false;
        for child in xot.children(use_node).collect::<Vec<_>>() {
            let Some(text) = xot.text_str(child) else { continue };
            for tok in text.split(|c: char| {
                c.is_whitespace() || matches!(c, ':' | '{' | '}' | ';' | ',')
            }) {
                match tok {
                    "as" => has_as = true,
                    "*" => has_wildcard = true,
                    _ => {}
                }
            }
            if text.contains("pub use") {
                has_reexport_keyword = true;
            }
        }
        // The `[pub]` marker on a use element implies a re-export.
        let has_pub_marker = xot.children(use_node)
            .any(|c| get_element_name(xot, c).as_deref() == Some("pub"));
        if has_pub_marker {
            has_reexport_keyword = true;
        }

        // 2. Note `<self>` element (e.g. `use std::fmt::self`).
        let self_child = xot.children(use_node)
            .find(|&c| get_element_name(xot, c).as_deref() == Some("self"));
        let has_self = self_child.is_some();
        if let Some(s) = self_child {
            xot.detach(s)?;
        }

        // 2b. BEFORE stripping noise text, capture which `<name>` pairs
        //     are joined by an `as` text node. This is the only signal
        //     we have that `Foo as Bar` belongs together inside a group
        //     `{X, Foo as Bar, Y}` — `use_as_clause` flattens its
        //     children, so the only remaining trace of pairing is the
        //     `as` text leaf between two adjacent name elements.
        let mut alias_pairs: Vec<(XotNode, XotNode)> = Vec::new();
        let children_seq: Vec<XotNode> = xot.children(use_node).collect();
        for window in children_seq.windows(3) {
            let (a, mid, b) = (window[0], window[1], window[2]);
            if get_element_name(xot, a).as_deref() == Some("name")
                && get_element_name(xot, b).as_deref() == Some("name")
            {
                if let Some(text) = xot.text_str(mid) {
                    if text.split_whitespace().any(|t| t == "as") {
                        alias_pairs.push((a, b));
                    }
                }
            }
        }

        // 3. Strip ALL noise text leaves on use_node.
        for child in xot.children(use_node).collect::<Vec<_>>() {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            }
        }

        // 4. Lift the trailing `<name>` out of the `<path>` IF this is
        //    a simple-leaf case (`use std::collections::HashMap`) or an
        //    alias case (`use std::collections::HashSet as Set` —
        //    which has the leaf inside path and `as Set` as a sibling
        //    name; we need both as siblings to wrap one as `<alias>`).
        //    DON'T lift for group / wildcard / self-only cases — the
        //    path-trailing segment IS a path segment there.
        let path_child = xot.children(use_node)
            .find(|&c| get_element_name(xot, c).as_deref() == Some("path"));
        if let Some(path) = path_child {
            // Flatten any nested `<path>` once.
            let inner_paths: Vec<XotNode> = xot.children(path)
                .filter(|&c| get_element_name(xot, c).as_deref() == Some("path"))
                .collect();
            for inner in inner_paths {
                let inner_children: Vec<_> = xot.children(inner).collect();
                for c in inner_children {
                    xot.detach(c)?;
                    xot.insert_before(inner, c)?;
                }
                xot.detach(inner)?;
            }
            // Strip path-internal noise.
            for child in xot.children(path).collect::<Vec<_>>() {
                if xot.text_str(child).is_some() {
                    xot.detach(child)?;
                }
            }
            // Count sibling names of path BEFORE the lift to classify the
            // variant.
            let pre_sibling_names = xot.children(use_node)
                .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
                .count();
            // Lift only when this is a simple-leaf (0 siblings + no
            // wildcard/self) OR an alias (has_as case where the alias
            // occupies one sibling slot but the leaf still lives inside
            // path).
            let should_lift = (!has_wildcard && !has_self && pre_sibling_names == 0)
                || (has_as && pre_sibling_names == 1);
            if should_lift {
                let path_names: Vec<XotNode> = xot.children(path)
                    .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
                    .collect();
                if path_names.len() >= 2 {
                    let leaf = *path_names.last().unwrap();
                    xot.detach(leaf)?;
                    if let Some(next) = xot.next_sibling(path) {
                        xot.insert_before(next, leaf)?;
                    } else if let Some(parent) = xot.parent(path) {
                        xot.append(parent, leaf)?;
                    }
                }
            }
        }

        // 5. Now the use_node has: optional <path>, then 0+ <name>
        //    siblings. The number of name siblings + has_as / has_self
        //    determines the variant.
        let leaf_names: Vec<XotNode> = xot.children(use_node)
            .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
            .collect();

        // Set of names that are the *alias* (second of an `as` pair).
        let alias_targets: std::collections::HashSet<XotNode> =
            alias_pairs.iter().map(|&(_, b)| b).collect();
        // Set of names that are the *original* (first of an `as` pair).
        let alias_originals: std::collections::HashSet<XotNode> =
            alias_pairs.iter().map(|&(a, _)| a).collect();

        let is_flat_alias_with_pair = has_as && alias_pairs.len() == 1
            && leaf_names.len() == 2
            && alias_pairs[0].0 == leaf_names[0]
            && alias_pairs[0].1 == leaf_names[1];
        // Flat alias when `use std::Foo as Bar` — original was inside
        // `<path>` so no name-name `as` pair was captured (the captured
        // adjacency was path-as-name). After step 4 lifted the path
        // leaf, we now have two name siblings. has_as + 2 names with
        // no captured pair = flat path-leaf alias.
        let is_flat_alias_path_form = has_as && alias_pairs.is_empty()
            && leaf_names.len() == 2;

        if is_flat_alias_with_pair || is_flat_alias_path_form {
            let alias_name = leaf_names[1];
            let alias_elt = xot.add_name("aliased");
            let alias_node = xot.new_element(alias_elt);
            xot.insert_before(alias_name, alias_node)?;
            xot.detach(alias_name)?;
            xot.append(alias_node, alias_name)?;
            xot.with_prepended_marker(use_node, Alias)?;
        } else if leaf_names.len() >= 2 || (leaf_names.len() >= 1 && has_self) {
            // Group form. For each leaf name that's NOT the second of an
            // alias pair, create an inner `<use>`. Pair-original names
            // get inner `<use[alias]>` wrappers that ALSO consume the
            // following alias-target name.
            let mut i = 0;
            while i < leaf_names.len() {
                let name = leaf_names[i];
                if alias_targets.contains(&name) {
                    // Already consumed by previous alias pair.
                    i += 1;
                    continue;
                }
                let inner_use_elt = xot.add_name("use");
                let inner_use = xot.new_element(inner_use_elt);
                xot.insert_before(name, inner_use)?;
                xot.detach(name)?;
                xot.append(inner_use, name)?;
                if alias_originals.contains(&name) {
                    // Find paired alias target and wrap in <alias>.
                    let paired = alias_pairs.iter()
                        .find(|&&(orig, _)| orig == name)
                        .map(|&(_, alias)| alias);
                    if let Some(alias_name) = paired {
                        let alias_elt = xot.add_name("aliased");
                        let alias_node = xot.new_element(alias_elt);
                        xot.append(inner_use, alias_node)?;
                        xot.detach(alias_name)?;
                        xot.append(alias_node, alias_name)?;
                        xot.with_prepended_marker(inner_use, Alias)?;
                    }
                }
                i += 1;
            }
            // If there was a `<self>` entry, add inner `<use[self]/>`.
            if has_self {
                let inner_use_elt = xot.add_name("use");
                let inner_use = xot.new_element(inner_use_elt);
                xot.append(use_node, inner_use)?;
                xot.with_prepended_marker(inner_use, Self_)?;
            }
            xot.with_prepended_marker(use_node, Group)?;
        } else if has_self && leaf_names.is_empty() {
            // Single self-import: `use std::fmt::self`.
            xot.with_prepended_marker(use_node, Self_)?;
        }

        if has_wildcard {
            xot.with_prepended_marker(use_node, Wildcard)?;
        }
        if has_reexport_keyword {
            xot.with_prepended_marker(use_node, Reexport)?;
        }
    }

    Ok(())
}
