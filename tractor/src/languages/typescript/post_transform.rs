//! TypeScript / JavaScript post_transform pipeline + helpers.
//!
//! Runs after `walk_transform` to apply structural rewrites: callee
//! unwrap (chain-inversion pre-pass), chain inversion, conditional
//! collapse, expression-position wrap, multi-target tagging, role
//! tagging, import restructure, brace strip, declarator flatten, list
//! distribution.
//!
//! Moved out of `tractor/src/languages/mod.rs` iter 330 per user
//! direction: per-language transform code belongs with the language
//! module, not in the generic registry.

use xot::{Xot, Node as XotNode};

use crate::languages::{collapse_conditionals, collect_named_elements};

/// TypeScript post-transform: collapse conditionals + wrap expression
/// positions in `<expression>` hosts (Principle #15).
pub fn typescript_post_transform(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    // Normalise the call shape so it matches the canonical right-deep
    // input expected by chain_inversion::extract_chain. TS wraps the
    // call's callee in `<callee>` (via FIELD_WRAPPINGS); unwrap it so
    // `<call>` directly contains the callee element (a `<member>` or
    // bare `<name>`/`<call>`/etc.). Same shape as Python/Go.
    typescript_unwrap_callee(xot, root)?;
    crate::transform::chain_inversion::invert_chains_in_tree(xot, root)?;
    collapse_conditionals(xot, root)?;
    crate::transform::wrap_expression_positions(
        xot,
        root,
        &["value", "condition", "left", "right", "return"],
    )?;
    // Multi-target / comma-expression assignments —
    // `total = (a, b)` produces `<right>` with multiple
    // `<expression>` siblings. Tag with `list="expressions"`
    // (mirrors Go iter pre-existing call). Without this TS
    // assignment-right siblings overflow.
    crate::transform::tag_multi_target_expressions(xot, root)?;
    crate::transform::tag_multi_same_name_children(xot, root, &["type", "pattern", "string", "import"])?;
    // TS function-type / object-type signatures: `<type>` parent with
    // multiple `<parameter>` (function type) or `<property>` (object
    // type) siblings — uniform-role children inside a role-MIXED
    // parent (since `<type>` is also used as a singleton type wrapper).
    // Targeted via tag_multi_role_children rather than bulk distribute.
    crate::transform::tag_multi_role_children(
        xot, root,
        &[
            ("type", "parameter"),
            ("type", "property"),
            // TS object literals: `{a: 1, b: 2}` → `<object>` parent
            // with multiple `<pair>` siblings.
            ("object", "pair"),
            // Multi-declarator (`let i = 0, j = 100`) keeps
            // `<declarator>` wrappers (per iter 264) so JSON
            // can render them as `declarators: [...]` array.
            // Single-declarator is flattened by
            // `flatten_single_declarator_children` and never
            // reaches this tag pass.
            ("variable", "declarator"),
            ("field", "declarator"),
            // C-style `for (...; ...; j--, i++) {}` — comma-
            // separated post-update produces multiple `<unary>`
            // siblings under `<for>`. Tag so JSON renders
            // `unaries: [...]` instead of overflowing to
            // `children`.
            ("for", "unary"),
            // Object destructuring with multiple aliased entries
            // (`{ a: aa, b: bb }`) produces `<pattern[object]>`
            // with multiple `<pair>` siblings. Mirrors iter 264's
            // `("object", "pair")` for object literals.
            ("pattern", "pair"),
            // TS template literals `` `${a}${b}` `` — `<template>`
            // parent with one or more `<interpolation>` chunks. Bulk-
            // distribute on `"template"` (removed iter 309) was
            // wrapping single-interp cases in 1-elem JSON arrays.
            ("template", "interpolation"),
            // TS array literals `[1, 2, 3]` and array destructure
            // patterns. Iter 324 dropped `"array"` from the bulk
            // distribute config (was wrapping singleton index/name in
            // array destructure patterns in 1-elem JSON arrays).
            // Targeted role tags below cover the multi-cardinality
            // element types in the blueprint (spread/number).
            ("array", "spread"),
            ("array", "number"),
            // IR-pipeline additions for TS (cover multi-cardinality
            // children that overflow $children otherwise):
            ("import", "spec"),
            ("spec", "name"),
            ("type", "name"),
            ("type", "type"),
            ("indexer", "type"),
            ("method", "parameter"),
            ("function", "parameter"),
            ("constructor", "parameter"),
            ("call", "name"),
            ("class", "extends"),
            ("class", "method"),
            ("class", "field"),
            ("interface", "method"),
            ("interface", "property"),
            ("template", "interpolation"),
            ("template", "string"),
            ("template", "unknown"),
            ("as", "name"),
            ("arrow", "parameter"),
            ("object", "call"),
            ("object", "name"),
            ("call", "interpolation"),
            ("call", "string"),
            ("switch", "arm"),
            ("pair", "name"),
        ],
    )?;
    typescript_restructure_import(xot, root)?;
    // Run AFTER restructure_import so the `<import>` group-form
    // element has its final inner `<import>` siblings.
    crate::transform::tag_multi_same_name_children(xot, root, &["import"])?;
    crate::transform::strip_body_braces(xot, root, &["body", "block", "then", "else"])?;
    // Single-declarator variable declarations flatten the
    // <declarator> wrapper. Multi-declarator (`let i = 0, j = 100`)
    // keeps wrappers so name↔value pairing is preserved per
    // declarator. Mirrors Java/C# iter 263.
    crate::transform::flatten_single_declarator_children(xot, root, &["variable", "field"])?;
    crate::transform::distribute_member_list_attrs(
        // `"array"` removed iter 324 — was wrapping singleton
        // `<index>`/`<name>` children of array-destructure patterns in
        // 1-elem JSON arrays. Targeted role tags above cover the
        // multi-cardinality cases (spread/number).
        xot, root, &["body", "block", "program", "tuple", "list", "dict", "hash", "repetition"],
    )?;
    Ok(())
}

/// Unwrap `<callee>` field-wrapper inside `<call>` so the call's
/// first element child is the actual callee (matching the canonical
/// right-deep input that `chain_inversion::extract_chain` expects).
///
/// FIELD_WRAPPINGS routes tree-sitter `field="function"` to
/// `<callee>X</callee>`, exposing the call target as a named slot.
/// For chain inversion this wrapper is in the way: the extractor
/// looks for the callee as the first non-marker child of `<call>`,
/// not nested under `<callee>`. Unwrapping post-build (and pre-
/// inversion) preserves the FIELD_WRAPPINGS contract for languages
/// that don't run chain inversion while letting TS adopt the
/// canonical shape.
fn typescript_unwrap_callee(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{get_element_name, XotWithExt};
    let root = if xot.is_document(root) {
        xot.document_element(root).unwrap_or(root)
    } else {
        root
    };
    let mut callees: Vec<XotNode> = Vec::new();
    fn collect(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some()
            && get_element_name(xot, node).as_deref() == Some("callee")
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut callees);
    for callee in callees {
        // Lift each child of <callee> up to the parent <call>, then
        // detach the now-empty <callee>.
        let children: Vec<XotNode> = xot.children(callee).collect();
        for child in children {
            xot.with_detach(child)?
                .with_insert_before(callee, child)?;
        }
        xot.with_detach(callee)?;
    }
    Ok(())
}

/// Restructure every TypeScript `<import>` element into the unified
/// shape (per `imports-grouping.md`).
fn typescript_restructure_import(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use crate::transform::helpers::{XotWithExt, get_element_name, get_text_content};
    use super::output::TractorNode::{
        Alias, Group, Namespace, Path, Sideeffect,
    };

    let mut targets: Vec<XotNode> = Vec::new();
    collect_named_elements(xot, root, "import", &mut targets);

    for import_node in targets {
        // Skip inner <import> children of an already-grouped import.
        if xot.parent(import_node)
            .and_then(|p| get_element_name(xot, p))
            .as_deref() == Some("import")
        {
            continue;
        }

        // 1. Identify structural children. Tree-sitter TS produces:
        //    - `<clause>` (import_clause: bindings)
        //    - `<string>` (path module specifier)
        //    plus noise text (`import`, `from`, `;`).
        let clause = xot.children(import_node)
            .find(|&c| get_element_name(xot, c).as_deref() == Some("clause"));
        let path_string = xot.children(import_node)
            .find(|&c| get_element_name(xot, c).as_deref() == Some("string"));

        // 2. Extract path text (strip surrounding quotes).
        let path_text = path_string
            .and_then(|s| get_text_content(xot, s))
            .map(|raw| raw.trim()
                .trim_start_matches('"').trim_end_matches('"')
                .trim_start_matches('\'').trim_end_matches('\'')
                .trim_start_matches('`').trim_end_matches('`')
                .to_string())
            .unwrap_or_default();

        // 3. Strip ALL direct text leaves and the path string element.
        for child in xot.children(import_node).collect::<Vec<_>>() {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            }
        }
        if let Some(s) = path_string {
            xot.detach(s)?;
        }

        // 4. Build new structure.
        if clause.is_none() {
            // No `<clause>` — could be:
            //  - side-effect: `import './x'` (only string)
            //  - TS legacy: `import x = require('y')` (has `<name>` directly)
            // Side-effect = no name child either; legacy keeps its <name>.
            let has_direct_name = xot.children(import_node)
                .any(|c| get_element_name(xot, c).as_deref() == Some("name"));
            if !path_text.is_empty() {
                let path_elt = xot.add_name(Path.as_str());
                let path_node = xot.new_element(path_elt);
                xot.append(import_node, path_node)?;
                let path_text_node = xot.new_text(&path_text);
                xot.append(path_node, path_text_node)?;
            }
            if !has_direct_name {
                xot.with_prepended_marker(import_node, Sideeffect)?;
            }
            continue;
        }
        let clause = clause.unwrap();

        // Append <path> (always, when clause is present).
        if !path_text.is_empty() {
            let path_elt = xot.add_name(Path.as_str());
            let path_node = xot.new_element(path_elt);
            xot.append(import_node, path_node)?;
            let path_text_node = xot.new_text(&path_text);
            xot.append(path_node, path_text_node)?;
        }

        // Inspect the clause's children to determine variant.
        let namespace_child = xot.children(clause)
            .find(|&c| get_element_name(xot, c).as_deref() == Some("namespace"));
        let imports_child = xot.children(clause)
            .find(|&c| get_element_name(xot, c).as_deref() == Some("imports"));

        if let Some(ns) = namespace_child {
            // `import * as ns from 'mod'`. Find the name inside <namespace>.
            let ns_name = xot.children(ns)
                .find(|&c| get_element_name(xot, c).as_deref() == Some("name"));
            if let Some(name) = ns_name {
                let alias_elt = xot.add_name("aliased");
                let alias_node = xot.new_element(alias_elt);
                xot.append(import_node, alias_node)?;
                xot.detach(name)?;
                xot.append(alias_node, name)?;
            }
            xot.detach(clause)?;
            xot.with_prepended_marker(import_node, Namespace)?;
            continue;
        }

        if let Some(imports) = imports_child {
            // Default name + group OR group only.
            // Default name: clause has a direct <name> child.
            let default_name = xot.children(clause)
                .find(|&c| get_element_name(xot, c).as_deref() == Some("name"));
            if let Some(d) = default_name {
                xot.detach(d)?;
                xot.append(import_node, d)?;
            }
            // Group: each <spec> child becomes inner <import>.
            for spec in xot.children(imports).filter(|&c|
                get_element_name(xot, c).as_deref() == Some("spec")
            ).collect::<Vec<_>>() {
                // Capture name-`as`-name pair if present.
                let names: Vec<XotNode> = xot.children(spec)
                    .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
                    .collect();
                let has_inner_as = xot.children(spec).any(|c|
                    xot.text_str(c).map(|t| t.split_whitespace().any(|tok| tok == "as"))
                        .unwrap_or(false)
                );
                // Build inner <import>.
                let inner_elt = xot.add_name("import");
                let inner = xot.new_element(inner_elt);
                xot.append(import_node, inner)?;
                if has_inner_as && names.len() == 2 {
                    let original = names[0];
                    let alias_name = names[1];
                    xot.detach(original)?;
                    xot.append(inner, original)?;
                    let alias_elt = xot.add_name("aliased");
                    let alias_node = xot.new_element(alias_elt);
                    xot.append(inner, alias_node)?;
                    xot.detach(alias_name)?;
                    xot.append(alias_node, alias_name)?;
                    xot.with_prepended_marker(inner, Alias)?;
                } else if let Some(&name) = names.first() {
                    xot.detach(name)?;
                    xot.append(inner, name)?;
                }
                xot.detach(spec)?;
            }
            xot.detach(clause)?;
            xot.with_prepended_marker(import_node, Group)?;
            continue;
        }

        // Default-only: `import def from 'mod'`. clause/<name>def</name>.
        let default_name = xot.children(clause)
            .find(|&c| get_element_name(xot, c).as_deref() == Some("name"));
        if let Some(d) = default_name {
            xot.detach(d)?;
            xot.append(import_node, d)?;
        }
        xot.detach(clause)?;
    }

    Ok(())
}
