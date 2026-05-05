//! Python post_transform pipeline + helpers.
//!
//! Runs after `walk_transform` to apply chain inversion, expression-
//! position wrap, multi-target tagging, role tagging, import
//! restructure, path flattening, brace strip, list distribution.
//!
//! Moved out of `tractor/src/languages/mod.rs` iter 330 per user
//! direction: per-language transform code belongs with the language
//! module, not in the generic registry.

use xot::{Xot, Node as XotNode};

use crate::languages::collect_named_elements;

/// Python post-transform: wrap expression positions in `<expression>`
/// hosts (Principle #15). Python doesn't run `collapse_conditionals`
/// because tree-sitter-python emits an explicit `elif_clause`.
pub fn python_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    // Inject `<public/>`/`<protected/>`/`<private/>` markers on
    // class-method `<function>` elements based on Python's name
    // convention (Principle #9). The imperative pipeline did this
    // during `function_definition` transform; for IR output we do
    // it as an XML post-pass since the IR doesn't know about
    // parent context during lowering.
    inject_python_visibility_markers(xot, root)?;
    // Invert right-deep `<member>`/`<call>` chains into nested
    // `<chain>` form (per `docs/design-chain-inversion.md`).
    // Python's tree already matches the canonical input shape:
    // `<call><member><object/><property/></member>...args</call>`
    // and `<member><object/><property/></member>`. Run early so
    // subsequent passes see the post-inversion shape.
    crate::transform::chain_inversion::invert_chains_in_tree(xot, root)?;
    crate::transform::wrap_expression_positions(
        xot,
        root,
        &["value", "condition", "left", "right", "return"],
    )?;
    crate::transform::tag_multi_target_expressions(xot, root)?;
    crate::transform::tag_multi_same_name_children(xot, root, &["type", "pattern", "string", "import"])?;
    crate::transform::tag_multi_role_children(
        xot, root,
        &[
            ("string", "interpolation"),
            // Python's `comparison_operator` doesn't tag operands
            // with field=left/right (unlike binary_operator), so
            // multi-name compare chains overflow without this.
            ("compare", "name"),
            // Same situation when both operands of a comparison
            // are member-access chains (`self.name == other.name`)
            // — both become <object[access]> siblings under
            // <compare> and collide on the singleton `object`
            // JSON key without this tag.
            ("compare", "object"),
            // Multi-value returns (`return a, b, c`) — after
            // `wrap_expression_positions` each becomes a sibling
            // <expression> direct child of <return>; tag them so
            // JSON renders as `expressions: [...]` instead of
            // colliding on the singleton `expression` key and
            // overflowing into `children`.
            ("return", "expression"),
            // Union patterns `case 1 | 2 | 3:` produce
            // `<pattern[union]>` with multiple `<int>` (or
            // `<string>`/`<name>`) siblings. Per Principle #19
            // they're role-uniform — each is one alternative
            // option. Mirrors Ruby alternative patterns.
            ("pattern", "int"),
            ("pattern", "string"),
            ("pattern", "name"),
            // Dict patterns `case {"k1": v1, "k2": v2}:` produce
            // `<pattern[dict]>` with multiple `<value>` siblings
            // (one per key-value pair). The `<string>` keys are
            // already list-tagged via `tag_multi_same_name_children`
            // (`<string>` is in the global whitelist); the `<value>`
            // children mirror that — role-uniform per Principle #19,
            // each value paired with the same-position key.
            ("pattern", "value"),
            // Python decorators stack on a function/class:
            // `@a\n@b\ndef f():`. Multiple `<decorator>` siblings
            // under `<function>` (or `<class>`) — role-uniform
            // (each decorates the same target).
            ("function", "decorator"),
            ("class", "decorator"),
            // Python multi-for generator `(x for x in xs for y in ys)`
            // produces `<generator>` with multiple `<left>` and
            // `<right>` siblings (one pair per for-clause). Per
            // Principle #19 each is a clause-position; tag with
            // `list="lefts"` / `list="rights"`.
            ("generator", "left"),
            ("generator", "right"),
            // Python interpolated f-strings: `<string>` parent with
            // one or more `<interpolation>` chunks. Bulk-distribute
            // on `"string"` (removed below iter 309) was wrapping
            // single-interp cases in 1-elem JSON arrays.
            ("string", "interpolation"),
        ],
    )?;
    python_restructure_imports(xot, root)?;
    // Run AFTER restructure_imports so the `<from>` element has its
    // final `<import>` siblings (the restructure pass rewires them).
    // `<from>`/`<import>` is tagged unconditionally — single-name
    // and multi-name imports both render as `imports: [...]` in
    // JSON. Per Principle #12, the `<import>` role is always a
    // list inside `<from>`; the cardinality discriminator used
    // elsewhere would split the JSON shape (`"import": {...}` vs
    // `"imports": [...]`) and force consumers to branch on count.
    python_tag_from_imports_uniform(xot, root)?;
    crate::transform::tag_multi_role_children(
        xot, root,
        &[
            // Python `with X as a, Y as b: ...` — `<with>` parent
            // with multiple `<value>` (as-clause) siblings.
            ("with", "value"),
            // Python `try: ... except A: ... except B: ...` — `<try>`
            // parent with multiple `<except>` siblings.
            ("try", "except"),
        ],
    )?;
    crate::transform::flatten_nested_paths(xot, root)?;
    crate::transform::strip_body_braces(xot, root, &["body"])?;
    crate::transform::wrap_relationship_targets_in_type(xot, root)?;
    crate::transform::distribute_member_list_attrs(
        xot, root, &["body", "module", "tuple", "list", "dict", "repetition"],
    )?;
    Ok(())
}

/// Tag every `<import>` child of `<from>` with `list="imports"`,
/// regardless of cardinality. Mirrors `tag_multi_role_children`'s
/// (`from`, `import`) entry but without the `>= 2` gate. Per
/// Principle #12 the `<import>` role inside `<from>` is always a
/// list; the cardinality-gated tag would split the JSON shape
/// (`"import": {...}` for single, `"imports": [...]` for multi) and
/// force consumers to branch on count.
fn python_tag_from_imports_uniform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{XotWithExt, get_attr, get_element_name};
    let mut froms: Vec<XotNode> = Vec::new();
    collect_named_elements(xot, root, "from", &mut froms);
    for from in froms {
        let kids: Vec<XotNode> = xot.children(from)
            .filter(|&c| {
                xot.element(c).is_some()
                    && get_element_name(xot, c).as_deref() == Some("import")
            })
            .collect();
        for k in kids {
            if get_attr(xot, k, "list").is_none() {
                xot.with_attr(k, "list", "imports");
            }
        }
    }
    Ok(())
}

/// Restructure Python `<import>` and `<from>` elements per the
/// imports-grouping shape: `<path>` for the module path, `<alias>` for
/// renamed bindings, inner `<import>` per imported entity inside `<from>`.
fn python_restructure_imports(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{XotWithExt, get_element_name};
    use super::output::TractorNode::{Alias, Path, Relative};

    // Handle `<import>` (plain `import X` and `import X as Y`).
    let mut imports: Vec<XotNode> = Vec::new();
    collect_named_elements(xot, root, "import", &mut imports);
    for imp in imports {
        // Skip if nested inside a <from> (we handle those separately
        // below — the outer pass already restructured them).
        if xot.parent(imp)
            .and_then(|p| get_element_name(xot, p))
            .as_deref() == Some("from")
        {
            continue;
        }
        // Capture name-`as`-name pair from text adjacency.
        let alias_pairs = python_alias_pairs(xot, imp);
        // Strip noise text (`import`, `as`, commas).
        for child in xot.children(imp).collect::<Vec<_>>() {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            }
        }
        // Now look at children. For dotted name `import a.b.c`, there
        // may be a wrapper `<name>` containing inner `<name>X</name>`
        // segments — flatten that into a `<path>`. For aliased
        // `import a.b as x`, alias_pairs has the (last_segment, alias)
        // pair captured.
        python_flatten_dotted_name(xot, imp)?;
        let names: Vec<XotNode> = xot.children(imp)
            .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
            .collect();

        if !alias_pairs.is_empty() && names.len() >= 2 {
            // Treat the last name as alias; rest become `<path>`.
            let alias_name = *names.last().unwrap();
            let path_segs = &names[..names.len() - 1];
            // Single-segment path also wraps in <path> for cross-language
            // consistency.
            if !path_segs.is_empty() {
                let path_elt = xot.add_name(Path.as_str());
                let path_node = xot.new_element(path_elt);
                xot.insert_before(path_segs[0], path_node)?;
                for &seg in path_segs {
                    xot.detach(seg)?;
                    xot.append(path_node, seg)?;
                }
            }
            let alias_elt = xot.add_name("aliased");
            let alias_node = xot.new_element(alias_elt);
            xot.insert_before(alias_name, alias_node)?;
            xot.detach(alias_name)?;
            xot.append(alias_node, alias_name)?;
            xot.with_prepended_marker(imp, Alias)?;
        } else if !names.is_empty() {
            // Plain dotted import: wrap all names in <path>.
            let path_elt = xot.add_name(Path.as_str());
            let path_node = xot.new_element(path_elt);
            xot.insert_before(names[0], path_node)?;
            for &seg in &names {
                xot.detach(seg)?;
                xot.append(path_node, seg)?;
            }
        }
    }

    // Handle `<from>`.
    let mut froms: Vec<XotNode> = Vec::new();
    collect_named_elements(xot, root, "from", &mut froms);
    for fnode in froms {
        // Look at text leaves to find the `import` keyword (separates
        // the module path from imported names) and any leading dots
        // (relative import marker).
        let mut import_kw_seen_at: Option<usize> = None;
        let mut has_relative = false;
        let mut has_relative_only = false;
        let children_seq: Vec<XotNode> = xot.children(fnode).collect();
        for (idx, child) in children_seq.iter().enumerate() {
            if let Some(text) = xot.text_str(*child) {
                let trimmed = text.trim();
                if trimmed.starts_with("from .") || trimmed == "from . import" {
                    has_relative = true;
                    if trimmed == "from . import" || trimmed == "from .. import" {
                        has_relative_only = true;
                    }
                }
                if trimmed.contains("import") {
                    import_kw_seen_at = Some(idx);
                }
            }
        }

        let alias_pairs = python_alias_pairs(xot, fnode);
        // Strip text noise.
        for child in xot.children(fnode).collect::<Vec<_>>() {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            }
        }
        python_flatten_dotted_name(xot, fnode)?;
        let names: Vec<XotNode> = xot.children(fnode)
            .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
            .collect();

        // Determine the boundary: how many leading names belong to the
        // module path. import_kw_seen_at tells us roughly where, but
        // since we stripped indices, count via text layout. Heuristic:
        //  - If all names are 0: nothing to do (relative-only).
        //  - Else module_path = first N names where N = total - num_imports.
        // We don't have an easy way to count num_imports without the
        // text layout. Fallback: assume the FIRST name is the module
        // (most common case `from X import a, b, c`); the rest are
        // imports. Aliases are tracked from alias_pairs.

        if names.is_empty() {
            if has_relative_only {
                xot.with_prepended_marker(fnode, Relative)?;
            }
            continue;
        }

        // For relative-only `from . import x`: all names are imports,
        // no module path. For `from .x import y`: first name is the
        // (relative) module, rest are imports.
        let path_count = if has_relative_only { 0 } else { 1 };
        let path_segs: Vec<XotNode> = names.iter().take(path_count).copied().collect();
        let import_names: Vec<XotNode> = names.iter().skip(path_count).copied().collect();

        // Build <path> from path_segs.
        if !path_segs.is_empty() {
            let path_elt = xot.add_name(Path.as_str());
            let path_node = xot.new_element(path_elt);
            xot.insert_before(path_segs[0], path_node)?;
            for &seg in &path_segs {
                xot.detach(seg)?;
                xot.append(path_node, seg)?;
            }
        }

        // Identify alias pair targets within import_names.
        let alias_target_set: std::collections::HashSet<XotNode> =
            alias_pairs.iter().map(|&(_, b)| b).collect();
        let alias_orig_pair: std::collections::HashMap<XotNode, XotNode> =
            alias_pairs.iter().map(|&(a, b)| (a, b)).collect();

        // Wrap each import-name in inner <import>; pair aliases.
        let mut idx = 0;
        while idx < import_names.len() {
            let name = import_names[idx];
            if alias_target_set.contains(&name) {
                idx += 1;
                continue;
            }
            let inner_imp_elt = xot.add_name("import");
            let inner_imp = xot.new_element(inner_imp_elt);
            xot.insert_before(name, inner_imp)?;
            xot.detach(name)?;
            xot.append(inner_imp, name)?;
            if let Some(&alias_name) = alias_orig_pair.get(&name) {
                let alias_elt = xot.add_name("aliased");
                let alias_node = xot.new_element(alias_elt);
                xot.append(inner_imp, alias_node)?;
                xot.detach(alias_name)?;
                xot.append(alias_node, alias_name)?;
                xot.with_prepended_marker(inner_imp, Alias)?;
            }
            idx += 1;
        }

        if has_relative {
            xot.with_prepended_marker(fnode, Relative)?;
        }
        let _ = import_kw_seen_at;
    }

    Ok(())
}

/// Capture (a, b) pairs of `<name>` siblings joined by an `as` text
/// node — used by the Python import restructure to identify alias
/// pairs inside `import x as y` / `from x import y as z`.
fn python_alias_pairs(xot: &Xot, node: XotNode) -> Vec<(XotNode, XotNode)> {
    use crate::transform::helpers::get_element_name;
    let mut out = Vec::new();
    let seq: Vec<XotNode> = xot.children(node).collect();
    for window in seq.windows(3) {
        let (a, mid, b) = (window[0], window[1], window[2]);
        if get_element_name(xot, a).as_deref() == Some("name")
            && get_element_name(xot, b).as_deref() == Some("name")
        {
            if let Some(text) = xot.text_str(mid) {
                if text.split_whitespace().any(|t| t == "as") {
                    out.push((a, b));
                }
            }
        }
    }
    out
}

/// Inject `<public/>`/`<protected/>`/`<private/>` markers onto
/// `<function>` elements that sit directly inside a `<class>` body
/// (Principle #9 — Python's name-convention visibility). Mirrors the
/// imperative pipeline's `function_definition` transform.
///
/// Rules:
///   - `__name__` (dunder, len > 4) → public
///   - `__name`                     → private
///   - `_name`                      → protected
///   - `name`                       → public
fn inject_python_visibility_markers(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{copy_source_location, get_element_name};

    // Walk; track nearest enclosing-class flag.
    fn walk(xot: &mut Xot, node: XotNode, in_class: bool) -> Result<(), xot::Error> {
        let name = get_element_name(xot, node);
        let is_class = name.as_deref() == Some("class");
        let is_function = name.as_deref() == Some("function");
        if is_function && in_class {
            // Find first child <name> with text content.
            let name_node = xot.children(node).find(|&c| {
                xot.element(c).is_some()
                    && get_element_name(xot, c).as_deref() == Some("name")
            });
            if let Some(name_node) = name_node {
                let text: String = xot.children(name_node)
                    .filter_map(|c| xot.text_str(c).map(|s| s.to_string()))
                    .collect();
                let trimmed = text.trim();
                let marker_name: Option<&'static str> = if trimmed.is_empty() {
                    None
                } else if trimmed.starts_with("__") && trimmed.ends_with("__") && trimmed.len() > 4 {
                    Some("public")
                } else if trimmed.starts_with("__") {
                    Some("private")
                } else if trimmed.starts_with('_') {
                    Some("protected")
                } else {
                    Some("public")
                };
                if let Some(m) = marker_name {
                    let already_has = xot.children(node).any(|c| {
                        get_element_name(xot, c).as_deref() == Some(m)
                    });
                    if !already_has {
                        let n = xot.add_name(m);
                        let elem = xot.new_element(n);
                        // Copy span attrs from the function node so
                        // the marker's location matches.
                        copy_source_location(xot, node, elem);
                        xot.prepend(node, elem)?;
                    }
                }
            }
        }
        // Recurse: a nested function is its own scope (in_class=false),
        // entering a class flips in_class=true regardless of nesting.
        let next_in_class = if is_function {
            false
        } else if is_class {
            true
        } else {
            in_class
        };
        let children: Vec<XotNode> = xot.children(node).collect();
        for c in children {
            if xot.element(c).is_some() {
                walk(xot, c, next_in_class)?;
            }
        }
        Ok(())
    }
    walk(xot, root, false)
}

/// Tree-sitter Python's `dotted_name` (e.g. `a.b.c`) gets wrapped in
/// the field `<name>` wrapper, producing `<name><name>a</name>"."<name>b</name>...</name>`.
/// Flatten any such inner `<name>` wrapper child of `node` so its
/// segments become direct children, ready for `<path>` wrapping.
fn python_flatten_dotted_name(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::get_element_name;
    let wrappers: Vec<XotNode> = xot.children(node)
        .filter(|&c| {
            get_element_name(xot, c).as_deref() == Some("name")
                && xot.children(c).any(|cc| {
                    get_element_name(xot, cc).as_deref() == Some("name")
                })
        })
        .collect();
    for wrapper in wrappers {
        let inner: Vec<XotNode> = xot.children(wrapper).collect();
        for c in inner {
            // Skip text "." separators inside the wrapper.
            if xot.text_str(c).is_some() {
                xot.detach(c)?;
                continue;
            }
            xot.detach(c)?;
            xot.insert_before(wrapper, c)?;
        }
        xot.detach(wrapper)?;
    }
    Ok(())
}
