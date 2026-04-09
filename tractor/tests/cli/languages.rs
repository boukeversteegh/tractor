use crate::common::{lang_dir, tractor_test, tractor_run_stdin, tractor_stdout};

// ==========================================================================
// Rust
// ==========================================================================

#[test]
fn rust() {
    let dir = lang_dir("rust");
    tractor_test(&dir, &["sample.rs", "-x", "function", "--expect", "4"]);
    tractor_test(&dir, &["sample.rs", "-x", "function[name='add']", "--expect", "1"]);
    tractor_test(&dir, &["sample.rs", "-x", "function[name='main']", "--expect", "1"]);
    tractor_test(&dir, &["sample.rs", "-x", "file", "--expect", "1"]);
    tractor_test(&dir, &["sample.rs", "-x", "let", "--expect", "1"]);
    tractor_test(&dir, &["sample.rs", "-x", "binary[op='+']", "--expect", "1"]);
    tractor_test(&dir, &["sample.rs", "-x", "call", "--expect", "1"]);
    tractor_test(&dir, &["sample.rs", "-x", "macro", "--expect", "1"]);
    tractor_test(&dir, &["sample.rs", "-x", "function[pub]", "--expect", "3"]);
    tractor_test(&dir, &["sample.rs", "-x", "function[pub[not(*)]]", "--expect", "1"]);
    tractor_test(&dir, &["sample.rs", "-x", "function[pub[crate]]", "--expect", "1"]);
    tractor_test(&dir, &["sample.rs", "-x", "function[private]", "--expect", "1"]);
}

// ==========================================================================
// Python
// ==========================================================================

#[test]
fn python() {
    let dir = lang_dir("python");
    tractor_test(&dir, &["sample.py", "-x", "function", "--expect", "3"]);
    tractor_test(&dir, &["sample.py", "-x", "function[name='add']", "--expect", "1"]);
    tractor_test(&dir, &["sample.py", "-x", "function[name='main']", "--expect", "1"]);
    tractor_test(&dir, &["sample.py", "-x", "module", "--expect", "1"]);
    tractor_test(&dir, &["sample.py", "-x", "return", "--expect", "2"]);
    tractor_test(&dir, &["sample.py", "-x", "binary[op='+']", "--expect", "1"]);
    tractor_test(&dir, &["sample.py", "-x", "call", "--expect", "3"]);
    tractor_test(&dir, &["sample.py", "-x", "function[async]", "--expect", "1"]);
}

#[test]
fn python_multiline_strings() {
    let dir = lang_dir("python");
    // tree-sitter normalizes CRLF to LF, so both files match with \n
    tractor_test(
        &dir,
        &["multiline-string-lf.py", "-x", "//string_content[.=\"hello\n\n\"]", "--expect", "1"],
    );
    tractor_test(
        &dir,
        &["multiline-string-crlf.py", "-x", "//string_content[.=\"hello\n\n\"]", "--expect", "1"],
    );
}

// ==========================================================================
// TypeScript
// ==========================================================================

#[test]
fn typescript() {
    let dir = lang_dir("typescript");
    tractor_test(&dir, &["sample.ts", "-x", "function[name]", "--expect", "4"]);
    tractor_test(&dir, &["sample.ts", "-x", "function[name='add']", "--expect", "1"]);
    tractor_test(&dir, &["sample.ts", "-x", "function[name='main']", "--expect", "1"]);
    tractor_test(&dir, &["sample.ts", "-x", "program", "--expect", "1"]);
    tractor_test(&dir, &["sample.ts", "-x", "variable", "--expect", "1"]);
    tractor_test(&dir, &["sample.ts", "-x", "binary[op='+']", "--expect", "1"]);
    tractor_test(&dir, &["sample.ts", "-x", "call", "--expect", "4"]);
    tractor_test(&dir, &["sample.ts", "-x", "//param[optional]", "--expect", "2"]);
    tractor_test(&dir, &["sample.ts", "-x", "//param[required]", "--expect", "5"]);
}

// ==========================================================================
// TSX
// ==========================================================================

#[test]
fn tsx() {
    let dir = lang_dir("tsx");
    tractor_test(&dir, &["sample.tsx", "-x", "program", "--expect", "1"]);
    tractor_test(&dir, &["sample.tsx", "-x", "function[name]", "--expect", "1"]);
    tractor_test(&dir, &["sample.tsx", "-x", "function[name='Greeting']", "--expect", "1"]);
    tractor_test(&dir, &["sample.tsx", "-x", "interface", "--expect", "1"]);
    tractor_test(&dir, &["sample.tsx", "-x", "variable", "--expect", "1"]);
    // JSX-specific nodes
    tractor_test(&dir, &["sample.tsx", "-x", "//jsx_element", "--expect", "4"]);
    tractor_test(&dir, &["sample.tsx", "-x", "//jsx_opening_element", "--expect", "4"]);
    tractor_test(&dir, &["sample.tsx", "-x", "//jsx_closing_element", "--expect", "4"]);
    tractor_test(&dir, &["sample.tsx", "-x", "//jsx_attribute", "--expect", "2"]);
    tractor_test(&dir, &["sample.tsx", "-x", "//jsx_expression", "--expect", "5"]);
    tractor_test(&dir, &["sample.tsx", "-x", "//jsx_text", "--expect", "5"]);
}

// ==========================================================================
// JavaScript
// ==========================================================================

#[test]
fn javascript() {
    let dir = lang_dir("javascript");
    tractor_test(&dir, &["sample.js", "-x", "function[name]", "--expect", "2"]);
    tractor_test(&dir, &["sample.js", "-x", "function[name='add']", "--expect", "1"]);
    tractor_test(&dir, &["sample.js", "-x", "function[name='main']", "--expect", "1"]);
    tractor_test(&dir, &["sample.js", "-x", "program", "--expect", "1"]);
    tractor_test(&dir, &["sample.js", "-x", "call", "--expect", "3"]);
    // Call structure
    tractor_test(&dir, &["sample.js", "-x", "call/function", "--expect", "3"]);
    tractor_test(&dir, &["sample.js", "-x", "call/function[ref]", "--expect", "2"]);
    tractor_test(&dir, &["sample.js", "-x", "call/function/member", "--expect", "1"]);
    // Member expression structure
    tractor_test(&dir, &["sample.js", "-x", "member/object", "--expect", "1"]);
    tractor_test(&dir, &["sample.js", "-x", "member/property", "--expect", "1"]);
}

// ==========================================================================
// Go
// ==========================================================================

#[test]
fn go() {
    let dir = lang_dir("go");
    tractor_test(&dir, &["sample.go", "-x", "function", "--expect", "3"]);
    tractor_test(&dir, &["sample.go", "-x", "function[name='add']", "--expect", "1"]);
    tractor_test(&dir, &["sample.go", "-x", "function[name='main']", "--expect", "1"]);
    tractor_test(&dir, &["sample.go", "-x", "file", "--expect", "1"]);
    tractor_test(&dir, &["sample.go", "-x", "package", "--expect", "1"]);
    tractor_test(&dir, &["sample.go", "-x", "binary[op='+']", "--expect", "1"]);
    tractor_test(&dir, &["sample.go", "-x", "call", "--expect", "2"]);
    tractor_test(&dir, &["sample.go", "-x", "function[exported]", "--expect", "1"]);
    tractor_test(&dir, &["sample.go", "-x", "function[unexported]", "--expect", "2"]);
}

// ==========================================================================
// Java
// ==========================================================================

#[test]
fn java() {
    let dir = lang_dir("java");
    tractor_test(&dir, &["sample.java", "-x", "method", "--expect", "5"]);
    tractor_test(&dir, &["sample.java", "-x", "method[name='add']", "--expect", "1"]);
    tractor_test(&dir, &["sample.java", "-x", "class[name='Sample']", "--expect", "1"]);
    tractor_test(&dir, &["sample.java", "-x", "program", "--expect", "1"]);
    tractor_test(&dir, &["sample.java", "-x", "static", "--expect", "2"]);
    tractor_test(&dir, &["sample.java", "-x", "binary[op='+']", "--expect", "2"]);
    tractor_test(&dir, &["sample.java", "-x", "call", "--expect", "3"]);
    tractor_test(&dir, &["sample.java", "-x", "//method[public]", "--expect", "2"]);
    tractor_test(&dir, &["sample.java", "-x", "//method[package-private]", "--expect", "2"]);
    tractor_test(&dir, &["sample.java", "-x", "//method[protected]", "--expect", "1"]);
}

// ==========================================================================
// C#
// ==========================================================================

#[test]
fn csharp_basic() {
    let dir = lang_dir("csharp");
    tractor_test(&dir, &["sample.cs", "-x", "method", "--expect", "5"]);
    tractor_test(&dir, &["sample.cs", "-x", "method[name='Add']", "--expect", "1"]);
    tractor_test(&dir, &["sample.cs", "-x", "class[name='Sample']", "--expect", "1"]);
    tractor_test(&dir, &["sample.cs", "-x", "unit", "--expect", "1"]);
    tractor_test(&dir, &["sample.cs", "-x", "static", "--expect", "2"]);
    tractor_test(&dir, &["sample.cs", "-x", "binary[op='+']", "--expect", "1"]);
    tractor_test(&dir, &["sample.cs", "-x", "call", "--expect", "4"]);
    tractor_test(&dir, &["sample.cs", "-x", "int", "--expect", "2"]);
    tractor_test(&dir, &["sample.cs", "-x", "//method[public]", "--expect", "1"]);
    tractor_test(&dir, &["sample.cs", "-x", "//method[private]", "--expect", "2"]);
    tractor_test(&dir, &["sample.cs", "-x", "//method[internal]", "--expect", "1"]);
    tractor_test(&dir, &["sample.cs", "-x", "//method[protected]", "--expect", "1"]);
}

#[test]
fn csharp_ast_grep_comparisons() {
    let dir = lang_dir("csharp");
    // MaxLength without AutoTruncate
    tractor_test(&dir, &[
        "attribute-maxlength-autotruncate.cs",
        "-x", "//property[attributes[contains(., 'MaxLength')]][not(attributes[contains(., 'AutoTruncate')])]/name",
        "--expect", "1",
    ]);
    // MaxLength on boolean
    tractor_test(&dir, &[
        "attribute-maxlength-boolean.cs",
        "-x", "//property[type='bool'][attributes[contains(., 'MaxLength')]]/name",
        "--expect", "1",
    ]);
    // Extension method detection
    tractor_test(&dir, &[
        "mapper-extension-method.cs",
        "-x", "//class[static][contains(name, 'Mapper')]//method[public][static][count(parameters/parameter)=1][not(parameters/parameter/this)]/name",
        "--expect", "1",
    ]);
    // Block-scoped namespaces
    tractor_test(&dir, &[
        "namespaces-file-scoped.cs",
        "-x", "//namespace[body]",
        "--expect", "1",
    ]);
    // Repository GetAll without OrderBy
    tractor_test(&dir, &[
        "repository-getall-orderby.cs",
        "-x", "//class[contains(name, 'Repository')][not(contains(name, 'Mock'))]//method[contains(name, 'GetAll')][not(contains(., 'OrderBy'))]/name",
        "--expect", "1",
    ]);
    // Query without AsNoTracking
    tractor_test(&dir, &[
        "query-asnotracking.cs",
        "-x", "//method[contains(name, 'Get')][contains(., '_context')][contains(., 'Map')][not(contains(., 'AsNoTracking'))]/name",
        "--expect", "1",
    ]);
}

#[test]
fn csharp_generic_type_matching() {
    let dir = lang_dir("csharp");
    tractor_test(&dir, &["generic-type-match.cs", "-x", "//type[.='List<string>']", "--expect", "2"]);
    tractor_test(&dir, &["generic-type-match.cs", "-x", "//type[.='Dictionary<string, int>']", "--expect", "2"]);
    tractor_test(&dir, &["generic-type-match.cs", "-x", "//type[generic]", "--expect", "8"]);
    tractor_test(&dir, &["generic-type-match.cs", "-x", "//type[.='List<Dictionary<string, User>>']", "--expect", "2"]);
    tractor_test(&dir, &["generic-type-match.cs", "-x", "//type[generic]/arguments/type[.='string']", "--expect", "6"]);
    // Whitespace-insensitive matching
    tractor_test(&dir, &["generic-type-match.cs", "-W", "-x", "//type[.='Dictionary<string,int>']", "--expect", "2"]);
}

#[test]
fn csharp_null_forgiving_operator() {
    let dir = lang_dir("csharp");
    tractor_test(&dir, &["null-forgiving-operator.cs", "-x", "//postfix_unary_expression", "--expect", "5"]);
    tractor_test(&dir, &["null-forgiving-operator.cs", "-x", "//ERROR", "--expect", "0"]);
    tractor_test(&dir, &["null-forgiving-operator.cs", "-x", "//member[postfix_unary_expression]", "--expect", "4"]);
}

// ==========================================================================
// Ruby
// ==========================================================================

#[test]
fn ruby() {
    let dir = lang_dir("ruby");
    tractor_test(&dir, &["sample.rb", "-x", "method", "--expect", "2"]);
    tractor_test(&dir, &["sample.rb", "-x", "method[name='add']", "--expect", "1"]);
    tractor_test(&dir, &["sample.rb", "-x", "method[name='main']", "--expect", "1"]);
    tractor_test(&dir, &["sample.rb", "-x", "call", "--expect", "2"]);
}

// ==========================================================================
// XML Passthrough
// ==========================================================================

#[test]
fn xml_passthrough() {
    let dir = lang_dir("xml");
    tractor_test(&dir, &["sample.xml", "-x", "item", "--expect", "3"]);
    tractor_test(&dir, &["sample.xml", "-x", "item[@type='feature']", "--expect", "2"]);
    tractor_test(&dir, &["sample.xml", "-x", "item[@type='bug']", "--expect", "1"]);
    tractor_test(&dir, &["sample.xml", "-x", "setting", "--expect", "2"]);
    tractor_test(&dir, &["sample.xml", "-x", "item[status='complete']", "--expect", "1"]);
    tractor_test(&dir, &["sample.xml", "-x", "project/@name", "--expect", "1"]);
    tractor_test(&dir, &["sample.xml", "-x", "name", "--expect", "3"]);
    tractor_test(&dir, &["sample.xml", "-x", "item/name", "-v", "value", "--expect", "some"]);
}

// ==========================================================================
// YAML
// ==========================================================================

#[test]
fn yaml_data_view() {
    let dir = lang_dir("yaml");
    tractor_test(&dir, &["sample.yaml", "-x", "//name[.='my-app']", "--expect", "1"]);
    tractor_test(&dir, &["sample.yaml", "-x", "//database/host[.='localhost']", "--expect", "1"]);
    tractor_test(&dir, &["sample.yaml", "-x", "//database/port[.='5432']", "--expect", "1"]);
    tractor_test(&dir, &["sample.yaml", "-x", "//database/credentials/username", "--expect", "1"]);
    tractor_test(&dir, &["sample.yaml", "-x", "//servers", "--expect", "2"]);
    tractor_test(&dir, &["sample.yaml", "-x", "//servers[name='web-1']", "--expect", "1"]);
    tractor_test(&dir, &["sample.yaml", "-x", "//servers[name='web-1']/port[.='8080']", "--expect", "1"]);
    tractor_test(&dir, &["sample.yaml", "-x", "//features", "--expect", "3"]);
    tractor_test(&dir, &["sample.yaml", "-x", "//features[.='auth']", "--expect", "1"]);
    tractor_test(&dir, &["sample.yaml", "-x", "//nested/level1/level2/value[.='deep']", "--expect", "1"]);
    tractor_test(&dir, &["sample.yaml", "-x", "//flow_map/x[.='1']", "--expect", "1"]);
    tractor_test(&dir, &["sample.yaml", "-x", "//flow_list", "--expect", "3"]);
    tractor_test(&dir, &["sample.yaml", "-x", "//quoted[.='hello world']", "--expect", "1"]);
    tractor_test(&dir, &["sample.yaml", "-x", "//multiline[contains(.,'line one')]", "--expect", "1"]);
    tractor_test(&dir, &["sample.yaml", "-x", "//first_name", "--expect", "1"]);
    tractor_test(&dir, &["sample.yaml", "-x", "//*[@key='first name']", "--expect", "1"]);
    tractor_test(&dir, &["sample.yaml", "-x", "//first_name[text()='Alice']", "--expect", "1"]);
}

#[test]
fn yaml_multi_document() {
    let dir = lang_dir("yaml");
    tractor_test(&dir, &["multi.yaml", "-x", "//document", "--expect", "3"]);
    tractor_test(&dir, &["multi.yaml", "-x", "//document[1]/name[.='doc1']", "--expect", "1"]);
    tractor_test(&dir, &["multi.yaml", "-x", "//document[2]/name[.='doc2']", "--expect", "1"]);
    tractor_test(&dir, &["multi.yaml", "-x", "//document[3]/value[.='three']", "--expect", "1"]);
    tractor_test(&dir, &["multi.yaml", "-x", "//name", "--expect", "3"]);
}

#[test]
fn yaml_structure_mode() {
    let dir = lang_dir("yaml");
    tractor_test(&dir, &["sample.yaml", "-t", "structure", "-x", "//document/object", "--expect", "1"]);
    tractor_test(&dir, &["sample.yaml", "-t", "structure", "-x", "//property[key/string='name']/value/string[.='my-app']", "--expect", "1"]);
}

// ==========================================================================
// Markdown
// ==========================================================================

#[test]
fn markdown() {
    let dir = lang_dir("markdown");
    tractor_test(&dir, &["sample.md", "-x", "//heading", "--expect", "2"]);
    tractor_test(&dir, &["sample.md", "-x", "//heading[h1]", "--expect", "1"]);
    tractor_test(&dir, &["sample.md", "-x", "//heading[h2]", "--expect", "1"]);
    tractor_test(&dir, &["sample.md", "-x", "//list[ordered]", "--expect", "1"]);
    tractor_test(&dir, &["sample.md", "-x", "//list[unordered]", "--expect", "1"]);
    tractor_test(&dir, &["sample.md", "-x", "//item", "--expect", "5"]);
    tractor_test(&dir, &["sample.md", "-x", "//blockquote", "--expect", "1"]);
    tractor_test(&dir, &["sample.md", "-x", "//code_block", "--expect", "3"]);
    tractor_test(&dir, &["sample.md", "-x", "//code_block[language='python']", "--expect", "1"]);
    tractor_test(&dir, &["sample.md", "-x", "//code_block[language='javascript']", "--expect", "1"]);
    tractor_test(&dir, &["sample.md", "-x", "//code_block[not(language)]", "--expect", "1"]);
    tractor_test(&dir, &["sample.md", "-x", "//hr", "--expect", "1"]);
}

#[test]
fn markdown_round_trip() {
    let dir = lang_dir("markdown");
    // Extract JavaScript code block from markdown
    let js_code = tractor_stdout(
        &dir,
        &["sample.md", "-x", "//code_block[language='javascript']/code", "-v", "value"],
    );

    // Pipe extracted code into tractor as JavaScript and count functions
    let result = tractor_run_stdin(
        &dir,
        &["-l", "javascript", "-x", "//function[name]", "-v", "count"],
        &js_code,
    );
    assert!(result.success, "round-trip parse should succeed");
    assert_eq!(
        result.stdout.trim(),
        "1",
        "extracted JS code should contain 1 function"
    );
}

// ==========================================================================
// T-SQL
// ==========================================================================

#[test]
fn tsql_basic() {
    let dir = lang_dir("tsql");
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "file", "--expect", "1"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "statement", "--expect", "24"]);
}

#[test]
fn tsql_dml() {
    let dir = lang_dir("tsql");
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "select", "--expect", "17"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "insert", "--expect", "1"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "delete", "--expect", "1"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "update", "--expect", "3"]);
}

#[test]
fn tsql_clauses() {
    let dir = lang_dir("tsql");
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "where", "--expect", "14"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "order_by", "--expect", "3"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "group_by", "--expect", "1"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "having", "--expect", "1"]);
}

#[test]
fn tsql_joins_subqueries() {
    let dir = lang_dir("tsql");
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "join", "--expect", "2"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "subquery", "--expect", "2"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "exists", "--expect", "1"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "cte", "--expect", "1"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "union", "--expect", "1"]);
}

#[test]
fn tsql_expressions() {
    let dir = lang_dir("tsql");
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "case", "--expect", "1"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "between", "--expect", "1"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "compare[op='>']", "--expect", "4"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "compare[op='>=']", "--expect", "1"]);
}

#[test]
fn tsql_functions() {
    let dir = lang_dir("tsql");
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "call", "--expect", "9"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "cast", "--expect", "1"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "window", "--expect", "1"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "partition_by", "--expect", "1"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "star", "--expect", "2"]);
}

#[test]
fn tsql_identifiers() {
    let dir = lang_dir("tsql");
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "alias", "--expect", "17"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "schema", "--expect", "4"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "var", "--expect", "6"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "temp_ref", "--expect", "1"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "direction", "--expect", "2"]);
}

#[test]
fn tsql_ddl() {
    let dir = lang_dir("tsql");
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "create_table", "--expect", "1"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "col_def", "--expect", "3"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "create_function", "--expect", "1"]);
}

#[test]
fn tsql_advanced() {
    let dir = lang_dir("tsql");
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "assign", "--expect", "4"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "when", "--expect", "2"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "transaction", "--expect", "1"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "set", "--expect", "1"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "go", "--expect", "1"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "exec", "--expect", "1"]);
    tractor_test(&dir, &["sample.sql", "--lang", "tsql", "-x", "comment", "--expect", "20"]);
}

// ==========================================================================
// INI
// ==========================================================================

#[test]
fn ini() {
    let dir = lang_dir("ini");
    tractor_test(&dir, &["sample.ini", "-x", "//name[.='my-app']", "--expect", "1"]);
    tractor_test(&dir, &["sample.ini", "-x", "//version[.='1.0.0']", "--expect", "1"]);
    tractor_test(&dir, &["sample.ini", "-x", "//database/host[.='localhost']", "--expect", "1"]);
    tractor_test(&dir, &["sample.ini", "-x", "//database/port[.='5432']", "--expect", "1"]);
    tractor_test(&dir, &["sample.ini", "-x", "//database/enabled[.='true']", "--expect", "1"]);
    tractor_test(&dir, &["sample.ini", "-x", "//database.credentials/username[.='admin']", "--expect", "1"]);
    tractor_test(&dir, &["sample.ini", "-x", "//database.credentials/password[.='secret']", "--expect", "1"]);
    tractor_test(&dir, &["sample.ini", "-x", "//servers/count[.='2']", "--expect", "1"]);
    tractor_test(&dir, &["sample.ini", "-x", "//paths/home[.='/usr/local']", "--expect", "1"]);
    tractor_test(&dir, &["sample.ini", "-x", "//paths/temp[.='/tmp']", "--expect", "1"]);
    tractor_test(&dir, &["sample.ini", "-x", "//comment", "--expect", "2"]);
    tractor_test(&dir, &["sample.ini", "-x", "//comment[.='Global settings']", "--expect", "1"]);
    tractor_test(&dir, &["sample.ini", "-x", "//document", "--expect", "1"]);
}

// ==========================================================================
// TOML
// ==========================================================================

#[test]
fn toml() {
    let dir = lang_dir("toml");
    tractor_test(&dir, &["sample.toml", "-x", "//title[.='My App']", "--expect", "1"]);
    tractor_test(&dir, &["sample.toml", "-x", "//version[.='1.0.0']", "--expect", "1"]);
    tractor_test(&dir, &["sample.toml", "-x", "//database/host[.='localhost']", "--expect", "1"]);
    tractor_test(&dir, &["sample.toml", "-x", "//database/port[.='5432']", "--expect", "1"]);
    tractor_test(&dir, &["sample.toml", "-x", "//database/enabled[.='true']", "--expect", "1"]);
    tractor_test(&dir, &["sample.toml", "-x", "//database/credentials/username", "--expect", "1"]);
    tractor_test(&dir, &["sample.toml", "-x", "//database/credentials/password[.='secret']", "--expect", "1"]);
    tractor_test(&dir, &["sample.toml", "-x", "//servers/item", "--expect", "2"]);
    tractor_test(&dir, &["sample.toml", "-x", "//servers/item[name='web-1']", "--expect", "1"]);
    tractor_test(&dir, &["sample.toml", "-x", "//servers/item[name='web-1']/port[.='8080']", "--expect", "1"]);
    tractor_test(&dir, &["sample.toml", "-x", "//features/item", "--expect", "3"]);
    tractor_test(&dir, &["sample.toml", "-x", "//features/item[.='auth']", "--expect", "1"]);
    tractor_test(&dir, &["sample.toml", "-x", "//inline/x[.='1']", "--expect", "1"]);
    tractor_test(&dir, &["sample.toml", "-x", "//inline/y[.='2']", "--expect", "1"]);
    tractor_test(&dir, &["sample.toml", "-x", "//quoted[.='hello world']", "--expect", "1"]);
    tractor_test(&dir, &["sample.toml", "-x", "//first_name", "--expect", "1"]);
    tractor_test(&dir, &["sample.toml", "-x", "//*[@key='first name']", "--expect", "1"]);
    tractor_test(&dir, &["sample.toml", "-x", "//nested/level1/level2/value[.='deep']", "--expect", "1"]);
    tractor_test(&dir, &["sample.toml", "-x", "//document", "--expect", "1"]);
}
