use crate::common::{lang_dir, tractor_run_stdin, tractor_stdout};

// ==========================================================================
// Rust
// ==========================================================================

tractor_tests!(rust, lang_dir("rust"),
    ["sample.rs", "-x", "function", "--expect", "4"],
    ["sample.rs", "-x", "function[name='add']", "--expect", "1"],
    ["sample.rs", "-x", "function[name='main']", "--expect", "1"],
    ["sample.rs", "-x", "file", "--expect", "1"],
    ["sample.rs", "-x", "let", "--expect", "1"],
    ["sample.rs", "-x", "binary[op='+']", "--expect", "1"],
    ["sample.rs", "-x", "call", "--expect", "1"],
    ["sample.rs", "-x", "macro", "--expect", "1"],
    ["sample.rs", "-x", "function[pub]", "--expect", "3"],
    ["sample.rs", "-x", "function[pub[not(*)]]", "--expect", "1"],
    ["sample.rs", "-x", "function[pub[crate]]", "--expect", "1"],
    ["sample.rs", "-x", "function[private]", "--expect", "1"],
);

// ==========================================================================
// Python
// ==========================================================================

tractor_tests!(python, lang_dir("python"),
    ["sample.py", "-x", "function", "--expect", "3"],
    ["sample.py", "-x", "function[name='add']", "--expect", "1"],
    ["sample.py", "-x", "function[name='main']", "--expect", "1"],
    ["sample.py", "-x", "module", "--expect", "1"],
    ["sample.py", "-x", "return", "--expect", "2"],
    ["sample.py", "-x", "binary[op='+']", "--expect", "1"],
    ["sample.py", "-x", "call", "--expect", "3"],
    ["sample.py", "-x", "function[async]", "--expect", "1"],
);

tractor_tests!(python_multiline_strings, lang_dir("python"),
    // tree-sitter normalizes CRLF to LF, so both files match with \n
    ["multiline-string-lf.py", "-x", "//string_content[.=\"hello\n\n\"]", "--expect", "1"],
    ["multiline-string-crlf.py", "-x", "//string_content[.=\"hello\n\n\"]", "--expect", "1"],
);

// ==========================================================================
// TypeScript
// ==========================================================================

tractor_tests!(typescript, lang_dir("typescript"),
    ["sample.ts", "-x", "function[name]", "--expect", "4"],
    ["sample.ts", "-x", "function[name='add']", "--expect", "1"],
    ["sample.ts", "-x", "function[name='main']", "--expect", "1"],
    ["sample.ts", "-x", "program", "--expect", "1"],
    ["sample.ts", "-x", "variable", "--expect", "1"],
    ["sample.ts", "-x", "binary[op='+']", "--expect", "1"],
    ["sample.ts", "-x", "call", "--expect", "4"],
    ["sample.ts", "-x", "//param[optional]", "--expect", "2"],
    ["sample.ts", "-x", "//param[required]", "--expect", "5"],
);

// ==========================================================================
// TSX
// ==========================================================================

tractor_tests!(tsx, lang_dir("tsx"),
    ["sample.tsx", "-x", "program", "--expect", "1"],
    ["sample.tsx", "-x", "function[name]", "--expect", "1"],
    ["sample.tsx", "-x", "function[name='Greeting']", "--expect", "1"],
    ["sample.tsx", "-x", "interface", "--expect", "1"],
    ["sample.tsx", "-x", "variable", "--expect", "1"],
    ["sample.tsx", "-x", "//jsx_element", "--expect", "4"],
    ["sample.tsx", "-x", "//jsx_opening_element", "--expect", "4"],
    ["sample.tsx", "-x", "//jsx_closing_element", "--expect", "4"],
    ["sample.tsx", "-x", "//jsx_attribute", "--expect", "2"],
    ["sample.tsx", "-x", "//jsx_expression", "--expect", "5"],
    ["sample.tsx", "-x", "//jsx_text", "--expect", "5"],
);

// ==========================================================================
// JavaScript
// ==========================================================================

tractor_tests!(javascript, lang_dir("javascript"),
    ["sample.js", "-x", "function[name]", "--expect", "2"],
    ["sample.js", "-x", "function[name='add']", "--expect", "1"],
    ["sample.js", "-x", "function[name='main']", "--expect", "1"],
    ["sample.js", "-x", "program", "--expect", "1"],
    ["sample.js", "-x", "call", "--expect", "3"],
    ["sample.js", "-x", "call/function", "--expect", "3"],
    ["sample.js", "-x", "call/function[ref]", "--expect", "2"],
    ["sample.js", "-x", "call/function/member", "--expect", "1"],
    ["sample.js", "-x", "member/object", "--expect", "1"],
    ["sample.js", "-x", "member/property", "--expect", "1"],
);

// ==========================================================================
// Go
// ==========================================================================

tractor_tests!(go, lang_dir("go"),
    ["sample.go", "-x", "function", "--expect", "3"],
    ["sample.go", "-x", "function[name='add']", "--expect", "1"],
    ["sample.go", "-x", "function[name='main']", "--expect", "1"],
    ["sample.go", "-x", "file", "--expect", "1"],
    ["sample.go", "-x", "package", "--expect", "1"],
    ["sample.go", "-x", "binary[op='+']", "--expect", "1"],
    ["sample.go", "-x", "call", "--expect", "2"],
    ["sample.go", "-x", "function[exported]", "--expect", "1"],
    ["sample.go", "-x", "function[unexported]", "--expect", "2"],
);

// ==========================================================================
// Java
// ==========================================================================

tractor_tests!(java, lang_dir("java"),
    ["sample.java", "-x", "method", "--expect", "5"],
    ["sample.java", "-x", "method[name='add']", "--expect", "1"],
    ["sample.java", "-x", "class[name='Sample']", "--expect", "1"],
    ["sample.java", "-x", "program", "--expect", "1"],
    ["sample.java", "-x", "static", "--expect", "2"],
    ["sample.java", "-x", "binary[op='+']", "--expect", "2"],
    ["sample.java", "-x", "call", "--expect", "3"],
    ["sample.java", "-x", "//method[public]", "--expect", "2"],
    ["sample.java", "-x", "//method[package-private]", "--expect", "2"],
    ["sample.java", "-x", "//method[protected]", "--expect", "1"],
);

// ==========================================================================
// C#
// ==========================================================================

tractor_tests!(csharp_basic, lang_dir("csharp"),
    ["sample.cs", "-x", "method", "--expect", "5"],
    ["sample.cs", "-x", "method[name='Add']", "--expect", "1"],
    ["sample.cs", "-x", "class[name='Sample']", "--expect", "1"],
    ["sample.cs", "-x", "unit", "--expect", "1"],
    ["sample.cs", "-x", "static", "--expect", "2"],
    ["sample.cs", "-x", "binary[op='+']", "--expect", "1"],
    ["sample.cs", "-x", "call", "--expect", "4"],
    ["sample.cs", "-x", "int", "--expect", "2"],
    ["sample.cs", "-x", "//method[public]", "--expect", "1"],
    ["sample.cs", "-x", "//method[private]", "--expect", "2"],
    ["sample.cs", "-x", "//method[internal]", "--expect", "1"],
    ["sample.cs", "-x", "//method[protected]", "--expect", "1"],
);

tractor_tests!(csharp_ast_grep_comparisons, lang_dir("csharp"),
    ["attribute-maxlength-autotruncate.cs", "-x",
     "//property[attributes[contains(., 'MaxLength')]][not(attributes[contains(., 'AutoTruncate')])]/name",
     "--expect", "1"],
    ["attribute-maxlength-boolean.cs", "-x",
     "//property[type='bool'][attributes[contains(., 'MaxLength')]]/name",
     "--expect", "1"],
    ["mapper-extension-method.cs", "-x",
     "//class[static][contains(name, 'Mapper')]//method[public][static][count(parameters/parameter)=1][not(parameters/parameter/this)]/name",
     "--expect", "1"],
    ["namespaces-file-scoped.cs", "-x", "//namespace[body]", "--expect", "1"],
    ["repository-getall-orderby.cs", "-x",
     "//class[contains(name, 'Repository')][not(contains(name, 'Mock'))]//method[contains(name, 'GetAll')][not(contains(., 'OrderBy'))]/name",
     "--expect", "1"],
    ["query-asnotracking.cs", "-x",
     "//method[contains(name, 'Get')][contains(., '_context')][contains(., 'Map')][not(contains(., 'AsNoTracking'))]/name",
     "--expect", "1"],
);

tractor_tests!(csharp_generic_type_matching, lang_dir("csharp"),
    ["generic-type-match.cs", "-x", "//type[.='List<string>']", "--expect", "2"],
    ["generic-type-match.cs", "-x", "//type[.='Dictionary<string, int>']", "--expect", "2"],
    ["generic-type-match.cs", "-x", "//type[generic]", "--expect", "8"],
    ["generic-type-match.cs", "-x", "//type[.='List<Dictionary<string, User>>']", "--expect", "2"],
    ["generic-type-match.cs", "-x", "//type[generic]/arguments/type[.='string']", "--expect", "6"],
    // whitespace-insensitive matching
    ["generic-type-match.cs", "-W", "-x", "//type[.='Dictionary<string,int>']", "--expect", "2"],
);

tractor_tests!(csharp_null_forgiving_operator, lang_dir("csharp"),
    ["null-forgiving-operator.cs", "-x", "//postfix_unary_expression", "--expect", "5"],
    ["null-forgiving-operator.cs", "-x", "//ERROR", "--expect", "0"],
    ["null-forgiving-operator.cs", "-x", "//member[postfix_unary_expression]", "--expect", "4"],
);

// ==========================================================================
// Ruby
// ==========================================================================

tractor_tests!(ruby, lang_dir("ruby"),
    ["sample.rb", "-x", "method", "--expect", "2"],
    ["sample.rb", "-x", "method[name='add']", "--expect", "1"],
    ["sample.rb", "-x", "method[name='main']", "--expect", "1"],
    ["sample.rb", "-x", "call", "--expect", "2"],
);

// ==========================================================================
// XML Passthrough
// ==========================================================================

tractor_tests!(xml_passthrough, lang_dir("xml"),
    ["sample.xml", "-x", "item", "--expect", "3"],
    ["sample.xml", "-x", "item[@type='feature']", "--expect", "2"],
    ["sample.xml", "-x", "item[@type='bug']", "--expect", "1"],
    ["sample.xml", "-x", "setting", "--expect", "2"],
    ["sample.xml", "-x", "item[status='complete']", "--expect", "1"],
    ["sample.xml", "-x", "project/@name", "--expect", "1"],
    ["sample.xml", "-x", "name", "--expect", "3"],
    ["sample.xml", "-x", "item/name", "-v", "value", "--expect", "some"],
);

// ==========================================================================
// YAML
// ==========================================================================

tractor_tests!(yaml_data_view, lang_dir("yaml"),
    ["sample.yaml", "-x", "//name[.='my-app']", "--expect", "1"],
    ["sample.yaml", "-x", "//database/host[.='localhost']", "--expect", "1"],
    ["sample.yaml", "-x", "//database/port[.='5432']", "--expect", "1"],
    ["sample.yaml", "-x", "//database/credentials/username", "--expect", "1"],
    ["sample.yaml", "-x", "//servers", "--expect", "2"],
    ["sample.yaml", "-x", "//servers[name='web-1']", "--expect", "1"],
    ["sample.yaml", "-x", "//servers[name='web-1']/port[.='8080']", "--expect", "1"],
    ["sample.yaml", "-x", "//features", "--expect", "3"],
    ["sample.yaml", "-x", "//features[.='auth']", "--expect", "1"],
    ["sample.yaml", "-x", "//nested/level1/level2/value[.='deep']", "--expect", "1"],
    ["sample.yaml", "-x", "//flow_map/x[.='1']", "--expect", "1"],
    ["sample.yaml", "-x", "//flow_list", "--expect", "3"],
    ["sample.yaml", "-x", "//quoted[.='hello world']", "--expect", "1"],
    ["sample.yaml", "-x", "//multiline[contains(.,'line one')]", "--expect", "1"],
    ["sample.yaml", "-x", "//first_name", "--expect", "1"],
    ["sample.yaml", "-x", "//*[@key='first name']", "--expect", "1"],
    ["sample.yaml", "-x", "//first_name[text()='Alice']", "--expect", "1"],
);

tractor_tests!(yaml_multi_document, lang_dir("yaml"),
    ["multi.yaml", "-x", "//document", "--expect", "3"],
    ["multi.yaml", "-x", "//document[1]/name[.='doc1']", "--expect", "1"],
    ["multi.yaml", "-x", "//document[2]/name[.='doc2']", "--expect", "1"],
    ["multi.yaml", "-x", "//document[3]/value[.='three']", "--expect", "1"],
    ["multi.yaml", "-x", "//name", "--expect", "3"],
);

tractor_tests!(yaml_structure_mode, lang_dir("yaml"),
    ["sample.yaml", "-t", "structure", "-x", "//document/object", "--expect", "1"],
    ["sample.yaml", "-t", "structure", "-x", "//property[key/string='name']/value/string[.='my-app']", "--expect", "1"],
);

// ==========================================================================
// Markdown
// ==========================================================================

tractor_tests!(markdown, lang_dir("markdown"),
    ["sample.md", "-x", "//heading", "--expect", "2"],
    ["sample.md", "-x", "//heading[h1]", "--expect", "1"],
    ["sample.md", "-x", "//heading[h2]", "--expect", "1"],
    ["sample.md", "-x", "//list[ordered]", "--expect", "1"],
    ["sample.md", "-x", "//list[unordered]", "--expect", "1"],
    ["sample.md", "-x", "//item", "--expect", "5"],
    ["sample.md", "-x", "//blockquote", "--expect", "1"],
    ["sample.md", "-x", "//code_block", "--expect", "3"],
    ["sample.md", "-x", "//code_block[language='python']", "--expect", "1"],
    ["sample.md", "-x", "//code_block[language='javascript']", "--expect", "1"],
    ["sample.md", "-x", "//code_block[not(language)]", "--expect", "1"],
    ["sample.md", "-x", "//hr", "--expect", "1"],
);

#[test]
fn markdown_round_trip() {
    let dir = lang_dir("markdown");
    let js_code = tractor_stdout(&dir, &["sample.md", "-x", "//code_block[language='javascript']/code", "-v", "value"]);
    let result = tractor_run_stdin(&dir, &["-l", "javascript", "-x", "//function[name]", "-v", "count"], &js_code);
    assert!(result.success, "round-trip parse should succeed");
    assert_eq!(result.stdout.trim(), "1", "extracted JS code should contain 1 function");
}

// ==========================================================================
// T-SQL
// ==========================================================================

tractor_tests!(tsql_basic, lang_dir("tsql"),
    ["sample.sql", "--lang", "tsql", "-x", "file", "--expect", "1"],
    ["sample.sql", "--lang", "tsql", "-x", "statement", "--expect", "24"],
);

tractor_tests!(tsql_dml, lang_dir("tsql"),
    ["sample.sql", "--lang", "tsql", "-x", "select", "--expect", "17"],
    ["sample.sql", "--lang", "tsql", "-x", "insert", "--expect", "1"],
    ["sample.sql", "--lang", "tsql", "-x", "delete", "--expect", "1"],
    ["sample.sql", "--lang", "tsql", "-x", "update", "--expect", "3"],
);

tractor_tests!(tsql_clauses, lang_dir("tsql"),
    ["sample.sql", "--lang", "tsql", "-x", "where", "--expect", "14"],
    ["sample.sql", "--lang", "tsql", "-x", "order_by", "--expect", "3"],
    ["sample.sql", "--lang", "tsql", "-x", "group_by", "--expect", "1"],
    ["sample.sql", "--lang", "tsql", "-x", "having", "--expect", "1"],
);

tractor_tests!(tsql_joins_subqueries, lang_dir("tsql"),
    ["sample.sql", "--lang", "tsql", "-x", "join", "--expect", "2"],
    ["sample.sql", "--lang", "tsql", "-x", "subquery", "--expect", "2"],
    ["sample.sql", "--lang", "tsql", "-x", "exists", "--expect", "1"],
    ["sample.sql", "--lang", "tsql", "-x", "cte", "--expect", "1"],
    ["sample.sql", "--lang", "tsql", "-x", "union", "--expect", "1"],
);

tractor_tests!(tsql_expressions, lang_dir("tsql"),
    ["sample.sql", "--lang", "tsql", "-x", "case", "--expect", "1"],
    ["sample.sql", "--lang", "tsql", "-x", "between", "--expect", "1"],
    ["sample.sql", "--lang", "tsql", "-x", "compare[op='>']", "--expect", "4"],
    ["sample.sql", "--lang", "tsql", "-x", "compare[op='>=']", "--expect", "1"],
);

tractor_tests!(tsql_functions, lang_dir("tsql"),
    ["sample.sql", "--lang", "tsql", "-x", "call", "--expect", "9"],
    ["sample.sql", "--lang", "tsql", "-x", "cast", "--expect", "1"],
    ["sample.sql", "--lang", "tsql", "-x", "window", "--expect", "1"],
    ["sample.sql", "--lang", "tsql", "-x", "partition_by", "--expect", "1"],
    ["sample.sql", "--lang", "tsql", "-x", "star", "--expect", "2"],
);

tractor_tests!(tsql_identifiers, lang_dir("tsql"),
    ["sample.sql", "--lang", "tsql", "-x", "alias", "--expect", "17"],
    ["sample.sql", "--lang", "tsql", "-x", "schema", "--expect", "4"],
    ["sample.sql", "--lang", "tsql", "-x", "var", "--expect", "6"],
    ["sample.sql", "--lang", "tsql", "-x", "temp_ref", "--expect", "1"],
    ["sample.sql", "--lang", "tsql", "-x", "direction", "--expect", "2"],
);

tractor_tests!(tsql_ddl, lang_dir("tsql"),
    ["sample.sql", "--lang", "tsql", "-x", "create_table", "--expect", "1"],
    ["sample.sql", "--lang", "tsql", "-x", "col_def", "--expect", "3"],
    ["sample.sql", "--lang", "tsql", "-x", "create_function", "--expect", "1"],
);

tractor_tests!(tsql_advanced, lang_dir("tsql"),
    ["sample.sql", "--lang", "tsql", "-x", "assign", "--expect", "4"],
    ["sample.sql", "--lang", "tsql", "-x", "when", "--expect", "2"],
    ["sample.sql", "--lang", "tsql", "-x", "transaction", "--expect", "1"],
    ["sample.sql", "--lang", "tsql", "-x", "set", "--expect", "1"],
    ["sample.sql", "--lang", "tsql", "-x", "go", "--expect", "1"],
    ["sample.sql", "--lang", "tsql", "-x", "exec", "--expect", "1"],
    ["sample.sql", "--lang", "tsql", "-x", "comment", "--expect", "20"],
);

// ==========================================================================
// INI
// ==========================================================================

tractor_tests!(ini, lang_dir("ini"),
    ["sample.ini", "-x", "//name[.='my-app']", "--expect", "1"],
    ["sample.ini", "-x", "//version[.='1.0.0']", "--expect", "1"],
    ["sample.ini", "-x", "//database/host[.='localhost']", "--expect", "1"],
    ["sample.ini", "-x", "//database/port[.='5432']", "--expect", "1"],
    ["sample.ini", "-x", "//database/enabled[.='true']", "--expect", "1"],
    ["sample.ini", "-x", "//database.credentials/username[.='admin']", "--expect", "1"],
    ["sample.ini", "-x", "//database.credentials/password[.='secret']", "--expect", "1"],
    ["sample.ini", "-x", "//servers/count[.='2']", "--expect", "1"],
    ["sample.ini", "-x", "//paths/home[.='/usr/local']", "--expect", "1"],
    ["sample.ini", "-x", "//paths/temp[.='/tmp']", "--expect", "1"],
    ["sample.ini", "-x", "//comment", "--expect", "2"],
    ["sample.ini", "-x", "//comment[.='Global settings']", "--expect", "1"],
    ["sample.ini", "-x", "//document", "--expect", "1"],
);

// ==========================================================================
// TOML
// ==========================================================================

tractor_tests!(toml, lang_dir("toml"),
    ["sample.toml", "-x", "//title[.='My App']", "--expect", "1"],
    ["sample.toml", "-x", "//version[.='1.0.0']", "--expect", "1"],
    ["sample.toml", "-x", "//database/host[.='localhost']", "--expect", "1"],
    ["sample.toml", "-x", "//database/port[.='5432']", "--expect", "1"],
    ["sample.toml", "-x", "//database/enabled[.='true']", "--expect", "1"],
    ["sample.toml", "-x", "//database/credentials/username", "--expect", "1"],
    ["sample.toml", "-x", "//database/credentials/password[.='secret']", "--expect", "1"],
    ["sample.toml", "-x", "//servers/item", "--expect", "2"],
    ["sample.toml", "-x", "//servers/item[name='web-1']", "--expect", "1"],
    ["sample.toml", "-x", "//servers/item[name='web-1']/port[.='8080']", "--expect", "1"],
    ["sample.toml", "-x", "//features/item", "--expect", "3"],
    ["sample.toml", "-x", "//features/item[.='auth']", "--expect", "1"],
    ["sample.toml", "-x", "//inline/x[.='1']", "--expect", "1"],
    ["sample.toml", "-x", "//inline/y[.='2']", "--expect", "1"],
    ["sample.toml", "-x", "//quoted[.='hello world']", "--expect", "1"],
    ["sample.toml", "-x", "//first_name", "--expect", "1"],
    ["sample.toml", "-x", "//*[@key='first name']", "--expect", "1"],
    ["sample.toml", "-x", "//nested/level1/level2/value[.='deep']", "--expect", "1"],
    ["sample.toml", "-x", "//document", "--expect", "1"],
);
