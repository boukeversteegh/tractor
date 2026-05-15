//! S6A / S6C — WASM↔CLI parity fixtures.
//!
//! Verifies that the WASM `parseToXml` path and the native CLI parse
//! path produce byte-identical semantic XML for the same source on
//! every migrated language.
//!
//! ## How parity is achieved
//!
//! Both paths feed the same lowering function (`lower_<lang>_root`) a
//! [`RawNode`] built from the same source bytes. On native the
//! `RawNode` comes from `RawNode::from_tree_sitter`; on WASM it comes
//! from `serde_json::from_str::<RawNode>(...)` decoding the JSON that
//! `web/src/parser.ts:serializeNode` emits.
//!
//! This test simulates the WASM path natively: it builds a
//! `SerializedNode` (the JSON shape `web-tree-sitter` produces) from a
//! tree-sitter parse, then routes it through
//! `wasm::parse_ast_to_xml` — the same function the WASM module calls.
//! The CLI path runs via the public `tractor::parse(...)` API.
//!
//! Any divergence indicates a bug in the JSON↔RawNode bridge, the
//! lowering signature, or the render path — all of which S6
//! re-architected for parity.

#![cfg(all(feature = "native", feature = "wasm"))]

use tractor::raw::RawNode;
use tractor::wasm::ast::SerializedNode;

// --- Helpers ------------------------------------------------------------

/// Convert a `RawNode` (what the native CLI builds from tree-sitter)
/// into a `SerializedNode` (the JSON shape `web-tree-sitter` would
/// emit). Both types are structurally identical with camelCase JSON;
/// round-tripping via JSON proves the bridge is faithful.
fn raw_to_serialized(raw: &RawNode) -> SerializedNode {
    let json = serde_json::to_string(raw).expect("RawNode → JSON");
    serde_json::from_str(&json).expect("JSON → SerializedNode")
}

/// Build the CLI XML by parsing through `parse_string_to_xot` (the
/// xot-producing native entry) and rendering with the same
/// `RenderOptions` the WASM path uses. Restricts to
/// `tree_mode=structure` so the comparison covers the typed pipeline.
fn cli_xml(language: &str, source: &str) -> String {
    let res = tractor::parser::parse_string_to_xot(
        source,
        language,
        "input".to_string(),
        Some(tractor::TreeMode::Structure),
    )
    .expect("CLI parse");
    let options = tractor::RenderOptions::new()
        .with_meta(false)
        .with_pretty_print(true);
    tractor::render_document(&res.xot, res.root, &options)
}

/// Build the WASM XML by going `tree-sitter → RawNode →
/// SerializedNode → wasm::parse_ast_to_xml`. Mirrors what the browser
/// playground does, minus the actual JS-side serialisation step.
fn wasm_xml(language: &str, source: &str) -> String {
    let lang_ops = tractor::languages::get_language(language)
        .expect("language registered");
    let grammar = (lang_ops.grammar)();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&grammar).expect("set_language");
    let tree = parser.parse(source, None).expect("tree-sitter parse");
    let raw = RawNode::from_tree_sitter(tree.root_node(), source);
    let serialized = raw_to_serialized(&raw);
    tractor::wasm::parse_ast_to_xml(
        &serialized,
        source,
        language,
        "input",
        Some(tractor::TreeMode::Structure),
        /*include_locations=*/ false,
        /*pretty_print=*/ true,
    )
    .expect("wasm parse")
}

fn assert_parity(language: &str, source: &str) {
    let cli = cli_xml(language, source);
    let wasm = wasm_xml(language, source);
    assert_eq!(
        cli, wasm,
        "\nLanguage: {language}\n--- CLI ---\n{cli}\n--- WASM ---\n{wasm}\n"
    );
}

// --- Per-language fixtures ---------------------------------------------

#[test]
fn typescript_parity() {
    assert_parity("typescript", "const x = 1;\nfunction f(a: number): number { return a + 1; }\n");
}

#[test]
fn csharp_parity() {
    assert_parity(
        "csharp",
        "namespace N { public class C { public int X { get; set; } } }\n",
    );
}

#[test]
fn python_parity() {
    assert_parity("python", "def f(x):\n    return x + 1\n");
}

#[test]
fn go_parity() {
    assert_parity("go", "package main\nfunc Add(a int, b int) int { return a + b }\n");
}

#[test]
fn rust_parity() {
    assert_parity("rust", "fn main() { let x = 1; println!(\"{}\", x); }\n");
}

#[test]
fn java_parity() {
    assert_parity(
        "java",
        "class C { public int add(int a, int b) { return a + b; } }\n",
    );
}

#[test]
fn ruby_parity() {
    assert_parity("ruby", "def add(a, b)\n  a + b\nend\n");
}

#[test]
fn php_parity() {
    assert_parity("php", "<?php\nfunction add($a, $b) { return $a + $b; }\n");
}

#[test]
fn tsql_parity() {
    assert_parity("tsql", "SELECT id, name FROM users WHERE id = 1;\n");
}

// --- Data-language fixtures (S6D: typed `DataTree` pipeline on WASM) ----

#[test]
fn json_parity() {
    assert_parity("json", r#"{"name": "alice", "age": 30, "tags": ["x", "y"]}"#);
}

#[test]
fn yaml_parity() {
    assert_parity(
        "yaml",
        "name: alice\nage: 30\ntags:\n  - x\n  - y\n",
    );
}

#[test]
fn toml_parity() {
    assert_parity("toml", "name = \"alice\"\nage = 30\n[address]\ncity = \"NY\"\n");
}

#[test]
fn ini_parity() {
    assert_parity("ini", "[section]\nname = alice\nage = 30\n");
}

#[test]
fn markdown_parity() {
    assert_parity("markdown", "# Title\n\nA paragraph with **bold** text.\n");
}
