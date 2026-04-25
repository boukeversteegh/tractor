/// Semantic element names — tractor's T-SQL XML vocabulary after transform.
/// Tree-sitter kind strings (left side of `match` arms) stay as bare
/// literals — they are external vocabulary.
use crate::languages::{KindEntry, KindHandling, NodeSpec};
use crate::output::syntax_highlight::SyntaxCategory;

// Named constants retained for use by the transform code. The NODES
// table below is the source of truth for marker/container role and
// syntax category. T-SQL's transform currently emits NO marker-only
// elements — every NODES entry is container-only.

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

use SyntaxCategory::*;

/// Per-name metadata — single source of truth for every element
/// name this language's transform can emit. T-SQL has no dual-use
/// entries (no marker-only names are currently emitted).
pub const NODES: &[NodeSpec] = &[
    // Top-level
    NodeSpec { name: FILE,      marker: false, container: true, syntax: Default },
    NodeSpec { name: STATEMENT, marker: false, container: true, syntax: Keyword },

    // DML
    NodeSpec { name: SELECT, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: INSERT, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: DELETE, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: UPDATE, marker: false, container: true, syntax: Keyword },

    // Clauses
    NodeSpec { name: FROM,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: WHERE,     marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ORDER,     marker: false, container: true, syntax: Default },
    NodeSpec { name: TARGET,    marker: false, container: true, syntax: Default },
    NodeSpec { name: GROUP,     marker: false, container: true, syntax: Default },
    NodeSpec { name: HAVING,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: JOIN,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: DIRECTION, marker: false, container: true, syntax: Keyword },

    // References and columns
    NodeSpec { name: RELATION, marker: false, container: true, syntax: Default },
    NodeSpec { name: REF,      marker: false, container: true, syntax: Type },
    NodeSpec { name: COLUMN,   marker: false, container: true, syntax: Identifier },
    NodeSpec { name: COL,      marker: false, container: true, syntax: Default },
    NodeSpec { name: STAR,     marker: false, container: true, syntax: Keyword },

    // Literals and values
    NodeSpec { name: LITERAL, marker: false, container: true, syntax: String },
    NodeSpec { name: LIST,    marker: false, container: true, syntax: Default },

    // Functions / calls
    NodeSpec { name: CALL, marker: false, container: true, syntax: Function },
    NodeSpec { name: BODY, marker: false, container: true, syntax: Default },
    NodeSpec { name: ARG,  marker: false, container: true, syntax: Default },

    // Subqueries / CTEs / set operations
    NodeSpec { name: SUBQUERY, marker: false, container: true, syntax: Default },
    NodeSpec { name: CTE,      marker: false, container: true, syntax: Keyword },
    NodeSpec { name: UNION,    marker: false, container: true, syntax: Keyword },
    NodeSpec { name: EXISTS,   marker: false, container: true, syntax: Keyword },

    // Window functions
    NodeSpec { name: WINDOW,    marker: false, container: true, syntax: Function },
    NodeSpec { name: OVER,      marker: false, container: true, syntax: Default },
    NodeSpec { name: PARTITION, marker: false, container: true, syntax: Default },

    // CASE
    NodeSpec { name: CASE, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: WHEN, marker: false, container: true, syntax: Keyword },

    // CAST
    NodeSpec { name: CAST, marker: false, container: true, syntax: Function },

    // DDL
    NodeSpec { name: CREATE,     marker: false, container: true, syntax: Default },
    NodeSpec { name: COLUMNS,    marker: false, container: true, syntax: Default },
    NodeSpec { name: DEFINITION, marker: false, container: true, syntax: Default },

    // MERGE
    NodeSpec { name: MERGE, marker: false, container: true, syntax: Keyword },

    // Transactions / SET / GO / EXEC / FUNCTION
    NodeSpec { name: TRANSACTION, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: SET,         marker: false, container: true, syntax: Keyword },
    NodeSpec { name: FUNCTION,    marker: false, container: true, syntax: Default },
    NodeSpec { name: GO,          marker: false, container: true, syntax: Keyword },
    NodeSpec { name: EXEC,        marker: false, container: true, syntax: Keyword },

    // ALTER TABLE / CREATE INDEX
    NodeSpec { name: ALTER_TABLE,  marker: false, container: true, syntax: Keyword },
    NodeSpec { name: ADD_COLUMN,   marker: false, container: true, syntax: Default },
    NodeSpec { name: CREATE_INDEX, marker: false, container: true, syntax: Keyword },
    NodeSpec { name: INDEX_FIELDS, marker: false, container: true, syntax: Default },

    // Data types
    NodeSpec { name: INT,      marker: false, container: true, syntax: Type },
    NodeSpec { name: VARCHAR,  marker: false, container: true, syntax: Type },
    NodeSpec { name: NVARCHAR, marker: false, container: true, syntax: Type },
    NodeSpec { name: DATETIME, marker: false, container: true, syntax: Type },

    // Expressions
    NodeSpec { name: COMPARE, marker: false, container: true, syntax: Operator },
    NodeSpec { name: BETWEEN, marker: false, container: true, syntax: Operator },
    NodeSpec { name: ASSIGN,  marker: false, container: true, syntax: Operator },

    // Identifiers and their variants
    NodeSpec { name: NAME,    marker: false, container: true, syntax: Identifier },
    NodeSpec { name: ALIAS,   marker: false, container: true, syntax: Identifier },
    NodeSpec { name: SCHEMA,  marker: false, container: true, syntax: Identifier },
    NodeSpec { name: VAR,     marker: false, container: true, syntax: Identifier },
    NodeSpec { name: TEMP,    marker: false, container: true, syntax: Identifier },
    NodeSpec { name: COMMENT, marker: false, container: true, syntax: Comment },

    // Operator child
    NodeSpec { name: OP, marker: false, container: true, syntax: Operator },
];

/// Tree-sitter kind catalogue — single source of truth for every
/// kind the T-SQL transform handles. Sorted alphabetically by kind
/// name. See `KindHandling` for variants.
///
/// T-SQL's grammar exposes ~80 `keyword_*` leaves (one per SQL
/// reserved word). The dispatcher detaches every one of them, so
/// they're catalogued as `Custom`. Adding a new keyword is a
/// one-line entry here.
pub const KINDS: &[KindEntry] = &[
    KindEntry { kind: "add_column",                    handling: KindHandling::Rename(ADD_COLUMN) },
    KindEntry { kind: "all_fields",                    handling: KindHandling::Rename(STAR) },
    KindEntry { kind: "alter_table",                   handling: KindHandling::Rename(ALTER_TABLE) },
    KindEntry { kind: "assignment",                    handling: KindHandling::Custom },
    KindEntry { kind: "between_expression",            handling: KindHandling::Custom },
    KindEntry { kind: "binary_expression",             handling: KindHandling::Custom },
    KindEntry { kind: "case",                          handling: KindHandling::Rename(CASE) },
    KindEntry { kind: "cast",                          handling: KindHandling::Rename(CAST) },
    KindEntry { kind: "column",                        handling: KindHandling::Rename(COL) },
    KindEntry { kind: "column_definition",             handling: KindHandling::Rename(DEFINITION) },
    KindEntry { kind: "column_definitions",            handling: KindHandling::Rename(COLUMNS) },
    KindEntry { kind: "comment",                       handling: KindHandling::PassThrough },
    KindEntry { kind: "create_function",               handling: KindHandling::Rename(FUNCTION) },
    KindEntry { kind: "create_index",                  handling: KindHandling::Rename(CREATE_INDEX) },
    KindEntry { kind: "create_table",                  handling: KindHandling::Rename(CREATE) },
    KindEntry { kind: "cte",                           handling: KindHandling::Rename(CTE) },
    KindEntry { kind: "datetime",                      handling: KindHandling::Rename(DATETIME) },
    KindEntry { kind: "delete",                        handling: KindHandling::Rename(DELETE) },
    KindEntry { kind: "direction",                     handling: KindHandling::Rename(DIRECTION) },
    KindEntry { kind: "execute_statement",             handling: KindHandling::Rename(EXEC) },
    KindEntry { kind: "exists",                        handling: KindHandling::Rename(EXISTS) },
    KindEntry { kind: "field",                         handling: KindHandling::Rename(COLUMN) },
    KindEntry { kind: "from",                          handling: KindHandling::Rename(FROM) },
    KindEntry { kind: "function_argument",             handling: KindHandling::Rename(ARG) },
    KindEntry { kind: "function_arguments",            handling: KindHandling::Rename(ARG) },
    KindEntry { kind: "function_body",                 handling: KindHandling::Rename(BODY) },
    KindEntry { kind: "go_statement",                  handling: KindHandling::Rename(GO) },
    KindEntry { kind: "group_by",                      handling: KindHandling::Rename(GROUP) },
    KindEntry { kind: "having",                        handling: KindHandling::Rename(HAVING) },
    KindEntry { kind: "identifier",                    handling: KindHandling::Custom },
    KindEntry { kind: "index_fields",                  handling: KindHandling::Rename(INDEX_FIELDS) },
    KindEntry { kind: "insert",                        handling: KindHandling::Rename(INSERT) },
    KindEntry { kind: "int",                           handling: KindHandling::Rename(INT) },
    KindEntry { kind: "invocation",                    handling: KindHandling::Rename(CALL) },
    KindEntry { kind: "join",                          handling: KindHandling::Rename(JOIN) },
    // T-SQL reserved-word leaves — every `keyword_*` is detached by the
    // dispatcher (the surrounding source text already carries the keyword).
    KindEntry { kind: "keyword_all",                   handling: KindHandling::Custom },
    KindEntry { kind: "keyword_and",                   handling: KindHandling::Custom },
    KindEntry { kind: "keyword_as",                    handling: KindHandling::Custom },
    KindEntry { kind: "keyword_begin",                 handling: KindHandling::Custom },
    KindEntry { kind: "keyword_between",               handling: KindHandling::Custom },
    KindEntry { kind: "keyword_by",                    handling: KindHandling::Custom },
    KindEntry { kind: "keyword_case",                  handling: KindHandling::Custom },
    KindEntry { kind: "keyword_cast",                  handling: KindHandling::Custom },
    KindEntry { kind: "keyword_commit",                handling: KindHandling::Custom },
    KindEntry { kind: "keyword_create",                handling: KindHandling::Custom },
    KindEntry { kind: "keyword_date",                  handling: KindHandling::Custom },
    KindEntry { kind: "keyword_datetime",              handling: KindHandling::Custom },
    KindEntry { kind: "keyword_default",               handling: KindHandling::Custom },
    KindEntry { kind: "keyword_delete",                handling: KindHandling::Custom },
    KindEntry { kind: "keyword_desc",                  handling: KindHandling::Custom },
    KindEntry { kind: "keyword_distinct",              handling: KindHandling::Custom },
    KindEntry { kind: "keyword_else",                  handling: KindHandling::Custom },
    KindEntry { kind: "keyword_end",                   handling: KindHandling::Custom },
    KindEntry { kind: "keyword_exec",                  handling: KindHandling::Custom },
    KindEntry { kind: "keyword_exists",                handling: KindHandling::Custom },
    KindEntry { kind: "keyword_from",                  handling: KindHandling::Custom },
    KindEntry { kind: "keyword_function",              handling: KindHandling::Custom },
    KindEntry { kind: "keyword_group",                 handling: KindHandling::Custom },
    KindEntry { kind: "keyword_having",                handling: KindHandling::Custom },
    KindEntry { kind: "keyword_in",                    handling: KindHandling::Custom },
    KindEntry { kind: "keyword_insert",                handling: KindHandling::Custom },
    KindEntry { kind: "keyword_int",                   handling: KindHandling::Custom },
    KindEntry { kind: "keyword_into",                  handling: KindHandling::Custom },
    KindEntry { kind: "keyword_is",                    handling: KindHandling::Custom },
    KindEntry { kind: "keyword_join",                  handling: KindHandling::Custom },
    KindEntry { kind: "keyword_key",                   handling: KindHandling::Custom },
    KindEntry { kind: "keyword_left",                  handling: KindHandling::Custom },
    KindEntry { kind: "keyword_like",                  handling: KindHandling::Custom },
    KindEntry { kind: "keyword_matched",               handling: KindHandling::Custom },
    KindEntry { kind: "keyword_merge",                 handling: KindHandling::Custom },
    KindEntry { kind: "keyword_not",                   handling: KindHandling::Custom },
    KindEntry { kind: "keyword_null",                  handling: KindHandling::Custom },
    KindEntry { kind: "keyword_nvarchar",              handling: KindHandling::Custom },
    KindEntry { kind: "keyword_on",                    handling: KindHandling::Custom },
    KindEntry { kind: "keyword_order",                 handling: KindHandling::Custom },
    KindEntry { kind: "keyword_over",                  handling: KindHandling::Custom },
    KindEntry { kind: "keyword_partition",             handling: KindHandling::Custom },
    KindEntry { kind: "keyword_primary",               handling: KindHandling::Custom },
    KindEntry { kind: "keyword_return",                handling: KindHandling::Custom },
    KindEntry { kind: "keyword_returns",               handling: KindHandling::Custom },
    KindEntry { kind: "keyword_select",                handling: KindHandling::Custom },
    KindEntry { kind: "keyword_set",                   handling: KindHandling::Custom },
    KindEntry { kind: "keyword_table",                 handling: KindHandling::Custom },
    KindEntry { kind: "keyword_then",                  handling: KindHandling::Custom },
    KindEntry { kind: "keyword_transaction",           handling: KindHandling::Custom },
    KindEntry { kind: "keyword_union",                 handling: KindHandling::Custom },
    KindEntry { kind: "keyword_update",                handling: KindHandling::Custom },
    KindEntry { kind: "keyword_using",                 handling: KindHandling::Custom },
    KindEntry { kind: "keyword_values",                handling: KindHandling::Custom },
    KindEntry { kind: "keyword_varchar",               handling: KindHandling::Custom },
    KindEntry { kind: "keyword_when",                  handling: KindHandling::Custom },
    KindEntry { kind: "keyword_where",                 handling: KindHandling::Custom },
    KindEntry { kind: "keyword_with",                  handling: KindHandling::Custom },
    KindEntry { kind: "list",                          handling: KindHandling::Rename(LIST) },
    KindEntry { kind: "literal",                       handling: KindHandling::Rename(LITERAL) },
    KindEntry { kind: "merge",                         handling: KindHandling::Rename(MERGE) },
    KindEntry { kind: "nvarchar",                      handling: KindHandling::Rename(NVARCHAR) },
    KindEntry { kind: "object_reference",              handling: KindHandling::Rename(REF) },
    KindEntry { kind: "op_unary_other",                handling: KindHandling::Custom },
    KindEntry { kind: "order_by",                      handling: KindHandling::Rename(ORDER) },
    KindEntry { kind: "order_target",                  handling: KindHandling::Rename(TARGET) },
    KindEntry { kind: "partition_by",                  handling: KindHandling::Rename(PARTITION) },
    KindEntry { kind: "program",                       handling: KindHandling::Rename(FILE) },
    KindEntry { kind: "relation",                      handling: KindHandling::Rename(RELATION) },
    KindEntry { kind: "select",                        handling: KindHandling::Rename(SELECT) },
    KindEntry { kind: "select_expression",             handling: KindHandling::Flatten },
    KindEntry { kind: "set_operation",                 handling: KindHandling::Rename(UNION) },
    KindEntry { kind: "set_statement",                 handling: KindHandling::Rename(SET) },
    KindEntry { kind: "statement",                     handling: KindHandling::Rename(STATEMENT) },
    KindEntry { kind: "subquery",                      handling: KindHandling::Rename(SUBQUERY) },
    KindEntry { kind: "term",                          handling: KindHandling::Flatten },
    KindEntry { kind: "transaction",                   handling: KindHandling::Rename(TRANSACTION) },
    KindEntry { kind: "unary_expression",              handling: KindHandling::Custom },
    KindEntry { kind: "update",                        handling: KindHandling::Rename(UPDATE) },
    KindEntry { kind: "varchar",                       handling: KindHandling::Rename(VARCHAR) },
    KindEntry { kind: "when_clause",                   handling: KindHandling::Rename(WHEN) },
    KindEntry { kind: "where",                         handling: KindHandling::Rename(WHERE) },
    KindEntry { kind: "window_function",               handling: KindHandling::Rename(WINDOW) },
    KindEntry { kind: "window_specification",          handling: KindHandling::Rename(OVER) },
];

/// Look up the rename target for a tree-sitter `kind` in this
/// language's catalogue. T-SQL has no markers — `RenameWithMarker`
/// variants don't appear.
pub fn rename_target(kind: &str) -> Option<&'static str> {
    KINDS.iter().find(|k| k.kind == kind).and_then(|k| match k.handling {
        KindHandling::Rename(s) | KindHandling::CustomThenRename(s) => Some(s),
        _ => None,
    })
}

pub fn spec(name: &str) -> Option<&'static NodeSpec> {
    NODES.iter().find(|n| n.name == name)
}

pub fn all_names() -> impl Iterator<Item = &'static str> {
    NODES.iter().map(|n| n.name)
}

pub fn is_marker_only(name: &str) -> bool {
    spec(name).map_or(false, |s| s.marker && !s.container)
}

pub fn is_declared(name: &str) -> bool {
    spec(name).is_some()
}
