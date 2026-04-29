//! Tree-sitter kind catalogue lint.
//!
//! For each programming language, parse its blueprint (or sample)
//! fixture with the raw tree-sitter grammar and assert that every
//! distinct named-node kind appears in the language's `KINDS`
//! catalogue (declared in `<lang>/semantic.rs`).
//!
//! When this test fails it means tree-sitter is emitting a kind the
//! transform doesn't yet know about. Add a `KindEntry` to the named
//! catalogue (file path is included in the failure message).

use std::collections::BTreeSet;
use std::path::PathBuf;

use tractor::languages::{KindEntry, KindHandling, NodeSpec};
use tractor::raw_kinds;

struct Lang {
    /// Language ID passed to `raw_kinds` (matches the dispatch table).
    id: &'static str,
    /// Filename relative to `tests/integration/languages/<id>/`.
    fixture: &'static str,
    /// Subdirectory under `tests/integration/languages/` (usually
    /// matches `id` but a few languages use a different folder name).
    fixture_dir: &'static str,
    /// Pretty path to the per-language catalogue file, used in the
    /// failure message so the engineer knows where to add the entry.
    catalogue_path: &'static str,
    /// The catalogue itself.
    kinds: &'static [KindEntry],
    /// Semantic node metadata emitted by the language transform.
    nodes: &'static [NodeSpec],
}

const LANGUAGES: &[Lang] = &[
    // C#, Go, Java, PHP, and Python have migrated to the typed-enum + rule()
    // shape — no `KindEntry` catalogue. Their blueprint coverage is checked by
    // `<lang>_catalogue_covers_blueprint` below using `<Lang>Kind::from_str`.
    Lang {
        id: "rust",
        fixture: "blueprint.rs",
        fixture_dir: "rust",
        catalogue_path: "tractor/src/languages/rust_lang/semantic.rs",
        kinds: tractor::languages::rust_lang::semantic::KINDS,
        nodes: tractor::languages::rust_lang::semantic::NODES,
    },
    Lang {
        id: "typescript",
        fixture: "blueprint.ts",
        fixture_dir: "typescript",
        catalogue_path: "tractor/src/languages/typescript/semantic.rs",
        kinds: tractor::languages::typescript::semantic::KINDS,
        nodes: tractor::languages::typescript::semantic::NODES,
    },
    Lang {
        id: "ruby",
        fixture: "blueprint.rb",
        fixture_dir: "ruby",
        catalogue_path: "tractor/src/languages/ruby/semantic.rs",
        kinds: tractor::languages::ruby::semantic::KINDS,
        nodes: tractor::languages::ruby::semantic::NODES,
    },
    Lang {
        id: "tsql",
        fixture: "blueprint.sql",
        fixture_dir: "tsql",
        catalogue_path: "tractor/src/languages/tsql/semantic.rs",
        kinds: tractor::languages::tsql::semantic::KINDS,
        nodes: tractor::languages::tsql::semantic::NODES,
    },
];

fn fixture_path(dir: &str, file: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR points at `tractor/tractor/`; the integration
    // fixtures live in `tractor/tests/integration/languages/`.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/integration/languages")
        .join(dir)
        .join(file)
}

fn check_lang(lang: &Lang) {
    let path = fixture_path(lang.fixture_dir, lang.fixture);
    let source = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("failed to read fixture {}: {}", path.display(), e)
    });

    let mut catalogue: BTreeSet<&str> = BTreeSet::new();
    for entry in lang.kinds {
        if !catalogue.insert(entry.kind) {
            panic!(
                "duplicate `{}` entry in {} — every kind appears at most once",
                entry.kind, lang.catalogue_path
            );
        }
    }

    let kinds = raw_kinds(lang.id, &source).expect("raw_kinds");
    let mut missing: Vec<String> = Vec::new();
    for k in &kinds {
        if !catalogue.contains(k.as_str()) {
            missing.push(k.clone());
        }
    }

    assert!(
        missing.is_empty(),
        "tree-sitter {} emitted {} kind(s) not in the catalogue:\n{}\n\n\
         Add a `KindEntry` for each one to {}.",
        lang.id,
        missing.len(),
        missing.iter().map(|k| format!("  - {}", k)).collect::<Vec<_>>().join("\n"),
        lang.catalogue_path,
    );
}

fn check_node_names(lang: &Lang) {
    let mut names: Vec<&str> = lang.nodes.iter().map(|n| n.name).collect();
    names.sort();
    let total = names.len();
    names.dedup();
    assert_eq!(
        names.len(),
        total,
        "{} contains duplicate node names",
        lang.catalogue_path
    );

    for node in lang.nodes {
        assert!(
            node.marker || node.container,
            "{}: <{}> is neither marker nor container",
            lang.catalogue_path,
            node.name
        );
    }
}

#[test]
fn rust_catalogue_covers_blueprint() {
    check_lang(&LANGUAGES[0]);
}

#[test]
fn typescript_catalogue_covers_blueprint() {
    check_lang(&LANGUAGES[1]);
}

/// Go-specific blueprint coverage check. Go has migrated to the
/// typed-enum + rule() dispatcher, so coverage is asserted via
/// `GoKind::from_str` rather than against a `KINDS` table.
///
/// In the new shape, kind drift is caught at compile time (the
/// exhaustive `rule(GoKind) -> Rule` match fails to build when
/// `kind.rs` is regenerated with new variants). This runtime check
/// adds the inverse guard: every kind tree-sitter actually emits when
/// parsing the blueprint must be a known `GoKind` variant — i.e.
/// `input.rs` is up to date with the grammar.
#[test]
fn go_catalogue_covers_blueprint() {
    use tractor::languages::go::input::GoKind;
    use tractor::raw_kinds;

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
        missing
            .iter()
            .map(|k| format!("  - {}", k))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

#[test]
fn go_node_metadata_is_well_formed() {
    use tractor::languages::go::output::NODES;
    let mut names: Vec<&str> = NODES.iter().map(|n| n.name).collect();
    names.sort();
    let total = names.len();
    names.dedup();
    assert_eq!(
        names.len(),
        total,
        "tractor/src/languages/go/output.rs contains duplicate node names"
    );
    for node in NODES {
        assert!(
            node.marker || node.container,
            "tractor/src/languages/go/output.rs: <{}> is neither marker nor container",
            node.name
        );
    }
}

#[test]
fn ruby_catalogue_covers_blueprint() {
    check_lang(&LANGUAGES[2]);
}

#[test]
fn tsql_catalogue_covers_blueprint() {
    check_lang(&LANGUAGES[3]);
}

/// Sanity check that every catalogue entry's `Rename` / `RenameWithMarker`
/// target is non-empty. Cheap, language-independent guardrail.
#[test]
fn rename_targets_are_non_empty() {
    for lang in LANGUAGES {
        for entry in lang.kinds {
            match entry.handling {
                KindHandling::Rename(s) | KindHandling::CustomThenRename(s) => {
                    assert!(
                        !s.is_empty(),
                        "{}: empty rename target for kind `{}`",
                        lang.id, entry.kind
                    );
                }
                KindHandling::RenameWithMarker(s, m)
                | KindHandling::CustomThenRenameWithMarker(s, m) => {
                    assert!(
                        !s.is_empty() && !m.is_empty(),
                        "{}: empty rename/marker for kind `{}`",
                        lang.id, entry.kind
                    );
                }
                _ => {}
            }
        }
    }
}

// Removed: `go_catalogue_entries_are_real_grammar_kinds`. That test
// validated the old `KINDS` array against `GoKind`; with `KINDS`
// gone, the equivalent guarantee comes from `rule(GoKind) -> Rule`
// being exhaustive over the typed enum (compile-time).

// Removed: `csharp_catalogue_entries_are_real_grammar_kinds`. That
// test validated the old `KINDS` array against `CsKind`; with
// `KINDS` gone, the equivalent guarantee comes from
// `rule(CsKind) -> Rule` being exhaustive over the typed enum
// (compile-time).

/// Python-specific blueprint coverage check. Python has migrated to
/// the typed-enum + rule() dispatcher; coverage is asserted via
/// `PyKind::from_str` rather than against a `KINDS` table.
#[test]
fn python_catalogue_covers_blueprint() {
    use tractor::languages::python::input::PyKind;
    use tractor::raw_kinds;

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
        missing
            .iter()
            .map(|k| format!("  - {}", k))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

#[test]
fn python_node_metadata_is_well_formed() {
    use tractor::languages::python::semantic::NODES;
    let mut names: Vec<&str> = NODES.iter().map(|n| n.name).collect();
    names.sort();
    let total = names.len();
    names.dedup();
    assert_eq!(
        names.len(),
        total,
        "tractor/src/languages/python/semantic.rs contains duplicate node names"
    );
    for node in NODES {
        assert!(
            node.marker || node.container,
            "tractor/src/languages/python/semantic.rs: <{}> is neither marker nor container",
            node.name
        );
    }
}

/// PHP-specific blueprint coverage check. PHP has migrated to the
/// typed-enum + rule() dispatcher, so coverage is asserted via
/// `PhpKind::from_str` rather than against a `KINDS` table.
#[test]
fn php_catalogue_covers_blueprint() {
    use tractor::languages::php::input::PhpKind;
    use tractor::raw_kinds;

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
        missing
            .iter()
            .map(|k| format!("  - {}", k))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

#[test]
fn php_node_metadata_is_well_formed() {
    use tractor::languages::php::output::NODES;
    let mut names: Vec<&str> = NODES.iter().map(|n| n.name).collect();
    names.sort();
    let total = names.len();
    names.dedup();
    assert_eq!(
        names.len(),
        total,
        "tractor/src/languages/php/output.rs contains duplicate node names"
    );
    for node in NODES {
        assert!(
            node.marker || node.container,
            "tractor/src/languages/php/output.rs: <{}> is neither marker nor container",
            node.name
        );
    }
}

/// Java-specific blueprint coverage check. Java has migrated to the
/// typed-enum + rule() dispatcher, so coverage is asserted via
/// `JavaKind::from_str` rather than against a `KINDS` table.
///
/// In the new shape, kind drift is caught at compile time (the
/// exhaustive `rule(JavaKind) -> Rule` match fails to build when
/// `input.rs` is regenerated with new variants). This runtime check
/// adds the inverse guard: every kind tree-sitter actually emits when
/// parsing the blueprint must be a known `JavaKind` variant — i.e.
/// `input.rs` is up to date with the grammar.
#[test]
fn java_catalogue_covers_blueprint() {
    use tractor::languages::java::input::JavaKind;
    use tractor::raw_kinds;

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
        missing
            .iter()
            .map(|k| format!("  - {}", k))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

#[test]
fn java_node_metadata_is_well_formed() {
    use tractor::languages::java::output::NODES;
    let mut names: Vec<&str> = NODES.iter().map(|n| n.name).collect();
    names.sort();
    let total = names.len();
    names.dedup();
    assert_eq!(
        names.len(),
        total,
        "tractor/src/languages/java/output.rs contains duplicate node names"
    );
    for node in NODES {
        assert!(
            node.marker || node.container,
            "tractor/src/languages/java/output.rs: <{}> is neither marker nor container",
            node.name
        );
    }
}

/// C#-specific blueprint coverage check. C# has migrated to the
/// typed-enum + rule() dispatcher, so coverage is asserted via
/// `CsKind::from_str` rather than against a `KINDS` table.
///
/// In the new shape, kind drift is caught at compile time (the
/// exhaustive `rule(CsKind) -> Rule` match fails to build when
/// `input.rs` is regenerated with new variants). This runtime check
/// adds the inverse guard: every kind tree-sitter actually emits when
/// parsing the blueprint must be a known `CsKind` variant — i.e.
/// `input.rs` is up to date with the grammar.
#[test]
fn csharp_catalogue_covers_blueprint() {
    use tractor::languages::csharp::input::CsKind;
    use tractor::raw_kinds;

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
        missing
            .iter()
            .map(|k| format!("  - {}", k))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

#[test]
fn csharp_node_metadata_is_well_formed() {
    use tractor::languages::csharp::output::NODES;
    let mut names: Vec<&str> = NODES.iter().map(|n| n.name).collect();
    names.sort();
    let total = names.len();
    names.dedup();
    assert_eq!(
        names.len(),
        total,
        "tractor/src/languages/csharp/output.rs contains duplicate node names"
    );
    for node in NODES {
        assert!(
            node.marker || node.container,
            "tractor/src/languages/csharp/output.rs: <{}> is neither marker nor container",
            node.name
        );
    }
}

/// Semantic node names should be unique and each node must have at
/// least one role. This belongs with the catalogue checks rather
/// than inside a language transform module.
#[test]
fn node_metadata_is_well_formed() {
    for lang in LANGUAGES {
        check_node_names(lang);
    }
}
