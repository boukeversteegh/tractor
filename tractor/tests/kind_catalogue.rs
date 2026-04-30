//! Per-language blueprint coverage checks.
//!
//! For each language, parse its blueprint fixture with the raw
//! tree-sitter grammar and assert that every distinct named-node
//! kind tree-sitter emits is a known `<Lang>Kind` variant — i.e.
//! `input.rs` is up to date with the grammar.
//!
//! Compile-time exhaustiveness is enforced by the `rule(<Lang>Kind)
//! -> Rule` match in each language's `rules.rs` (a new grammar kind
//! makes the lib fail to build until classified). This runtime check
//! adds the inverse guard: every kind the grammar actually emits is
//! known to our enum — catches grammar drift on regenerate.

use std::path::PathBuf;

use tractor::raw_kinds;

fn fixture_path(dir: &str, file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/integration/languages")
        .join(dir)
        .join(file)
}

#[test]
fn go_catalogue_covers_blueprint() {
    use tractor::languages::go::input::GoKind;

    let path = fixture_path("go", "blueprint.go");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {}", path.display(), e));

    let kinds = raw_kinds("go", &source).expect("raw_kinds");
    let mut missing: Vec<String> = Vec::new();
    for k in &kinds {
        if GoKind::from_str(k).is_none() {
            missing.push(k.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "tree-sitter go emitted {} kind(s) not in `GoKind`:\n{}\n\n\
         Regenerate `tractor/src/languages/go/input.rs` via \
         `task gen:kinds` so the typed enum reflects the current grammar.",
        missing.len(),
        missing.iter().map(|k| format!("  - {}", k)).collect::<Vec<_>>().join("\n"),
    );
}

#[test]
fn go_node_metadata_is_well_formed() {
    use tractor::languages::go::output;
    check_node_metadata(output::nodes(), "tractor/src/languages/go/output.rs");
}

#[test]
fn csharp_catalogue_covers_blueprint() {
    use tractor::languages::csharp::input::CsKind;

    let path = fixture_path("csharp", "blueprint.cs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {}", path.display(), e));

    let kinds = raw_kinds("csharp", &source).expect("raw_kinds");
    let mut missing: Vec<String> = Vec::new();
    for k in &kinds {
        if CsKind::from_str(k).is_none() {
            missing.push(k.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "tree-sitter csharp emitted {} kind(s) not in `CsKind`:\n{}\n\n\
         Regenerate `tractor/src/languages/csharp/input.rs` via \
         `task gen:kinds` so the typed enum reflects the current grammar.",
        missing.len(),
        missing.iter().map(|k| format!("  - {}", k)).collect::<Vec<_>>().join("\n"),
    );
}

#[test]
fn csharp_node_metadata_is_well_formed() {
    use tractor::languages::csharp::output;
    check_node_metadata(output::nodes(), "tractor/src/languages/csharp/output.rs");
}

#[test]
fn java_catalogue_covers_blueprint() {
    use tractor::languages::java::input::JavaKind;

    let path = fixture_path("java", "blueprint.java");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {}", path.display(), e));

    let kinds = raw_kinds("java", &source).expect("raw_kinds");
    let mut missing: Vec<String> = Vec::new();
    for k in &kinds {
        if JavaKind::from_str(k).is_none() {
            missing.push(k.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "tree-sitter java emitted {} kind(s) not in `JavaKind`:\n{}\n\n\
         Regenerate `tractor/src/languages/java/input.rs` via \
         `task gen:kinds` so the typed enum reflects the current grammar.",
        missing.len(),
        missing.iter().map(|k| format!("  - {}", k)).collect::<Vec<_>>().join("\n"),
    );
}

#[test]
fn java_node_metadata_is_well_formed() {
    use tractor::languages::java::output;
    check_node_metadata(output::nodes(), "tractor/src/languages/java/output.rs");
}

#[test]
fn php_catalogue_covers_blueprint() {
    use tractor::languages::php::input::PhpKind;

    let path = fixture_path("php", "blueprint.php");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {}", path.display(), e));

    let kinds = raw_kinds("php", &source).expect("raw_kinds");
    let mut missing: Vec<String> = Vec::new();
    for k in &kinds {
        if PhpKind::from_str(k).is_none() {
            missing.push(k.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "tree-sitter php emitted {} kind(s) not in `PhpKind`:\n{}\n\n\
         Regenerate `tractor/src/languages/php/input.rs` via \
         `task gen:kinds` so the typed enum reflects the current grammar.",
        missing.len(),
        missing.iter().map(|k| format!("  - {}", k)).collect::<Vec<_>>().join("\n"),
    );
}

#[test]
fn php_node_metadata_is_well_formed() {
    use tractor::languages::php::output;
    check_node_metadata(output::nodes(), "tractor/src/languages/php/output.rs");
}

#[test]
fn python_catalogue_covers_blueprint() {
    use tractor::languages::python::input::PyKind;

    let path = fixture_path("python", "blueprint.py");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {}", path.display(), e));

    let kinds = raw_kinds("python", &source).expect("raw_kinds");
    let mut missing: Vec<String> = Vec::new();
    for k in &kinds {
        if PyKind::from_str(k).is_none() {
            missing.push(k.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "tree-sitter python emitted {} kind(s) not in `PyKind`:\n{}\n\n\
         Regenerate `tractor/src/languages/python/input.rs` via \
         `task gen:kinds` so the typed enum reflects the current grammar.",
        missing.len(),
        missing.iter().map(|k| format!("  - {}", k)).collect::<Vec<_>>().join("\n"),
    );
}

#[test]
fn python_node_metadata_is_well_formed() {
    use tractor::languages::python::output;
    check_node_metadata(output::nodes(), "tractor/src/languages/python/output.rs");
}

#[test]
fn rust_catalogue_covers_blueprint() {
    use tractor::languages::rust_lang::input::RustKind;

    let path = fixture_path("rust", "blueprint.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {}", path.display(), e));

    let kinds = raw_kinds("rust", &source).expect("raw_kinds");
    let mut missing: Vec<String> = Vec::new();
    for k in &kinds {
        if RustKind::from_str(k).is_none() {
            missing.push(k.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "tree-sitter rust emitted {} kind(s) not in `RustKind`:\n{}\n\n\
         Regenerate `tractor/src/languages/rust_lang/input.rs` via \
         `task gen:kinds` so the typed enum reflects the current grammar.",
        missing.len(),
        missing.iter().map(|k| format!("  - {}", k)).collect::<Vec<_>>().join("\n"),
    );
}

#[test]
fn rust_node_metadata_is_well_formed() {
    use tractor::languages::rust_lang::output;
    check_node_metadata(output::nodes(), "tractor/src/languages/rust_lang/output.rs");
}

#[test]
fn typescript_catalogue_covers_blueprint() {
    use tractor::languages::typescript::input::TsKind;

    let path = fixture_path("typescript", "blueprint.ts");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {}", path.display(), e));

    let kinds = raw_kinds("typescript", &source).expect("raw_kinds");
    let mut missing: Vec<String> = Vec::new();
    for k in &kinds {
        if TsKind::from_str(k).is_none() {
            missing.push(k.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "tree-sitter typescript emitted {} kind(s) not in `TsKind`:\n{}\n\n\
         Regenerate `tractor/src/languages/typescript/input.rs` via \
         `task gen:kinds` so the typed enum reflects the current grammar.",
        missing.len(),
        missing.iter().map(|k| format!("  - {}", k)).collect::<Vec<_>>().join("\n"),
    );
}

#[test]
fn typescript_node_metadata_is_well_formed() {
    use tractor::languages::typescript::output;
    check_node_metadata(output::nodes(), "tractor/src/languages/typescript/output.rs");
}

#[test]
fn ruby_catalogue_covers_blueprint() {
    use tractor::languages::ruby::input::RubyKind;

    let path = fixture_path("ruby", "blueprint.rb");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {}", path.display(), e));

    let kinds = raw_kinds("ruby", &source).expect("raw_kinds");
    let mut missing: Vec<String> = Vec::new();
    for k in &kinds {
        if RubyKind::from_str(k).is_none() {
            missing.push(k.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "tree-sitter ruby emitted {} kind(s) not in `RubyKind`:\n{}\n\n\
         Regenerate `tractor/src/languages/ruby/input.rs` via \
         `task gen:kinds` so the typed enum reflects the current grammar.",
        missing.len(),
        missing.iter().map(|k| format!("  - {}", k)).collect::<Vec<_>>().join("\n"),
    );
}

#[test]
fn ruby_node_metadata_is_well_formed() {
    use tractor::languages::ruby::output;
    check_node_metadata(output::nodes(), "tractor/src/languages/ruby/output.rs");
}

#[test]
fn tsql_catalogue_covers_blueprint() {
    use tractor::languages::tsql::input::TsqlKind;

    let path = fixture_path("tsql", "blueprint.sql");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {}", path.display(), e));

    let kinds = raw_kinds("tsql", &source).expect("raw_kinds");
    let mut missing: Vec<String> = Vec::new();
    for k in &kinds {
        if TsqlKind::from_str(k).is_none() {
            missing.push(k.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "tree-sitter tsql emitted {} kind(s) not in `TsqlKind`:\n{}\n\n\
         Regenerate `tractor/src/languages/tsql/input.rs` via \
         `task gen:kinds` so the typed enum reflects the current grammar.",
        missing.len(),
        missing.iter().map(|k| format!("  - {}", k)).collect::<Vec<_>>().join("\n"),
    );
}

#[test]
fn tsql_node_metadata_is_well_formed() {
    use tractor::languages::tsql::output;
    check_node_metadata(output::nodes(), "tractor/src/languages/tsql/output.rs");
}

/// Shared NODES well-formedness check: names are unique, every node
/// is at least one of marker / container.
fn check_node_metadata(nodes: &[tractor::languages::NodeSpec], path: &str) {
    let mut names: Vec<&str> = nodes.iter().map(|n| n.name).collect();
    names.sort();
    let total = names.len();
    names.dedup();
    assert_eq!(
        names.len(),
        total,
        "{} contains duplicate node names",
        path
    );
    for node in nodes {
        assert!(
            node.marker || node.container,
            "{}: <{}> is neither marker nor container",
            path,
            node.name
        );
    }
}
