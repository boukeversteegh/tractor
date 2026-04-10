#[macro_use]
mod support;

use support::{case, expect, inline, query};

cli_suite! {
    rust in "languages/rust" {
        functions_exist => expect("sample.rs", "function", "4");
        add_name => expect("sample.rs", "function[name='add']", "1");
        main_name => expect("sample.rs", "function[name='main']", "1");
        file_rename => expect("sample.rs", "file", "1");
        let_rename => expect("sample.rs", "let", "1");
        binary_op => expect("sample.rs", "binary[op='+']", "1");
        call_rename => expect("sample.rs", "call", "1");
        macro_rename => expect("sample.rs", "macro", "1");
        pub_functions => expect("sample.rs", "function[pub]", "3");
        plain_pub => expect("sample.rs", "function[pub[not(*)]]", "1");
        pub_crate => expect("sample.rs", "function[pub[crate]]", "1");
        private_marker => expect("sample.rs", "function[private]", "1");
    }
}

cli_suite! {
    csharp in "languages/csharp" {
        methods_exist => expect("sample.cs", "method", "5");
        method_name => expect("sample.cs", "method[name='Add']", "1");
        class_name => expect("sample.cs", "class[name='Sample']", "1");
        unit_rename => expect("sample.cs", "unit", "1");
        static_marker => expect("sample.cs", "static", "2");
        binary_op => expect("sample.cs", "binary[op='+']", "1");
        call_rename => expect("sample.cs", "call", "4");
        ints_exist => expect("sample.cs", "int", "2");
        public_methods => expect("sample.cs", "//method[public]", "1");
        private_methods => expect("sample.cs", "//method[private]", "2");
        internal_methods => expect("sample.cs", "//method[internal]", "1");
        protected_methods => expect("sample.cs", "//method[protected]", "1");
        maxlength_missing_autotruncate => expect("attribute-maxlength-autotruncate.cs", "//property[attributes[contains(., 'MaxLength')]][not(attributes[contains(., 'AutoTruncate')])]/name", "1");
        maxlength_on_bool => expect("attribute-maxlength-boolean.cs", "//property[type='bool'][attributes[contains(., 'MaxLength')]]/name", "1");
        mapper_extension_method => expect("mapper-extension-method.cs", "//class[static][contains(name, 'Mapper')]//method[public][static][count(parameters/parameter)=1][not(parameters/parameter/this)]/name", "1");
        block_scoped_namespace => expect("namespaces-file-scoped.cs", "//namespace[body]", "1");
        repository_getall_missing_orderby => expect("repository-getall-orderby.cs", "//class[contains(name, 'Repository')][not(contains(name, 'Mock'))]//method[contains(name, 'GetAll')][not(contains(., 'OrderBy'))]/name", "1");
        query_missing_asnotracking => expect("query-asnotracking.cs", "//method[contains(name, 'Get')][contains(., '_context')][contains(., 'Map')][not(contains(., 'AsNoTracking'))]/name", "1");
        generic_list_match => expect("generic-type-match.cs", "//type[.='List<string>']", "2");
        generic_dictionary_match => expect("generic-type-match.cs", "//type[.='Dictionary<string, int>']", "2");
        generic_count => expect("generic-type-match.cs", "//type[generic]", "8");
        nested_generic_match => expect("generic-type-match.cs", "//type[.='List<Dictionary<string, User>>']", "2");
        generic_string_args => expect("generic-type-match.cs", "//type[generic]/arguments/type[.='string']", "6");
        ignore_whitespace => expect("generic-type-match.cs", "//type[.='Dictionary<string,int>']", "2").arg("-W");
        null_forgiving_postfix => expect("null-forgiving-operator.cs", "//postfix_unary_expression", "5");
        null_forgiving_no_errors => expect("null-forgiving-operator.cs", "//ERROR", "0");
        null_forgiving_member_access => expect("null-forgiving-operator.cs", "//member[postfix_unary_expression]", "4");
    }
}

cli_suite! {
    go in "languages/go" {
        functions_exist => expect("sample.go", "function", "3");
        add_name => expect("sample.go", "function[name='add']", "1");
        main_name => expect("sample.go", "function[name='main']", "1");
        file_rename => expect("sample.go", "file", "1");
        package_clause => expect("sample.go", "package", "1");
        binary_op => expect("sample.go", "binary[op='+']", "1");
        call_rename => expect("sample.go", "call", "2");
        exported => expect("sample.go", "function[exported]", "1");
        unexported => expect("sample.go", "function[unexported]", "2");
    }
}

cli_suite! {
    ini in "languages/ini" {
        global_name => expect("sample.ini", "//name[.='my-app']", "1");
        version => expect("sample.ini", "//version[.='1.0.0']", "1");
        host => expect("sample.ini", "//database/host[.='localhost']", "1");
        port => expect("sample.ini", "//database/port[.='5432']", "1");
        enabled => expect("sample.ini", "//database/enabled[.='true']", "1");
        dotted_section_user => expect("sample.ini", "//database.credentials/username[.='admin']", "1");
        dotted_section_password => expect("sample.ini", "//database.credentials/password[.='secret']", "1");
        servers_count => expect("sample.ini", "//servers/count[.='2']", "1");
        home_path => expect("sample.ini", "//paths/home[.='/usr/local']", "1");
        temp_path => expect("sample.ini", "//paths/temp[.='/tmp']", "1");
        comments => expect("sample.ini", "//comment", "2");
        comment_text => expect("sample.ini", "//comment[.='Global settings']", "1");
        document_root => expect("sample.ini", "//document", "1");
    }
}

cli_suite! {
    java in "languages/java" {
        methods_exist => expect("sample.java", "method", "5");
        method_name => expect("sample.java", "method[name='add']", "1");
        class_name => expect("sample.java", "class[name='Sample']", "1");
        program => expect("sample.java", "program", "1");
        static_marker => expect("sample.java", "static", "2");
        binary_ops => expect("sample.java", "binary[op='+']", "2");
        calls => expect("sample.java", "call", "3");
        public_methods => expect("sample.java", "//method[public]", "2");
        package_private => expect("sample.java", "//method[package-private]", "2");
        protected_methods => expect("sample.java", "//method[protected]", "1");
    }
}

cli_suite! {
    javascript in "languages/javascript" {
        named_functions => expect("sample.js", "function[name]", "2");
        add_name => expect("sample.js", "function[name='add']", "1");
        main_name => expect("sample.js", "function[name='main']", "1");
        program => expect("sample.js", "program", "1");
        calls => expect("sample.js", "call", "3");
        call_function_child => expect("sample.js", "call/function", "3");
        direct_call_ref => expect("sample.js", "call/function[ref]", "2");
        member_call_shape => expect("sample.js", "call/function/member", "1");
        member_object => expect("sample.js", "member/object", "1");
        member_property => expect("sample.js", "member/property", "1");
    }
}

cli_suite! {
    markdown in "languages/markdown" {
        headings => expect("sample.md", "//heading", "2");
        h1 => expect("sample.md", "//heading[h1]", "1");
        h2 => expect("sample.md", "//heading[h2]", "1");
        ordered_list => expect("sample.md", "//list[ordered]", "1");
        unordered_list => expect("sample.md", "//list[unordered]", "1");
        items => expect("sample.md", "//item", "5");
        blockquote => expect("sample.md", "//blockquote", "1");
        code_blocks => expect("sample.md", "//code_block", "3");
        python_block => expect("sample.md", "//code_block[language='python']", "1");
        javascript_block => expect("sample.md", "//code_block[language='javascript']", "1");
        unlabeled_block => expect("sample.md", "//code_block[not(language)]", "1");
        hr => expect("sample.md", "//hr", "1");
    }
}

cli_suite! {
    python in "languages/python" {
        functions_exist => expect("sample.py", "function", "3");
        add_name => expect("sample.py", "function[name='add']", "1");
        main_name => expect("sample.py", "function[name='main']", "1");
        module => expect("sample.py", "module", "1");
        returns => expect("sample.py", "return", "2");
        binary_op => expect("sample.py", "binary[op='+']", "1");
        calls => expect("sample.py", "call", "3");
        async_function => expect("sample.py", "function[async]", "1");
        multiline_lf => expect("multiline-string-lf.py", "//string_content[.=\"hello\n\n\"]", "1");
        multiline_crlf => expect("multiline-string-crlf.py", "//string_content[.=\"hello\n\n\"]", "1");
    }
}

cli_suite! {
    ruby in "languages/ruby" {
        methods_exist => expect("sample.rb", "method", "2");
        add_name => expect("sample.rb", "method[name='add']", "1");
        main_name => expect("sample.rb", "method[name='main']", "1");
        calls => expect("sample.rb", "call", "2");
    }
}

cli_suite! {
    toml in "languages/toml" {
        title => expect("sample.toml", "//title[.='My App']", "1");
        version => expect("sample.toml", "//version[.='1.0.0']", "1");
        host => expect("sample.toml", "//database/host[.='localhost']", "1");
        port => expect("sample.toml", "//database/port[.='5432']", "1");
        enabled => expect("sample.toml", "//database/enabled[.='true']", "1");
        dotted_user => expect("sample.toml", "//database/credentials/username", "1");
        dotted_password => expect("sample.toml", "//database/credentials/password[.='secret']", "1");
        servers => expect("sample.toml", "//servers/item", "2");
        server_web1 => expect("sample.toml", "//servers/item[name='web-1']", "1");
        server_web1_port => expect("sample.toml", "//servers/item[name='web-1']/port[.='8080']", "1");
        features => expect("sample.toml", "//features/item", "3");
        feature_auth => expect("sample.toml", "//features/item[.='auth']", "1");
        inline_x => expect("sample.toml", "//inline/x[.='1']", "1");
        inline_y => expect("sample.toml", "//inline/y[.='2']", "1");
        quoted => expect("sample.toml", "//quoted[.='hello world']", "1");
        sanitized_key => expect("sample.toml", "//first_name", "1");
        original_key => expect("sample.toml", "//*[@key='first name']", "1");
        deep_nested => expect("sample.toml", "//nested/level1/level2/value[.='deep']", "1");
        document_root => expect("sample.toml", "//document", "1");
    }
}

cli_suite! {
    tsql in "languages/tsql" {
        file_root => expect("sample.sql", "file", "1").lang("tsql");
        statements => expect("sample.sql", "statement", "24").lang("tsql");
        selects => expect("sample.sql", "select", "17").lang("tsql");
        inserts => expect("sample.sql", "insert", "1").lang("tsql");
        deletes => expect("sample.sql", "delete", "1").lang("tsql");
        updates => expect("sample.sql", "update", "3").lang("tsql");
        where_clauses => expect("sample.sql", "where", "14").lang("tsql");
        order_by => expect("sample.sql", "order_by", "3").lang("tsql");
        group_by => expect("sample.sql", "group_by", "1").lang("tsql");
        having => expect("sample.sql", "having", "1").lang("tsql");
        joins => expect("sample.sql", "join", "2").lang("tsql");
        subqueries => expect("sample.sql", "subquery", "2").lang("tsql");
        exists_predicate => expect("sample.sql", "exists", "1").lang("tsql");
        cte => expect("sample.sql", "cte", "1").lang("tsql");
        union_all => expect("sample.sql", "union", "1").lang("tsql");
        case_expr => expect("sample.sql", "case", "1").lang("tsql");
        between_expr => expect("sample.sql", "between", "1").lang("tsql");
        compare_gt => expect("sample.sql", "compare[op='>']", "4").lang("tsql");
        compare_gte => expect("sample.sql", "compare[op='>=']", "1").lang("tsql");
        calls => expect("sample.sql", "call", "9").lang("tsql");
        cast_expr => expect("sample.sql", "cast", "1").lang("tsql");
        window => expect("sample.sql", "window", "1").lang("tsql");
        partition_by => expect("sample.sql", "partition_by", "1").lang("tsql");
        star => expect("sample.sql", "star", "2").lang("tsql");
        aliases => expect("sample.sql", "alias", "17").lang("tsql");
        schema_refs => expect("sample.sql", "schema", "4").lang("tsql");
        variables => expect("sample.sql", "var", "6").lang("tsql");
        temp_table => expect("sample.sql", "temp_ref", "1").lang("tsql");
        direction => expect("sample.sql", "direction", "2").lang("tsql");
        create_table => expect("sample.sql", "create_table", "1").lang("tsql");
        column_defs => expect("sample.sql", "col_def", "3").lang("tsql");
        create_function => expect("sample.sql", "create_function", "1").lang("tsql");
        assignments => expect("sample.sql", "assign", "4").lang("tsql");
        merge_when => expect("sample.sql", "when", "2").lang("tsql");
        transaction => expect("sample.sql", "transaction", "1").lang("tsql");
        set_stmt => expect("sample.sql", "set", "1").lang("tsql");
        go_separator => expect("sample.sql", "go", "1").lang("tsql");
        exec => expect("sample.sql", "exec", "1").lang("tsql");
        comments => expect("sample.sql", "comment", "20").lang("tsql");
    }
}

cli_suite! {
    tsx in "languages/tsx" {
        program => expect("sample.tsx", "program", "1");
        functions => expect("sample.tsx", "function[name]", "1");
        component_name => expect("sample.tsx", "function[name='Greeting']", "1");
        interface => expect("sample.tsx", "interface", "1");
        variable => expect("sample.tsx", "variable", "1");
        jsx_elements => expect("sample.tsx", "//jsx_element", "4");
        jsx_opening => expect("sample.tsx", "//jsx_opening_element", "4");
        jsx_closing => expect("sample.tsx", "//jsx_closing_element", "4");
        jsx_attributes => expect("sample.tsx", "//jsx_attribute", "2");
        jsx_expressions => expect("sample.tsx", "//jsx_expression", "5");
        jsx_text => expect("sample.tsx", "//jsx_text", "5");
    }
}

cli_suite! {
    typescript in "languages/typescript" {
        functions => expect("sample.ts", "function[name]", "4");
        add_name => expect("sample.ts", "function[name='add']", "1");
        main_name => expect("sample.ts", "function[name='main']", "1");
        program => expect("sample.ts", "program", "1");
        variable => expect("sample.ts", "variable", "1");
        binary_op => expect("sample.ts", "binary[op='+']", "1");
        calls => expect("sample.ts", "call", "4");
        optional_params => expect("sample.ts", "//param[optional]", "2");
        required_params => expect("sample.ts", "//param[required]", "5");
    }
}

cli_suite! {
    xml in "languages/xml" {
        items => expect("sample.xml", "item", "3");
        feature_items => expect("sample.xml", "item[@type='feature']", "2");
        bug_items => expect("sample.xml", "item[@type='bug']", "1");
        settings => expect("sample.xml", "setting", "2");
        complete_items => expect("sample.xml", "item[status='complete']", "1");
        attributes => expect("sample.xml", "project/@name", "1");
        names => expect("sample.xml", "name", "3");
        value_view => expect("sample.xml", "item/name", "some").view("value");
    }
}

cli_suite! {
    yaml in "languages/yaml" {
        top_level_scalar => expect("sample.yaml", "//name[.='my-app']", "1");
        nested_host => expect("sample.yaml", "//database/host[.='localhost']", "1");
        nested_port => expect("sample.yaml", "//database/port[.='5432']", "1");
        deep_mapping => expect("sample.yaml", "//database/credentials/username", "1");
        repeated_servers => expect("sample.yaml", "//servers", "2");
        server_mapping => expect("sample.yaml", "//servers[name='web-1']", "1");
        server_port => expect("sample.yaml", "//servers[name='web-1']/port[.='8080']", "1");
        features => expect("sample.yaml", "//features", "3");
        feature_auth => expect("sample.yaml", "//features[.='auth']", "1");
        deep_nested => expect("sample.yaml", "//nested/level1/level2/value[.='deep']", "1");
        flow_map => expect("sample.yaml", "//flow_map/x[.='1']", "1");
        flow_list => expect("sample.yaml", "//flow_list", "3");
        quoted => expect("sample.yaml", "//quoted[.='hello world']", "1");
        multiline => expect("sample.yaml", "//multiline[contains(.,'line one')]", "1");
        sanitized_key => expect("sample.yaml", "//first_name", "1");
        original_key => expect("sample.yaml", "//*[@key='first name']", "1");
        sanitized_text => expect("sample.yaml", "//first_name[text()='Alice']", "1");
        multi_doc_root => expect("multi.yaml", "//document", "3");
        multi_doc_first => expect("multi.yaml", "//document[1]/name[.='doc1']", "1");
        multi_doc_second => expect("multi.yaml", "//document[2]/name[.='doc2']", "1");
        multi_doc_third => expect("multi.yaml", "//document[3]/value[.='three']", "1");
        multi_doc_descendants => expect("multi.yaml", "//name", "3");
        structure_root => expect("sample.yaml", "//document/object", "1").tree("structure");
        structure_vocab => expect("sample.yaml", "//property[key/string='name']/value/string[.='my-app']", "1").tree("structure");
    }
}

cli_suite! {
    string_input in "string-input" {
        rust_string => inline("rust", "fn add(a: i32, b: i32) -> i32 { a + b }", "function").expect("1");
        python_string => inline("python", "def hello(): pass", "function").expect("1");
        csharp_string => inline("csharp", "public class Foo { public void Bar() {} }", "class").expect("1");
        javascript_string => inline("javascript", "function greet() { return 'hi'; }", "function").expect("1");
        typescript_string => inline("typescript", "const greet = (): string => 'hi';", "lambda").expect("1");
        short_flag => inline("rust", "fn main() {}", "function").expect("1");
        expect_exact => inline("rust", "fn a() {} fn b() {}", "function").expect("2");
        expect_some => inline("rust", "fn a() {} fn b() {}", "function").expect("some");
        expect_none => inline("rust", "let x = 1;", "function").expect("none");
        value_output => inline("csharp", "class Foo { }", "class/name").view("value").expect("1");
        count_output => inline("csharp", "class Foo { }", "class").view("count").expect("1");
        gcc_output => inline("csharp", "class Foo { }", "class").format("gcc").expect("1");
        without_xpath => case(["test", "-s", "let x = 1;", "-l", "rust", "-v", "count", "--expect", "1"]);
    }
}

cli_suite! {
    xpath_expressions in "xpath-expressions" {
        let_expression => inline("typescript", "let x = 1; let y = 2;", "let $v := //variable return $v/name").view("value").expect("2");
        for_expression => inline("typescript", "let x = 1; let y = 2;", "for $v in //name return string($v)").view("value").expect("2");
        if_true_branch => inline("typescript", "let x = 1;", "if (//variable) then //name else ()").view("value").expect("1");
        if_false_branch => inline("typescript", "let x = 1;", "if (//function) then //name else //variable").view("value").expect("1");
        some_quantified => inline("javascript", "function add(a, b) { return a + b; }", "some $f in //function satisfies $f/name = 'add'").expect("1");
        every_quantified => inline("javascript", "function add(a, b) { return a + b; }", "every $f in //function satisfies $f/name = 'add'").expect("1");
        variable_reference => inline("typescript", "let x = 1;", "let $v := //name return $v").view("value").expect("1");
        bare_element_name => inline("typescript", "let x = 1;", "variable").expect("1");
        bare_element_predicate => inline("javascript", "function foo() {}", "function[name='foo']").expect("1");
    }
}

#[test]
fn markdown_round_trip_extracts_javascript_block() {
    let extracted = query("sample.md", "//code_block[language='javascript']/code")
        .view("value")
        .in_fixture("languages/markdown")
        .capture();
    assert_eq!(0, extracted.status);

    let parsed = case(["-l", "javascript", "-x", "//function[name]", "-v", "count"])
        .stdin(format!("{}\n", extracted.stdout))
        .capture();

    assert_eq!(0, parsed.status);
    assert_eq!("1", parsed.stdout);
}

#[test]
fn string_input_requires_language() {
    case(["--string", "let x = 1;"]).fails().run();
}

#[test]
fn set_snapshot_text_default() {
    case(["set", "sample.yaml", "-x", "//database/host", "--value", "db.example.com"])
        .in_fixture("formats/set")
        .temp_fixture()
        .replace_output("sample.yaml", "tests/integration/formats/set/sample.yaml")
        .stdout_snapshot("formats/set/set.txt")
        .run();
}

#[test]
fn set_snapshot_text_unchanged() {
    case(["set", "sample.yaml", "-x", "//database/host", "--value", "localhost"])
        .in_fixture("formats/set")
        .temp_fixture()
        .replace_output("sample.yaml", "tests/integration/formats/set/sample.yaml")
        .stdout_snapshot("formats/set/set-unchanged.txt")
        .run();
}

#[test]
fn set_snapshot_stdout_mode() {
    case(["set", "sample.yaml", "-x", "//database/host", "--value", "db.example.com", "--stdout"])
        .in_fixture("formats/set")
        .temp_fixture()
        .stdout_snapshot("formats/set/set-stdout.txt")
        .run();
}

#[test]
fn set_snapshot_json() {
    case(["set", "sample.yaml", "-x", "//database/host", "--value", "db.example.com", "-f", "json"])
        .in_fixture("formats/set")
        .temp_fixture()
        .replace_output("sample.yaml", "tests/integration/formats/set/sample.yaml")
        .stdout_snapshot("formats/set/set.json")
        .run();
}

#[test]
fn set_snapshot_xml() {
    case(["set", "sample.yaml", "-x", "//database/host", "--value", "db.example.com", "-f", "xml"])
        .in_fixture("formats/set")
        .temp_fixture()
        .replace_output("sample.yaml", "tests/integration/formats/set/sample.yaml")
        .stdout_snapshot("formats/set/set.xml")
        .run();
}

#[test]
fn replace_updates_yaml_values_in_place() {
    case(["set", "single.yaml", "-x", "//database/host", "--value", "db.example.com"])
        .in_fixture("replace")
        .temp_fixture()
        .seed_file("single.yaml", "name: my-app\ndatabase:\n  host: localhost\n  port: 5432\n")
        .file_eq("single.yaml", "name: my-app\ndatabase:\n  host: db.example.com\n  port: 5432")
        .run();
}

#[test]
fn replace_updates_multiple_yaml_values() {
    case(["set", "multi.yaml", "-x", "//servers/port[.='8080']", "--value", "3000"])
        .in_fixture("replace")
        .temp_fixture()
        .seed_file("multi.yaml", "servers:\n  - name: web-1\n    port: 8080\n  - name: web-2\n    port: 8080\n  - name: web-3\n    port: 9090\n")
        .file_eq("multi.yaml", "servers:\n  - name: web-1\n    port: 3000\n  - name: web-2\n    port: 3000\n  - name: web-3\n    port: 9090")
        .run();
}

#[test]
fn replace_stdout_mode_does_not_modify_file() {
    case(["set", "stdout.yaml", "-x", "//host", "--value", "example.com", "--stdout"])
        .in_fixture("replace")
        .temp_fixture()
        .seed_file("stdout.yaml", "host: localhost\n")
        .stdout("host: example.com")
        .file_eq("stdout.yaml", "host: localhost")
        .run();
}

#[test]
fn replace_stdin_implicitly_writes_stdout() {
    case(["set", "-l", "yaml", "-x", "//name", "--value", "newvalue"])
        .in_fixture("replace")
        .temp_fixture()
        .stdin("name: test\n")
        .stdout("name: newvalue")
        .run();
}

#[test]
fn replace_without_xpath_fails() {
    case(["set", "data.json", "--value", "foo"])
        .in_fixture("replace")
        .temp_fixture()
        .seed_file("data.json", "{\n  \"name\": \"value\"\n}\n")
        .fails()
        .run();
}

#[test]
fn update_changes_existing_values_but_not_missing_nodes() {
    case(["update", "single.yaml", "-x", "//database/host", "--value", "db.example.com"])
        .in_fixture("update")
        .temp_fixture()
        .seed_file("single.yaml", "name: my-app\ndatabase:\n  host: localhost\n  port: 5432\n")
        .file_eq("single.yaml", "name: my-app\ndatabase:\n  host: db.example.com\n  port: 5432")
        .run();
}

#[test]
fn update_missing_path_fails_without_creating_nodes() {
    case(["update", "nocreate.yaml", "-x", "//database/host", "--value", "localhost"])
        .in_fixture("update")
        .temp_fixture()
        .seed_file("nocreate.yaml", "name: my-app\n")
        .fails()
        .file_eq("nocreate.yaml", "name: my-app")
        .run();
}

#[test]
fn update_partial_path_fails_without_creating_leaf() {
    case(["update", "partial.yaml", "-x", "//database/port", "--value", "5432"])
        .in_fixture("update")
        .temp_fixture()
        .seed_file("partial.yaml", "database:\n  host: localhost\n")
        .fails()
        .file_eq("partial.yaml", "database:\n  host: localhost")
        .run();
}

#[test]
fn update_rejects_stdin_input() {
    case(["update", "--lang", "yaml", "-x", "//name", "--value", "new"])
        .in_fixture("update")
        .temp_fixture()
        .stdin("name: test\n")
        .fails()
        .run();
}

#[test]
fn run_multirule_output_is_stable() {
    case(["run", "check-multirule.yaml"])
        .in_fixture("run")
        .fixture_prefix("")
        .status(1)
        .combined("settings.yaml:3:10: error: debug should be disabled in production\n3 |   debug: true\n             ^~~~\n\nsettings.yaml:4:14: warning: log level should not be debug in production\n4 |   log_level: debug\n                 ^~~~~\n\n1 error in 1 file")
        .run();
}

#[test]
fn run_scope_intersection_respects_root() {
    case(["run", "scope-intersection/intersect-narrow.yaml"])
        .in_fixture("run")
        .fixture_prefix("")
        .combined("scope-intersection/frontend/config.yml:1:8: warning: debug must be disabled\n1 | debug: true\n           ^~~~\n\n1 warning in 1 file")
        .run();
}

#[test]
fn run_double_star_glob_matches_recursively() {
    case(["run", "glob-double-star/check-double-star.yaml"])
        .in_fixture("run")
        .fixture_prefix("")
        .combined("glob-double-star/config.yml:1:8: warning: debug must be disabled\n1 | debug: true\n           ^~~~\n\nglob-double-star/nested/config.yml:1:8: warning: debug must be disabled\n1 | debug: true\n           ^~~~\n\n2 warnings in 2 files")
        .run();
}

#[test]
fn run_nested_double_star_glob_matches_nested_files() {
    case(["run", "glob-double-star/check-dir-double-star.yaml"])
        .in_fixture("run")
        .fixture_prefix("")
        .combined("glob-double-star/nested/config.yml:1:8: warning: debug must be disabled\n1 | debug: true\n           ^~~~\n\n1 warning in 1 file")
        .run();
}

#[test]
fn run_absolute_cli_path_with_root_files_intersection() {
    case(["run", "absolute-paths/check-root-files.yaml"])
        .in_fixture("run")
        .fixture_prefix("")
        .abs_arg("absolute-paths/config.yml")
        .combined("absolute-paths/config.yml:1:8: warning: debug must be disabled\n1 | debug: true\n           ^~~~\n\n1 warning in 1 file")
        .run();
}

#[test]
fn run_mixed_language_rules_report_all_findings() {
    case(["run", "mixed-language/three-langs.yaml"])
        .in_fixture("run")
        .fixture_prefix("")
        .status(1)
        .combined("mixed-language/config.yaml:3:10: error: Debug mode must be disabled\n3 |   debug: true\n             ^~~~\n\nmixed-language/sample.js:1:1: error: TODO comment found\n1 | // TODO: Fix this code\n    ^~~~~~~~~~~~~~~~~~~~~~\n\nmixed-language/todo-doc.md:3:1: warning: TODO comment found\n3 >| <!-- TODO: Complete this section -->\n4 >| \n\n2 errors in 3 files")
        .run();
}

#[test]
fn view_modifier_can_drop_lines_in_gcc_output() {
    let result = case(["check", "sample.cs", "-x", "//class", "--reason", "class found", "-f", "gcc", "-v=-lines"])
        .in_fixture("formats")
        .capture();

    assert_eq!(1, result.status);
    assert_eq!(2, result.stdout.lines().filter(|line| line.contains(": error:")).count());
    assert!(!result.stdout.lines().any(|line| line.trim_start().chars().next().is_some_and(|c| c.is_ascii_digit()) && line.contains(">|")));
}

#[test]
fn view_modifier_can_add_source_and_remove_tree() {
    let without_tree = query("sample.cs", "//class/name")
        .arg("-v=-tree")
        .in_fixture("formats")
        .capture();
    assert_eq!(0, without_tree.status);
    assert!(!without_tree.stdout.contains('<'));

    let with_source = query("sample.cs", "//class/name")
        .arg("-v=+source")
        .in_fixture("formats")
        .capture();
    assert_eq!(0, with_source.status);
    assert!(with_source.stdout.lines().any(|line| line == "Foo" || line == "Qux"));
}

#[test]
fn view_modifier_is_idempotent_for_existing_fields() {
    let default_out = query("sample.cs", "//class/name")
        .in_fixture("formats")
        .capture();
    let modified_out = query("sample.cs", "//class/name")
        .arg("-v=+tree")
        .in_fixture("formats")
        .capture();

    assert_eq!(0, default_out.status);
    assert_eq!(0, modified_out.status);
    assert_eq!(default_out.stdout, modified_out.stdout);
}

#[test]
fn view_modifier_rejects_invalid_combinations() {
    case(["sample.cs", "-x", "//class", "-v=tree,+source"])
        .in_fixture("formats")
        .fails()
        .run();

    case(["sample.cs", "-x", "//class/name", "-v=-file,-line,-tree"])
        .in_fixture("formats")
        .fails()
        .run();

    case(["sample.cs", "-x", "//class", "-v=-nosuchfield"])
        .in_fixture("formats")
        .fails()
        .run();
}
