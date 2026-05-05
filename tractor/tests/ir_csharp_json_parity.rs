//! Compare IR→JSON output against the legacy XML→JSON snapshot for
//! the C# blueprint. As the IR-direct path matures, this test pins
//! its output against the existing JSON snapshot — both paths must
//! agree byte-for-byte until the legacy snapshot is updated.

#![allow(unused)]

use std::fs;

use serde_json::Value;
use tree_sitter::Parser;

use tractor::ir::{ir_to_json, lower_csharp_root};

fn blueprint_path() -> String {
    let candidates = [
        "../tests/integration/languages/csharp/blueprint.cs",
        "tests/integration/languages/csharp/blueprint.cs",
    ];
    for c in &candidates {
        if fs::metadata(c).is_ok() {
            return c.to_string();
        }
    }
    panic!("blueprint.cs not found under any candidate path");
}

fn snapshot_path() -> String {
    let candidates = [
        "../tests/integration/languages/csharp/blueprint.cs.snapshot.json",
        "tests/integration/languages/csharp/blueprint.cs.snapshot.json",
    ];
    for c in &candidates {
        if fs::metadata(c).is_ok() {
            return c.to_string();
        }
    }
    panic!("snapshot.json not found");
}

#[test]
#[ignore]
fn ir_json_matches_snapshot() {
    let source = fs::read_to_string(blueprint_path()).expect("blueprint");
    let mut p = Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(&source, None).unwrap();
    let ir = lower_csharp_root(tree.root_node(), &source);
    let json = ir_to_json(&ir, &source);

    let snap = fs::read_to_string(snapshot_path()).expect("snapshot");
    let expected: Value = serde_json::from_str(&snap).expect("parse snapshot");

    if json != expected {
        // Print first few diffs.
        let actual = serde_json::to_string_pretty(&json).unwrap();
        let expected = serde_json::to_string_pretty(&expected).unwrap();
        // Short prefix mismatch report
        let mismatch = actual.bytes().zip(expected.bytes()).position(|(a, b)| a != b).unwrap_or(0);
        let from = mismatch.saturating_sub(60);
        let to_a = (mismatch + 200).min(actual.len());
        let to_e = (mismatch + 200).min(expected.len());
        panic!(
            "IR→JSON differs from snapshot at byte {mismatch}\n\
             ----- got -----\n{}\n\
             ----- want ----\n{}\n",
            &actual[from..to_a],
            &expected[from..to_e],
        );
    }
}

#[test]
#[ignore]
fn dump_ir_json() {
    let source = fs::read_to_string(blueprint_path()).expect("blueprint");
    let mut p = Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(&source, None).unwrap();
    let ir = lower_csharp_root(tree.root_node(), &source);
    let json = ir_to_json(&ir, &source);
    println!("{}", serde_json::to_string_pretty(&json).unwrap());
}
