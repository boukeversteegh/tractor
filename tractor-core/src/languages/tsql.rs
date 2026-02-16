//! T-SQL (Microsoft SQL Server) transform logic

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

/// Transform a T-SQL AST node
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        // Skip expression wrappers - flatten children up
        "term" | "value" | "left" | "right" => Ok(TransformAction::Skip),

        // Flatten select_expression children into select
        "select_expression" => Ok(TransformAction::Flatten),

        // Remove keyword_* nodes - they're just SQL keywords (SELECT, FROM, etc.)
        k if k.starts_with("keyword_") => {
            detach(xot, node)?;
            Ok(TransformAction::Done)
        }

        // Name wrappers - inline identifier text
        "name" => {
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
            Ok(TransformAction::Continue)
        }

        // Binary expressions - extract operator
        "binary_expression" => {
            extract_operator(xot, node)?;
            rename(xot, node, "compare");
            Ok(TransformAction::Continue)
        }

        "between_expression" => {
            rename(xot, node, "between");
            Ok(TransformAction::Continue)
        }

        // Identifiers - classify based on context
        "identifier" => {
            let classification = classify_identifier(xot, node);
            rename(xot, node, classification);
            Ok(TransformAction::Continue)
        }

        // Rename to standard tractor conventions
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
        "program" => Some("file"),
        "statement" => Some("statement"),
        "select" => Some("select"),
        "from" => Some("from"),
        "where" => Some("where"),
        "order_by" => Some("order_by"),
        "order_target" => Some("order_target"),
        "group_by" => Some("group_by"),
        "having" => Some("having"),
        "join" => Some("join"),
        "relation" => Some("relation"),
        "object_reference" => Some("ref"),
        "field" => Some("column"),
        "literal" => Some("literal"),
        "invocation" => Some("call"),
        "insert" => Some("insert"),
        "delete" => Some("delete"),
        "update" => Some("update"),
        "list" => Some("list"),
        "column" => Some("col"),
        "function_body" => Some("body"),
        "function_arguments" | "function_argument" => Some("arg"),
        "int" => Some("int"),
        _ => None,
    }
}

fn extract_operator(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);
    let operator = texts.iter().find(|t| {
        let trimmed = t.trim();
        !trimmed.is_empty()
            && !trimmed.chars().all(|c| matches!(c, '(' | ')' | ',' | ';' | '{' | '}' | '[' | ']' | '.'))
    });
    if let Some(op) = operator {
        prepend_element_with_text(xot, node, "op", op.trim())?;
    }
    Ok(())
}

fn classify_identifier(xot: &Xot, node: XotNode) -> &'static str {
    // Check field attribute for alias
    if let Some(field_val) = get_attr(xot, node, "field") {
        if field_val == "alias" {
            return "alias";
        }
    }

    let parent = match get_parent(xot, node) {
        Some(p) => p,
        None => return "name",
    };
    let parent_kind = get_element_name(xot, parent).unwrap_or_default();

    match parent_kind.as_str() {
        // Column references in lists
        "column" | "col" => "name",
        // Table/schema context
        "object_reference" | "ref" => "name",
        _ => "name",
    }
}

/// Map a transformed element name to a syntax category for highlighting
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        // Identifiers
        "name" => SyntaxCategory::Identifier,
        "alias" => SyntaxCategory::Identifier,

        // Literals
        "literal" => SyntaxCategory::String,

        // Keywords - statements
        "select" | "insert" | "update" | "delete" => SyntaxCategory::Keyword,
        "from" | "where" | "order_by" | "group_by" | "having" => SyntaxCategory::Keyword,
        "join" => SyntaxCategory::Keyword,
        "statement" => SyntaxCategory::Keyword,

        // Types
        "int" | "ref" => SyntaxCategory::Type,

        // Functions/calls
        "call" => SyntaxCategory::Function,

        // Operators
        "op" => SyntaxCategory::Operator,
        "compare" | "between" => SyntaxCategory::Operator,

        // Column references
        "column" => SyntaxCategory::Identifier,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Structural elements - no color
        _ => SyntaxCategory::Default,
    }
}
