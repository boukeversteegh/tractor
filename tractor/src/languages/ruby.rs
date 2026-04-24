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
        "body_statement" => Ok(TransformAction::Flatten),

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
