//! `SyntaxTree → JSON → SyntaxTree` round-trip tests.
//!
//! Verifies that the generated `tree_from_json` reconstructs a
//! `SyntaxTree` shape sufficient to round-trip through the variant-blind
//! JSON walker. Bit-perfect identity is **not** required (synthetic
//! ranges, lossy access-segment chains, dropped ordering of duplicate
//! keys); the goal is "same `$type` shape when re-serialized".
//!
//! See `tractor/build_codegen.rs::render_from_json` for the codegen.

#![cfg(feature = "native")]

use serde_json::{json, Value};
use tree_sitter::Parser;

use tractor::tree::{tree_from_json, tree_to_json};
use tractor::languages::python::lower_python_root;
use tractor::languages::ruby::lower_ruby_root;
use tractor::raw::RawNode;

fn parse_python(src: &str) -> tractor::tree::SyntaxTree {
    let mut p = Parser::new();
    p.set_language(&tree_sitter_python::LANGUAGE.into()).unwrap();
    let cst = p.parse(src, None).unwrap();
    lower_python_root(&RawNode::from_tree_sitter(cst.root_node(), src), src)
}

fn parse_ruby(src: &str) -> tractor::tree::SyntaxTree {
    let mut p = Parser::new();
    p.set_language(&tree_sitter_ruby::LANGUAGE.into()).unwrap();
    let cst = p.parse(src, None).unwrap();
    lower_ruby_root(&RawNode::from_tree_sitter(cst.root_node(), src), src)
}

/// Strip every `line`/`column`/`end_line`/`end_column`/`id` attribute
/// recursively. After round-tripping through `from_json` the
/// reconstructed tree carries synthetic spans, so comparing the
/// re-serialized JSON requires ignoring position metadata. The current
/// JSON walker doesn't emit those attributes (they're XML-only), but
/// future additions might — strip defensively.
fn strip_positions(v: &mut Value) {
    match v {
        Value::Object(map) => {
            for key in ["line", "column", "end_line", "end_column", "id"] {
                map.remove(key);
            }
            for (_, val) in map.iter_mut() {
                strip_positions(val);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                strip_positions(item);
            }
        }
        _ => {}
    }
}

/// Walk a JSON value and reduce every object to just its `$type`
/// field, recursively. Lets us assert structural shape ("the
/// reconstructed tree has the same nested-`$type` skeleton as the
/// original") without comparing scalar leaves byte-for-byte.
fn shape_skeleton(v: &Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            if let Some(t) = map.get("$type") {
                out.insert("$type".into(), t.clone());
            }
            for (k, val) in map {
                if k == "$type" { continue; }
                out.insert(k.clone(), shape_skeleton(val));
            }
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(shape_skeleton).collect()),
        // Replace scalars with a marker so leaf-text differences (e.g.
        // "true" vs "True") don't fail structural equality.
        _ => json!("<leaf>"),
    }
}

#[test]
fn scalar_string_round_trips_as_name() {
    let value = json!("hello");
    let tree = tree_from_json(&value, None);
    let re = tree_to_json(&tree, "", None);
    assert_eq!(re, json!("hello"));
}

#[test]
fn scalar_bool_round_trips_as_true_false() {
    let true_tree = tree_from_json(&json!(true), None);
    let false_tree = tree_from_json(&json!(false), None);
    assert_eq!(tree_to_json(&true_tree, "", None), json!("true"));
    assert_eq!(tree_to_json(&false_tree, "", None), json!("false"));
}

#[test]
fn scalar_number_round_trips_as_int() {
    let tree = tree_from_json(&json!(42), None);
    assert_eq!(tree_to_json(&tree, "", None), json!("42"));
}

#[test]
fn unknown_dollar_type_becomes_unknown_variant() {
    let value = json!({ "$type": "not-a-real-variant", "anything": "value" });
    let tree = tree_from_json(&value, None);
    // The Unknown variant carries the original tag in its `kind`
    // field. Forward to_json doesn't surface `kind` (it's neither a
    // child nor a marker), so we assert on the kind directly.
    if let tractor::tree::SyntaxTree::Unknown { kind, .. } = &tree {
        assert!(
            kind.contains("not-a-real-variant"),
            "expected Unknown::kind to mention the original tag, got: {}",
            kind,
        );
    } else {
        panic!("expected Unknown variant, got: {:?}", tree);
    }
}

#[test]
fn python_simple_module_skeleton_round_trips() {
    let src = "x = 1\n";
    let tree = parse_python(src);
    let forward = tree_to_json(&tree, src, None);
    let reconstructed = tree_from_json(&forward, None);
    let backward = tree_to_json(&reconstructed, "", None);

    let mut a = forward.clone();
    let mut b = backward.clone();
    strip_positions(&mut a);
    strip_positions(&mut b);

    // Compare $type skeletons. Full-leaf equality fails because
    // reconstructed leaves use synthetic ranges and the forward walker
    // can pull text from source bytes; the skeleton check is the
    // structural assertion the codegen is responsible for.
    assert_eq!(
        shape_skeleton(&a),
        shape_skeleton(&b),
        "round-trip changed the $type skeleton\nforward:\n{}\nbackward:\n{}",
        serde_json::to_string_pretty(&a).unwrap(),
        serde_json::to_string_pretty(&b).unwrap(),
    );
}

#[test]
fn python_import_with_alias_round_trips() {
    let src = "import os as system\n";
    let tree = parse_python(src);
    let forward = tree_to_json(&tree, src, None);
    let reconstructed = tree_from_json(&forward, None);
    let backward = tree_to_json(&reconstructed, "", None);

    assert_eq!(
        shape_skeleton(&forward),
        shape_skeleton(&backward),
        "round-trip changed the $type skeleton on aliased import",
    );
}

/// Collect every JSON key that appears under any object, recursively.
/// Used to assert that round-tripping preserves the variant tags
/// that survive as keys after `$type`-stripping.
fn json_key_set(v: &Value) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    fn walk(v: &Value, out: &mut std::collections::BTreeSet<String>) {
        match v {
            Value::Object(map) => {
                for (k, val) in map {
                    if !k.starts_with('$') { out.insert(k.clone()); }
                    walk(val, out);
                }
            }
            Value::Array(arr) => for item in arr { walk(item, out); },
            _ => {}
        }
    }
    walk(v, &mut out);
    out
}

#[test]
fn ruby_class_with_method_keeps_class_and_method_keys() {
    let src = "class Foo\n  def bar(x)\n    x + 1\n  end\nend\n";
    let tree = parse_ruby(src);
    let forward = tree_to_json(&tree, src, None);
    let reconstructed = tree_from_json(&forward, None);
    let backward = tree_to_json(&reconstructed, "", None);

    let forward_keys = json_key_set(&forward);
    let backward_keys = json_key_set(&backward);

    // The high-level declaration tags must round-trip. Lossy slot
    // shapes (which children land under `<expression>` vs raw, etc.)
    // are tolerated — those are documented limitations of the
    // type-driven inverse.
    for required in ["class", "body", "method", "name", "parameter"] {
        assert!(
            forward_keys.contains(required),
            "forward JSON missing key {:?}: {:?}",
            required, forward_keys,
        );
        assert!(
            backward_keys.contains(required),
            "backward JSON missing key {:?} after round-trip: {:?}",
            required, backward_keys,
        );
    }
}

#[test]
fn array_value_round_trips_through_inline() {
    let value = json!(["a", "b", "c"]);
    let tree = tree_from_json(&value, None);
    let re = tree_to_json(&tree, "", None);
    // `Inline` has no element name, so its three `Name` children
    // flow directly to the parent renderer. With no parent, the
    // children list collapses back to a JSON array — clean
    // round-trip.
    assert_eq!(re, json!(["a", "b", "c"]));
}

#[test]
fn object_without_dollar_type_becomes_inline_or_unknown() {
    let value = json!({ "name": "x", "value": "1" });
    let tree = tree_from_json(&value, None);
    // No $type → falls through to the Unknown arm with tag="".
    // Round-tripped JSON should still serialize cleanly.
    let re = tree_to_json(&tree, "", None);
    let s = serde_json::to_string(&re).unwrap();
    assert!(s.contains("unknown") || s.contains("$type"));
}
