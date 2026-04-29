//! T-SQL semantic shape: DML statements, clauses, joins,
//! subqueries, CTEs, window functions, identifier forms, and DDL.
//!
//! T-SQL has by far the largest element vocabulary of any tractor
//! language; one focused snippet per construct keeps each test
//! readable and the failure message specific.

use crate::support::semantic::*;

// ---- basic DML ----------------------------------------------------------

#[test]
fn tsql_select_with_where_and_order() {
    let mut tree = parse_src("tsql", "SELECT u.Name FROM Users u WHERE u.Age > 18 ORDER BY u.Name;");

    claim("SELECT renders as <select>", &mut tree, "//select", 1);
    claim("WHERE clause renders as <where>", &mut tree, "//where", 1);
    claim("ORDER BY renders as <order>", &mut tree, "//order", 1);
    claim("`>` comparison extracts as <compare[op='>']>", &mut tree, "//compare[op='>']", 1);
    claim("table alias `Users u` renders as <alias>", &mut tree, "//alias", 1);
}

#[test]
fn tsql_insert_into_values() {
    claim("INSERT renders as <insert>",
        &mut parse_src("tsql", "INSERT INTO Audit (Action) VALUES ('foo');"),
        "//insert", 1);
}

#[test]
fn tsql_delete() {
    claim("DELETE renders as <delete>",
        &mut parse_src("tsql", "DELETE FROM Old WHERE x < 1;"),
        "//delete", 1);
}

#[test]
fn tsql_update_set() {
    claim("UPDATE renders as <update>",
        &mut parse_src("tsql", "UPDATE Config SET v = 1 WHERE k = 'x';"),
        "//update", 1);
}

// ---- aggregation clauses ------------------------------------------------

#[test]
fn tsql_group_by_having() {
    let mut tree = parse_src("tsql", r#"
        SELECT Department, COUNT(*) AS HeadCount
        FROM Employees
        GROUP BY Department
        HAVING COUNT(*) > 5;
    "#);

    claim("GROUP BY renders as <group>", &mut tree, "//group", 1);
    claim("HAVING renders as <having>", &mut tree, "//having", 1);
    claim("`COUNT(*)` star argument renders as <star>", &mut tree, "//star", 2);
}

// ---- joins / subqueries / EXISTS ----------------------------------------

#[test]
fn tsql_inner_and_left_join() {
    claim("each JOIN clause renders as <join>",
        &mut parse_src("tsql", r#"
            SELECT *
            FROM Orders o
            JOIN Users u ON o.UserID = u.ID
            LEFT JOIN Addresses a ON u.ID = a.UserID;
        "#),
        "//join", 2);
}

#[test]
fn tsql_subquery_in_predicate() {
    claim("nested SELECT in `IN (...)` renders as <subquery>",
        &mut parse_src("tsql", "SELECT Name FROM Users WHERE ID IN (SELECT UserID FROM Orders);"),
        "//subquery", 1);
}

#[test]
fn tsql_exists_predicate() {
    claim("EXISTS renders as <exists>",
        &mut parse_src("tsql", "SELECT 1 FROM Users u WHERE EXISTS (SELECT 1 FROM Orders WHERE UserID = u.ID);"),
        "//exists", 1);
}

// ---- CTE / UNION --------------------------------------------------------

#[test]
fn tsql_common_table_expression() {
    claim("WITH … AS (...) renders as <cte>",
        &mut parse_src("tsql", r#"
            WITH ActiveUsers AS (
                SELECT ID, Name FROM Users WHERE Active = 1
            )
            SELECT Name FROM ActiveUsers;
        "#),
        "//cte", 1);
}

#[test]
fn tsql_union_all() {
    claim("UNION ALL renders as <union>",
        &mut parse_src("tsql", r#"
            SELECT Name FROM Users
            UNION ALL
            SELECT Name FROM Admins;
        "#),
        "//union", 1);
}

// ---- conditional / predicate forms --------------------------------------

#[test]
fn tsql_case_when() {
    claim("CASE renders as <case>",
        &mut parse_src("tsql", r#"
            SELECT CASE WHEN Age >= 18 THEN 'Adult' ELSE 'Minor' END FROM Users;
        "#),
        "//case", 1);
}

#[test]
fn tsql_between() {
    claim("BETWEEN renders as <between>",
        &mut parse_src("tsql", "SELECT * FROM Users WHERE Age BETWEEN 18 AND 65;"),
        "//between", 1);
}

// ---- window function ----------------------------------------------------

#[test]
fn tsql_window_with_partition_by() {
    let mut tree = parse_src("tsql", r#"
        SELECT ROW_NUMBER() OVER (PARTITION BY Department ORDER BY Salary DESC) FROM Employees;
    "#);

    claim("OVER (...) renders as <window>", &mut tree, "//window", 1);
    claim("PARTITION BY renders as <partition>", &mut tree, "//partition", 1);
    claim("DESC sort direction renders as <direction>", &mut tree, "//direction", 1);
}

// ---- function calls / CAST ----------------------------------------------

#[test]
fn tsql_function_calls_and_cast() {
    let mut tree = parse_src("tsql", r#"
        SELECT
            COALESCE(Nickname, Name),
            CAST(Age AS VARCHAR(10))
        FROM Users;
    "#);

    claim("ordinary function invocation renders as <call>", &mut tree, "//call", 1);
    claim("CAST renders as a distinct <cast> element (not <call>)", &mut tree, "//cast", 1);
}

// ---- identifier forms ---------------------------------------------------

#[test]
fn tsql_schema_qualified_identifiers() {
    claim("`dbo.Users` schema-qualified name produces a <schema> element",
        &mut parse_src("tsql", "SELECT * FROM dbo.Users;"),
        "//schema", 1);
}

#[test]
fn tsql_variable() {
    claim("`@StartDate` renders as <var>",
        &mut parse_src("tsql", "SELECT * FROM Users WHERE Created >= @StartDate;"),
        "//var", 1);
}

#[test]
fn tsql_temp_table_marker() {
    claim("`#TempUsers` (local temp table) renders with a <temp> marker",
        &mut parse_src("tsql", "SELECT Name INTO #TempUsers FROM Users;"),
        "//temp", 1);
}

#[test]
fn tsql_alias_keyword_optional() {
    claim("both `SELECT x AS y` and `SELECT x y` produce <alias>",
        &mut parse_src("tsql", "SELECT a AS one, b two FROM T;"),
        "//alias", 2);
}

// ---- DDL ---------------------------------------------------------------

#[test]
fn tsql_create_table_with_definitions() {
    let mut tree = parse_src("tsql", r#"
        CREATE TABLE Audit (
            ID INT PRIMARY KEY,
            Action NVARCHAR(100) NOT NULL,
            CreatedAt DATETIME DEFAULT GETDATE()
        );
    "#);

    claim("CREATE renders as <create>", &mut tree, "//create", 1);
    claim("each column produces a <definition>", &mut tree, "//definition", 3);
}

#[test]
fn tsql_create_function() {
    claim("CREATE FUNCTION renders as <function>",
        &mut parse_src("tsql", r#"
            CREATE FUNCTION dbo.GetAge(@BirthDate DATE)
            RETURNS INT
            AS
            BEGIN
                RETURN DATEDIFF(YEAR, @BirthDate, GETDATE())
            END;
        "#),
        "//function", 1);
}

// ---- transaction / SET / GO / EXEC -------------------------------------

#[test]
fn tsql_transaction_block() {
    claim("BEGIN TRANSACTION ... COMMIT renders as <transaction>",
        &mut parse_src("tsql", r#"
            BEGIN TRANSACTION;
            UPDATE T SET v = 1 WHERE k = 'a';
            COMMIT;
        "#),
        "//transaction", 1);
}

#[test]
fn tsql_set_statement() {
    claim("`SET @x = ...` renders as <set>",
        &mut parse_src("tsql", "SET @Threshold = 42;"),
        "//set", 1);
}

#[test]
fn tsql_go_separator() {
    claim("`GO` batch separator renders as <go>",
        &mut parse_src("tsql", "SELECT 1;\nGO\nSELECT 2;\n"),
        "//go", 1);
}

#[test]
fn tsql_exec_statement() {
    claim("`EXEC sp_helpdb;` renders as <exec>",
        &mut parse_src("tsql", "EXEC sp_helpdb;"),
        "//exec", 1);
}

// ---- MERGE -------------------------------------------------------------

#[test]
fn tsql_merge_with_when_clauses() {
    claim("MERGE … WHEN MATCHED / WHEN NOT MATCHED produces two <when> clauses",
        &mut parse_src("tsql", r#"
            MERGE INTO Target t
            USING Source s ON t.ID = s.ID
            WHEN MATCHED THEN UPDATE SET t.Name = s.Name
            WHEN NOT MATCHED THEN INSERT (ID, Name) VALUES (s.ID, s.Name);
        "#),
        "//when", 2);
}

// ---- comments ----------------------------------------------------------

#[test]
fn tsql_line_comments() {
    claim("`-- ...` lines render as <comment>",
        &mut parse_src("tsql", "-- top comment\nSELECT 1;\n-- another\nSELECT 2;"),
        "//comment", 2);
}
