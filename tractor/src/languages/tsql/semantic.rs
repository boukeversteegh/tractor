/// Semantic element names — tractor's T-SQL XML vocabulary after transform.
use crate::languages::NodeSpec;
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
