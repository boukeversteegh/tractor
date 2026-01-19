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

        // Name wrappers - inline identifier text directly
        "name" => {
            if let Some(parent) = get_parent(xot, node) {
                let parent_kind = get_element_name(xot, parent).unwrap_or_default();
                if matches!(parent_kind.as_str(), "method" | "class" | "module") {
                    let children: Vec<_> = xot.children(node).collect();
                    for child in children {
                        if let Some(child_name) = get_element_name(xot, child) {
                            if child_name == "identifier" {
                                if let Some(text) = get_text_content(xot, child) {
                                    let all_children: Vec<_> = xot.children(node).collect();
                                    for c in all_children {
                                        xot.detach(c)?;
                                    }
                                    let text_node = xot.new_text(&text);
                                    xot.append(node, text_node)?;
                                    return Ok(TransformAction::Done);
                                }
                            }
                        }
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
        "if" | "unless" | "else" | "elsif" => SyntaxCategory::Keyword,
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
