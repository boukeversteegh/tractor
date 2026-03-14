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
        "value" | "left" | "right" => Ok(TransformAction::Skip),
        // Use Flatten for term instead of Skip to avoid freed-node issue
        "term" => Ok(TransformAction::Flatten),

        // Flatten select_expression children into select
        "select_expression" => Ok(TransformAction::Flatten),

        // Remove keyword_* nodes - they're just SQL keywords (SELECT, FROM, etc.)
        k if k.starts_with("keyword_") => {
            detach(xot, node)?;
            Ok(TransformAction::Done)
        }

        // Remove op_unary_other (e.g., # prefix on temp tables) - the # is kept in text
        "op_unary_other" => {
            detach(xot, node)?;
            Ok(TransformAction::Done)
        }

        // Unary expressions - handle #temp_table references
        "unary_expression" => {
            transform_unary(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // Name wrappers - inline identifier text (with bracket stripping and @var detection)
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
                            if text.starts_with('@') {
                                // @variable → <var>variable</var>
                                let text_node = xot.new_text(&text[1..]);
                                xot.append(node, text_node)?;
                                rename(xot, node, "var");
                            } else {
                                let clean = strip_brackets(&text);
                                let text_node = xot.new_text(&clean);
                                xot.append(node, text_node)?;
                            }
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

        // BETWEEN expression - rename and clean up
        "between_expression" => {
            rename(xot, node, "between");
            Ok(TransformAction::Continue)
        }

        // Assignment (UPDATE SET Name = value) - extract operator
        "assignment" => {
            extract_operator(xot, node)?;
            rename(xot, node, "assign");
            Ok(TransformAction::Continue)
        }

        // Identifiers - classify based on context and content
        "identifier" => {
            transform_identifier(xot, node)?;
            Ok(TransformAction::Done)
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

/// Strip T-SQL bracket delimiters from identifier text: [dbo] → dbo
fn strip_brackets(text: &str) -> String {
    if text.starts_with('[') && text.ends_with(']') && text.len() >= 2 {
        text[1..text.len() - 1].to_string()
    } else {
        text.to_string()
    }
}

/// Transform an identifier node based on its content and context
fn transform_identifier(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let text = match get_text_content(xot, node) {
        Some(t) => t,
        None => {
            rename(xot, node, "name");
            return Ok(());
        }
    };

    // Check field attribute for special roles
    if let Some(field_val) = get_attr(xot, node, "field") {
        match field_val.as_str() {
            "alias" => {
                let clean = strip_brackets(&text);
                replace_text(xot, node, &clean);
                rename(xot, node, "alias");
                return Ok(());
            }
            "schema" => {
                let clean = strip_brackets(&text);
                replace_text(xot, node, &clean);
                rename(xot, node, "schema");
                return Ok(());
            }
            _ => {}
        }
    }

    // Classify by content prefix
    if text.starts_with('@') {
        // @variable → <var>variable</var>
        let var_name = &text[1..];
        replace_text(xot, node, var_name);
        rename(xot, node, "var");
    } else {
        // Regular identifier - strip brackets and rename to "name"
        let clean = strip_brackets(&text);
        replace_text(xot, node, &clean);
        rename(xot, node, "name");
    }

    Ok(())
}

/// Replace all text content of a node
fn replace_text(xot: &mut Xot, node: XotNode, new_text: &str) {
    // Remove existing text children
    let text_children: Vec<_> = xot.children(node)
        .filter(|&c| xot.text_str(c).is_some())
        .collect();
    for c in text_children {
        let _ = xot.detach(c);
    }
    let text_node = xot.new_text(new_text);
    let _ = xot.append(node, text_node);
}

/// Transform unary expressions - specifically handles #temp_table
fn transform_unary(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    // Check if this is a # prefix (temp table)
    let children: Vec<_> = xot.children(node).collect();
    let mut is_temp = false;
    for &child in &children {
        if let Some(child_name) = get_element_name(xot, child) {
            if child_name == "op_unary_other" {
                if let Some(text) = get_text_content(xot, child) {
                    if text.trim() == "#" {
                        is_temp = true;
                    }
                }
            }
        }
    }

    if is_temp {
        // Extract the operand field name and prefix with #
        for &child in &children {
            if let Some(field_val) = get_attr(xot, child, "field") {
                if field_val == "operand" {
                    // Get the identifier text from inside the field/name/identifier chain
                    if let Some(inner_text) = get_deep_identifier_text(xot, child) {
                        // Replace the whole unary expression with a temp_ref
                        let all_children: Vec<_> = xot.children(node).collect();
                        for c in all_children {
                            xot.detach(c)?;
                        }
                        let text_node = xot.new_text(&format!("#{}", inner_text));
                        xot.append(node, text_node)?;
                        rename(xot, node, "temp_ref");
                        return Ok(());
                    }
                }
            }
        }
    }

    Ok(())
}

/// Recursively find identifier text deep in a node tree
fn get_deep_identifier_text(xot: &Xot, node: XotNode) -> Option<String> {
    // Check if this node itself has text
    if let Some(name) = get_element_name(xot, node) {
        if name == "identifier" {
            return get_text_content(xot, node);
        }
    }
    // Search children
    for child in xot.children(node) {
        if xot.element(child).is_some() {
            if let Some(text) = get_deep_identifier_text(xot, child) {
                return Some(text);
            }
        }
    }
    None
}

fn map_element_name(kind: &str) -> Option<&'static str> {
    match kind {
        // Top-level
        "program" => Some("file"),
        "statement" => Some("statement"),

        // DML statements
        "select" => Some("select"),
        "insert" => Some("insert"),
        "delete" => Some("delete"),
        "update" => Some("update"),

        // Clauses
        "from" => Some("from"),
        "where" => Some("where"),
        "order_by" => Some("order_by"),
        "order_target" => Some("order_target"),
        "group_by" => Some("group_by"),
        "having" => Some("having"),
        "join" => Some("join"),
        "direction" => Some("direction"),

        // References and columns
        "relation" => Some("relation"),
        "object_reference" => Some("ref"),
        "field" => Some("column"),
        "column" => Some("col"),
        "all_fields" => Some("star"),

        // Literals and values
        "literal" => Some("literal"),
        "list" => Some("list"),

        // Functions/calls
        "invocation" => Some("call"),
        "function_body" => Some("body"),
        "function_arguments" | "function_argument" => Some("arg"),

        // Subqueries, CTEs, set operations
        "subquery" => Some("subquery"),
        "cte" => Some("cte"),
        "set_operation" => Some("union"),
        "exists" => Some("exists"),

        // Window functions
        "window_function" => Some("window"),
        "window_specification" => Some("over"),
        "partition_by" => Some("partition_by"),

        // CASE expression
        "case" => Some("case"),
        "when_clause" => Some("when"),

        // CAST
        "cast" => Some("cast"),

        // DDL
        "create_table" => Some("create_table"),
        "column_definitions" => Some("columns"),
        "column_definition" => Some("col_def"),

        // MERGE
        "merge" => Some("merge"),

        // Transactions
        "transaction" => Some("transaction"),

        // SET variable
        "set_statement" => Some("set"),

        // CREATE FUNCTION
        "create_function" => Some("create_function"),

        // GO batch separator
        "go_statement" => Some("go"),

        // EXEC
        "execute_statement" => Some("exec"),

        // ALTER TABLE
        "alter_table" => Some("alter_table"),
        "add_column" => Some("add_column"),

        // CREATE INDEX
        "create_index" => Some("create_index"),
        "index_fields" => Some("index_fields"),

        // Data types
        "int" => Some("int"),
        "varchar" => Some("varchar"),
        "nvarchar" => Some("nvarchar"),
        "datetime" => Some("datetime"),

        // Assignment
        "assignment" => Some("assign"),

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
        prepend_op_element(xot, node, op.trim())?;
    }
    Ok(())
}

/// Map a transformed element name to a syntax category for highlighting
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        // Identifiers
        "name" | "alias" | "schema" | "var" | "temp_ref" | "column" => SyntaxCategory::Identifier,

        // Literals
        "literal" => SyntaxCategory::String,

        // Keywords - statements and clauses
        "select" | "insert" | "update" | "delete" => SyntaxCategory::Keyword,
        "from" | "where" | "order_by" | "group_by" | "having" => SyntaxCategory::Keyword,
        "join" | "union" | "exists" | "merge" => SyntaxCategory::Keyword,
        "statement" | "create_table" | "alter_table" | "create_index" | "cte" => SyntaxCategory::Keyword,
        "create_function" | "exec" | "set" | "transaction" | "go" => SyntaxCategory::Keyword,
        "case" | "when" | "direction" => SyntaxCategory::Keyword,
        "star" => SyntaxCategory::Keyword,

        // Types
        "int" | "varchar" | "nvarchar" | "datetime" | "ref" => SyntaxCategory::Type,

        // Functions/calls
        "call" | "cast" | "window" => SyntaxCategory::Function,

        // Operators
        "op" => SyntaxCategory::Operator,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        "compare" | "between" | "assign" => SyntaxCategory::Operator,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Structural elements - no color
        _ => SyntaxCategory::Default,
    }
}
