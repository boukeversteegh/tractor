//! `DataTree → serde_json::Value` direct-path tests.
//!
//! These tests confirm the format-agnostic tree can be converted
//! to JSON without going through Xot. The same `DataTree` tree
//! also goes through `data_to_xot` for queries, so the two paths
//! must agree on the *data* (xot adds source-attribute noise
//! `<x line="..."/>` that JSON projection drops anyway).

#![cfg(feature = "native")]

use serde_json::json;
use tree_sitter::Parser;

use tractor::tree::{data_to_json, lower_json_data_root};

fn parse_json(src: &str) -> tractor::tree::DataTree {
    let mut p = Parser::new();
    p.set_language(&tree_sitter_json::LANGUAGE.into()).unwrap();
    let cst = p.parse(src, None).unwrap();
    lower_json_data_root(cst.root_node(), src)
}

#[test]
fn json_object_round_trips_to_json_value() {
    let tree = parse_json(r#"{"name":"alice","age":30,"active":true,"meta":null}"#);
    let value = data_to_json(&tree);
    assert_eq!(
        value,
        json!({
            "name": "alice",
            "age": 30,
            "active": true,
            "meta": null,
        })
    );
}

#[test]
fn json_array_preserves_order() {
    let tree = parse_json(r#"[1, 2.5, "three", false, null]"#);
    let value = data_to_json(&tree);
    assert_eq!(value, json!([1, 2.5, "three", false, null]));
}

#[test]
fn json_nested_structure() {
    let tree = parse_json(r#"{"users":[{"id":1},{"id":2}],"count":2}"#);
    let value = data_to_json(&tree);
    assert_eq!(
        value,
        json!({
            "users": [{"id": 1}, {"id": 2}],
            "count": 2,
        })
    );
}

#[test]
fn json_integers_stay_integers() {
    // Number literals in source: integer-shaped input must produce
    // integer JSON (not 1.0).
    let tree = parse_json(r#"{"n": 42}"#);
    let value = data_to_json(&tree);
    let n = &value["n"];
    assert!(n.is_i64(), "expected i64, got {n:?}");
    assert_eq!(n.as_i64().unwrap(), 42);
}

#[test]
fn json_string_escapes_resolved() {
    // The DataTree's String.value is the *parsed* string (escapes
    // resolved); the direct JSON path emits that, not the raw
    // source bytes.
    let tree = parse_json(r#"{"text": "line1\nline2"}"#);
    let value = data_to_json(&tree);
    assert_eq!(value["text"], json!("line1\nline2"));
}

#[test]
fn json_top_level_array() {
    let tree = parse_json(r#"[1, 2, 3]"#);
    let value = data_to_json(&tree);
    assert_eq!(value, json!([1, 2, 3]));
}
