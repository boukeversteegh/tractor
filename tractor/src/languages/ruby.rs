//! Ruby transform logic

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

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
        | "block_parameters" => Ok(TransformAction::Flatten),

        // Hash/keyword parameter syntax (`**kwargs` / `key:` params).
        "hash_splat_parameter" => {
            rename(xot, node, "spread");
            Ok(TransformAction::Continue)
        }
        "keyword_parameter" => {
            rename(xot, node, "parameter");
            Ok(TransformAction::Continue)
        }

        // Trailing `if` / `unless` modifier — still a conditional,
        // same vocabulary as the full form.
        "if_modifier" => {
            rename(xot, node, "if");
            Ok(TransformAction::Continue)
        }
        "unless_modifier" => {
            rename(xot, node, "unless");
            Ok(TransformAction::Continue)
        }
        "while_modifier" => {
            rename(xot, node, "while");
            Ok(TransformAction::Continue)
        }
        "until_modifier" => {
            rename(xot, node, "until");
            Ok(TransformAction::Continue)
        }

        // Ruby instance / class / global variables (`@x`, `@@y`, `$z`)
        // are distinct node kinds in the grammar but they're all
        // "variable references" at the semantic layer. Render as
        // `<name>` — the leading sigil survives as text so the
        // source is preserved. A future refinement could add a
        // `<instance/>` / `<class/>` / `<global/>` marker child.
        "instance_variable" | "class_variable" | "global_variable" => {
            rename(xot, node, "name");
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
            rename(xot, node, "name");
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
                // identifiers); also accept already-renamed <name>
                // when walk order leaves one around.
                if matches!(child_name.as_str(), "identifier" | "constant" | "name") {
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
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }
    }
}

fn map_element_name(kind: &str) -> Option<&'static str> {
    match kind {
        "program" => Some("program"),
        "method" => Some("method"),
        "class" => Some("class"),
        "module" => Some("module"),
        "if" => Some("if"),
        "unless" => Some("unless"),
        // Ruby's tree-sitter nests `elsif` chains (each `elsif`/`else`
        // lives inside the previous `elsif`). The post-transform in
        // `languages/mod.rs` lifts them to flat children of `<if>` per
        // the cross-cutting conditional shape; here we just rename.
        "elsif" => Some("else_if"),
        "else" => Some("else"),
        "case" => Some("case"),
        "while" => Some("while"),
        "until" => Some("until"),
        "for" => Some("for"),
        "begin" => Some("begin"),
        "rescue" => Some("rescue"),
        "ensure" => Some("ensure"),
        "call" => Some("call"),
        "method_call" => Some("call"),
        "assignment" => Some("assign"),
        "binary" => Some("binary"),
        "string" => Some("string"),
        "integer" => Some("int"),
        "float" => Some("float"),
        "symbol" => Some("symbol"),
        "array" => Some("array"),
        "hash" => Some("hash"),
        "operator_assignment" => Some("assign"),
        "break_statement" => Some("break"),
        "continue_statement" | "next_statement" => Some("continue"),
        _ => None,
    }
}

/// Map a transformed element name to a syntax category for highlighting
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        // Identifiers
        "name" => SyntaxCategory::Identifier,
        "type" => SyntaxCategory::Type,

        // Literals
        "string" => SyntaxCategory::String,
        "int" | "float" => SyntaxCategory::Number,
        "symbol" => SyntaxCategory::String,
        "true" | "false" | "nil" => SyntaxCategory::Keyword,

        // Keywords - declarations
        "class" | "module" | "method" => SyntaxCategory::Keyword,

        // Keywords - control flow
        "if" | "unless" | "else" | "else_if" => SyntaxCategory::Keyword,
        "case" | "when" => SyntaxCategory::Keyword,
        "while" | "until" | "for" => SyntaxCategory::Keyword,
        "begin" | "rescue" | "ensure" | "raise" => SyntaxCategory::Keyword,
        "return" | "break" | "next" | "redo" | "retry" => SyntaxCategory::Keyword,
        "yield" => SyntaxCategory::Keyword,

        // Keywords - other
        "def" | "end" | "do" => SyntaxCategory::Keyword,
        "self" | "super" => SyntaxCategory::Keyword,

        // Collections
        "array" | "hash" => SyntaxCategory::Type,

        // Functions/calls
        "call" => SyntaxCategory::Function,

        // Operators
        "op" => SyntaxCategory::Operator,
        "binary" | "unary" | "assign" => SyntaxCategory::Operator,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Structural elements - no color
        _ => SyntaxCategory::Default,
    }
}
