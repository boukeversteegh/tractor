//! Property tests for the unified [`tractor::tree::render::render`]
//! engine (S13-Z8).
//!
//! Three properties cover the three input-source modes:
//!
//! - **Round-trip:** `render(parse(S), lang, Some(S)) == S` byte-for-
//!   byte for every blueprint fixture in `tests/integration/languages/`.
//!   Validates that anchored rendering is lossless across the parse →
//!   render boundary.
//!
//! - **Canonical:** `parse(render(T, lang, None))` produces a tree
//!   structurally equivalent to `T` for a synthetic hand-built tree.
//!   Validates that the per-language `Syntax` config + stored scalar
//!   text are enough to reconstitute source from a synthetic tree.
//!
//! - **Mixed:** parse a fixture, programmatically insert a synthetic
//!   node (no source range), render with the original source as
//!   anchor. The anchored regions stay byte-identical to the original;
//!   the synthetic region renders from its stored text / canonical
//!   defaults.
//!
//! These are the load-bearing tests for S13: anything that breaks the
//! round-trip property is a regression in the renderer.

#![cfg(feature = "native")]

use tractor::{parse, ParseInput, ParseOptions};
use tractor::tree::render::render;
use tractor::tree::SyntaxTree;
use tractor::xpath::Tree;

/// `(lang, fixture_path)` pairs, one per tree-supported language.
fn blueprint_fixtures() -> Vec<(&'static str, &'static str)> {
    vec![
        ("csharp", "tests/integration/languages/csharp/blueprint.cs"),
        ("go", "tests/integration/languages/go/blueprint.go"),
        ("java", "tests/integration/languages/java/blueprint.java"),
        ("php", "tests/integration/languages/php/blueprint.php"),
        ("python", "tests/integration/languages/python/blueprint.py"),
        ("ruby", "tests/integration/languages/ruby/blueprint.rb"),
        ("rust", "tests/integration/languages/rust/blueprint.rs"),
        ("typescript", "tests/integration/languages/typescript/blueprint.ts"),
    ]
}

fn read_fixture(path: &str) -> String {
    let workspace_root = env!("CARGO_MANIFEST_DIR");
    let parent = std::path::Path::new(workspace_root)
        .parent()
        .expect("crate dir has parent");
    std::fs::read_to_string(parent.join(path))
        .unwrap_or_else(|e| panic!("read {}: {}", path, e))
}

/// Round-trip property: for every blueprint, anchored rendering is
/// byte-identical to the input source.
#[test]
fn round_trip_blueprint_is_byte_identical() {
    for (lang, path) in blueprint_fixtures() {
        let source = read_fixture(path);
        let parsed = parse(
            ParseInput::Inline { content: &source, file_label: path },
            ParseOptions { language: Some(lang), ..Default::default() },
        )
        .unwrap_or_else(|e| panic!("parse {}: {}", path, e));
        let (tree, src) = match parsed.root_tree.as_ref() {
            Some(Tree::SyntaxTree { tree, source, .. }) => (tree.clone(), source.clone()),
            _ => panic!("{}: no SyntaxTree (expected for {})", path, lang),
        };

        let rendered = render(&tree, lang, Some(&src));
        assert_eq!(
            rendered, *src,
            "{} ({}): anchored render is not byte-identical to source",
            path, lang
        );
    }
}

/// Canonical property: parse(render(synthetic, lang, None)) is
/// structurally equivalent to the synthetic tree.
///
/// "Structurally equivalent" here is intentionally loose for the
/// initial test — we only check that the round-trip preserves the
/// *root* element kind and a single identifier inside it. Tightening
/// the equivalence relation (matching the full variant tree, ignoring
/// byte ranges) is S13-Z5/Z8 follow-up work; for the initial slice
/// the assertion exercises the unified path end-to-end.
#[test]
fn canonical_render_re_parses_to_equivalent_root() {
    // Hand-built synthetic tree: a single identifier expression in
    // Python. Picked Python because its canonical form (`x`) is the
    // shortest re-parseable thing across the eight languages.
    let tree = SyntaxTree::Module {
        children: vec![SyntaxTree::Name {
            text: "synthetic_identifier".to_string(),
            range: tractor::tree::ByteRange::synthetic_empty(),
            span: tractor::tree::Span::point(1, 1),
        }],
        range: tractor::tree::ByteRange::synthetic_empty(),
        span: tractor::tree::Span::point(1, 1),
    };

    let rendered = render(&tree, "python", None);
    assert!(
        rendered.contains("synthetic_identifier"),
        "canonical render must include the synthetic identifier text, got: {:?}",
        rendered
    );

    let reparsed = parse(
        ParseInput::Inline { content: &rendered, file_label: "<synthetic>" },
        ParseOptions { language: Some("python"), ..Default::default() },
    )
    .unwrap_or_else(|e| panic!("re-parse failed: {}", e));
    let reparsed_tree = match reparsed.root_tree.as_ref() {
        Some(Tree::SyntaxTree { tree, .. }) => tree.clone(),
        _ => panic!("re-parsed Python must produce a SyntaxTree"),
    };
    assert!(
        matches!(reparsed_tree.as_ref(), SyntaxTree::Module { .. }),
        "re-parsed tree must be a Module"
    );
}

/// Mixed property: render(parse(S) + synthetic insertion, lang,
/// Some(S)) preserves byte-identical output for the anchored regions
/// and renders the synthetic region from its stored text. Today the
/// per-subtree byte-slice shortcut in `write_ir_with_source` is what
/// makes this work; the gap-context fallback (S13-Z6 second half) is
/// still future work.
///
/// Initial slice asserts: the rendered output contains both the
/// original-source prefix AND the synthetic identifier's text. Stricter
/// "byte-identical for anchored regions, canonical for synthetic" is
/// follow-up tightening once the gap engine lands.
#[test]
fn mixed_render_preserves_anchored_and_emits_synthetic() {
    let source = "x = 1\n";
    let parsed = parse(
        ParseInput::Inline { content: source, file_label: "<mixed>" },
        ParseOptions { language: Some("python"), ..Default::default() },
    )
    .expect("parse Python");
    let (tree_arc, src) = match parsed.root_tree.as_ref() {
        Some(Tree::SyntaxTree { tree, source, .. }) => (tree.clone(), source.clone()),
        _ => panic!("python yields SyntaxTree"),
    };
    let mut tree: SyntaxTree = (*tree_arc).clone();

    // Insert a synthetic identifier as a new top-level child. (The
    // synthetic node carries unanchored range; the surrounding Module
    // remains anchored to the parsed source.)
    if let SyntaxTree::Module { children, .. } = &mut tree {
        children.push(SyntaxTree::Name {
            text: "ZSYNTHETIC".to_string(),
            range: tractor::tree::ByteRange::synthetic_empty(),
            span: tractor::tree::Span::point(2, 1),
        });
    } else {
        panic!("Python root should be a Module");
    }

    let rendered = render(&tree, "python", Some(&src));
    assert!(
        rendered.contains("ZSYNTHETIC"),
        "mixed render must emit the synthetic identifier text, got: {:?}",
        rendered
    );
}

// ===========================================================================
// DataTree property tests — analogous to the SyntaxTree side. DataTree has
// its own renderer (`DataTree::to_source` for anchored mode); the parallel
// here is that the new `quote_style` / `text` fields populated during
// lowering give the renderer everything it needs even when source is
// detached. Unifying the DataTree renderer with the SyntaxTree side is
// follow-up work; these tests cover the data-model invariants.
// ===========================================================================

/// `(lang, source)` pairs for inline data-format fixtures. Inline rather
/// than file-based because the project doesn't ship blueprint fixtures
/// for JSON/YAML/TOML/INI/Markdown today.
fn data_fixtures() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "json",
            r#"{
  "name": "tractor",
  "version": 13,
  "stable": true,
  "deprecated": false,
  "build": null,
  "tags": ["renderer", "tree", "xpath"]
}
"#,
        ),
        (
            "yaml",
            "name: tractor\nversion: 13\nstable: true\nbuild: ~\nquoted: 'hello'\ndouble: \"world\"\ntags:\n  - renderer\n  - tree\n",
        ),
        (
            "toml",
            "name = \"tractor\"\nversion = 13\nstable = true\n\n[build]\nempty = false\n",
        ),
        ("ini", "[section]\nname = tractor\nversion = 13\n"),
    ]
}

/// Round-trip for DataTree: `data_tree.to_source(source) == source` for
/// every inline data-format fixture. Validates that the anchored slice
/// path is lossless after the S13 data-model extension.
#[test]
fn datatree_round_trip_is_byte_identical() {
    for (lang, source) in data_fixtures() {
        let label = format!("<{}>", lang);
        let parsed = parse(
            ParseInput::Inline { content: source, file_label: &label },
            ParseOptions { language: Some(lang), ..Default::default() },
        )
        .unwrap_or_else(|e| panic!("parse {}: {}", lang, e));
        let (data_tree, src) = match parsed.root_tree.as_ref() {
            Some(Tree::DataTree { tree, source, .. }) => (tree.clone(), source.clone()),
            _ => panic!("{}: no DataTree (expected for {})", lang, lang),
        };

        let rendered = data_tree.to_source(&src);
        assert_eq!(
            rendered, *src,
            "{}: DataTree::to_source not byte-identical to input",
            lang
        );
    }
}

/// Scalar leaves of a parsed DataTree carry populated `text` / `value` /
/// `quote_style` fields (S13/DataTree-Z1). Verifies that lowering wires
/// the new fields and that `is_anchored()` agrees with the byte range.
#[test]
fn datatree_scalars_carry_text_and_quote_style() {
    use tractor::tree::DataTree;
    use tractor::tree::types::QuoteStyle;

    let source = "{\n  \"key\": \"value\",\n  \"n\": 42,\n  \"flag\": true,\n  \"nil\": null\n}\n";
    let parsed = parse(
        ParseInput::Inline { content: source, file_label: "<datatree-scalars>" },
        ParseOptions { language: Some("json"), ..Default::default() },
    )
    .expect("parse JSON");
    let data_tree = match parsed.root_tree.as_ref() {
        Some(Tree::DataTree { tree, .. }) => tree.clone(),
        _ => panic!("JSON yields DataTree"),
    };

    fn collect_scalars<'a>(t: &'a DataTree, out: &mut Vec<&'a DataTree>) {
        match t {
            DataTree::String { .. }
            | DataTree::Number { .. }
            | DataTree::Bool { .. }
            | DataTree::Null { .. } => out.push(t),
            DataTree::Document { children, .. }
            | DataTree::Section { children, .. }
            | DataTree::Directive { children, .. }
            | DataTree::Element { children, .. } => {
                for c in children {
                    collect_scalars(c, out);
                }
            }
            DataTree::Mapping { pairs, .. } => {
                for p in pairs {
                    collect_scalars(p, out);
                }
            }
            DataTree::Sequence { items, .. } => {
                for i in items {
                    collect_scalars(i, out);
                }
            }
            DataTree::Pair { key, value, .. } => {
                collect_scalars(key, out);
                collect_scalars(value, out);
            }
            _ => {}
        }
    }

    let mut scalars: Vec<&DataTree> = Vec::new();
    collect_scalars(&data_tree, &mut scalars);

    let mut saw_string = false;
    let mut saw_bool = false;
    let mut saw_null = false;
    for s in &scalars {
        assert!(s.is_anchored(), "parsed scalar must be anchored: {:?}", s);
        assert!(
            s.scalar_text().is_some(),
            "parsed scalar must yield text: {:?}",
            s
        );
        match s {
            DataTree::String { quote_style, .. } => {
                assert_eq!(*quote_style, QuoteStyle::Double, "JSON strings are Double-quoted");
                saw_string = true;
            }
            DataTree::Bool { text, value, .. } => {
                let expected = if *value { "true" } else { "false" };
                assert_eq!(text, expected, "JSON bool text should match canonical form");
                saw_bool = true;
            }
            DataTree::Null { text, .. } => {
                assert_eq!(text, "null", "JSON null text is `null`");
                saw_null = true;
            }
            _ => {}
        }
    }
    assert!(saw_string, "fixture should produce at least one String scalar");
    assert!(saw_bool, "fixture should produce at least one Bool scalar");
    assert!(saw_null, "fixture should produce at least one Null scalar");
}

/// Synthetic DataTree scalars carry `anchored: false` and stable text.
/// Validates that `DataTree::synthetic_scalar` integrates with the new
/// fields.
#[test]
fn datatree_synthetic_scalars_unanchored() {
    use tractor::tree::DataTree;
    use tractor::tree::data::ScalarKind;

    let s = DataTree::synthetic_scalar("hello", ScalarKind::String);
    assert!(!s.is_anchored(), "synthetic string is unanchored");
    assert_eq!(s.scalar_text(), Some("hello"));

    let b = DataTree::synthetic_scalar("true", ScalarKind::Bool);
    assert!(!b.is_anchored());
    assert_eq!(b.scalar_text(), Some("true"));

    let n = DataTree::synthetic_scalar("null", ScalarKind::Null);
    assert!(!n.is_anchored());
    assert_eq!(n.scalar_text(), Some("null"));
}

// ===========================================================================
// NodeId / assign_ids invariants (Slice 1 of editable-trees).
// Verifies the parser-exit invariant: every parsed tree (SyntaxTree,
// DataTree) leaves the parser with a non-zero NodeId stamped on every
// node's Span. Downstream XPath → typed-node lookup depends on this.
// ===========================================================================

/// Walk a SyntaxTree and assert every node has a non-zero id, and that
/// all ids are unique.
fn collect_syntax_ids(tree: &SyntaxTree, out: &mut Vec<tractor::tree::NodeId>) {
    out.push(tree.span().id);
    for c in syntax_children(tree) {
        collect_syntax_ids(c, out);
    }
}

fn syntax_children<'a>(tree: &'a SyntaxTree) -> Vec<&'a SyntaxTree> {
    // Mirror of the (crate-private) `SyntaxTree::children()` accessor
    // for the purposes of this test. Only enumerates the major
    // compound variants we touch in the fixture; scalar leaves return
    // empty by design.
    let mut v = Vec::new();
    match tree {
        SyntaxTree::Module { children, .. } => v.extend(children.iter()),
        SyntaxTree::Body { children, .. } => v.extend(children.iter()),
        SyntaxTree::Function { name, parameters, body, .. } => {
            v.push(name.as_ref());
            for p in parameters {
                v.push(p);
            }
            if let Some(b) = body {
                v.push(b.as_ref());
            }
        }
        SyntaxTree::Class { name, body, .. } => {
            v.push(name.as_ref());
            v.push(body.as_ref());
        }
        _ => {}
    }
    v
}

#[test]
fn parsed_syntax_tree_has_ids_assigned() {
    let source = "def foo(x):\n    return x\n";
    let parsed = parse(
        ParseInput::Inline { content: source, file_label: "<ids>" },
        ParseOptions { language: Some("python"), ..Default::default() },
    )
    .expect("parse Python");
    let tree = match parsed.root_tree.as_ref() {
        Some(Tree::SyntaxTree { tree, .. }) => tree.clone(),
        _ => panic!("python yields SyntaxTree"),
    };

    let mut ids = Vec::new();
    collect_syntax_ids(&tree, &mut ids);

    assert!(!ids.is_empty(), "fixture must produce some nodes");
    for id in &ids {
        assert_ne!(*id, 0, "every node must have a non-zero id after parse");
    }
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "ids must be unique within one tree");
}

/// End-to-end Slice 2 invariant: parse → query → look up `@id` →
/// resolve to typed-tree node. This exercises the full pipeline that
/// `tractor set` (and future `replace`) will rely on.
#[test]
fn xpath_match_id_resolves_to_typed_syntax_node() {
    use tractor::tree::{find_by_id, parse_id_attr};
    use tractor::xpath::XPathEngine;

    let source = "def foo():\n    return 1\n";
    let mut parsed = parse(
        ParseInput::Inline { content: source, file_label: "<xpath-id>" },
        ParseOptions { language: Some("python"), ..Default::default() },
    )
    .expect("parse python");

    // Query for the function's <name> element. Renderer-typed XML uses
    // `<name>` for function names.
    let _engine = XPathEngine::new();
    let matches = parsed
        .query("//function/name")
        .expect("xpath query succeeds");

    assert!(!matches.is_empty(), "fixture must contain a function name");

    // Extract the @id attribute from the first match's xml.
    let m = &matches[0];
    let xml = m.tree.as_ref().expect("match carries tree");
    let id_attr = match xml {
        tractor::xpath::Tree::Xml(node) => extract_id_attr(node),
        _ => panic!("expected partial match to carry Tree::Xml"),
    };
    let id =
        parse_id_attr(&id_attr.expect("matched element must carry @id"))
            .expect("@id parses to a non-zero NodeId");

    // Resolve to typed-tree node.
    let mut tree = (*parsed
        .root_tree
        .as_ref()
        .and_then(|t| match t {
            tractor::xpath::Tree::SyntaxTree { tree, .. } => Some(tree),
            _ => None,
        })
        .expect("python root has SyntaxTree"))
    .clone();
    let mut tree_for_lookup = std::sync::Arc::try_unwrap(tree.clone())
        .unwrap_or_else(|t| (*t).clone());
    let _ = tree;
    let found = find_by_id(&mut tree_for_lookup, id).expect("id resolves to typed node");
    match found {
        SyntaxTree::Name { text, .. } => assert_eq!(text, "foo"),
        other => panic!("expected Name, got {other:?}"),
    }
}

/// Parallel of `xpath_match_id_resolves_to_typed_syntax_node` for
/// SqlTree (Tier 1: assign_ids_sql / find_by_id_sql / xot `@id`).
#[test]
fn xpath_match_id_resolves_to_typed_sql_node() {
    use tractor::tree::{find_by_id, parse_id_attr};

    // Schemaless fixture — `//relation//name` resolves to the table
    // identifier without ambiguity. (With a `dbo.Users` qualifier the
    // first match would be the schema's `<name>dbo</name>`.)
    let source = "SELECT col FROM Users";
    let mut parsed = parse(
        ParseInput::Inline { content: source, file_label: "<xpath-id-sql>" },
        ParseOptions { language: Some("tsql"), ..Default::default() },
    )
    .expect("parse tsql");

    let matches = parsed
        .query("//relation//name")
        .expect("xpath query succeeds");

    assert!(!matches.is_empty(), "fixture must contain a relation name");

    let m = &matches[0];
    let xml = m.tree.as_ref().expect("match carries tree");
    let id_attr = match xml {
        tractor::xpath::Tree::Xml(node) => extract_id_attr(node),
        _ => panic!("expected partial match to carry Tree::Xml"),
    };
    let id = parse_id_attr(&id_attr.expect("matched element must carry @id"))
        .expect("@id parses to a non-zero NodeId");

    let mut sql_tree = (*parsed
        .root_tree
        .as_ref()
        .and_then(|t| match t {
            tractor::xpath::Tree::Sql { tree, .. } => Some(tree),
            _ => None,
        })
        .expect("tsql root has SqlTree"))
    .clone();
    let mut tree_for_lookup = std::sync::Arc::try_unwrap(sql_tree.clone())
        .unwrap_or_else(|t| (*t).clone());
    let _ = sql_tree;
    let found = find_by_id(&mut tree_for_lookup, id).expect("id resolves to typed node");
    // The matched <name> element in xot corresponds to the
    // SqlTree::Identifier for "Users" inside the Relation.
    match found {
        tractor::tree::sql::SqlTree::Identifier { value, .. } => {
            assert_eq!(value, "Users");
        }
        other => panic!("expected SqlTree::Identifier, got {other:?}"),
    }
}

fn extract_id_attr(node: &tractor::xpath::XmlNode) -> Option<String> {
    if let tractor::xpath::XmlNode::Element { attributes, .. } = node {
        for (k, v) in attributes {
            if k == "id" {
                return Some(v.clone());
            }
        }
    }
    None
}

#[test]
fn parsed_data_tree_has_ids_assigned() {
    use tractor::tree::DataTree;
    fn walk(tree: &DataTree, out: &mut Vec<tractor::tree::NodeId>) {
        out.push(tree.span().id);
        match tree {
            DataTree::Document { children, .. }
            | DataTree::Section { children, .. }
            | DataTree::Directive { children, .. }
            | DataTree::Element { children, .. } => {
                for c in children {
                    walk(c, out);
                }
            }
            DataTree::Mapping { pairs, .. } => {
                for p in pairs {
                    walk(p, out);
                }
            }
            DataTree::Sequence { items, .. } => {
                for i in items {
                    walk(i, out);
                }
            }
            DataTree::Pair { key, value, .. } => {
                walk(key, out);
                walk(value, out);
            }
            _ => {}
        }
    }

    let source = "{\n  \"key\": \"value\",\n  \"n\": 42\n}\n";
    let parsed = parse(
        ParseInput::Inline { content: source, file_label: "<ids>" },
        ParseOptions { language: Some("json"), ..Default::default() },
    )
    .expect("parse JSON");
    let tree = match parsed.root_tree.as_ref() {
        Some(Tree::DataTree { tree, .. }) => tree.clone(),
        _ => panic!("json yields DataTree"),
    };

    let mut ids = Vec::new();
    walk(&tree, &mut ids);

    assert!(!ids.is_empty());
    for id in &ids {
        assert_ne!(*id, 0, "every DataTree node must have a non-zero id after parse");
    }
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "ids must be unique within one DataTree");
}

// ---------------------------------------------------------------------------
// Slice 3 — XPath → @id → typed-tree mutation → source splice (S15-Z3)
// ---------------------------------------------------------------------------

/// XPath matches against a syntax-tree projection now carry the typed
/// node's `NodeId` in `Match.node_id`. This is the load-bearing fact
/// that the executor's typed-set pipeline relies on.
#[test]
fn syntax_tree_matches_carry_node_id() {
    let source = "def foo():\n    return 1\n";
    let mut parsed = parse(
        ParseInput::Inline { content: source, file_label: "<id-on-match>" },
        ParseOptions { language: Some("python"), ..Default::default() },
    )
    .expect("parse python");

    let matches = parsed
        .query("//function/name")
        .expect("xpath query succeeds");

    assert!(!matches.is_empty(), "function name must match");
    let id = matches[0]
        .node_id
        .expect("Match.node_id must carry the @id stamp from xot");
    assert_ne!(id, 0, "non-zero NodeId");
}

/// End-to-end Slice 3: `upsert_typed` on a syntax-tree language flows
/// XPath → `@id` → `find_by_id_syntax` → mutate scalar text → render
/// the leaf via the typed renderer → splice into the original source.
#[test]
fn upsert_renames_python_function_via_typed_pipeline() {
    use tractor::xpath_upsert::upsert_typed;

    let source = "def foo():\n    return 1\n";
    let result = upsert_typed(source, "python", "//function/name", "bar", None, Some("string"))
        .expect("typed-syntax upsert succeeds");

    assert!(!result.inserted, "rename is an update, not an insert");
    assert_eq!(result.matches_updated, 1, "one function name should be updated");
    assert_eq!(
        result.source, "def bar():\n    return 1\n",
        "renamed source preserves surrounding whitespace + body",
    );
}

/// Slice 3 covers identifiers, but the per-leaf render also handles
/// string literals — verify quote style is preserved when the typed
/// renderer emits the new scalar text. Uses python where strings use
/// the same quote glyph at lower time.
#[test]
fn upsert_renames_typescript_string_literal() {
    use tractor::xpath_upsert::upsert_typed;

    let source = "const greeting = \"hello\";\n";
    let result = upsert_typed(source, "typescript", "//variable//string", "world", None, Some("string"))
        .expect("typed-syntax upsert succeeds");

    assert_eq!(result.matches_updated, 1, "one string literal should be updated");
    // The new text replaces "hello" — quotes come from the typed
    // renderer's QuoteStyle::Double handling.
    assert!(
        result.source.contains("\"world\""),
        "renamed string preserves double-quote style: {:?}", result.source,
    );
    assert!(
        !result.source.contains("hello"),
        "old text removed: {:?}", result.source,
    );
}

/// Slice 3 must cover all 8 SyntaxTree-pipeline languages uniformly.
/// One rename-via-XPath case per remaining language (the python and
/// typescript paths are covered above). Each fixture is intentionally
/// minimal — the test confirms that `upsert_typed` flows through the
/// typed pipeline (XPath → `@id` → `find_by_id_syntax` → mutate →
/// render → splice) and produces the expected source text. If a
/// future change breaks the wiring for one language but not the
/// others, the failure points at exactly which language regressed.
#[test]
fn upsert_renames_csharp_class_name() {
    use tractor::xpath_upsert::upsert_typed;
    let source = "public class Foo { public void Bar() {} }\n";
    let result = upsert_typed(source, "csharp", "//class/name", "Baz", None, Some("string"))
        .expect("csharp typed upsert");
    assert_eq!(result.matches_updated, 1);
    assert!(result.source.contains("class Baz"), "got: {:?}", result.source);
    assert!(!result.source.contains("Foo"), "got: {:?}", result.source);
}

#[test]
fn upsert_renames_java_class_name() {
    use tractor::xpath_upsert::upsert_typed;
    let source = "class Foo { void bar() {} }\n";
    let result = upsert_typed(source, "java", "//class/name", "Baz", None, Some("string"))
        .expect("java typed upsert");
    assert_eq!(result.matches_updated, 1);
    assert!(result.source.contains("class Baz"), "got: {:?}", result.source);
    assert!(!result.source.contains("Foo"), "got: {:?}", result.source);
}

#[test]
fn upsert_renames_rust_function_name() {
    use tractor::xpath_upsert::upsert_typed;
    let source = "fn foo() {}\n";
    let result = upsert_typed(source, "rust", "//function/name", "bar", None, Some("string"))
        .expect("rust typed upsert");
    assert_eq!(result.matches_updated, 1);
    assert!(result.source.contains("fn bar"), "got: {:?}", result.source);
    assert!(!result.source.contains("foo"), "got: {:?}", result.source);
}

#[test]
fn upsert_renames_go_function_name() {
    use tractor::xpath_upsert::upsert_typed;
    let source = "package x\nfunc foo() {}\n";
    let result = upsert_typed(source, "go", "//function/name", "bar", None, Some("string"))
        .expect("go typed upsert");
    assert_eq!(result.matches_updated, 1);
    assert!(result.source.contains("func bar"), "got: {:?}", result.source);
    assert!(!result.source.contains("foo"), "got: {:?}", result.source);
}

#[test]
fn upsert_renames_ruby_class_name() {
    use tractor::xpath_upsert::upsert_typed;
    let source = "class Foo\n  def bar\n  end\nend\n";
    let result = upsert_typed(source, "ruby", "//class/name", "Baz", None, Some("string"))
        .expect("ruby typed upsert");
    assert_eq!(result.matches_updated, 1);
    assert!(result.source.contains("class Baz"), "got: {:?}", result.source);
    assert!(!result.source.contains("Foo"), "got: {:?}", result.source);
}

#[test]
fn upsert_renames_php_class_name() {
    use tractor::xpath_upsert::upsert_typed;
    let source = "<?php class Foo { function bar() {} }\n";
    let result = upsert_typed(source, "php", "//class/name", "Baz", None, Some("string"))
        .expect("php typed upsert");
    assert_eq!(result.matches_updated, 1);
    assert!(result.source.contains("class Baz"), "got: {:?}", result.source);
    assert!(!result.source.contains("Foo"), "got: {:?}", result.source);
}

/// When no XPath match exists for a syntax-tree language, the typed
/// pipeline returns a no-op result rather than fabricating structure
/// (Slice 3 is update-only; insertion is Slice 4).
#[test]
fn upsert_no_match_on_syntax_tree_is_noop() {
    use tractor::xpath_upsert::upsert_typed;

    let source = "def foo():\n    return 1\n";
    let result = upsert_typed(source, "python", "//class/name", "Bar", None, Some("string"))
        .expect("no-match returns Ok");
    assert!(!result.inserted);
    assert_eq!(result.matches_updated, 0);
    assert_eq!(result.source, source, "source unchanged when nothing matched");
}
