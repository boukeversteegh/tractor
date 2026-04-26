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
    Lang {
        id: "csharp",
        fixture: "blueprint.cs",
        fixture_dir: "csharp",
        catalogue_path: "tractor/src/languages/csharp/semantic.rs",
        kinds: tractor::languages::csharp::semantic::KINDS,
        nodes: tractor::languages::csharp::semantic::NODES,
    },
    Lang {
        id: "java",
        fixture: "blueprint.java",
        fixture_dir: "java",
        catalogue_path: "tractor/src/languages/java/semantic.rs",
        kinds: tractor::languages::java::semantic::KINDS,
        nodes: tractor::languages::java::semantic::NODES,
    },
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
        id: "python",
        fixture: "blueprint.py",
        fixture_dir: "python",
        catalogue_path: "tractor/src/languages/python/semantic.rs",
        kinds: tractor::languages::python::semantic::KINDS,
        nodes: tractor::languages::python::semantic::NODES,
    },
    Lang {
        id: "go",
        fixture: "blueprint.go",
        fixture_dir: "go",
        catalogue_path: "tractor/src/languages/go/semantic.rs",
        kinds: tractor::languages::go::semantic::KINDS,
        nodes: tractor::languages::go::semantic::NODES,
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
        id: "php",
        fixture: "blueprint.php",
        fixture_dir: "php",
        catalogue_path: "tractor/src/languages/php/semantic.rs",
        kinds: tractor::languages::php::semantic::KINDS,
        nodes: tractor::languages::php::semantic::NODES,
    },
    Lang {
        // No blueprint.sql exists yet — sample.sql is the largest fixture.
        id: "tsql",
        fixture: "sample.sql",
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
fn csharp_catalogue_covers_blueprint() {
    check_lang(&LANGUAGES[0]);
}

#[test]
fn java_catalogue_covers_blueprint() {
    check_lang(&LANGUAGES[1]);
}

#[test]
fn rust_catalogue_covers_blueprint() {
    check_lang(&LANGUAGES[2]);
}

#[test]
fn typescript_catalogue_covers_blueprint() {
    check_lang(&LANGUAGES[3]);
}

#[test]
fn python_catalogue_covers_blueprint() {
    check_lang(&LANGUAGES[4]);
}

#[test]
fn go_catalogue_covers_blueprint() {
    check_lang(&LANGUAGES[5]);
}

#[test]
fn ruby_catalogue_covers_blueprint() {
    check_lang(&LANGUAGES[6]);
}

#[test]
fn php_catalogue_covers_blueprint() {
    check_lang(&LANGUAGES[7]);
}

#[test]
fn tsql_catalogue_covers_blueprint() {
    check_lang(&LANGUAGES[8]);
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

/// Semantic node names should be unique and each node must have at
/// least one role. This belongs with the catalogue checks rather
/// than inside a language transform module.
#[test]
fn node_metadata_is_well_formed() {
    for lang in LANGUAGES {
        check_node_names(lang);
    }
}
