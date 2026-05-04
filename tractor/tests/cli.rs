#![recursion_limit = "2048"]

#[macro_use]
mod support;

use support::{command, query_command};
use quick_xml::events::Event;
use serde_json::Value;

fn assert_well_formed_xml(xml: &str) {
    let mut reader = quick_xml::Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(err) => panic!("expected well-formed XML, got {err}: {xml}"),
        }
    }
}








cli_suite! {
    string_input in "string-input" {
        rust_string => tractor query -s "fn add(a: i32, b: i32) -> i32 { a + b }" -l "rust" -x "function" => count 1;
        python_string => tractor query -s "def hello(): pass" -l "python" -x "function" => count 1;
        csharp_string => tractor query -s "public class Foo { public void Bar() {} }" -l "csharp" -x "class" => count 1;
        javascript_string => tractor query -s "function greet() { return 'hi'; }" -l "javascript" -x "function" => count 1;
        typescript_string => tractor query -s "const greet = (): string => 'hi';" -l "typescript" -x "arrow" => count 1;
        short_flag => tractor query -s "fn main() {}" -l "rust" -x "function" => count 1;
        expect_exact => tractor query -s "fn a() {} fn b() {}" -l "rust" -x "function" => count 2;
        expect_some => tractor query -s "fn a() {} fn b() {}" -l "rust" -x "function" => count some;
        expect_none => tractor query -s "let x = 1;" -l "rust" -x "function" => count none;
        value_output => tractor query -s "class Foo { }" -l "csharp" -x "class/name" -v "value" => count 1;
        count_output => tractor query -s "class Foo { }" -l "csharp" -x "class" -v "count" -p "count" => stdout "1";
        gcc_output => tractor query -s "class Foo { }" -l "csharp" -x "class" -f "gcc" => count 1;
        without_xpath => tractor query -s "let x = 1;" -l "rust" => count some;
    }
}

cli_suite! {
    xpath_expressions in "xpath-expressions" {
        let_expression => tractor query -s "let x = 1; let y = 2;" -l "typescript" -x "let $v := //variable return $v/name" -v "value" => count 2;
        for_expression => tractor query -s "let x = 1; let y = 2;" -l "typescript" -x "for $v in //name return string($v)" -v "value" => count 2;
        if_true_branch => tractor query -s "let x = 1;" -l "typescript" -x "if (//variable) then //name else ()" -v "value" => count 1;
        if_false_branch => tractor query -s "let x = 1;" -l "typescript" -x "if (//function) then //name else //variable" -v "value" => count 1;
        some_quantified => tractor query -s "function add(a, b) { return a + b; }" -l "javascript" -x "some $f in //function satisfies $f/name = 'add'" => count 1;
        every_quantified => tractor query -s "function add(a, b) { return a + b; }" -l "javascript" -x "every $f in //function satisfies $f/name = 'add'" => count 1;
        variable_reference => tractor query -s "let x = 1;" -l "typescript" -x "let $v := //name return $v" -v "value" => count 1;
        bare_element_name => tractor query -s "let x = 1;" -l "typescript" -x "variable" => count 1;
        bare_element_predicate => tractor query -s "function foo() {}" -l "javascript" -x "function[name='foo']" => count 1;
    }
}

// ---------------------------------------------------------------------------
// $file variable tests
// ---------------------------------------------------------------------------

cli_suite! {
    file_variable in "variables" {
        // $file returns the file path as a string
        file_path => tractor query "MyApp/Services/UserService.cs" -x "$file" => count 1;
        // Detect C# namespace/directory mismatch:
        // MyApp/Services/UserService.cs has namespace MyApp.Services → OK (path contains MyApp/Services)
        // MyApp/Models/UserService.cs has namespace MyApp.Services → MISMATCH (path has Models, not Services)
        namespace_mismatch => tractor query "MyApp/**/*.cs" -x "//namespace[not(contains($file, translate(string(name), '.', '/')))]" => count 1;
    }
}

/// Round-trip: extract a fenced JavaScript code block from a
/// markdown source via `-s`, pipe the extracted code to a second
/// tractor invocation parsing it as JavaScript, and confirm it
/// finds the function. Exercises stdin handoff, value extraction,
/// and multi-language parsing in one go.
#[test]
fn markdown_round_trip_extracts_javascript_block() {
    let md_source = "```javascript\nfunction greet(name) { return name; }\n```\n";

    let extracted = command([
        "query",
        "-s", md_source,
        "-l", "markdown",
        "-x", "//codeblock[language='javascript']/code",
        "-v", "value",
    ])
    .capture();
    assert_eq!(0, extracted.status);

    let parsed = command([
        "query",
        "-l", "javascript",
        "-x", "//function[name]",
        "-v", "count",
        "-p", "count",
    ])
    .stdin(format!("{}\n", extracted.stdout))
    .capture();

    assert_eq!(0, parsed.status);
    assert_eq!("1", parsed.stdout);
}

#[test]
fn string_input_requires_language() {
    command(["query", "--string", "let x = 1;"])
        .assert_exit(1)
        .run();
}

#[test]
fn missing_empty_fixture_directory_falls_back_to_temp_workspace() {
    command([
        "query",
        "-s",
        "fn main() {}",
        "-l",
        "rust",
        "-x",
        "function",
        "-v",
        "count",
        "-p",
        "count",
    ])
    .in_fixture("missing-empty-fixture")
    .assert_stdout("1")
    .run();
}

#[test]
fn project_tree_json_is_sequence_and_single_unwraps() {
    let multi = cli_case!({
        tractor query -s "<root><item>one</item><item>two</item></root>" -l "xml" -x "//item" -p "tree" -f "json";
    })
    .run();
    let multi_json: Value = serde_json::from_str(&multi.stdout).expect("multi projection should be json");
    assert!(multi_json.is_array());
    assert_eq!(2, multi_json.as_array().unwrap().len());

    let single = cli_case!({
        tractor query -s "<root><item>one</item><item>two</item></root>" -l "xml" -x "//item" -p "tree" --single -f "json";
    })
    .run();
    let single_json: Value = serde_json::from_str(&single.stdout).expect("single projection should be json");
    assert!(!single_json.is_array());
}

#[test]
fn project_tree_xml_multi_wraps_named_tree_elements() {
    let result = cli_case!({
        tractor query -s "<root><item>one</item><item>two</item></root>" -l "xml" -x "//item" -p "tree" -f "xml";
    })
    .run();
    assert!(result.stdout.contains("<results>"));
    assert!(result.stdout.contains("<tree>"));
    assert!(result.stdout.contains("</tree>"));
    assert_well_formed_xml(&result.stdout);
}

#[test]
fn project_tree_xml_single_stays_bare() {
    let result = cli_case!({
        tractor query -s "<root><item>one</item><item>two</item></root>" -l "xml" -x "//item" -p "tree" --single -f "xml";
    })
    .run();
    assert!(!result.stdout.contains("<tree>"));
    assert!(result.stdout.contains("<item>"));
    assert_well_formed_xml(&result.stdout);
}

#[test]
fn project_shape_keeps_leaf_values_but_drops_dangling_text() {
    // `-p shape` keeps element names + queryable predicates + leaf
    // property values (`item = "one"`), but drops dangling tokens
    // (parens, semicolons, keyword duplicates from marker lifts).
    // `<item>one</item>` is a leaf shape — its text *is* the property
    // value, so it stays. (Pre-Phase-3 the shape mode aggressively
    // dropped all text including leaf values, leaving snapshots
    // largely meaningless.)
    let shape = cli_case!({
        tractor query -s "<root><item>one</item><item>two</item></root>" -l "xml" -x "//item" -p "shape";
    })
    .run();
    assert!(
        shape.stdout.contains("\"one\"") && shape.stdout.contains("\"two\""),
        "shape projection must keep leaf property values, got: {}",
        shape.stdout
    );
    assert!(
        shape.stdout.contains("item"),
        "shape projection must keep element names, got: {}",
        shape.stdout
    );

    // Sanity: `-p tree` on the same input keeps the text values,
    // confirming the difference is real.
    let tree = cli_case!({
        tractor query -s "<root><item>one</item><item>two</item></root>" -l "xml" -x "//item" -p "tree";
    })
    .run();
    assert!(
        tree.stdout.contains("\"one\"") || tree.stdout.contains("one"),
        "tree projection should retain text content, got: {}",
        tree.stdout
    );
}

#[test]
fn project_tree_single_empty_exits_with_empty_stdout() {
    cli_case!({
        tractor query -s "<root/>" -l "xml" -x "//item" -p "tree" --single -f "xml";
        expect => {
            exit 1;
            stdout "";
            stderr "";
        }
    })
    .run();
}

#[test]
fn project_results_keeps_message_field_in_json() {
    let result = cli_case!({
        tractor query -s "<root><item>one</item></root>" -l "xml" -x "//item" -m "hit" -p "results" -f "json";
    })
    .run();
    let json: Value = serde_json::from_str(&result.stdout).expect("results projection should be json");
    assert_eq!("hit", json[0]["message"]);
}

#[test]
fn project_results_preserves_grouping_in_json() {
    let result = cli_case!({
        tractor query "sample.cs" "sample2.cs" -x "//class/name" -v "file,value" -g "file" -p "results" -f "json";
    })
    .in_fixture("")
    .run();
    let json: Value = serde_json::from_str(&result.stdout).expect("grouped results projection should be json");
    let groups = json.as_array().expect("results projection should stay a sequence");
    assert_eq!(2, groups.len());

    for group in groups {
        assert!(group["file"].is_string(), "expected file key on grouped result: {group}");
        let matches = group["results"].as_array().expect("expected nested results under each group");
        assert!(!matches.is_empty(), "expected at least one match in grouped results");
        assert!(matches.iter().all(|item| item.get("value").is_some()));
        assert!(matches.iter().all(|item| item.get("file").is_none()));
    }
}

#[test]
fn project_summary_warns_when_message_is_unreachable() {
    let result = cli_case!({
        tractor query -s "<root><item>one</item></root>" -l "xml" -x "//item" -m "hit" -p "summary" -f "json";
    })
    .run();
    let json: Value = serde_json::from_str(&result.stdout).expect("summary projection should be json");
    assert!(json.is_object());
    assert!(result
        .stderr
        .contains("warning: -m message template has no effect with -p summary"));
}

#[test]
fn count_view_uses_report_path() {
    cli_case!({
        tractor query -s "<root><item>one</item><item>two</item></root>" -l "xml" -x "//item" -v "count";
        expect => stdout "2 matches";
    })
    .run();
}

#[test]
fn count_projection_restores_scalar() {
    cli_case!({
        tractor query -s "<root><item>one</item><item>two</item></root>" -l "xml" -x "//item" -v "count" -p "count";
        expect => stdout "2";
    })
    .run();
}

#[test]
fn projection_invalid_is_rejected_with_valid_values() {
    cli_case!({
        tractor query -s "<root><item>one</item></root>" -l "xml" -x "//item" -p "INVALID";
        expect => {
            exit 1;
            combined_contains "invalid projection 'INVALID'";
            combined_contains "tree, shape, value, source, lines, schema, count, summary, totals, results, report";
        }
    })
    .run();
}

#[test]
fn project_tree_empty_sequence_preserves_wrapper_and_parseability() {
    let xml = cli_case!({
        tractor query -s "<root><item>one</item></root>" -l "xml" -x "string(//item)" -p "tree" -f "xml";
    })
    .run();
    assert!(xml.stdout.contains("<results"));
    assert_well_formed_xml(&xml.stdout);

    let json = cli_case!({
        tractor query -s "<root><item>one</item></root>" -l "xml" -x "string(//item)" -p "tree" -f "json";
    })
    .run();
    let parsed: Value = serde_json::from_str(&json.stdout).expect("empty tree projection should be json");
    assert_eq!(Value::Array(vec![]), parsed);
}

#[test]
fn project_replacement_warning_lists_all_dropped_fields_and_suggests_results() {
    cli_case!({
        tractor query -s "<root><item>one</item></root>" -l "xml" -x "//item" -v "tree,file" -m "hit" -p "tree" -f "json";
        expect => {
            stderr_contains "warning: requested view items {file, message}";
            stderr_contains "use `-p results` (respects -v/-m)";
        }
    })
    .run();
}

#[test]
fn project_redundant_overlap_does_not_warn() {
    cli_case!({
        tractor query -s "<root><item>one</item></root>" -l "xml" -x "//item" -v "tree" -p "tree" -f "json";
        expect => stderr "";
    })
    .run();
}

#[test]
fn project_empty_explicit_view_does_not_warn() {
    cli_case!({
        tractor query -s "<root><item>one</item></root>" -l "xml" -x "//item" -v "" -p "tree" -f "json";
        expect => stderr "";
    })
    .run();
}

#[test]
fn project_count_and_schema_follow_the_report_pipeline_in_structured_formats() {
    let count = cli_case!({
        tractor query -s "<root><item>one</item><item>two</item></root>" -l "xml" -x "//item" -v "count" -f "json";
    })
    .run();
    let count_json: Value = serde_json::from_str(&count.stdout).expect("count view should be json");
    assert_eq!(2, count_json["summary"]["totals"]["results"]);

    let schema_report = cli_case!({
        tractor query -s "<root><item>one</item></root>" -l "xml" -x "//item" -v "schema" -f "xml";
    })
    .run();
    assert!(schema_report.stdout.contains("<report>"));
    assert!(schema_report.stdout.contains("<schema>"));
    assert_well_formed_xml(&schema_report.stdout);

    let schema_bare = cli_case!({
        tractor query -s "<root><item>one</item></root>" -l "xml" -x "//item" -v "schema" -p "schema" -f "xml";
    })
    .run();
    assert!(!schema_bare.stdout.contains("<report>"));
    assert!(schema_bare.stdout.contains("<schema>"));
    assert_well_formed_xml(&schema_bare.stdout);
}

#[test]
fn project_summary_is_mode_specific() {
    let query = cli_case!({
        tractor query -s "<root><item>one</item></root>" -l "xml" -x "//item" -p "summary" -f "json";
    })
    .run();
    let query_json: Value = serde_json::from_str(&query.stdout).expect("query summary should be json");
    assert!(query_json.get("success").is_none());
    assert_eq!(1, query_json["totals"]["results"]);

    let test = cli_case!({
        tractor test -s "<root><item>one</item></root>" -l "xml" -x "//item" --expect "1" -p "summary" -f "json";
    })
    .run();
    let test_json: Value = serde_json::from_str(&test.stdout).expect("test summary should be json");
    assert_eq!(Value::Bool(true), test_json["success"]);
    assert_eq!(Value::String("1".to_string()), test_json["expected"]);
}

#[test]
fn project_schema_respects_color_output() {
    let result = cli_case!({
        tractor query -s "<root><item>one</item></root>" -l "xml" -x "//item" -p "schema" --color "always";
    })
    .no_color(false)
    .run();
    assert!(result.stdout.contains("\u{1b}["));
}

#[test]
fn view_schema_renders_in_text_output() {
    cli_case!({
        tractor query -s "<root><item>one</item></root>" -l "xml" -x "//item" -v "schema";
        expect => stdout_contains "item = \"one\"";
    })
    .run();
}

#[test]
fn project_totals_single_is_a_noop_with_warning() {
    cli_case!({
        tractor query -s "<root><item>one</item><item>two</item></root>" -l "xml" -x "//item" -p "totals" --single -f "xml";
        expect => {
            stdout_contains "<totals>";
            stderr_contains "warning: --single has no effect with -p totals";
        }
    })
    .run();
}

#[test]
fn set_snapshot_text_default() {
    cli_case!({
        tractor set "sample.yaml" -x "//database/host" --value "db.example.com";
        expect => stdout_snapshot "cli/set/set.snapshot.txt";
    })
    .in_fixture("cli/set")
    .temp_fixture()
    .strip_temp_prefix()
    .replace_output("sample.yaml", "tests/integration/cli/set/sample.yaml")
    .run();
}

#[test]
fn set_snapshot_text_unchanged() {
    cli_case!({
        tractor set "sample.yaml" -x "//database/host" --value "localhost";
        expect => stdout_snapshot "cli/set/set-unchanged.snapshot.txt";
    })
    .in_fixture("cli/set")
    .temp_fixture()
    .strip_temp_prefix()
    .replace_output("sample.yaml", "tests/integration/cli/set/sample.yaml")
    .run();
}

#[test]
fn set_snapshot_text_declarative_mode() {
    cli_case!({
        tractor set "sample.yaml" "database[host='db.example.com']";
        expect => stdout_snapshot "cli/set/set-declarative.snapshot.txt";
    })
    .in_fixture("cli/set")
    .temp_fixture()
    .replace_output("sample.yaml", "tests/integration/cli/set/sample.yaml")
    .run();
}

#[test]
fn set_snapshot_stdout_mode() {
    cli_case!({
        tractor set "sample.yaml" -x "//database/host" --value "db.example.com" --stdout;
        expect => stdout_snapshot "cli/set/set-stdout.snapshot.txt";
    })
    .in_fixture("cli/set")
    .temp_fixture()
    .run();
}

#[test]
fn set_snapshot_stdout_mode_from_stdin() {
    cli_case!({
        tractor set -l "yaml" "database[host='db.example.com']" --stdout;
        expect => stdout_snapshot "cli/set/set-stdin-stdout.snapshot.txt";
    })
    .stdin("database:\n  host: localhost\n  port: 5432\n")
    .run();
}

#[test]
fn set_snapshot_stdout_mode_multiple_files() {
    cli_case!({
        tractor set "sample-a.yaml" "sample-b.yaml" -x "//database/host" --value "db.example.com" --stdout;
        expect => stdout_snapshot "cli/set/set-stdout-multi.snapshot.txt";
    })
    .in_fixture("cli/set")
    .temp_fixture()
    .seed_file("sample-a.yaml", "database:\n  host: localhost\n  port: 5432\n")
    .seed_file("sample-b.yaml", "database:\n  host: localhost\n  port: 5432\n")
    .replace_output("sample-a.yaml", "tests/integration/cli/set/sample-a.yaml")
    .replace_output("sample-b.yaml", "tests/integration/cli/set/sample-b.yaml")
    .run();
}

#[test]
fn set_snapshot_json() {
    cli_case!({
        tractor set "sample.yaml" -x "//database/host" --value "db.example.com" -f "json";
        expect => stdout_snapshot "cli/set/set.snapshot.json";
    })
    .in_fixture("cli/set")
    .temp_fixture()
    .replace_output("sample.yaml", "tests/integration/cli/set/sample.yaml")
    .run();
}

#[test]
fn set_snapshot_xml() {
    cli_case!({
        tractor run --config "set.config.yaml" -f "xml";
        expect => stdout_snapshot "cli/set/set.snapshot.xml";
    })
    .in_fixture("cli/set")
    .temp_fixture()
    .strip_temp_prefix()
    .replace_output("sample.yaml", "tests/integration/cli/set/sample.yaml")
    .run();
}

#[test]
fn set_snapshot_stdout_xml() {
    cli_case!({
        tractor set "sample.yaml" -x "//database/host" --value "db.example.com" --stdout -f "xml";
        expect => stdout_snapshot "cli/set/set-stdout.snapshot.xml";
    })
    .in_fixture("cli/set")
    .temp_fixture()
    .replace_output("sample.yaml", "tests/integration/cli/set/sample.yaml")
    .run();
}

#[test]
fn run_set_capture_duplicate_file_outputs_stay_rooted() {
    cli_case!({
        tractor run --config "set-stdout-duplicate.config.yaml" -f "xml";
        expect => stdout_snapshot "cli/set/set-stdout-duplicate.snapshot.xml";
    })
    .in_fixture("cli/set")
    .fixture_prefix("tests/integration/cli/set")
    .run();
}

#[test]
fn run_set_capture_duplicate_file_outputs_stay_rooted_json() {
    cli_case!({
        tractor run --config "set-stdout-duplicate.config.yaml" -f "json";
        expect => stdout_snapshot "cli/set/set-stdout-duplicate.snapshot.json";
    })
    .in_fixture("cli/set")
    .fixture_prefix("tests/integration/cli/set")
    .run();
}

#[test]
fn run_set_capture_duplicate_file_outputs_stay_rooted_yaml() {
    cli_case!({
        tractor run --config "set-stdout-duplicate.config.yaml" -f "yaml";
        expect => stdout_snapshot "cli/set/set-stdout-duplicate.snapshot.yaml";
    })
    .in_fixture("cli/set")
    .fixture_prefix("tests/integration/cli/set")
    .run();
}

#[test]
fn set_snapshot_stdout_json() {
    cli_case!({
        tractor set "sample.yaml" -x "//database/host" --value "db.example.com" --stdout -f "json";
        expect => stdout_snapshot "cli/set/set-stdout.snapshot.json";
    })
    .in_fixture("cli/set")
    .temp_fixture()
    .replace_output("sample.yaml", "tests/integration/cli/set/sample.yaml")
    .run();
}

#[test]
fn set_snapshot_stdout_yaml() {
    cli_case!({
        tractor set "sample.yaml" -x "//database/host" --value "db.example.com" --stdout -f "yaml";
        expect => stdout_snapshot "cli/set/set-stdout.snapshot.yaml";
    })
    .in_fixture("cli/set")
    .temp_fixture()
    .replace_output("sample.yaml", "tests/integration/cli/set/sample.yaml")
    .run();
}

#[test]
fn replace_updates_yaml_values_in_place() {
    cli_case!({
        tractor set "single.yaml" -x "//database/host" --value "db.example.com";
        expect => file_eq "single.yaml" "name: my-app\ndatabase:\n  host: db.example.com\n  port: 5432";
    })
    .in_fixture("replace")
    .temp_fixture()
    .seed_file("single.yaml", "name: my-app\ndatabase:\n  host: localhost\n  port: 5432\n")
    .run();
}

#[test]
fn replace_updates_multiple_yaml_values() {
    cli_case!({
        tractor set "multi.yaml" -x "//servers/port[.='8080']" --value "3000";
        expect => file_eq "multi.yaml" "servers:\n  - name: web-1\n    port: 3000\n  - name: web-2\n    port: 3000\n  - name: web-3\n    port: 9090";
    })
    .in_fixture("replace")
    .temp_fixture()
    .seed_file("multi.yaml", "servers:\n  - name: web-1\n    port: 8080\n  - name: web-2\n    port: 8080\n  - name: web-3\n    port: 9090\n")
    .run();
}

#[test]
fn replace_path_expression_predicates_filter_targets() {
    cli_case!({
        tractor set "multi.yaml" "servers[host='localhost']/port" --value "5433";
        expect => file_eq "multi.yaml" "servers:\n  - host: localhost\n    port: 5433\n  - host: prod-db\n    port: 5432";
    })
    .in_fixture("replace")
    .temp_fixture()
    .seed_file("multi.yaml", "servers:\n  - host: localhost\n    port: 5432\n  - host: prod-db\n    port: 5432\n")
    .run();
}

#[test]
fn replace_respects_limit() {
    cli_case!({
        tractor set "limit.yaml" -x "//items/value[.='old']" -n "1" --value "new";
        expect => file_eq "limit.yaml" "items:\n  - value: new\n  - value: old\n  - value: old";
    })
    .in_fixture("replace")
    .temp_fixture()
    .seed_file("limit.yaml", "items:\n  - value: old\n  - value: old\n  - value: old\n")
    .run();
}

#[test]
fn replace_updates_json_string_values() {
    cli_case!({
        tractor set "data.json" -x "//database/host" --value "db.example.com";
        expect => file_eq "data.json" "{\n  \"database\": {\n    \"host\": \"db.example.com\",\n    \"port\": 5432\n  }\n}";
    })
    .in_fixture("replace")
    .temp_fixture()
    .seed_file("data.json", "{\n  \"database\": {\n    \"host\": \"localhost\",\n    \"port\": 5432\n  }\n}\n")
    .run();
}

#[test]
fn replace_stdout_mode_does_not_modify_file() {
    cli_case!({
        tractor set "stdout.yaml" -x "//host" --value "example.com" --stdout;
        expect => {
            stdout "host: example.com";
            file_eq "stdout.yaml" "host: localhost";
        }
    })
    .in_fixture("replace")
    .temp_fixture()
    .seed_file("stdout.yaml", "host: localhost\n")
    .run();
}

#[test]
fn replace_stdin_implicitly_writes_stdout() {
    cli_case!({
        tractor set -l "yaml" -x "//name" --value "newvalue";
        expect => stdout "name: newvalue";
    })
    .in_fixture("replace")
    .temp_fixture()
    .stdin("name: test\n")
    .run();
}

#[test]
fn replace_without_xpath_fails() {
    cli_case!({
        tractor set "data.json" --value "foo";
        expect => exit 1;
    })
    .in_fixture("replace")
    .temp_fixture()
    .seed_file("data.json", "{\n  \"name\": \"value\"\n}\n")
    .run();
}

#[test]
fn replace_creates_missing_nodes_when_set_has_no_matches() {
    cli_case!({
        tractor set "data.json" -x "//nonexistent" --value "x";
        expect => file_eq "data.json" "{\n  \"database\": {\n    \"host\": \"localhost\",\n    \"port\": 5432\n  },\n  \"nonexistent\": \"x\"\n}";
    })
    .in_fixture("replace")
    .temp_fixture()
    .seed_file("data.json", "{\n  \"database\": {\n    \"host\": \"localhost\",\n    \"port\": 5432\n  }\n}\n")
    .run();
}

#[test]
fn update_changes_existing_values_but_not_missing_nodes() {
    cli_case!({
        tractor update "single.yaml" -x "//database/host" --value "db.example.com";
        expect => file_eq "single.yaml" "name: my-app\ndatabase:\n  host: db.example.com\n  port: 5432";
    })
    .in_fixture("update")
    .temp_fixture()
    .seed_file("single.yaml", "name: my-app\ndatabase:\n  host: localhost\n  port: 5432\n")
    .run();
}

#[test]
fn update_missing_path_fails_without_creating_nodes() {
    cli_case!({
        tractor update "nocreate.yaml" -x "//database/host" --value "localhost";
        expect => {
            exit 1;
            file_eq "nocreate.yaml" "name: my-app";
        }
    })
    .in_fixture("update")
    .temp_fixture()
    .seed_file("nocreate.yaml", "name: my-app\n")
    .run();
}

#[test]
fn update_partial_path_fails_without_creating_leaf() {
    cli_case!({
        tractor update "partial.yaml" -x "//database/port" --value "5432";
        expect => {
            exit 1;
            file_eq "partial.yaml" "database:\n  host: localhost";
        }
    })
    .in_fixture("update")
    .temp_fixture()
    .seed_file("partial.yaml", "database:\n  host: localhost\n")
    .run();
}

// ---------------------------------------------------------------------------
// Virtual paths for inline sources (issue #133)
//
// The positional `files` arg, when an inline source is active (stdin or -s),
// becomes a virtual path. The executor treats the inline content as if it
// lived at that path for glob matching, diff-lines, and diagnostics.
// ---------------------------------------------------------------------------

#[test]
fn inline_stdin_virtual_path_matches_include_glob() {
    // Rule has `include: ["src/**/*.js"]`. When stdin content is piped with
    // a virtual path under src/, the rule fires — proof that globs see the
    // virtual path instead of the old "<stdin>" sentinel that never matched.
    let config = "check:\n  rules:\n    - id: no-console\n      xpath: \"//object[access and name='console']\"\n      severity: error\n      reason: \"no console\"\n      language: javascript\n      include: [\"src/**/*.js\"]\n";
    cli_case!({
        tractor check --config "tractor.yml" -l "javascript" "src/foo.js";
        expect => exit 1;
    })
    .in_fixture("replace")
    .temp_fixture()
    .seed_file("tractor.yml", config)
    .stdin("console.log('hi');\n")
    .run();
}

#[test]
fn inline_stdin_virtual_path_outside_include_glob_is_clean() {
    // Same rule/content, but virtual path doesn't match `include:` — rule
    // doesn't fire. Mirrors the disk-file behaviour for non-matching paths.
    let config = "check:\n  rules:\n    - id: no-console\n      xpath: \"//object[access and name='console']\"\n      severity: error\n      reason: \"no console\"\n      language: javascript\n      include: [\"src/**/*.js\"]\n";
    cli_case!({
        tractor check --config "tractor.yml" -l "javascript" "test/foo.js";
        expect => exit 0;
    })
    .in_fixture("replace")
    .temp_fixture()
    .seed_file("tractor.yml", config)
    .stdin("console.log('hi');\n")
    .run();
}

#[test]
fn inline_string_virtual_path_appears_in_diagnostic_output() {
    // `-s` content + virtual path: the diagnostic should mention the virtual
    // path, never the "<string>" or "<stdin>" sentinel. Regression guard
    // against the sentinel leaking past the input boundary.
    let config = "check:\n  rules:\n    - id: no-console\n      xpath: \"//object[access and name='console']\"\n      severity: error\n      reason: \"no console\"\n      language: javascript\n      include: [\"src/**/*.js\"]\n";
    let result = command([
        "check",
        "--config",
        "tractor.yml",
        "-l",
        "javascript",
        "-s",
        "console.log('hi');",
        "src/foo.js",
    ])
    .in_fixture("replace")
    .temp_fixture()
    .seed_file("tractor.yml", config)
    .capture();

    assert_eq!(1, result.status);
    assert!(
        result.stdout.contains("src/foo.js"),
        "virtual path should appear in output: {}",
        result.stdout
    );
    assert!(
        !result.stdout.contains("<string>") && !result.stdout.contains("<stdin>"),
        "sentinel must not leak into output: {}",
        result.stdout
    );
}

#[test]
fn inline_stdin_pathless_does_not_match_include_globs() {
    // Pathless inline source (no positional path) cannot match any rule
    // with an `include:` pattern — preserves the prior behaviour and makes
    // the difference vs. virtual-path mode explicit.
    let config = "check:\n  rules:\n    - id: no-console\n      xpath: \"//object[access and name='console']\"\n      severity: error\n      reason: \"no console\"\n      language: javascript\n      include: [\"src/**/*.js\"]\n";
    cli_case!({
        tractor check --config "tractor.yml" -l "javascript";
        expect => exit 0;
    })
    .in_fixture("replace")
    .temp_fixture()
    .seed_file("tractor.yml", config)
    .stdin("console.log('hi');\n")
    .run();
}

#[test]
fn update_rejects_stdin_input() {
    cli_case!({
        tractor update --lang "yaml" -x "//name" --value "new";
        expect => exit 1;
    })
    .in_fixture("update")
    .temp_fixture()
    .stdin("name: test\n")
    .run();
}

#[test]
fn run_multirule_output_is_stable() {
    cli_case!({
        tractor run --config "check-multirule.yaml";
        expect => {
            exit 1;
            combined "settings.yaml:3:10: error: debug should be disabled in production\n3 |   debug: true\n             ^~~~\n\nsettings.yaml:4:14: warning: log level should not be debug in production\n4 |   log_level: debug\n                 ^~~~~\n\n1 error in 1 file";
        }
    })
    .in_fixture("run")
    .fixture_prefix("")
    .run();
}

#[test]
fn run_multifile_check_scans_multiple_files() {
    cli_case!({
        tractor run --config "check-multifile.yaml";
        expect => {
            exit 1;
            combined "settings.yaml:3:10: error: debug mode must be disabled\n3 |   debug: true\n             ^~~~\n\n1 error in 1 file";
        }
    })
    .in_fixture("run")
    .fixture_prefix("")
    .run();
}

#[test]
fn run_set_applies_mappings_to_files() {
    cli_case!({
        tractor run --config "set-config.yaml";
        expect => combined "app-config.json:3:13: note: updated //database/host\napp-config.json:8:12: note: updated //cache/ttl\nupdated 1 file";
    })
    .in_fixture("run")
    .fixture_prefix("")
    .temp_fixture()
    .strip_temp_prefix()
    .run();
}

#[test]
fn run_scope_intersection_respects_root() {
    cli_case!({
        tractor run --config "scope-intersection/intersect-narrow.yaml";
        expect => combined "scope-intersection/frontend/config.yml:1:8: warning: debug must be disabled\n1 | debug: true\n           ^~~~\n\n1 warning in 1 file";
    })
    .in_fixture("run")
    .fixture_prefix("")
    .run();
}

#[test]
fn run_scope_intersection_falls_back_to_root_when_operation_has_no_files() {
    cli_case!({
        tractor run --config "scope-intersection/intersect-fallback.yaml";
        expect => combined "";
    })
    .in_fixture("run")
    .fixture_prefix("")
    .run();
}

#[test]
fn run_scope_intersection_fatal_when_empty() {
    // Running checks on zero files is almost always a mistake (typo,
    // stale scope, misconfigured rule) — surface it as a fatal error
    // rather than silently succeeding. This holds whether the emptiness
    // came from a pattern genuinely matching nothing or from sibling
    // intersections (root ∩ operation) reducing the set to zero.
    let result = command(["run", "--config", "scope-intersection/intersect-disjoint.yaml"])
        .in_fixture("run")
        .fixture_prefix("")
        .assert_exit(1)
        .capture();
    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        combined.contains("file patterns matched 0 files"),
        "expected fatal about empty expansion, got: {}",
        combined
    );
}

#[test]
fn run_double_star_glob_matches_recursively() {
    cli_case!({
        tractor run --config "glob-double-star/check-double-star.yaml";
        expect => combined "glob-double-star/config.yml:1:8: warning: debug must be disabled\n1 | debug: true\n           ^~~~\n\nglob-double-star/nested/config.yml:1:8: warning: debug must be disabled\n1 | debug: true\n           ^~~~\n\n2 warnings in 2 files";
    })
    .in_fixture("run")
    .fixture_prefix("")
    .run();
}

#[test]
fn run_nested_double_star_glob_matches_nested_files() {
    cli_case!({
        tractor run --config "glob-double-star/check-dir-double-star.yaml";
        expect => combined "glob-double-star/nested/config.yml:1:8: warning: debug must be disabled\n1 | debug: true\n           ^~~~\n\n1 warning in 1 file";
    })
    .in_fixture("run")
    .fixture_prefix("")
    .run();
}

#[test]
fn run_absolute_cli_path_with_root_files_intersection() {
    command(["run", "--config", "absolute-paths/check-root-files.yaml"])
        .abs_arg("absolute-paths/config.yml")
        .in_fixture("run")
        .fixture_prefix("")
    .assert_combined("absolute-paths/config.yml:1:8: warning: debug must be disabled\n1 | debug: true\n           ^~~~\n\n1 warning in 1 file")
    .run();
}

#[test]
fn run_absolute_cli_path_with_per_rule_include_matches() {
    command(["run", "--config", "absolute-paths/check-per-rule-include.yaml"])
        .abs_arg("absolute-paths/config.yml")
        .in_fixture("run")
        .fixture_prefix("")
        .assert_combined("absolute-paths/config.yml:1:8: warning: debug must be disabled\n1 | debug: true\n           ^~~~\n\n1 warning in 1 file")
        .run();
}

#[test]
fn run_absolute_cli_path_with_per_rule_exclude_filters_out() {
    command(["run", "--config", "absolute-paths/check-per-rule-exclude.yaml"])
        .abs_arg("absolute-paths/config.yml")
        .in_fixture("run")
        .fixture_prefix("")
        .assert_combined("")
        .run();
}

#[test]
fn run_absolute_cli_path_with_root_exclude_filters_out() {
    command(["run", "--config", "absolute-paths/check-root-exclude.yaml"])
        .abs_arg("absolute-paths/config.yml")
        .in_fixture("run")
        .fixture_prefix("")
        .assert_combined("")
        .run();
}

#[test]
fn run_dot_relative_cli_path_with_per_rule_include_matches() {
    command(["run", "--config", "absolute-paths/check-per-rule-include.yaml"])
        .arg("./absolute-paths/config.yml")
        .in_fixture("run")
        .fixture_prefix("")
        .assert_combined("absolute-paths/config.yml:1:8: warning: debug must be disabled\n1 | debug: true\n           ^~~~\n\n1 warning in 1 file")
        .run();
}

#[test]
fn run_dot_relative_cli_path_with_per_rule_exclude_filters_out() {
    command(["run", "--config", "absolute-paths/check-per-rule-exclude.yaml"])
        .arg("./absolute-paths/config.yml")
        .in_fixture("run")
        .fixture_prefix("")
        .assert_combined("")
        .run();
}

#[test]
fn run_dot_relative_cli_path_with_root_files_intersection() {
    command(["run", "--config", "absolute-paths/check-root-files.yaml"])
        .arg("./absolute-paths/config.yml")
        .in_fixture("run")
        .fixture_prefix("")
        .assert_combined("absolute-paths/config.yml:1:8: warning: debug must be disabled\n1 | debug: true\n           ^~~~\n\n1 warning in 1 file")
        .run();
}

#[test]
fn run_dot_relative_cli_path_with_root_exclude_filters_out() {
    command(["run", "--config", "absolute-paths/check-root-exclude.yaml"])
        .arg("./absolute-paths/config.yml")
        .in_fixture("run")
        .fixture_prefix("")
        .assert_combined("")
        .run();
}

#[test]
fn run_mixed_language_rules_report_all_findings() {
    cli_case!({
        tractor run --config "mixed-language/three-langs.yaml";
        expect => {
            exit 1;
            combined "mixed-language/config.yaml:3:10: error: Debug mode must be disabled\n3 |   debug: true\n             ^~~~\n\nmixed-language/sample.js:1:1: error: TODO comment found\n1 | // TODO: Fix this code\n    ^~~~~~~~~~~~~~~~~~~~~~\n\nmixed-language/todo-doc.md:3:1: warning: TODO comment found\n3 >| <!-- TODO: Complete this section -->\n4 >| \n\n2 errors in 3 files";
        }
    })
    .in_fixture("run")
    .fixture_prefix("")
    .run();
}

#[test]
fn run_mixed_language_rules_report_javascript_and_markdown_findings() {
    cli_case!({
        tractor run --config "mixed-language/mixed-rules.yaml";
        expect => {
            exit 1;
            combined "mixed-language/sample.js:1:1: error: TODO comment found\n1 | // TODO: Fix this code\n    ^~~~~~~~~~~~~~~~~~~~~~\n\nmixed-language/todo-doc.md:3:1: warning: TODO comment found\n3 >| <!-- TODO: Complete this section -->\n4 >| \n\n1 error in 2 files";
        }
    })
    .in_fixture("run")
    .fixture_prefix("")
    .run();
}

#[test]
fn run_mixed_language_javascript_only_rules_skip_markdown() {
    cli_case!({
        tractor run --config "mixed-language/js-only-rules.yaml";
        expect => {
            exit 1;
            combined "mixed-language/sample.js:1:1: error: TODO comment found\n1 | // TODO: Fix this code\n    ^~~~~~~~~~~~~~~~~~~~~~\n\n1 error in 1 file";
        }
    })
    .in_fixture("run")
    .fixture_prefix("")
    .run();
}

#[test]
fn run_mixed_language_markdown_only_rules_skip_javascript() {
    cli_case!({
        tractor run --config "mixed-language/md-only-rules.yaml";
        expect => combined "mixed-language/todo-doc.md:3:1: warning: TODO comment found\n3 >| <!-- TODO: Complete this section -->\n4 >| \n\n1 warning in 1 file";
    })
    .in_fixture("run")
    .fixture_prefix("")
    .run();
}

#[test]
fn run_mixed_language_auto_detect_uses_file_extension() {
    cli_case!({
        tractor run --config "mixed-language/auto-detect.yaml";
        expect => {
            exit 1;
            combined "mixed-language/sample.js:1:1: error: TODO comment found\n1 | // TODO: Fix this code\n    ^~~~~~~~~~~~~~~~~~~~~~\n\n1 error in 1 file";
        }
    })
    .in_fixture("run")
    .fixture_prefix("")
    .run();
}

#[test]
fn run_mixed_language_multiple_rules_for_same_language_report_all_findings() {
    cli_case!({
        tractor run --config "mixed-language/same-lang-rules.yaml";
        expect => {
            exit 1;
            combined "mixed-language/sample.js:1:1: error: TODO comment found\n1 | // TODO: Fix this code\n    ^~~~~~~~~~~~~~~~~~~~~~\n\nmixed-language/sample.js:3:5: warning: No console.log calls allowed\n3 |     console.log(\"Hello\");\n        ^~~~~~~~~~~~~~~~~~~~\n\nmixed-language/sample.js:7:5: warning: No console.log calls allowed\n7 |     console.log(\"Goodbye\");\n        ^~~~~~~~~~~~~~~~~~~~~~\n\n1 error in 1 file";
        }
    })
    .in_fixture("run")
    .fixture_prefix("")
    .run();
}

#[test]
fn run_mixed_language_aliases_are_resolved() {
    cli_case!({
        tractor run --config "mixed-language/lang-alias.yaml";
        expect => {
            exit 1;
            combined "mixed-language/sample.js:1:1: error: TODO comment found\n1 | // TODO: Fix this code\n    ^~~~~~~~~~~~~~~~~~~~~~\n\n1 error in 1 file";
        }
    })
    .in_fixture("run")
    .fixture_prefix("")
    .run();
}

#[test]
fn run_mixed_check_and_set_succeeds_when_check_passes() {
    cli_case!({
        tractor run --config "mixed-ops.yaml";
        expect => combined "app-config.json:3:13: note: updated //database/host\nupdated 1 file";
    })
    .in_fixture("run")
    .fixture_prefix("")
    .temp_fixture()
    .strip_temp_prefix()
    .run();
}

const DEFAULT_CONFIG_CONTENTS: &str =
    "check:\n  files: [\"settings.yaml\"]\n  rules:\n    - id: no-debug\n      xpath: \"//debug[.='true']\"\n      reason: \"debug should be disabled\"\n      severity: error\n";

const DEFAULT_CONFIG_SETTINGS: &str = "app:\n  name: myapp\n  debug: true\n";

#[test]
fn run_without_path_uses_default_tractor_yml() {
    cli_case!({
        tractor run;
        expect => {
            exit 1;
            combined "settings.yaml:3:10: error: debug should be disabled\n3 |   debug: true\n             ^~~~\n\n1 error in 1 file";
        }
    })
    .temp_fixture()
    .seed_file("tractor.yml", DEFAULT_CONFIG_CONTENTS)
    .seed_file("settings.yaml", DEFAULT_CONFIG_SETTINGS)
    .strip_temp_prefix()
    .run();
}

#[test]
fn run_without_path_ignores_tractor_yaml() {
    // `.yaml` is not on the default probe list — only `tractor.yml` is. Users
    // can still point at `.yaml` explicitly, but with no path argument and no
    // `tractor.yml`, tractor errors even if a `tractor.yaml` sits right there.
    cli_case!({
        tractor run;
        expect => {
            exit 1;
            combined_contains "no tractor.yml";
        }
    })
    .temp_fixture()
    .seed_file("tractor.yaml", DEFAULT_CONFIG_CONTENTS)
    .seed_file("settings.yaml", DEFAULT_CONFIG_SETTINGS)
    .run();
}

#[test]
fn run_without_path_errors_when_no_default_exists() {
    cli_case!({
        tractor run;
        expect => {
            exit 1;
            combined_contains "no tractor.yml";
        }
    })
    .temp_fixture()
    .run();
}

/// Source of truth for the scaffolded config — used when we need to seed a
/// temp directory with the starter to exercise an end-to-end flow. The
/// byte-equality check against the fixture lives in the DSL tests below via
/// `file_snapshot`.
const STARTER_SNAPSHOT: &str =
    include_str!("../../tests/integration/init/tractor.yml");

#[test]
fn init_writes_the_snapshot_starter_config() {
    cli_case!({
        tractor init;
        expect => {
            stdout "created tractor.yml\nrun `tractor run` to execute it";
            file_snapshot "tractor.yml" "init/tractor.yml";
        }
    })
    .no_color(false)
    .temp_fixture()
    .run();
}

#[test]
fn starter_config_flags_the_sample_rule_when_run() {
    // The starter config deliberately scans tractor.yml itself with an xpath
    // that matches its own rule by id. A bare `tractor run` should report a
    // single warning pointing at the sample rule block — a self-explanatory
    // demo of how rules map onto the YAML tree.
    cli_case!({
        tractor run;
        expect => combined "tractor.yml:19:7: warning: replace this sample rule with your own checks\n19 >|     - id: sample-rule\n20  |       xpath: \"//check/rules[id='sample-rule']\"\n21  |       reason: \"replace this sample rule with your own checks\"\n22 >|       severity: warning\n\n1 warning in 1 file";
    })
    .temp_fixture()
    .seed_file("tractor.yml", STARTER_SNAPSHOT)
    .strip_temp_prefix()
    .run();
}

#[test]
fn init_refuses_to_overwrite_without_force() {
    cli_case!({
        tractor init;
        expect => {
            exit 1;
            combined "cli\nfatal(cli): tractor.yml already exists — pass --force to overwrite\n1 fatal in 0 files";
        }
    })
    .no_color(false)
    .temp_fixture()
    .seed_file("tractor.yml", "# hand-edited\n")
    .run();
}

#[test]
fn init_force_overwrites_existing_file() {
    cli_case!({
        tractor init --force;
        expect => {
            stdout "created tractor.yml\nrun `tractor run` to execute it";
            file_snapshot "tractor.yml" "init/tractor.yml";
        }
    })
    .no_color(false)
    .temp_fixture()
    .seed_file("tractor.yml", "# hand-edited\n")
    .run();
}

#[test]
fn view_modifier_can_drop_lines_in_gcc_output() {
    let result = command([
        "check",
        "sample.cs",
        "-x",
        "//class",
        "--reason",
        "class found",
        "-f",
        "gcc",
        "-v=-lines",
    ])
    .in_fixture("")
    .capture();

    assert_eq!(1, result.status);
    assert_eq!(
        2,
        result
            .stdout
            .lines()
            .filter(|line| line.contains(": error:"))
            .count()
    );
    assert!(!result.stdout.lines().any(|line| line
        .trim_start()
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit())
        && line.contains(">|")));
}

#[test]
fn view_modifier_can_add_source_and_remove_tree() {
    let without_tree = query_command("sample.cs", "//class/name")
        .arg("-v=-tree")
        .in_fixture("")
        .capture();
    assert_eq!(0, without_tree.status);
    assert!(!without_tree.stdout.contains('<'));

    let with_source = query_command("sample.cs", "//class/name")
        .arg("-v=+source")
        .in_fixture("")
        .capture();
    assert_eq!(0, with_source.status);
    assert!(with_source.stdout.contains("public class Calculator"));
    assert!(with_source.stdout.contains("public class Greeter"));
}

#[test]
fn view_modifier_is_idempotent_for_existing_fields() {
    let default_out = query_command("sample.cs", "//class/name")
        .in_fixture("")
        .capture();
    let modified_out = query_command("sample.cs", "//class/name")
        .arg("-v=+tree")
        .in_fixture("")
        .capture();

    assert_eq!(0, default_out.status);
    assert_eq!(0, modified_out.status);
    assert_eq!(default_out.stdout, modified_out.stdout);
}

#[test]
fn view_modifier_rejects_invalid_combinations() {
    command(["query", "sample.cs", "-x", "//class", "-v=tree,+source"])
        .in_fixture("")
        .assert_exit(1)
        .run();

    command([
        "query",
        "sample.cs",
        "-x",
        "//class/name",
        "-v=-file,-line,-tree",
    ])
    .in_fixture("")
    .assert_exit(1)
    .run();

    command(["query", "sample.cs", "-x", "//class", "-v=-nosuchfield"])
        .in_fixture("")
        .assert_exit(1)
        .run();
}

// ---------------------------------------------------------------------------
// `--diff-lines` + pathless inline input: fatal at plan time
//
// Pathless inline input has no git baseline to compute hunks against, so
// combining it with `--diff-lines` is a misconfiguration. The planner rejects
// it with a fatal diagnostic rather than silently dropping every match. This
// makes `Filters` apply uniformly across all executors (see `executor/set.rs`
// — no longer bypasses filters for `InlineWithPath`).
// ---------------------------------------------------------------------------

#[test]
fn diff_lines_with_pathless_string_is_fatal() {
    // `-s` + `--diff-lines` with no positional path → fatal at plan time.
    let result = command([
        "check",
        "--diff-lines",
        "HEAD",
        "-l",
        "javascript",
        "-s",
        "let x = 1",
        "-x",
        "//call",
    ])
    .capture();
    assert_eq!(1, result.status, "expected fatal exit");
    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        combined.contains("--diff-lines"),
        "fatal must mention --diff-lines: {}",
        combined
    );
    assert!(
        combined.contains("file path") || combined.contains("positional path"),
        "fatal should explain the path requirement: {}",
        combined
    );
}

#[test]
fn diff_lines_with_pathless_stdin_is_fatal() {
    // Piped stdin + `--diff-lines` with no positional path → fatal at plan time.
    let result = command([
        "check",
        "--diff-lines",
        "HEAD",
        "-l",
        "javascript",
        "-x",
        "//call",
    ])
    .stdin("let x = 1\n")
    .capture();
    assert_eq!(1, result.status, "expected fatal exit");
    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        combined.contains("--diff-lines"),
        "fatal must mention --diff-lines: {}",
        combined
    );
}

#[test]
fn set_diff_lines_with_pathless_stdin_is_fatal() {
    // `tractor set` with piped stdin (pathless) + `--diff-lines` → fatal.
    // Regression guard for the set.rs bug where filters were silently
    // bypassed for inline sources (including InlineWithPath). The new
    // contract: reject pathless + diff-lines at plan time; other inline
    // shapes flow through the executor with filters applied uniformly.
    let result = command([
        "set",
        "--diff-lines",
        "HEAD",
        "-l",
        "yaml",
        "//foo",
        "--value",
        "y",
    ])
    .stdin("foo: bar\n")
    .capture();
    assert_eq!(1, result.status, "expected fatal exit");
    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        combined.contains("--diff-lines"),
        "fatal must mention --diff-lines: {}",
        combined
    );
}

#[test]
fn set_inline_with_path_plus_diff_lines_is_accepted_at_plan_time() {
    // Regression guard for the pre-refactor set.rs bug that silently
    // bypassed filters for *all* inline sources. The new contract:
    //
    //   InlinePathless + --diff-lines → fatal at plan time.
    //   InlineWithPath + --diff-lines → plan succeeds; the operation runs
    //       with its filter envelope built, just like disk sources.
    //
    // This test guards the InlineWithPath path: with a real git baseline
    // and a virtual path pointing at a committed file, the planner must
    // NOT emit the pathless fatal and the run must complete without
    // error. Previously this combination could reach the executor with a
    // filter silently dropped; now it threads through uniformly.
    use std::io::Write;

    let temp = tempfile::tempdir().expect("temp dir");
    let repo = temp.path();

    // Initial commit baseline: two entries on distinct lines.
    let src_dir = repo.join("src");
    std::fs::create_dir_all(&src_dir).expect("mkdir src");
    let baseline = "kept: old\nchanged: old\n";
    std::fs::write(src_dir.join("x.yaml"), baseline).expect("write baseline");

    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(repo)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["init", "-q"]);
    git(&["add", "src/x.yaml"]);
    git(&["-c", "commit.gpgsign=false", "commit", "-q", "-m", "baseline"]);

    // Modified content fed via stdin: only `changed:` has a new value.
    // `kept:` stays identical — it's outside the changed hunk.
    let modified = "kept: old\nchanged: new\n";

    // Run `tractor set --diff-lines HEAD -l yaml src/x.yaml //changed
    // --value patched` with the modified content piped over stdin. The
    // positional path `src/x.yaml` makes this an InlineWithPath source.
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_tractor"))
        .args([
            "set",
            "--diff-lines",
            "HEAD",
            "-l",
            "yaml",
            "--stdout",
            "src/x.yaml",
            "//changed",
            "--value",
            "patched",
            "--no-color",
        ])
        .current_dir(repo)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn tractor");
    {
        let stdin = child.stdin.as_mut().expect("open stdin");
        stdin.write_all(modified.as_bytes()).expect("write stdin");
    }
    let out = child.wait_with_output().expect("wait tractor");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // Must succeed (no pathless-fatal) and must NOT mention --diff-lines
    // in an error context. The operation runs end-to-end.
    assert_eq!(
        0,
        out.status.code().unwrap_or(-1),
        "InlineWithPath + --diff-lines must not trip the pathless fatal: {}",
        combined
    );
    assert!(
        !combined.contains("fatal"),
        "no fatal diagnostic expected for InlineWithPath: {}",
        combined
    );
    // The mutation reaches the output channel (the captured content
    // carries the patched value) — proving the source was actually
    // processed rather than silently dropped.
    assert!(
        combined.contains("patched"),
        "set should have produced mutated output: {}",
        combined
    );

    // On-disk baseline must be untouched: set with stdin input captures
    // to stdout rather than writing back to the file.
    let on_disk = std::fs::read_to_string(src_dir.join("x.yaml")).expect("read baseline");
    assert_eq!(on_disk, baseline, "working tree must be untouched");
}
