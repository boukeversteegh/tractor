//! T-SQL (Microsoft SQL Server) transform logic

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

use semantic::*;

/// Semantic element names — tractor's T-SQL XML vocabulary after transform.
/// Tree-sitter kind strings (left side of `match` arms) stay as bare
/// literals — they are external vocabulary.
pub mod semantic {
    // Structural — containers.

    // Top-level
    pub const FILE: &str = "file";
    pub const STATEMENT: &str = "statement";

    // DML statements
    pub const SELECT: &str = "select";
    pub const INSERT: &str = "insert";
    pub const DELETE: &str = "delete";
    pub const UPDATE: &str = "update";

    // Clauses
    pub const FROM: &str = "from";
    pub const WHERE: &str = "where";
    pub const ORDER: &str = "order";
    pub const TARGET: &str = "target";
    pub const GROUP: &str = "group";
    pub const HAVING: &str = "having";
    pub const JOIN: &str = "join";
    pub const DIRECTION: &str = "direction";

    // References and columns
    pub const RELATION: &str = "relation";
    pub const REF: &str = "ref";
    pub const COLUMN: &str = "column";
    pub const COL: &str = "col";
    pub const STAR: &str = "star";

    // Literals and values
    pub const LITERAL: &str = "literal";
    pub const LIST: &str = "list";

    // Functions / calls
    pub const CALL: &str = "call";
    pub const BODY: &str = "body";
    pub const ARG: &str = "arg";

    // Subqueries, CTEs, set operations
    pub const SUBQUERY: &str = "subquery";
    pub const CTE: &str = "cte";
    pub const UNION: &str = "union";
    pub const EXISTS: &str = "exists";

    // Window functions
    pub const WINDOW: &str = "window";
    pub const OVER: &str = "over";
    pub const PARTITION: &str = "partition";

    // CASE expression
    pub const CASE: &str = "case";
    pub const WHEN: &str = "when";

    // CAST
    pub const CAST: &str = "cast";

    // DDL
    pub const CREATE: &str = "create";
    pub const COLUMNS: &str = "columns";
    pub const DEFINITION: &str = "definition";

    // MERGE
    pub const MERGE: &str = "merge";

    // Transactions
    pub const TRANSACTION: &str = "transaction";

    // SET variable
    pub const SET: &str = "set";

    // CREATE FUNCTION — function variant.
    pub const FUNCTION: &str = "function";

    // GO batch separator
    pub const GO: &str = "go";

    // EXEC
    pub const EXEC: &str = "exec";

    // ALTER TABLE
    pub const ALTER_TABLE: &str = "alter_table";
    pub const ADD_COLUMN: &str = "add_column";

    // CREATE INDEX
    pub const CREATE_INDEX: &str = "create_index";
    pub const INDEX_FIELDS: &str = "index_fields";

    // Data types
    pub const INT: &str = "int";
    pub const VARCHAR: &str = "varchar";
    pub const NVARCHAR: &str = "nvarchar";
    pub const DATETIME: &str = "datetime";

    // Expressions
    pub const COMPARE: &str = "compare";
    pub const BETWEEN: &str = "between";
    pub const ASSIGN: &str = "assign";

    // Identifiers and their variants
    pub const NAME: &str = "name";
    pub const ALIAS: &str = "alias";
    pub const SCHEMA: &str = "schema";
    pub const VAR: &str = "var";
    pub const TEMP: &str = "temp";
    pub const COMMENT: &str = "comment";

    // Operator child (from prepend_op_element).
    pub const OP: &str = "op";

    // Markers — always empty when emitted.
    //
    // T-SQL's transform currently emits NO disambiguation markers —
    // every `map_element_name` entry is a bare rename (no tuple with a
    // marker child). The invariant still benefits from a `MARKER_ONLY`
    // slice so the central registry has a uniform shape.

    /// Names that, when emitted, are always empty elements (no text,
    /// no element children). Used by the markers-stay-empty invariant.
    pub const MARKER_ONLY: &[&str] = &[];

    /// Every semantic name this language's transform can emit.
    pub const ALL_NAMES: &[&str] = &[
        FILE, STATEMENT,
        SELECT, INSERT, DELETE, UPDATE,
        FROM, WHERE, ORDER, TARGET, GROUP, HAVING, JOIN, DIRECTION,
        RELATION, REF, COLUMN, COL, STAR,
        LITERAL, LIST,
        CALL, BODY, ARG,
        SUBQUERY, CTE, UNION, EXISTS,
        WINDOW, OVER, PARTITION,
        CASE, WHEN,
        CAST,
        CREATE, COLUMNS, DEFINITION,
        MERGE, TRANSACTION, SET, FUNCTION, GO, EXEC,
        ALTER_TABLE, ADD_COLUMN,
        CREATE_INDEX, INDEX_FIELDS,
        INT, VARCHAR, NVARCHAR, DATETIME,
        COMPARE, BETWEEN, ASSIGN,
        NAME, ALIAS, SCHEMA, VAR, TEMP, COMMENT,
        OP,
    ];
}

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
                                rename(xot, node, VAR);
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
            rename(xot, node, COMPARE);
            Ok(TransformAction::Continue)
        }

        // BETWEEN expression - rename and clean up
        "between_expression" => {
            rename(xot, node, BETWEEN);
            Ok(TransformAction::Continue)
        }

        // Assignment (UPDATE SET Name = value) - extract operator
        "assignment" => {
            extract_operator(xot, node)?;
            rename(xot, node, ASSIGN);
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
            rename(xot, node, NAME);
            return Ok(());
        }
    };

    // Check field attribute for special roles
    if let Some(field_val) = get_attr(xot, node, "field") {
        match field_val.as_str() {
            "alias" => {
                let clean = strip_brackets(&text);
                replace_text(xot, node, &clean);
                rename(xot, node, ALIAS);
                return Ok(());
            }
            "schema" => {
                let clean = strip_brackets(&text);
                replace_text(xot, node, &clean);
                rename(xot, node, SCHEMA);
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
        rename(xot, node, VAR);
    } else {
        // Regular identifier - strip brackets and rename to "name"
        let clean = strip_brackets(&text);
        replace_text(xot, node, &clean);
        rename(xot, node, NAME);
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
                        rename(xot, node, TEMP);
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
        "program" => Some(FILE),
        "statement" => Some(STATEMENT),

        // DML statements
        "select" => Some(SELECT),
        "insert" => Some(INSERT),
        "delete" => Some(DELETE),
        "update" => Some(UPDATE),

        // Clauses
        "from" => Some(FROM),
        "where" => Some(WHERE),
        "order_by" => Some(ORDER),
        "order_target" => Some(TARGET),
        "group_by" => Some(GROUP),
        "having" => Some(HAVING),
        "join" => Some(JOIN),
        "direction" => Some(DIRECTION),

        // References and columns
        "relation" => Some(RELATION),
        "object_reference" => Some(REF),
        "field" => Some(COLUMN),
        "column" => Some(COL),
        "all_fields" => Some(STAR),

        // Literals and values
        "literal" => Some(LITERAL),
        "list" => Some(LIST),

        // Functions/calls
        "invocation" => Some(CALL),
        "function_body" => Some(BODY),
        "function_arguments" | "function_argument" => Some(ARG),

        // Subqueries, CTEs, set operations
        "subquery" => Some(SUBQUERY),
        "cte" => Some(CTE),
        "set_operation" => Some(UNION),
        "exists" => Some(EXISTS),

        // Window functions
        "window_function" => Some(WINDOW),
        "window_specification" => Some(OVER),
        "partition_by" => Some(PARTITION),

        // CASE expression
        "case" => Some(CASE),
        "when_clause" => Some(WHEN),

        // CAST
        "cast" => Some(CAST),

        // DDL
        "create_table" => Some(CREATE),
        "column_definitions" => Some(COLUMNS),
        "column_definition" => Some(DEFINITION),

        // MERGE
        "merge" => Some(MERGE),

        // Transactions
        "transaction" => Some(TRANSACTION),

        // SET variable
        "set_statement" => Some(SET),

        // CREATE FUNCTION — function variant.
        "create_function" => Some(FUNCTION),

        // GO batch separator
        "go_statement" => Some(GO),

        // EXEC
        "execute_statement" => Some(EXEC),

        // ALTER TABLE
        "alter_table" => Some(ALTER_TABLE),
        "add_column" => Some(ADD_COLUMN),

        // CREATE INDEX
        "create_index" => Some(CREATE_INDEX),
        "index_fields" => Some(INDEX_FIELDS),

        // Data types
        "int" => Some(INT),
        "varchar" => Some(VARCHAR),
        "nvarchar" => Some(NVARCHAR),
        "datetime" => Some(DATETIME),

        // Assignment
        "assignment" => Some(ASSIGN),

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
        "name" | "alias" | "schema" | "var" | "temp" | "column" => SyntaxCategory::Identifier,

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
