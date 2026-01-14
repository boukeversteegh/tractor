/// Integration tests for tractor-core
///
/// These tests verify:
/// 1. XML pass-through functionality
/// 2. Snapshot loading and querying
/// 3. XPath querying against pre-generated XML

use std::path::PathBuf;
use tractor_core::{
    load_xml, load_xml_file, generate_xml_document,
    XPathEngine, parse_file,
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

#[test]
fn test_load_xml_passthrough() {
    // Test that we can load XML directly without parsing source
    let xml = r#"<file>
  <function>
    fn
    <name><type>test</type></name>
  </function>
</file>"#.to_string();

    let result = load_xml(xml.clone(), "test.xml".to_string());

    assert_eq!(result.xml, xml);
    assert_eq!(result.file_path, "test.xml");
    assert_eq!(result.language, "xml");
}

#[test]
fn test_query_xml_passthrough() {
    // Test that we can query XML loaded via pass-through
    let xml = r#"<file>
  <function>fn <name><type>test</type></name></function>
  <function>fn <name><type>main</type></name></function>
</file>"#.to_string();

    let result = load_xml(xml, "test.xml".to_string());
    let doc = generate_xml_document(&[result.clone()]);

    let engine = XPathEngine::new();
    let matches = engine.query(&doc, "//function", &result.source_lines, &result.file_path)
        .expect("Query should succeed");

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

    let result = load_xml_file(&snapshot_path)
        .expect("Should load snapshot");

    // The snapshot already has Files/File wrapper, so use it directly
    let engine = XPathEngine::new();
    let matches = engine.query(&result.xml, "//function", &result.source_lines, &result.file_path)
        .expect("Query should succeed");

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

    // Parse the fixture
    let parsed = parse_file(&fixture_path, None, false)
        .expect("Should parse fixture");
    let current_xml = generate_xml_document(&[parsed]);

    // Load the snapshot
    let snapshot = std::fs::read_to_string(&snapshot_path)
        .expect("Should read snapshot");

    // Normalize before comparing:
    // 1. Remove XML declaration
    // 2. Remove path attribute (snapshots have absolute paths)
    // 3. Remove location attributes (start/end) as they may vary
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
        // Print first 500 chars of each for debugging
        eprintln!("Current (first 500 chars):\n{}", &normalized_current.chars().take(500).collect::<String>());
        eprintln!("\nSnapshot (first 500 chars):\n{}", &normalized_snapshot.chars().take(500).collect::<String>());

        // Find first difference
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

    let parsed = parse_file(&fixture_path, None, false)
        .expect("Should parse fixture");
    let xml = generate_xml_document(&[parsed.clone()]);

    let engine = XPathEngine::new();

    // Assert: Should have 2 functions
    let matches = engine.query(&xml, "//function", &parsed.source_lines, &parsed.file_path)
        .expect("Query should succeed");
    assert_eq!(matches.len(), 2, "Should have 2 functions");

    // Assert: Should have 'add' function
    let matches = engine.query(&xml, "//function/name[type='add']", &parsed.source_lines, &parsed.file_path)
        .expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should have 'add' function");

    // Assert: Should have 'main' function
    let matches = engine.query(&xml, "//function/name[type='main']", &parsed.source_lines, &parsed.file_path)
        .expect("Query should succeed");
    assert_eq!(matches.len(), 1, "Should have 'main' function");

    // Assert: Should have binary operator +
    let matches = engine.query(&xml, "//binary[@op='+']", &parsed.source_lines, &parsed.file_path)
        .expect("Query should succeed");
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

        let result = load_xml_file(&snapshot_path)
            .expect(&format!("Should load {}", snapshot_name));

        let matches = engine.query(&result.xml, xpath, &result.source_lines, &result.file_path)
            .expect(&format!("Query should succeed for {}", snapshot_name));

        assert_eq!(
            matches.len(), expected_count,
            "{}: Expected {} matches for '{}', got {}",
            snapshot_name, expected_count, xpath, matches.len()
        );
    }
}
