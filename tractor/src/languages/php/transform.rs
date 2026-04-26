//! PHP transform logic.
//!
//! Applies the shared design principles:
//!   - Renames tree-sitter kinds to short, developer-friendly names.
//!   - Lifts visibility / static / final / abstract keywords to
//!     empty markers while preserving the source keyword as a
//!     dangling text sibling.
//!   - Flattens grammar wrappers (Principle #12) — parameter_list,
//!     arguments, declaration_list, property_element, ...
//!
//! Still rough — focuses on the most-visible constructs so queries
//! work uniformly. Refine as blueprint snapshots surface specifics.

use xot::{Xot, Node as XotNode};
use crate::transform::{TransformAction, helpers::*};
use crate::transform::operators::{prepend_op_element, is_operator_marker};
use crate::output::syntax_highlight::SyntaxCategory;

use super::semantic::*;


/// Transform a PHP AST node.
///
/// Dispatch is split in two:
///   1. If the node carries a `kind` attribute (set by the builder from
///      the original tree-sitter kind), match on that — it never changes
///      mid-walk, so an arm like `"identifier"` always wins.
///   2. Otherwise the node is a builder-inserted wrapper (e.g. the
///      `<name>` field wrapper) — match on the element name for the
///      few wrappers we need to handle.
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_kind(xot, node) {
        Some(k) => k,
        None => {
            // Builder-inserted wrapper (no `kind` attribute).
            let name = get_element_name(xot, node).unwrap_or_default();
            return match name.as_str() {
                // PHP emits `name` directly on identifiers — our field
                // wrappings already produce <name>foo</name>, so nothing
                // to rewrite here except collapsing wrappers that sit
                // inside a <name> field wrapper:
                // `<name><name>foo</name></name>` (from field+identifier
                // double-wrapping) and `<name><variable>$foo</variable></name>`
                // (from field-on-variable_name — tree-sitter tags `$foo`
                // as a `variable_name` kind, but in any field slot it's
                // still just the bound name, so the outer <name> should
                // be the text leaf).
                //
                // Multi-segment qualified names (`App\Blueprint`) are
                // flattened — each segment becomes a direct sibling of
                // the enclosing namespace / use / etc. (Principle #12).
                // This matches C#'s qualified_name handling.
                "name" => {
                    let children: Vec<_> = xot.children(node).collect();
                    let element_children: Vec<_> = children
                        .iter()
                        .copied()
                        .filter(|&c| xot.element(c).is_some())
                        .collect();
                    if element_children.len() == 1 {
                        let child = element_children[0];
                        // Match on the original tree-sitter kind (stable
                        // across walk order) and on post-rename element
                        // names for the `<name><name>…</name></name>` case.
                        let ts_kind = get_kind(xot, child);
                        let el_name = get_element_name(xot, child);
                        // If the single child is a `namespace_name` /
                        // `qualified_name`, that child will flatten into
                        // multiple segments + "\\" separators. Flattening
                        // the outer wrapper now hoists the segments to
                        // the enclosing namespace/use so each becomes a
                        // direct `<name>` sibling.
                        if matches!(
                            ts_kind.as_deref(),
                            Some("namespace_name") | Some("qualified_name"),
                        ) {
                            return Ok(TransformAction::Flatten);
                        }
                        let inlineable = matches!(
                            ts_kind.as_deref(),
                            Some("name") | Some("variable_name"),
                        ) || matches!(
                            el_name.as_deref(),
                            Some("name") | Some("variable"),
                        );
                        if inlineable {
                            let text = descendant_text(xot, child);
                            let trimmed = text.trim().to_string();
                            if !trimmed.is_empty() {
                                for c in children {
                                    xot.detach(c)?;
                                }
                                let text_node = xot.new_text(&trimmed);
                                xot.append(node, text_node)?;
                                return Ok(TransformAction::Done);
                            }
                        }
                    } else if element_children.len() > 1 {
                        // Multiple element children — this is a qualified
                        // name that flattened into segments + separators.
                        // Flatten the outer <name> wrapper so each segment
                        // becomes a direct child of the enclosing node.
                        return Ok(TransformAction::Flatten);
                    }
                    Ok(TransformAction::Continue)
                }
                _ => Ok(TransformAction::Continue),
            };
        }
    };

    match kind.as_str() {
        // Purely-grouping wrappers — Principle #12. Drop the
        // container so children become direct siblings of the
        // enclosing class / method / …
        "declaration_list"
        | "compound_statement"
        | "property_element"
        | "match_block"
        | "match_condition_list"
        | "namespace_name"
        | "namespace_use_clause"
        | "namespace_use_group"
        | "string_content"
        | "escape_sequence"
        | "array_element_initializer"
        // `attribute_group` = `#[Attr1, Attr2]` wrapper; `attribute_list` =
        // the list of attribute_group for a declaration. Both are pure
        // grouping wrappers — flatten so individual attributes surface as
        // direct siblings.
        | "attribute_group"
        | "attribute_list"
        // `anonymous_function_use_clause` = `use ($x, $y)` on a closure —
        // grouping wrapper for captured variables; flatten so the captured
        // names become direct siblings with their field role intact.
        | "anonymous_function_use_clause"
        // `declare_directive` = the `strict_types=1` bit inside
        // `declare(strict_types=1);` — wrapper around the assignment.
        | "declare_directive"
        // `enum_declaration_list` = the `{ … }` body of `enum E { … }` —
        // grouping wrapper, flatten so `case` entries surface as siblings.
        | "enum_declaration_list"
        => Ok(TransformAction::Flatten),

        // Expression statement / parenthesized expression —
        // grammar wrappers, flatten so children become siblings of
        // the enclosing node (Principle #12). Flatten is safer than
        // Skip for parenthesized expressions (the walker's Skip
        // path trips xot's text consolidation on nested ternaries).
        "expression_statement" => Ok(TransformAction::Skip),
        "parenthesized_expression" => Ok(TransformAction::Flatten),

        // PHP interpolated string — `"hello $name"` or `"x {$obj->y}"`.
        // Tree-sitter nests the interpolated expressions (variable_name /
        // member_access_expression / …) directly inside the string; every
        // other language we support wraps these in an `<interpolation>`
        // element so the shape is uniform. Match that shape here:
        // wrap every element child of the string in `<interpolation>`
        // so `//string/interpolation/name` works cross-language.
        //
        // Complex interpolation (`{$expr}`) keeps `{` / `}` in the
        // surrounding string text — absorbing them into the
        // interpolation element would require scanning adjacent text
        // tokens and is deferred. The existing delimiters still yield
        // a correct round-trip via `text_preservation`.
        "encapsed_string" => {
            // Tree-sitter PHP nests interpolated expressions (variable_name /
            // member_access_expression / …) directly inside the string,
            // alongside `string_content` / `escape_sequence` text-fragment
            // wrappers. To match the uniform cross-language shape
            // (`<string>…<interpolation>EXPR</interpolation>…</string>`),
            // wrap each real expression in an `<interpolation>`. Skip the
            // text-fragment kinds; those are just literal string text and
            // get flattened in their own handler.
            let children: Vec<_> = xot.children(node)
                .filter(|&c| xot.element(c).is_some())
                .collect();
            for child in children {
                let ts_kind = get_kind(xot, child);
                // Skip text fragments and already-renamed interpolation wrappers.
                if matches!(
                    ts_kind.as_deref(),
                    Some("string_content") | Some("string_value") | Some("escape_sequence")
                        | Some("text_interpolation") | None,
                ) {
                    continue;
                }
                let interp_name = xot.add_name("interpolation");
                let interp = xot.new_element(interp_name);
                copy_source_location(xot, child, interp);
                xot.insert_before(child, interp)?;
                xot.detach(child)?;
                xot.append(interp, child)?;
            }
            rename(xot, node, STRING);
            Ok(TransformAction::Continue)
        }

        // Qualified names (`App\Hello\Greeter`) collapse to a single
        // text leaf inside their enclosing <name> — same design as
        // C# qualified_name. The outer <name> field wrapper handles
        // the collapse; here we just flatten the inner wrapper so
        // its segments become siblings of the enclosing <name>,
        // which then consolidates.
        "qualified_name" => Ok(TransformAction::Flatten),

        // Comments — PHP supports `//` and `#` line comments and
        // `/* */` blocks. Tree-sitter emits all of them as a single
        // `comment` kind; the shared classifier handles trailing /
        // leading / floating + line-comment grouping. Both `//` and
        // `#` count as line-comment prefixes — runs of either family
        // (or a mix on adjacent lines) merge into one <comment>.
        "comment" => {
            rename(xot, node, COMMENT);
            static CLASSIFIER: crate::languages::comments::CommentClassifier =
                crate::languages::comments::CommentClassifier { line_prefixes: &["//", "#"] };
            CLASSIFIER.classify_and_group(xot, node, TRAILING, LEADING)
        }

        // Flat lists (Principle #12) — parameters and arguments
        // become direct siblings with field="parameters" / "arguments".
        "formal_parameters" => {
            distribute_field_to_children(xot, node, "parameters");
            Ok(TransformAction::Flatten)
        }
        "arguments" => {
            distribute_field_to_children(xot, node, "arguments");
            Ok(TransformAction::Flatten)
        }

        // Modifier wrappers. PHP's grammar gives us
        // `visibility_modifier`, `static_modifier`, `final_modifier`,
        // `abstract_modifier`, `readonly_modifier` — each a text
        // token like "public" / "static". Convert to empty markers
        // with the source keyword preserved as a dangling sibling.
        "visibility_modifier"
        | "static_modifier"
        | "final_modifier"
        | "abstract_modifier"
        | "readonly_modifier"
        | "class_modifier" => {
            if let Some(text) = get_text_content(xot, node) {
                let text = text.trim().to_string();
                if !text.is_empty() {
                    rename_to_marker(xot, node, &text)?;
                    insert_text_after(xot, node, &text)?;
                    return Ok(TransformAction::Done);
                }
            }
            Ok(TransformAction::Continue)
        }

        // Base class / implements — wrap the type reference in <type>
        // (Principle #14).
        "base_clause" => {
            rename(xot, node, EXTENDS);
            Ok(TransformAction::Continue)
        }
        "class_interface_clause" => {
            rename(xot, node, IMPLEMENTS);
            Ok(TransformAction::Continue)
        }

        // Binary / assignment / unary expressions — lift the operator.
        "binary_expression" | "assignment_expression" | "unary_op_expression" => {
            extract_operator(xot, node)?;
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }

        // Class members default to public when no visibility modifier
        // is written (PHP spec). Inject `<public/>` so the invariant
        // "every class member has an access marker" holds exhaustively
        // (Principle #9).
        "method_declaration" | "property_declaration" => {
            if !has_visibility_marker(xot, node) {
                prepend_empty_element(xot, node, PUBLIC)?;
            }
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }

        _ => {
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }
    }
}

/// Returns true if `node` has a PHP visibility modifier child.
/// Walk order: when we enter method/property_declaration, the
/// visibility_modifier child may still be raw (pre-rename) or already
/// transformed to a marker element — check both.
fn has_visibility_marker(xot: &Xot, node: XotNode) -> bool {
    for child in xot.children(node) {
        if xot.element(child).is_none() { continue; }
        let ts_kind = get_kind(xot, child);
        if ts_kind.as_deref() == Some("visibility_modifier") {
            return true;
        }
        if let Some(name) = get_element_name(xot, child) {
            if matches!(name.as_str(), "public" | "private" | "protected") {
                return true;
            }
        }
    }
    false
}

/// Apply `map_element_name` to a node: rename + prepend marker (if any).
fn apply_rename(xot: &mut Xot, node: XotNode, kind: &str) -> Result<(), xot::Error> {
    if let Some((new_name, marker)) = map_element_name(kind) {
        rename(xot, node, new_name);
        if let Some(m) = marker {
            prepend_empty_element(xot, node, m)?;
        }
    }
    Ok(())
}

fn extract_operator(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);
    let operator = texts.iter().find(|t| {
        !t.chars().all(|c| matches!(c, '(' | ')' | ',' | ';' | '{' | '}' | '[' | ']'))
    });
    if let Some(op) = operator {
        prepend_op_element(xot, node, op)?;
    }
    Ok(())
}

/// Map tree-sitter node kinds to semantic element names.
///
/// Derived from `semantic::KINDS` — the catalogue is the single source
/// of truth, this is just the rename projection.
fn map_element_name(kind: &str) -> Option<(&'static str, Option<&'static str>)> {
    super::semantic::rename_target(kind)
}

/// Map a transformed element name to a syntax category for highlighting.
///
/// Consults the per-name NODES table first (one source of truth);
/// falls back to cross-cutting rules for names not in NODES.
pub fn syntax_category(element: &str) -> SyntaxCategory {
    if let Some(spec) = super::semantic::spec(element) {
        return spec.syntax;
    }
    match element {
        // Raw tree-sitter kinds / builder wrappers not in NODES:
        "parameters" | "arguments" => SyntaxCategory::Keyword,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        _ => SyntaxCategory::Default,
    }
}

#[cfg(test)]
mod tests {
    use crate::languages::php::semantic::NODES;

    #[test]
    fn no_duplicate_node_names() {
        let mut names: Vec<&str> = NODES.iter().map(|n| n.name).collect();
        names.sort();
        let total = names.len();
        names.dedup();
        assert_eq!(names.len(), total, "duplicate NODES entry");
    }

    #[test]
    fn no_unused_role() {
        for n in NODES {
            assert!(
                n.marker || n.container,
                "<{}> is neither marker nor container — dead entry?",
                n.name,
            );
        }
    }
}
