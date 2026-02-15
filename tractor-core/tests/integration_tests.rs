/// Integration tests for tractor-core
///
/// These tests verify:
/// 1. XML pass-through functionality
/// 2. Snapshot loading and querying
/// 3. XPath querying against parsed code

use std::path::PathBuf;
use tractor_core::{
    load_xml_string_to_documents, load_xml_file_to_documents,
    parse_string_to_documents, parse_to_documents,
    XPathEngine, XeeParseResult, SchemaCollector,
    output::{render_node, RenderOptions},
};

fn get_test_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/integration/fixtures")
}

fn get_test_snapshots_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/integration/snapshots")
}

/// Helper to render XeeParseResult to XML string (for comparison)
fn render_to_xml(result: &XeeParseResult) -> String {
    let doc_node = result.documents.document_node(result.doc_handle).unwrap();
    let xot = result.documents.xot();
    let render_opts = RenderOptions::new().with_pretty_print(true);
    xot.children(doc_node)
        .map(|child| render_node(xot, child, &render_opts))
        .collect()
}

#[test]
fn test_load_xml_passthrough() {
    // Test that we can load XML directly without parsing source
    let xml = r#"<file>
  <function>
    fn
    <name><type>test</type></name>
  </function>
</file>"#;

    let result = load_xml_string_to_documents(xml, "test.xml".to_string())
        .expect("Should load XML");

    assert_eq!(result.file_path, "test.xml");
    assert_eq!(result.language, "xml");
}

#[test]
fn test_query_xml_passthrough() {
    // Test that we can query XML loaded via pass-through
    let xml = r#"<file>
  <function>fn <name><type>test</type></name></function>
  <function>fn <name><type>main</type></name></function>
</file>"#;

    let mut result = load_xml_string_to_documents(xml, "test.xml".to_string())
        .expect("Should load XML");

    let engine = XPathEngine::new();
    let matches = engine.query_documents(
        &mut result.documents,
        result.doc_handle,
        "//function",
        result.source_lines.clone(),
        &result.file_path,
    ).expect("Query should succeed");

    assert_eq!(matches.len(), 2, "Should find 2 functions");
}

#[test]
fn test_load_snapshot_and_query() {
    // Test loading a snapshot file and querying it
    let snapshots_dir = get_test_snapshots_dir();
    let snapshot_path = snapshots_dir.join("sample.rs.xml");

    if !snapshot_path.exists() {
        eprintln!("Snapshot not found, skipping test: {:?}", snapshot_path);
        return;
    }

    let mut result = load_xml_file_to_documents(&snapshot_path)
        .expect("Should load snapshot");

    let engine = XPathEngine::new();
    let matches = engine.query_documents(
        &mut result.documents,
        result.doc_handle,
        "//function",
        result.source_lines.clone(),
        &result.file_path,
    ).expect("Query should succeed");

    assert!(matches.len() >= 2, "Should find at least 2 functions in sample.rs");
}

#[test]
fn test_snapshot_matches_current_output() {
    // Test that current output matches snapshot
    let fixtures_dir = get_test_fixtures_dir();
    let snapshots_dir = get_test_snapshots_dir();

    let fixture_path = fixtures_dir.join("sample.rs");
    let snapshot_path = snapshots_dir.join("sample.rs.xml");

    if !fixture_path.exists() || !snapshot_path.exists() {
        eprintln!("Fixture or snapshot not found, skipping test");
        return;
    }

    // Parse the fixture using unified pipeline
    let parsed = parse_to_documents(&fixture_path, None, false, false, None)
        .expect("Should parse fixture");
    let current_xml = render_to_xml(&parsed);

    // Load the snapshot
    let snapshot = std::fs::read_to_string(&snapshot_path)
        .expect("Should read snapshot");

    // Normalize before comparing
    let normalize = |s: &str| {
        use regex::Regex;
        let mut normalized = s.to_string();

        // Remove XML declaration
        let xml_decl_re = Regex::new(r#"<\?xml[^?]*\?>\s*"#).unwrap();
        normalized = xml_decl_re.replace_all(&normalized, "").to_string();

        // Remove path attributes
        let path_re = Regex::new(r#"\s*path="[^"]*""#).unwrap();
        normalized = path_re.replace_all(&normalized, "").to_string();

        // Remove location attributes
        let loc_re = Regex::new(r#"\s*(start|end)="[^"]*""#).unwrap();
        normalized = loc_re.replace_all(&normalized, "").to_string();

        normalized
    };

    let normalized_current = normalize(&current_xml);
    let normalized_snapshot = normalize(&snapshot);

    if normalized_current != normalized_snapshot {
        eprintln!("Current (first 500 chars):\n{}", &normalized_current.chars().take(500).collect::<String>());
        eprintln!("\nSnapshot (first 500 chars):\n{}", &normalized_snapshot.chars().take(500).collect::<String>());

        for (i, (c1, c2)) in normalized_current.chars().zip(normalized_snapshot.chars()).enumerate() {
            if c1 != c2 {
                eprintln!("\nFirst difference at position {}: '{}' vs '{}'", i, c1, c2);
                eprintln!("Context: ...{}...", &normalized_current.chars().skip(i.saturating_sub(20)).take(40).collect::<String>());
                break;
            }
        }
    }

    assert_eq!(
        normalized_current, normalized_snapshot,
        "Current output should match snapshot (paths and locations normalized)"
    );
}

#[test]
fn test_xpath_structure_assertions() {
    // Test XPath structure assertions on fixtures
    let fixtures_dir = get_test_fixtures_dir();
    let fixture_path = fixtures_dir.join("sample.rs");

    if !fixture_path.exists() {
        eprintln!("Fixture not found, skipping test");
        return;
    }

    let mut parsed = parse_to_documents(&fixture_path, None, false, false, None)
        .expect("Should parse fixture");

    let engine = XPathEngine::new();

    // Assert: Should have 2 functions
    let matches = engine.query_documents(
        &mut parsed.documents,
        parsed.doc_handle,
        "//function",
        parsed.source_lines.clone(),
        &parsed.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 2, "Should have 2 functions");

    // Assert: Should have 'add' function
    let matches = engine.query_documents(
        &mut parsed.documents,
        parsed.doc_handle,
        "//function/name[type='add']",
        parsed.source_lines.clone(),
        &parsed.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should have 'add' function");

    // Assert: Should have 'main' function
    let matches = engine.query_documents(
        &mut parsed.documents,
        parsed.doc_handle,
        "//function/name[type='main']",
        parsed.source_lines.clone(),
        &parsed.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should have 'main' function");

    // Assert: Should have binary operator +
    let matches = engine.query_documents(
        &mut parsed.documents,
        parsed.doc_handle,
        "//binary[@op='+']",
        parsed.source_lines.clone(),
        &parsed.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should have + operator");
}

#[test]
fn test_multi_language_snapshots() {
    // Test that we can load and query snapshots from multiple languages
    let snapshots_dir = get_test_snapshots_dir();

    let languages = vec![
        ("sample.rs.xml", "//function", 2),
        ("sample.py.xml", "//function", 2),
        ("sample.js.xml", "//function", 2),
        ("sample.ts.xml", "//function", 2),
        ("sample.go.xml", "//function", 2),
        ("sample.java.xml", "//method", 2),
        ("sample.cs.xml", "//method", 2),
        ("sample.rb.xml", "//method", 2),
    ];

    let engine = XPathEngine::new();

    for (snapshot_name, xpath, expected_count) in languages {
        let snapshot_path = snapshots_dir.join(snapshot_name);

        if !snapshot_path.exists() {
            eprintln!("Snapshot not found, skipping: {:?}", snapshot_path);
            continue;
        }

        let mut result = load_xml_file_to_documents(&snapshot_path)
            .expect(&format!("Should load {}", snapshot_name));

        let matches = engine.query_documents(
            &mut result.documents,
            result.doc_handle,
            xpath,
            result.source_lines.clone(),
            &result.file_path,
        ).expect(&format!("Query should succeed for {}", snapshot_name));

        assert_eq!(
            matches.len(), expected_count,
            "{}: Expected {} matches for '{}', got {}",
            snapshot_name, expected_count, xpath, matches.len()
        );
    }
}

#[test]
fn test_xpath_string_value_preserves_whitespace() {
    // Test that inter-token whitespace is preserved in string-value
    let source = "let mut batches = Vec::new();";
    let mut result = parse_string_to_documents(source, "rust", "<test>".to_string(), true, false)
        .expect("Should parse Rust");

    let engine = XPathEngine::new();

    // Test 1: String value should include spaces between tokens
    let matches = engine.query_documents(
        &mut result.documents,
        result.doc_handle,
        "//let_declaration",
        result.source_lines.clone(),
        &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should find let_declaration");

    let value = &matches[0].value;
    assert!(
        value.contains("let mut batches"),
        "String value should preserve whitespace between tokens, got: {:?}",
        value
    );

    // Test 2: Exact string matching should work with whitespace
    let matches = engine.query_documents(
        &mut result.documents,
        result.doc_handle,
        "//let_declaration[contains(.,'let mut batches')]",
        result.source_lines.clone(),
        &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should match with contains() and whitespace");
}

#[test]
fn test_xpath_exact_string_match_without_formatting_whitespace() {
    // Test that exact string matching works (no extra formatting whitespace)
    let source = "class T { List<string> x; }";
    let mut result = parse_string_to_documents(source, "csharp", "<test>".to_string(), false, false)
        .expect("Should parse C#");

    let engine = XPathEngine::new();

    // Exact match on type should work
    let matches = engine.query_documents(
        &mut result.documents,
        result.doc_handle,
        "//type[.='List<string>']",
        result.source_lines.clone(),
        &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should find type with exact string match");
    assert_eq!(matches[0].value, "List<string>");
}

#[test]
fn test_csharp_null_forgiving_operator() {
    // Test that C# null-forgiving operator (!) is parsed correctly as postfix_unary_expression
    // This was historically broken due to shell escaping issues during testing (! -> \!)
    let source = "class T { void M() { var x = name!.Length; } }";
    let mut result = parse_string_to_documents(source, "csharp", "<test>".to_string(), false, false)
        .expect("Should parse C#");

    let engine = XPathEngine::new();

    // The ! should be parsed as postfix_unary_expression, not ERROR
    let matches = engine.query_documents(
        &mut result.documents,
        result.doc_handle,
        "//postfix_unary_expression",
        result.source_lines.clone(),
        &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should find postfix_unary_expression for null-forgiving operator");

    // Verify there are no ERROR nodes (which would indicate parsing failure)
    let errors = engine.query_documents(
        &mut result.documents,
        result.doc_handle,
        "//ERROR",
        result.source_lines.clone(),
        &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(errors.len(), 0, "Should have no ERROR nodes - null-forgiving operator should parse correctly");

    // Can query for member access on null-forgiving expression
    let matches = engine.query_documents(
        &mut result.documents,
        result.doc_handle,
        "//member[postfix_unary_expression]",
        result.source_lines.clone(),
        &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should find member access with postfix_unary_expression child");
}

// ============================================================================
// Data View (dual-branch) Tests
// ============================================================================

#[test]
fn test_json_dual_branch_structure() {
    // Verify JSON produces both <syntax> and <data> branches under <File>
    let source = r#"{"name": "John", "age": 30}"#;
    let mut result = parse_string_to_documents(source, "json", "<test>".to_string(), false, false)
        .expect("Should parse JSON");

    let engine = XPathEngine::new();

    // Both branches should exist
    let ast_matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//File/syntax", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(ast_matches.len(), 1, "Should have one <syntax> branch");

    let data_matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//File/data", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(data_matches.len(), 1, "Should have one <data> branch");

    // File should have format attribute
    let format_matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//File[@format='json']", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(format_matches.len(), 1, "File should have format='json'");
}

#[test]
fn test_json_syntax_vocabulary() {
    // Verify JSON syntax branch uses normalized vocabulary
    let source = r#"{"name": "John", "age": 30, "active": true, "x": null}"#;
    let mut result = parse_string_to_documents(source, "json", "<test>".to_string(), false, false)
        .expect("Should parse JSON");

    let engine = XPathEngine::new();

    // object at root of syntax branch
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//syntax/object", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Syntax should have <object> root");

    // properties
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//syntax//property", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 4, "Should have 4 properties");

    // key/value structure
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//syntax//property/key/string", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 4, "Each property should have key/string");

    // typed values
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//syntax//property[key/string='age']/value/number", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "age should have number value");
    assert_eq!(matches[0].value, "30");

    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//syntax//bool", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should have one bool");
    assert_eq!(matches[0].value, "true");

    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//syntax//null", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should have one null");
}

#[test]
fn test_json_data_view_simple() {
    // Verify JSON data view has key-as-element-name projection
    let source = r#"{"user": {"name": "John", "age": 30}}"#;
    let mut result = parse_string_to_documents(source, "json", "<test>".to_string(), false, false)
        .expect("Should parse JSON");

    let engine = XPathEngine::new();

    // Navigate by key names
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/user/name", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].value, "John");

    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/user/age", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].value, "30");
}

#[test]
fn test_json_data_view_arrays() {
    // Verify array handling in data view
    let source = r#"{"tags": ["math", "science"]}"#;
    let mut result = parse_string_to_documents(source, "json", "<test>".to_string(), false, false)
        .expect("Should parse JSON");

    let engine = XPathEngine::new();

    // Arrays repeat the parent key element
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/tags", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 2, "Should have 2 repeated tags elements");
    assert_eq!(matches[0].value, "math");
    assert_eq!(matches[1].value, "science");

    // Index access
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/tags[2]", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].value, "science");
}

#[test]
fn test_json_data_view_top_level_array() {
    // Top-level arrays should use <item> directly under <data>
    let source = r#"[1, "two", [3, 4]]"#;
    let mut result = parse_string_to_documents(source, "json", "<test>".to_string(), false, false)
        .expect("Should parse JSON");

    let engine = XPathEngine::new();

    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/item", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 3, "Top-level array should have 3 items");

    // Nested array items
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/item[3]/item", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 2, "Nested array should have 2 items");
}

#[test]
fn test_json_data_view_objects_in_array() {
    // Objects inside arrays get <item> wrappers with nested key elements
    let source = r#"[{"a": 1}, {"b": 2}]"#;
    let mut result = parse_string_to_documents(source, "json", "<test>".to_string(), false, false)
        .expect("Should parse JSON");

    let engine = XPathEngine::new();

    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/item[1]/a", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].value, "1");

    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/item[2]/b", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].value, "2");
}

#[test]
fn test_json_source_output_from_data() {
    // -o source should work from data branch nodes via span attributes
    let source = r#"{"name": "John", "age": 30}"#;
    let mut result = parse_string_to_documents(source, "json", "<test>".to_string(), false, false)
        .expect("Should parse JSON");

    let engine = XPathEngine::new();

    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/name", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1);

    // The match should have valid source location (not 1:1 default)
    assert!(matches[0].line >= 1);
    assert!(matches[0].column >= 1);

    // Extract source snippet — data view spans point to the VALUE, not the whole property
    let snippet = matches[0].extract_source_snippet();
    assert!(snippet.contains("John"), "Source should contain the value 'John': {:?}", snippet);
}

#[test]
fn test_json_data_view_spans_point_to_values() {
    // Data view spans should point to the VALUE, not the whole property
    let source = r#"{"user": {"name": "John"}, "age": 30}"#;
    let mut result = parse_string_to_documents(source, "json", "<test>".to_string(), false, false)
        .expect("Should parse JSON");

    let engine = XPathEngine::new();

    // //data/user/name span should cover "John" (the value including quotes)
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/user/name", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1);
    let snippet = matches[0].extract_source_snippet();
    assert_eq!(snippet, r#""John""#, "name span should cover the string value including quotes");

    // //data/user span should cover the entire object {...}
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/user", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1);
    let snippet = matches[0].extract_source_snippet();
    assert!(snippet.starts_with("{"), "user span should start with '{{': {:?}", snippet);
    assert!(snippet.contains("John"), "user span should contain 'John': {:?}", snippet);

    // //data/age span should cover the number value 30
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/age", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1);
    let snippet = matches[0].extract_source_snippet();
    assert_eq!(snippet, "30", "age span should cover the number value");
}

#[test]
fn test_json_data_view_escape_decoding() {
    // Data view should decode JSON escape sequences
    let source = r#"{"greeting": "hello\nworld", "tab": "a\tb", "quote": "say \"hi\""}"#;
    let mut result = parse_string_to_documents(source, "json", "<test>".to_string(), false, false)
        .expect("Should parse JSON");

    let engine = XPathEngine::new();

    // \n should be decoded to actual newline
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/greeting", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].value, "hello\nworld", "\\n should be decoded to newline");

    // \t should be decoded to actual tab
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/tab", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].value, "a\tb", "\\t should be decoded to tab");

    // \" should be decoded to literal quote
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/quote", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].value, "say \"hi\"", "\\\" should be decoded to quote");
}

#[test]
fn test_json_data_view_null_handling() {
    // Null values should appear as "null" text in data view
    let source = r#"{"name": "John", "nickname": null, "active": true, "count": false}"#;
    let mut result = parse_string_to_documents(source, "json", "<test>".to_string(), false, false)
        .expect("Should parse JSON");

    let engine = XPathEngine::new();

    // null value should have "null" as text content
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/nickname", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].value, "null", "null should appear as text 'null'");

    // boolean values
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/active[.='true']", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "true should be queryable as text");

    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/count[.='false']", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "false should be queryable as text");
}

#[test]
fn test_yaml_data_view_spans_point_to_values() {
    // YAML data view spans should point to values
    let source = "name: John\nage: 30";
    let mut result = parse_string_to_documents(source, "yaml", "<test>".to_string(), false, false)
        .expect("Should parse YAML");

    let engine = XPathEngine::new();

    // //data/name span should cover "John" (the value)
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/name", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1);
    let snippet = matches[0].extract_source_snippet();
    assert_eq!(snippet, "John", "name span should cover just the value");

    // //data/age span should cover "30"
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/age", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1);
    let snippet = matches[0].extract_source_snippet();
    assert_eq!(snippet, "30", "age span should cover just the value");
}

#[test]
fn test_yaml_data_view_null_handling() {
    // YAML null values should appear as "null" text in data view
    let source = "name: John\nnickname: null\nempty: ~";
    let mut result = parse_string_to_documents(source, "yaml", "<test>".to_string(), false, false)
        .expect("Should parse YAML");

    let engine = XPathEngine::new();

    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/nickname", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].value, "null", "null should appear as text 'null'");

    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/empty", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].value, "~", "tilde null should be queryable");
}

#[test]
fn test_json_raw_mode_unchanged() {
    // --raw mode should produce single tree (no syntax/data branches)
    let source = r#"{"a": 1}"#;
    let mut result = parse_string_to_documents(source, "json", "<test>".to_string(), true, false)
        .expect("Should parse JSON in raw mode");

    let engine = XPathEngine::new();

    // Should NOT have syntax/data branches
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//syntax", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 0, "Raw mode should not have <syntax> branch");

    // Should have raw TreeSitter nodes
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//document/object/pair", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Raw mode should have TreeSitter pair node");
}

#[test]
fn test_yaml_dual_branch_structure() {
    // Verify YAML produces both <syntax> and <data> branches
    let source = "name: John\nage: 30";
    let mut result = parse_string_to_documents(source, "yaml", "<test>".to_string(), false, false)
        .expect("Should parse YAML");

    let engine = XPathEngine::new();

    let syntax = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//File/syntax", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(syntax.len(), 1, "Should have <syntax> branch");

    let data = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//File/data", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(data.len(), 1, "Should have <data> branch");

    let format = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//File[@format='yaml']", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(format.len(), 1, "File should have format='yaml'");
}

#[test]
fn test_yaml_syntax_vocabulary() {
    // Verify YAML syntax uses same vocabulary as JSON syntax
    let source = "name: John\ncount: 42\nactive: true\nempty: null";
    let mut result = parse_string_to_documents(source, "yaml", "<test>".to_string(), false, false)
        .expect("Should parse YAML");

    let engine = XPathEngine::new();

    // Syntax branch should have object/property/key/value/string/number/bool/null
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//syntax/document/object", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Syntax should have document/object");

    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//syntax//property", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 4, "Should have 4 properties");

    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//syntax//property[key/string='count']/value/number", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "count should be a number");

    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//syntax//bool", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should have one bool");

    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//syntax//null", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should have one null");
}

#[test]
fn test_yaml_data_view() {
    // Verify YAML data view navigation
    // Single-doc YAML has <document> flattened, so //data/user works directly
    let source = "user:\n  name: John\n  age: 30\n  tags:\n    - math\n    - science";
    let mut result = parse_string_to_documents(source, "yaml", "<test>".to_string(), false, false)
        .expect("Should parse YAML");

    let engine = XPathEngine::new();

    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/user/name", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].value, "John");

    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/user/tags", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 2, "Should have 2 repeated tags elements");
    assert_eq!(matches[0].value, "math");
    assert_eq!(matches[1].value, "science");
}

#[test]
fn test_typescript_not_affected() {
    // Non-data languages should not have dual branches
    let source = "let x = 1;";
    let mut result = parse_string_to_documents(source, "typescript", "<test>".to_string(), false, false)
        .expect("Should parse TypeScript");

    let engine = XPathEngine::new();

    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//syntax", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 0, "TypeScript should not have <syntax> branch");

    // Should still have normal structure
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//variable", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "TypeScript should still work normally");
}

#[test]
fn test_json_empty_structures() {
    // Empty objects and arrays
    let source = r#"{"obj": {}, "arr": []}"#;
    let mut result = parse_string_to_documents(source, "json", "<test>".to_string(), false, false)
        .expect("Should parse JSON");

    let engine = XPathEngine::new();

    // Empty object in syntax branch
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//syntax//property[key/string='obj']/value/object", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should find empty object");

    // Empty array in syntax branch
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//syntax//property[key/string='arr']/value/array", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should find empty array");

    // Data view: empty containers become empty elements
    let matches = engine.query_documents(
        &mut result.documents, result.doc_handle,
        "//data/obj", result.source_lines.clone(), &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should find obj in data view");
}

// ============================================================================
// Schema Format Tests
// ============================================================================

#[test]
fn test_schema_collector_from_xot() {
    // Test schema collection directly from Xot tree
    let source = r#"
        class Foo { void Bar() {} }
        class Baz { void Qux() {} int x; }
    "#;
    let result = parse_string_to_documents(source, "csharp", "<test>".to_string(), false, false)
        .expect("Should parse C#");

    let doc_node = result.documents.document_node(result.doc_handle).unwrap();
    let mut collector = SchemaCollector::new();
    collector.collect_from_xot(result.documents.xot(), doc_node);

    let output = collector.format(None, false);

    // Should show class appearing twice
    assert!(output.contains("class (2)"), "Should show 2 classes: {}", output);
    // Should show method appearing twice (one in each class)
    assert!(output.contains("method (2)"), "Should show 2 methods: {}", output);
    // Should show field appearing once
    assert!(output.contains("field"), "Should show field: {}", output);
    // Should show class names
    assert!(output.contains("Foo") || output.contains("Baz"), "Should show class names: {}", output);
}

#[test]
fn test_schema_collector_from_xml_string() {
    // Test schema collection from XML string (simulates XPath match xml_fragments)
    let mut collector = SchemaCollector::new();

    // Simulate two matched class fragments
    collector.collect_from_xml_string("<class><name>Foo</name><body>{}</body></class>");
    collector.collect_from_xml_string("<class><name>Bar</name><body>{}</body></class>");

    let output = collector.format(None, false);

    // Should aggregate both classes
    assert!(output.contains("class (2)"), "Should show 2 classes: {}", output);
    assert!(output.contains("name (2)"), "Should show 2 names: {}", output);
    // Should collect unique text values
    assert!(output.contains("Foo"), "Should show Foo: {}", output);
    assert!(output.contains("Bar"), "Should show Bar: {}", output);
}

#[test]
fn test_schema_depth_limit() {
    // Test that depth limit works
    let source = "class Foo { void Bar() { int x = 1; } }";
    let result = parse_string_to_documents(source, "csharp", "<test>".to_string(), false, false)
        .expect("Should parse C#");

    let doc_node = result.documents.document_node(result.doc_handle).unwrap();
    let mut collector = SchemaCollector::new();
    collector.collect_from_xot(result.documents.xot(), doc_node);

    // With depth 2, should not show deeply nested elements
    let shallow = collector.format(Some(2), false);
    let deep = collector.format(None, false);

    // Deep output should have more lines than shallow
    assert!(
        deep.lines().count() > shallow.lines().count(),
        "Deep output should have more lines than shallow"
    );
}

#[test]
fn test_schema_structural_pairs() {
    // Test that structural pairs like {} are shown as {…}
    // Note: The structural pair detection only triggers when values are exactly "{" and "}"
    // We test this via XML string where we control the exact values
    let mut collector = SchemaCollector::new();
    collector.collect_from_xml_string("<body>{<child/>}</body>");

    let output = collector.format(None, false);

    // Body should show {…} for structural pair (exactly "{" and "}")
    assert!(output.contains("{…}"), "Should show structural pair as {{…}}: {}", output);
}

#[test]
fn test_schema_multiple_values_truncation() {
    // Test that more than 5 unique values are truncated with (+N)
    let source = r#"
        class A { } class B { } class C { } class D { }
        class E { } class F { } class G { }
    "#;
    let result = parse_string_to_documents(source, "csharp", "<test>".to_string(), false, false)
        .expect("Should parse C#");

    let doc_node = result.documents.document_node(result.doc_handle).unwrap();
    let mut collector = SchemaCollector::new();
    collector.collect_from_xot(result.documents.xot(), doc_node);

    let output = collector.format(None, false);

    // Should show (+N) for truncated values
    assert!(output.contains("(+"), "Should truncate values with (+N): {}", output);
}
