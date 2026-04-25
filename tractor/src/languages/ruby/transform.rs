//! Ruby transform logic

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

use super::semantic::*;


/// Transform a Ruby AST node.
///
/// Dispatch is split in two:
///   1. If the node carries a `kind` attribute (set by the builder from
///      the original tree-sitter kind), match on that — it never changes
///      mid-walk, so an arm like `"identifier"` always wins.
///   2. Otherwise the node is a builder-inserted wrapper (e.g. the
///      `<name>` field wrapper) — match on the element name for the
///      few wrappers we need to handle.
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(kind) = get_kind(xot, node) {
        match kind.as_str() {
            "body_statement"
            | "parenthesized_statements"
            | "block_body"
            | "heredoc_content"
            | "heredoc_beginning"
            | "heredoc_body"
            | "heredoc_end"
            | "hash_key_symbol"
            | "block_parameters"
            // `->(...)` lambda parameter list — grouping wrapper; flatten
            // so the parameters bubble up to the `<lambda>`.
            | "lambda_parameters" => Ok(TransformAction::Flatten),

            // Hash/keyword parameter syntax handled via map_element_name
            // with markers — `**kwargs` → `<spread><dict/>`, `key:` → parameter+keyword.

            // Trailing `if` / `unless` modifier — still a conditional,
            // same vocabulary as the full form.
            "if_modifier" => {
                rename(xot, node, IF);
                Ok(TransformAction::Continue)
            }
            "unless_modifier" => {
                rename(xot, node, UNLESS);
                Ok(TransformAction::Continue)
            }
            "while_modifier" => {
                rename(xot, node, WHILE);
                Ok(TransformAction::Continue)
            }
            "until_modifier" => {
                rename(xot, node, UNTIL);
                Ok(TransformAction::Continue)
            }

            // Ruby instance / class / global variables (`@x`, `@@y`, `$z`)
            // are distinct node kinds in the grammar but they're all
            // "variable references" at the semantic layer. Render as
            // `<name>` — the leading sigil survives as text so the
            // source is preserved. A future refinement could add a
            // `<instance/>` / `<class/>` / `<global/>` marker child.
            "instance_variable" | "class_variable" | "global_variable" => {
                rename(xot, node, NAME);
                Ok(TransformAction::Continue)
            }

            // String internals — grammar wrappers around the literal
            // text. Flatten so `<string>` reads as text + interpolations
            // (Principle #12).
            "string_content"
            | "escape_sequence"
            | "simple_symbol"
            | "bare_string"
            | "bare_symbol" => Ok(TransformAction::Flatten),

            // Flat lists (Principle #12)
            "method_parameters" => {
                distribute_field_to_children(xot, node, "parameters");
                Ok(TransformAction::Flatten)
            }
            "argument_list" => {
                distribute_field_to_children(xot, node, "arguments");
                Ok(TransformAction::Flatten)
            }

            // Ruby's grammar has no type_identifier — every identifier is a
            // value reference, so the rename is unconditional. Matches Python
            // and the rest of the languages on the value-namespace side
            // (Principle #14).
            "identifier" => {
                rename(xot, node, NAME);
                Ok(TransformAction::Continue)
            }

            // Comments — Ruby uses `#` for line comments. Multiline
            // begin/end style comments are out of scope for now (rare
            // in modern Ruby and not part of this classifier).
            "comment" => {
                rename(xot, node, COMMENT);
                static CLASSIFIER: crate::languages::comments::CommentClassifier =
                    crate::languages::comments::CommentClassifier { line_prefixes: &["#"] };
                CLASSIFIER.classify_and_group(xot, node, TRAILING, LEADING)
            }

            _ => {
                apply_rename(xot, node, &kind)?;
                Ok(TransformAction::Continue)
            }
        }
    } else {
        // Builder-inserted wrapper (no `kind` attribute) — dispatch
        // on the element name for the few wrappers we need to handle.
        let name = get_element_name(xot, node).unwrap_or_default();
        match name.as_str() {
            // `<name>` field wrapper — inline the single identifier/
            // constant child into plain text. Applies everywhere a
            // `<name>` field wrapper wraps a single renamable child
            // — declarations (method/class/module) AND references
            // (singleton method, call receiver, etc.) — so the
            // design-doc "identifiers are a single <name> text leaf"
            // rule holds uniformly.
            "name" => {
                let children: Vec<_> = xot.children(node).collect();
                let element_children: Vec<_> = children
                    .iter()
                    .copied()
                    .filter(|&c| xot.element(c).is_some())
                    .collect();
                if element_children.len() == 1 {
                    let child = element_children[0];
                    let child_name = get_element_name(xot, child).unwrap_or_default();
                    // `identifier` for methods, `constant` for classes/
                    // modules (Ruby uses constant for capitalized
                    // identifiers); `operator` for `def ==(other)` and
                    // friends — Ruby's tree-sitter grammar tags the
                    // operator token as an element inside `<name>`.
                    // Also accept already-renamed <name> when walk
                    // order leaves one around.
                    if matches!(child_name.as_str(), "identifier" | "constant" | "name" | "operator") {
                        if let Some(text) = get_text_content(xot, child) {
                            for c in children {
                                xot.detach(c)?;
                            }
                            let text_node = xot.new_text(&text);
                            xot.append(node, text_node)?;
                            return Ok(TransformAction::Done);
                        }
                    }
                }
                Ok(TransformAction::Continue)
            }
            _ => Ok(TransformAction::Continue),
        }
    }
}

/// Map tree-sitter node kinds to semantic element names.
///
/// Derived from `semantic::KINDS` — the catalogue is the single source
/// of truth, this is just the rename projection.
fn map_element_name(kind: &str) -> Option<(&'static str, Option<&'static str>)> {
    super::semantic::rename_target(kind)
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
        "type" => SyntaxCategory::Type,
        "raise" | "return" => SyntaxCategory::Keyword,
        "def" | "end" | "super" => SyntaxCategory::Keyword,
        _ => SyntaxCategory::Default,
    }
}

#[cfg(test)]
mod tests {
    use crate::languages::ruby::semantic::NODES;

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
