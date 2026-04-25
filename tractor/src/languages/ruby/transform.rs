//! Ruby transform logic

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

use super::semantic::*;


/// Transform a Ruby AST node
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

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

        // Name wrappers - inline identifier text directly
        // Inline the single identifier/constant child into plain
        // text. Applies everywhere a `<name>` field wrapper wraps a
        // single renamable child — declarations (method/class/module)
        // AND references (singleton method, call receiver, etc.) —
        // so the design-doc "identifiers are a single <name> text
        // leaf" rule holds uniformly.
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
                // Also accept already-renamed <name> when walk order
                // leaves one around.
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

        _ => {
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }
    }
}

/// Map tree-sitter node kinds to semantic element names.
///
/// Second tuple element is an optional disambiguation marker — lets
/// the map declare inline that e.g. `string_array` renames to
/// `<array>` with a `<string/>` marker child so `//array[string]`
/// finds every `%w[…]` literal.
fn map_element_name(kind: &str) -> Option<(&'static str, Option<&'static str>)> {
    match kind {
        "program" => Some((PROGRAM, None)),
        "method" => Some((METHOD, None)),
        "class" => Some((CLASS, None)),
        "module" => Some((MODULE, None)),
        "if" => Some((IF, None)),
        "unless" => Some((UNLESS, None)),
        // Ruby's tree-sitter nests `elsif` chains (each `elsif`/`else`
        // lives inside the previous `elsif`). The post-transform in
        // `languages/mod.rs` lifts them to flat children of `<if>` per
        // the cross-cutting conditional shape; here we just rename.
        "elsif" => Some((ELSE_IF, None)),
        "else" => Some((ELSE, None)),
        "case" => Some((CASE, None)),
        "while" => Some((WHILE, None)),
        "until" => Some((UNTIL, None)),
        "for" => Some((FOR, None)),
        "begin" => Some((BEGIN, None)),
        "rescue" => Some((RESCUE, None)),
        "ensure" => Some((ENSURE, None)),
        "call" => Some((CALL, None)),
        "method_call" => Some((CALL, None)),
        "assignment" => Some((ASSIGN, None)),
        "binary" => Some((BINARY, None)),
        "string" => Some((STRING, None)),
        "integer" => Some((INT, None)),
        "float" => Some((FLOAT, None)),
        "symbol" => Some((SYMBOL, None)),
        "array" => Some((ARRAY, None)),
        "hash" => Some((HASH, None)),
        "operator_assignment" => Some((ASSIGN, None)),
        "break_statement" => Some((BREAK, None)),
        "continue_statement" | "next_statement" => Some((CONTINUE, None)),
        // Percent-literal arrays — %w[…] gives a string_array, %i[…]
        // gives a symbol_array. Both collapse to <array> with a shape
        // marker so the element kind is uniform but queryable.
        "string_array" => Some((ARRAY, Some(STRING))),
        "symbol_array" => Some((ARRAY, Some(SYMBOL))),
        // Splat parameters — `*args` vs `**kwargs` distinguished by
        // list/dict marker, matching Python's shape.
        "splat_parameter" => Some((SPREAD, Some(LIST))),
        "hash_splat_parameter" => Some((SPREAD, Some(DICT))),
        // `key:` keyword parameter — a parameter variant; the marker
        // lets us find every keyword parameter without matching on text.
        "keyword_parameter" => Some((PARAMETER, Some(KEYWORD))),
        // `&block` capture — identifies the block parameter as a
        // <parameter> with <block/> marker.
        "block_parameter" => Some((PARAMETER, Some(BLOCK))),
        // `arg = default` — optional parameter shape.
        "optional_parameter" => Some((PARAMETER, Some(DEFAULT))),
        // `do … end` block — collapse to <block> with a <do/> marker
        // so `//block[do]` finds the do-style, `//block[brace]` the
        // `{ … }` style (once we add it).
        "do_block" => Some((BLOCK, Some(DO))),
        // `begin … end` — explicit Ruby block.
        "begin_block" => Some((BLOCK, Some(BEGIN))),
        // Splat call-site args — `*args` / `**kwargs` distinguished by
        // list/dict marker, matching the parameter shape above.
        "splat_argument" => Some((SPREAD, Some(LIST))),
        "hash_splat_argument" => Some((SPREAD, Some(DICT))),
        // `:"dyn#{foo}"` delimited symbol — shape-marker the collapse
        // so `//symbol[delimited]` finds them.
        "delimited_symbol" => Some((SYMBOL, Some(DELIMITED))),
        // `rescue => e` — the `=> e` binding the exception.
        "exception_variable" => Some((VARIABLE, None)),
        // `class << self` — singleton class body.
        "singleton_class" => Some((CLASS, Some(SINGLETON))),
        // `def self.foo` — singleton method.
        "singleton_method" => Some((METHOD, Some(SINGLETON))),
        // `a, b, c = …` — LHS of a multi-assignment.
        "left_assignment_list" => Some((LEFT, None)),
        // `*rest = …` — the splat position on the LHS.
        "rest_assignment" => Some((SPREAD, None)),
        // `->(...) { ... }` — Ruby lambdas; flatten the parameter list.
        _ => None,
    }
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
