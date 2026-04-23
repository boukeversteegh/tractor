//! Render round-trip integration tests.
//!
//! Each language has a single fixture file under
//! `tests/integration/render/<lang>/supported.<ext>`. The test parses the
//! fixture, renders the resulting semantic tree back to source, and asserts
//! byte equality with the fixture.
//!
//! The fixture IS the snapshot of what the renderer supports: anything that
//! stays in the file round-trips, anything new you add to the file must
//! round-trip too. When the test fails, the fixture's git diff pinpoints the
//! regression.

use std::path::PathBuf;

use tractor::parser::parse_string_to_xot;
use tractor::render::{parse_xml, render, RenderOptions as CodeRenderOptions};
use tractor::{render_document, RenderOptions as XmlRenderOptions, TreeMode};

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR points at `<repo>/tractor`; the fixtures live under
    // the workspace-level `tests/` directory.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn fixture(lang_dir: &str, name: &str) -> PathBuf {
    repo_root()
        .join("tests/integration/render")
        .join(lang_dir)
        .join(name)
}

/// Parse `source` using the given language, render the tree back to source,
/// and return the rendered string.
fn round_trip(source: &str, lang: &str) -> String {
    let parsed = parse_string_to_xot(source, lang, "<fixture>".to_string(), None)
        .expect("parse_string_to_xot");
    let xml_opts = XmlRenderOptions {
        include_meta: false,
        pretty_print: true,
        use_color: false,
        ..XmlRenderOptions::new()
    };
    let xml = render_document(&parsed.xot, parsed.root, &xml_opts);
    let node = parse_xml(&xml).expect("parse_xml");
    render(&node, lang, TreeMode::Data, &CodeRenderOptions::default()).expect("render")
}

/// Assert that the given fixture file round-trips through parse→render
/// unchanged. Trailing newlines on the fixture are preserved; the renderer's
/// output is compared against the file content byte-for-byte (after stripping
/// a single optional trailing newline on either side so editors that save a
/// final newline don't cause false failures).
fn assert_fixture_round_trips(path: &PathBuf, lang: &str) {
    let source = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()));
    let rendered = round_trip(&source, lang);

    let lhs = source.trim_end_matches('\n');
    let rhs = rendered.trim_end_matches('\n');

    if lhs != rhs {
        // Render a compact diff for the failure message: show the first
        // differing line so CI logs stay focused.
        let first_diff = lhs
            .lines()
            .zip(rhs.lines())
            .enumerate()
            .find(|(_, (a, b))| a != b);
        let where_ = match first_diff {
            Some((line, (a, b))) => format!(
                "\nfirst diff at line {}:\n  source:   {a}\n  rendered: {b}",
                line + 1
            ),
            None => format!(
                "\n(length differs: source={} lines, rendered={} lines)",
                lhs.lines().count(),
                rhs.lines().count()
            ),
        };
        panic!(
            "round-trip mismatch for {}{}\n\n--- source ---\n{}\n\n--- rendered ---\n{}\n",
            path.display(),
            where_,
            lhs,
            rhs
        );
    }
}

#[test]
fn csharp_supported_fixture_round_trips() {
    let path = fixture("csharp", "supported.cs");
    assert_fixture_round_trips(&path, "csharp");
}

#[test]
fn python_supported_fixture_round_trips() {
    let path = fixture("python", "supported.py");
    assert_fixture_round_trips(&path, "python");
}

#[test]
fn typescript_supported_fixture_round_trips() {
    let path = fixture("typescript", "supported.ts");
    assert_fixture_round_trips(&path, "typescript");
}

#[test]
fn java_supported_fixture_round_trips() {
    let path = fixture("java", "supported.java");
    assert_fixture_round_trips(&path, "java");
}
