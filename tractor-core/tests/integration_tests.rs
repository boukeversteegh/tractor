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
        &result.source_lines,
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
        &result.source_lines,
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
    let parsed = parse_to_documents(&fixture_path, None, false, false)
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

    let mut parsed = parse_to_documents(&fixture_path, None, false, false)
        .expect("Should parse fixture");

    let engine = XPathEngine::new();

    // Assert: Should have 2 functions
    let matches = engine.query_documents(
        &mut parsed.documents,
        parsed.doc_handle,
        "//function",
        &parsed.source_lines,
        &parsed.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 2, "Should have 2 functions");

    // Assert: Should have 'add' function
    let matches = engine.query_documents(
        &mut parsed.documents,
        parsed.doc_handle,
        "//function/name[type='add']",
        &parsed.source_lines,
        &parsed.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should have 'add' function");

    // Assert: Should have 'main' function
    let matches = engine.query_documents(
        &mut parsed.documents,
        parsed.doc_handle,
        "//function/name[type='main']",
        &parsed.source_lines,
        &parsed.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should have 'main' function");

    // Assert: Should have binary operator +
    let matches = engine.query_documents(
        &mut parsed.documents,
        parsed.doc_handle,
        "//binary[@op='+']",
        &parsed.source_lines,
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
            &result.source_lines,
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
        &result.source_lines,
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
        &result.source_lines,
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
        &result.source_lines,
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
        &result.source_lines,
        &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should find postfix_unary_expression for null-forgiving operator");

    // Verify there are no ERROR nodes (which would indicate parsing failure)
    let errors = engine.query_documents(
        &mut result.documents,
        result.doc_handle,
        "//ERROR",
        &result.source_lines,
        &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(errors.len(), 0, "Should have no ERROR nodes - null-forgiving operator should parse correctly");

    // Can query for member access on null-forgiving expression
    let matches = engine.query_documents(
        &mut result.documents,
        result.doc_handle,
        "//member[postfix_unary_expression]",
        &result.source_lines,
        &result.file_path,
    ).expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should find member access with postfix_unary_expression child");
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
