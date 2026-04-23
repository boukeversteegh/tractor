//! Generate/update or check XML snapshots for integration tests.
//!
//! Walks `tests/integration/*/`, finds source files by known extensions,
//! and runs tractor on each to produce `.xml` and `.raw.xml` snapshots.
//!
//! Also handles output-format combination snapshots in
//! `tests/integration/formats/snapshots/` and feature-invariant snapshots
//! listed in `FEATURE_FIXTURES` (each produces `.snapshot.xml` + `.snapshot.json`).
//!
//! Usage:
//!   cargo run --release --bin update-snapshots          # update snapshots
//!   cargo run --release --bin update-snapshots -- --check  # check only (no writes)

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::{self, Command};

/// File extensions to skip (not source fixtures).
/// `json` is skipped to avoid picking up generated `.snapshot.json` outputs.
const SKIP_EXTENSIONS: &[&str] = &["xml", "sh", "md", "json"];

/// Feature fixtures: minimal source files that demonstrate a single
/// transform invariant. Paired with an XPath that extracts the focused
/// subtree for visual reference. Regression protection lives in the
/// assertion suite (`tests/semantic_tree.rs`) — these fragment snapshots
/// are for reviewers who want to see the shape of each construct.
///
/// Output: `<source>.snapshot.txt` containing just the matched subtree.
///
/// Paths are relative to the project root.
/// Feature fixtures: minimal source files that demonstrate a single
/// transform invariant. Paired with an XPath that extracts the focused
/// subtree for visual reference, and an optional depth cap that hides
/// deep internals when the important information is the shape of the
/// direct children (useful for conditionals, where the else-if chain
/// is the point but the binary expressions inside each condition are
/// noise).
///
/// A depth of 0 means "no limit".
///
/// Regression protection lives in the assertion suite
/// (`tests/semantic_tree.rs`) — these fragment snapshots are for
/// reviewers who want to see the shape of each construct.
///
/// Output: `<source>.snapshot.txt` containing just the matched subtree.
///
/// Paths are relative to the project root.
const FEATURE_FIXTURES: &[(&str, &str, u32)] = &[
    // — TypeScript —
    // (source, xpath, depth cap — 0 = no limit; pick the smallest
    //  construct that demonstrates the invariant.)
    ("tests/integration/languages/typescript/accessors.ts", "//method[get]", 0),
    ("tests/integration/languages/typescript/async-generator.ts", "//function[async][generator]", 0),
    ("tests/integration/languages/typescript/conditionals.ts", "//if", 3),
    ("tests/integration/languages/typescript/flat-lists.ts", "//function[name='first']", 0),
    ("tests/integration/languages/typescript/parameter-marking.ts", "//function[name='call']", 0),
    ("tests/integration/languages/typescript/type-vocabulary.ts", "//class[name='Dog']", 0),

    // — Java —
    ("tests/integration/languages/java/conditionals.java", "//if", 3),
    ("tests/integration/languages/java/constructor-rename.java", "//constructor[1]", 0),
    ("tests/integration/languages/java/flat-lists.java", "//method[1]", 0),
    ("tests/integration/languages/java/interface-public.java", "//interface/body/method[public][1]", 0),
    ("tests/integration/languages/java/modifiers.java", "//field[private]", 0),
    ("tests/integration/languages/java/type-vocabulary.java", "//class[name='Dog']", 3),

    // — C# —
    ("tests/integration/languages/csharp/accessor-flattening.cs", "//property[name='Manual']", 0),
    ("tests/integration/languages/csharp/conditionals.cs", "//if", 3),
    ("tests/integration/languages/csharp/flat-lists.cs", "//method[1]", 0),
    ("tests/integration/languages/csharp/interface-public.cs", "//interface/body/method[public][1]", 0),
    ("tests/integration/languages/csharp/type-vocabulary.cs", "//class[name='Dog']", 0),
    ("tests/integration/languages/csharp/where-clause.cs", "//class", 4),

    // — Rust —
    ("tests/integration/languages/rust/conditionals.rs", "//if", 3),
    ("tests/integration/languages/rust/flat-lists.rs", "//function[name='first']", 0),
    ("tests/integration/languages/rust/match-expression.rs", "//match", 3),
    ("tests/integration/languages/rust/method-call.rs", "//call[1]", 0),
    ("tests/integration/languages/rust/reference-type.rs", "//param[type[borrowed]][1]", 0),
    ("tests/integration/languages/rust/struct-expression.rs", "//literal[name='Point']", 0),
    ("tests/integration/languages/rust/type-vocabulary.rs", "//struct[name='Dog']", 3),
    ("tests/integration/languages/rust/typedef.rs", "//alias[1]", 0),
    ("tests/integration/languages/rust/visibility.rs", "//function[pub][1]", 0),

    // — Python —
    ("tests/integration/languages/python/augmented-assign.py", "//assign[op][1]", 0),
    ("tests/integration/languages/python/collection-markers.py", "//list[comprehension]", 0),
    ("tests/integration/languages/python/conditionals.py", "//if", 3),
    ("tests/integration/languages/python/expression-list.py", "//return[1]", 0),
    ("tests/integration/languages/python/f-strings.py", "//string[interpolation]", 0),

    // — Go —
    ("tests/integration/languages/go/conditionals.go", "//if", 3),
    ("tests/integration/languages/go/defined-type-vs-alias.go", "//alias", 0),
    ("tests/integration/languages/go/flat-lists.go", "//function", 0),
    ("tests/integration/languages/go/raw-string.go", "//string[raw]", 0),
    ("tests/integration/languages/go/struct-interface-hoist.go", "//struct", 0),
    ("tests/integration/languages/go/type-declaration.go", "//interface", 0),

    // — Ruby —
    ("tests/integration/languages/ruby/conditionals.rb", "//if", 3),
    ("tests/integration/languages/ruby/name-inlining.rb", "//class", 0),
];

/// Output-format snapshot cases: (relative path under formats/, tractor args).
/// Directory = -f value, name = command + params, extension = file format.
const OUTPUT_FORMAT_CASES: &[(&str, &[&str])] = &[
    // -f text
    ("text/query.txt", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
    ]),
    ("text/query-value.txt", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class", "-v", "value",
    ]),
    ("text/query-count.txt", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class", "-v", "count",
    ]),
    ("text/query-message.txt", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
        "-m", "{file}:{line}",
    ]),
    ("text/check.txt", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found", "-f", "text",
    ]),
    ("text/check-composable.txt", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found", "-v", "tree,reason,severity", "-f", "text",
    ]),
    ("text/query-summary.txt", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
        "-v", "summary",
    ]),
    ("text/query-query.txt", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
        "-v", "query",
    ]),
    ("text/query-location.txt", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
        "-v", "file,line",
    ]),
    ("text/query-meta.txt", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
        "--meta", "--depth", "1",
    ]),
    ("text/explore-tree-source.txt", &[
        "tests/integration/languages/csharp/comments.cs", "-v", "tree,source",
    ]),
    ("text/explore-tree-source-color.txt", &[
        "tests/integration/languages/csharp/comments.cs", "-v", "tree,source",
        "--color", "always",
    ]),
    // -f gcc
    ("gcc/check.txt", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found",
    ]),
    ("gcc/check-no-matches.txt", &[
        "check", "tests/integration/formats/sample.cs", "-x", "interface",
        "--reason", "interface found",
    ]),
    // -f json
    ("json/query.json", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class", "-f", "json",
    ]),
    ("json/query-value.json", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
        "-v", "value", "-f", "json",
    ]),
    ("json/query-message.json", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
        "-m", "{file}:{line}", "-f", "json",
    ]),
    ("json/check.json", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found", "-f", "json",
    ]),
    ("json/check-composable.json", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found", "-v", "tree,reason,severity", "-f", "json",
    ]),
    ("json/query-summary.json", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
        "-v", "summary", "-f", "json",
    ]),
    ("json/query-query.json", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
        "-v", "query", "-f", "json",
    ]),
    // -f xml
    ("xml/query.xml", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class", "-f", "xml",
    ]),
    ("xml/check.xml", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found", "-f", "xml",
    ]),
    ("xml/query-summary.xml", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
        "-v", "summary", "-f", "xml",
    ]),
    ("xml/query-query.xml", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
        "-v", "query", "-f", "xml",
    ]),
    // -f yaml
    ("yaml/query.yaml", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class", "-f", "yaml",
    ]),
    ("yaml/check.yaml", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found", "-f", "yaml",
    ]),
    ("yaml/query-summary.yaml", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
        "-v", "summary", "-f", "yaml",
    ]),
    ("yaml/query-query.yaml", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
        "-v", "query", "-f", "yaml",
    ]),
    // --depth snapshots: verify tree truncation at various depths
    ("text/query-depth1.txt", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class", "--depth", "1",
    ]),
    ("text/query-depth2.txt", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class", "--depth", "2",
    ]),
    ("json/query-depth1.json", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
        "-v", "tree", "-f", "json", "--depth", "1",
    ]),
    ("json/query-depth2.json", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
        "-v", "tree", "-f", "json", "--depth", "2",
    ]),
    ("xml/check-composable-depth1.xml", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found", "-v", "tree,reason,severity", "-f", "xml", "--depth", "1",
    ]),
    // Projection snapshots
    ("text/project-tree.txt", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class",
        "-p", "tree",
    ]),
    ("json/project-tree.json", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class",
        "-p", "tree", "-f", "json",
    ]),
    ("yaml/project-tree.yaml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class",
        "-p", "tree", "-f", "yaml",
    ]),
    ("xml/project-tree.xml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class",
        "-p", "tree", "-f", "xml",
    ]),
    ("json/project-tree-single.json", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class",
        "-p", "tree", "--single", "-f", "json",
    ]),
    ("yaml/project-tree-single.yaml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class",
        "-p", "tree", "--single", "-f", "yaml",
    ]),
    ("xml/project-tree-single.xml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class",
        "-p", "tree", "--single", "-f", "xml",
    ]),
    ("text/project-tree-single.txt", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class",
        "-p", "tree", "--single",
    ]),
    ("text/project-value.txt", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "value",
    ]),
    ("text/project-value-single.txt", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "value", "--single",
    ]),
    ("json/project-value.json", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "value", "-f", "json",
    ]),
    ("json/project-value-single.json", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "value", "--single", "-f", "json",
    ]),
    ("yaml/project-value.yaml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "value", "-f", "yaml",
    ]),
    ("yaml/project-value-single.yaml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "value", "--single", "-f", "yaml",
    ]),
    ("xml/project-value.xml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "value", "-f", "xml",
    ]),
    ("xml/project-value-single.xml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "value", "--single", "-f", "xml",
    ]),
    ("text/project-source.txt", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "source",
    ]),
    ("text/project-source-single.txt", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "source", "--single",
    ]),
    ("json/project-source.json", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "source", "-f", "json",
    ]),
    ("json/project-source-single.json", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "source", "--single", "-f", "json",
    ]),
    ("yaml/project-source.yaml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "source", "-f", "yaml",
    ]),
    ("yaml/project-source-single.yaml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "source", "--single", "-f", "yaml",
    ]),
    ("xml/project-source.xml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "source", "-f", "xml",
    ]),
    ("xml/project-source-single.xml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "source", "--single", "-f", "xml",
    ]),
    ("text/project-lines.txt", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "lines",
    ]),
    ("text/project-lines-single.txt", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "lines", "--single",
    ]),
    ("json/project-lines.json", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "lines", "-f", "json",
    ]),
    ("json/project-lines-single.json", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "lines", "--single", "-f", "json",
    ]),
    ("yaml/project-lines.yaml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "lines", "-f", "yaml",
    ]),
    ("yaml/project-lines-single.yaml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "lines", "--single", "-f", "yaml",
    ]),
    ("xml/project-lines.xml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "lines", "-f", "xml",
    ]),
    ("xml/project-lines-single.xml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-p", "lines", "--single", "-f", "xml",
    ]),
    ("text/project-schema.txt", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class",
        "-p", "schema",
    ]),
    ("text/project-schema-color.txt", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class",
        "-p", "schema", "--color", "always",
    ]),
    ("json/project-schema.json", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class",
        "-p", "schema", "-f", "json",
    ]),
    ("yaml/project-schema.yaml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class",
        "-p", "schema", "-f", "yaml",
    ]),
    ("xml/project-schema.xml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class",
        "-p", "schema", "-f", "xml",
    ]),
    ("json/project-results-message.json", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
        "-m", "hit", "-p", "results", "-f", "json",
    ]),
    ("json/project-summary.json", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found", "-p", "summary", "-f", "json",
    ]),
    ("yaml/project-summary.yaml", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found", "-p", "summary", "-f", "yaml",
    ]),
    ("xml/project-summary.xml", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found", "-p", "summary", "-f", "xml",
    ]),
    ("text/project-summary.txt", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found", "-p", "summary",
    ]),
    ("text/project-results.txt", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-v", "file,value", "-p", "results",
    ]),
    ("text/project-results-single.txt", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-v", "file,value", "-p", "results", "--single",
    ]),
    ("json/project-results.json", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-v", "file,value", "-p", "results", "-f", "json",
    ]),
    ("json/project-results-single.json", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-v", "file,value", "-p", "results", "--single", "-f", "json",
    ]),
    ("yaml/project-results.yaml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-v", "file,value", "-p", "results", "-f", "yaml",
    ]),
    ("yaml/project-results-single.yaml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-v", "file,value", "-p", "results", "--single", "-f", "yaml",
    ]),
    ("xml/project-results.xml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-v", "file,value", "-p", "results", "-f", "xml",
    ]),
    ("xml/project-results-single.xml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-v", "file,value", "-p", "results", "--single", "-f", "xml",
    ]),
    ("text/project-report.txt", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-v", "summary,file,value", "-p", "report",
    ]),
    ("json/project-report.json", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-v", "summary,file,value", "-p", "report", "-f", "json",
    ]),
    ("yaml/project-report.yaml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-v", "summary,file,value", "-p", "report", "-f", "yaml",
    ]),
    ("xml/project-report.xml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class/name",
        "-v", "summary,file,value", "-p", "report", "-f", "xml",
    ]),
    ("text/project-count.txt", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class",
        "-p", "count",
    ]),
    ("json/project-count.json", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class",
        "-p", "count", "-f", "json",
    ]),
    ("xml/project-count.xml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class",
        "-p", "count", "-f", "xml",
    ]),
    ("yaml/project-count.yaml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-x", "class",
        "-p", "count", "-f", "yaml",
    ]),
    ("text/project-totals.txt", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found", "-p", "totals",
    ]),
    ("json/project-totals.json", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found", "-p", "totals", "-f", "json",
    ]),
    ("yaml/project-totals.yaml", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found", "-p", "totals", "-f", "yaml",
    ]),
    ("xml/project-totals.xml", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found", "-p", "totals", "-f", "xml",
    ]),
    // map/array constructor output: XPath 3.1 structured results rendered natively.
    // Uses sample-classes.cs (Calculator + Greeter, each with multiple methods) so
    // the output is realistic and easy to inspect visually.
    // The map is wrapped in { "classes": [...] } so the result mirrors an intuitive
    // data shape: a single object whose "classes" key holds all class records.
    ("json/query-map.json", &[
        "query", "tests/integration/formats/sample-classes.cs", "-l", "csharp",
        "-x", r#"/! map { "classes": array { //class ! map { "name": string(name), "methods": array { body/method/name/string(.) } } } }"#,
        "-f", "json",
    ]),
    ("json/query-array.json", &[
        "query", "tests/integration/formats/sample-classes.cs", "-l", "csharp",
        "-x", r#"//class ! array { string(name) }"#, "-f", "json",
    ]),
    ("yaml/query-map.yaml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-l", "csharp",
        "-x", r#"/! map { "classes": array { //class ! map { "name": string(name), "methods": array { body/method/name/string(.) } } } }"#,
        "-f", "yaml",
    ]),
    ("yaml/query-array.yaml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-l", "csharp",
        "-x", r#"//class ! array { string(name) }"#, "-f", "yaml",
    ]),
    ("xml/query-map.xml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-l", "csharp",
        "-x", r#"/! map { "classes": array { //class ! map { "name": string(name), "methods": array { body/method/name/string(.) } } } }"#,
        "-f", "xml",
    ]),
    ("xml/query-array.xml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-l", "csharp",
        "-x", r#"//class ! array { string(name) }"#, "-f", "xml",
    ]),
    // Issue #60: map with sequence-valued key (no explicit array{}).
    // The "methods" value is a bare sequence — previously silently dropped,
    // now auto-wrapped in an array. Output should match query-map.* above.
    ("json/query-map-sequence-value.json", &[
        "query", "tests/integration/formats/sample-classes.cs", "-l", "csharp",
        "-x", r#"/! map { "classes": array { //class ! map { "name": string(name), "methods": body/method/name/string(.) } } }"#,
        "-f", "json",
    ]),
    ("yaml/query-map-sequence-value.yaml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-l", "csharp",
        "-x", r#"/! map { "classes": array { //class ! map { "name": string(name), "methods": body/method/name/string(.) } } }"#,
        "-f", "yaml",
    ]),
    ("xml/query-map-sequence-value.xml", &[
        "query", "tests/integration/formats/sample-classes.cs", "-l", "csharp",
        "-x", r#"/! map { "classes": array { //class ! map { "name": string(name), "methods": body/method/name/string(.) } } }"#,
        "-f", "xml",
    ]),
    // -g file (group-by) snapshots: query mode with multi-file grouping
    ("json/query-group-file.json", &[
        "query", "tests/integration/formats/sample.cs", "tests/integration/formats/sample2.cs",
        "-x", "class", "-g", "file", "-f", "json",
    ]),
    ("xml/query-group-file.xml", &[
        "query", "tests/integration/formats/sample.cs", "tests/integration/formats/sample2.cs",
        "-x", "class", "-g", "file", "-f", "xml",
    ]),
    ("json/check-no-group.json", &[
        "check", "tests/integration/formats/sample.cs", "tests/integration/formats/sample2.cs",
        "-x", "class", "--reason", "class found", "-g", "none", "-f", "json",
    ]),
    // Color snapshots: ANSI codes rendered as \e so they are visible in text editors.
    // These document what colored output looks like for each format.
    ("text/query-color.txt", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
        "--color", "always",
    ]),
    ("xml/query-color.xml", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class",
        "-f", "xml", "--color", "always",
    ]),
    ("xml/check-color.xml", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found", "-f", "xml", "--color", "always",
    ]),
    ("gcc/check-color.txt", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found", "--color", "always",
    ]),
    // Multi-rule check via --config flag
    ("xml/check-multirule.xml", &[
        "check",
        "--config", "tests/integration/formats/multirule.yaml", "-f", "xml",
    ]),
    ("json/check-multirule.json", &[
        "check",
        "--config", "tests/integration/formats/multirule.yaml", "-f", "json",
    ]),
    ("gcc/check-multirule.txt", &[
        "check",
        "--config", "tests/integration/formats/multirule.yaml",
    ]),
    // Multi-op run report (check + test in one config)
    ("xml/run-multiop.xml", &[
        "run", "--config", "tests/integration/formats/multiop.yaml", "-f", "xml",
    ]),
    ("json/run-multiop.json", &[
        "run", "--config", "tests/integration/formats/multiop.yaml", "-f", "json",
    ]),
    // --help snapshots: track changes to CLI help text per subcommand
    ("help/query.txt", &["query", "--help"]),
    ("help/check.txt", &["check", "--help"]),
    ("help/test.txt",  &["test",  "--help"]),
    ("help/run.txt",   &["run",   "--help"]),
    ("help/init.txt",  &["init",  "--help"]),
    // Error/diagnostic snapshots: invalid XPath across all output formats.
    ("errors/xpath-invalid-text.txt", &[
        "query", "tests/integration/formats/sample.cs", "-x", "//class[bad=(",
        "--no-color",
    ]),
    ("errors/xpath-invalid.json", &[
        "query", "tests/integration/formats/sample.cs", "-x", "//class[bad=(",
        "-f", "json",
    ]),
    ("errors/xpath-invalid.yaml", &[
        "query", "tests/integration/formats/sample.cs", "-x", "//class[bad=(",
        "-f", "yaml",
    ]),
    ("errors/xpath-invalid-gcc.txt", &[
        "query", "tests/integration/formats/sample.cs", "-x", "//class[bad=(",
        "-f", "gcc", "--no-color",
    ]),
    ("errors/xpath-invalid-github.txt", &[
        "query", "tests/integration/formats/sample.cs", "-x", "//class[bad=(",
        "-f", "github",
    ]),
    ("errors/xpath-invalid.xml", &[
        "query", "tests/integration/formats/sample.cs", "-x", "//class[bad=(",
        "-f", "xml", "--no-color",
    ]),
    ("errors/xpath-invalid-check.txt", &[
        "check", "tests/integration/formats/sample.cs", "-x", "//class[bad=(",
        "--reason", "test", "--no-color",
    ]),
];

struct Mismatch {
    path: String,
    expected: String,
    actual: String,
    missing: bool,
}

impl Mismatch {
    fn changed(path: &str, expected: &str, actual: &str) -> Self {
        Self {
            path: path.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
            missing: false,
        }
    }

    fn missing(path: &str, actual: &str) -> Self {
        Self {
            path: path.to_string(),
            expected: String::new(),
            actual: actual.to_string(),
            missing: true,
        }
    }
}

/// Print a minimal line-based diff between expected and actual snapshot contents.
///
/// Uses a naive line-by-line alignment (no LCS): good enough for spotting the
/// actual bytes that differ when CI fails, without pulling in a diff crate.
fn print_mismatch(m: &Mismatch) {
    println!("\x1b[1m--- {}\x1b[0m", m.path);
    if m.missing {
        println!("  (snapshot file missing — showing full actual output)");
        println!("\x1b[32m+++ actual ({} bytes)\x1b[0m", m.actual.len());
        for line in m.actual.lines().take(200) {
            println!("\x1b[32m+ {}\x1b[0m", line);
        }
        if m.actual.lines().count() > 200 {
            println!("  ... ({} more lines)", m.actual.lines().count() - 200);
        }
        println!();
        return;
    }

    let expected: Vec<&str> = m.expected.lines().collect();
    let actual: Vec<&str> = m.actual.lines().collect();
    let max = expected.len().max(actual.len());
    let mut printed = 0usize;
    let mut suppressed = 0usize;
    for i in 0..max {
        let e = expected.get(i).copied();
        let a = actual.get(i).copied();
        match (e, a) {
            (Some(e), Some(a)) if e == a => {
                // Print a little context around diffs (max 2 lines before/after shown
                // naturally by nearby diffs). For simplicity skip equal lines entirely.
                continue;
            }
            (Some(e), Some(a)) => {
                if printed >= 80 {
                    suppressed += 2;
                    continue;
                }
                println!("\x1b[31m- {:4} {}\x1b[0m", i + 1, e);
                println!("\x1b[32m+ {:4} {}\x1b[0m", i + 1, a);
                printed += 2;
            }
            (Some(e), None) => {
                if printed >= 80 {
                    suppressed += 1;
                    continue;
                }
                println!("\x1b[31m- {:4} {}\x1b[0m", i + 1, e);
                printed += 1;
            }
            (None, Some(a)) => {
                if printed >= 80 {
                    suppressed += 1;
                    continue;
                }
                println!("\x1b[32m+ {:4} {}\x1b[0m", i + 1, a);
                printed += 1;
            }
            (None, None) => {}
        }
    }
    if suppressed > 0 {
        println!("  ... ({} more differing line(s) suppressed)", suppressed);
    }
    if !m.expected.ends_with('\n') || !m.actual.ends_with('\n') {
        println!(
            "  (trailing newline: expected={}, actual={})",
            m.expected.ends_with('\n'),
            m.actual.ends_with('\n')
        );
    }
    println!();
}

fn main() {
    let check_mode = std::env::args().any(|a| a == "--check");

    let tests_dir = Path::new("tests/integration");
    if !tests_dir.is_dir() {
        eprintln!("error: {} not found — run from project root", tests_dir.display());
        process::exit(1);
    }

    let tractor_bin = find_tractor_bin();
    let skip: HashSet<&str> = SKIP_EXTENSIONS.iter().copied().collect();
    // Feature fixtures get their own snapshot pair and must be excluded from
    // the default language walker (which would otherwise emit redundant
    // .xml / .raw.xml outputs for them).
    let feature_set: HashSet<String> = FEATURE_FIXTURES
        .iter()
        .map(|(p, _, _)| p.replace('\\', "/"))
        .collect();

    let mut processed = 0;
    let mut mismatches: Vec<Mismatch> = Vec::new();

    // --- Language parse-tree snapshots ---

    let languages_dir = tests_dir.join("languages");
    let mut dirs: Vec<_> = fs::read_dir(&languages_dir)
        .expect("cannot read tests/integration/languages")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();
    dirs.sort_by_key(|e| e.file_name());

    for entry in dirs {
        let lang_dir = entry.path();

        let mut files: Vec<_> = fs::read_dir(&lang_dir)
            .expect("cannot read language dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .collect();
        files.sort_by_key(|e| e.file_name());

        for file_entry in files {
            let path = file_entry.path();
            let file_name = path.file_name().unwrap().to_string_lossy().to_string();

            // Skip generated snapshot outputs — these are products of this
            // tool, not source fixtures. Without this guard, the walker
            // would treat e.g. `foo.snapshot.txt` as an input and emit
            // `foo.snapshot.txt.xml` etc.
            if file_name.contains(".snapshot.") {
                continue;
            }

            let ext = match path.extension().and_then(|e| e.to_str()) {
                Some(e) => e,
                None => continue,
            };

            if skip.contains(ext) {
                continue;
            }

            // Normalize to forward slashes for consistent cross-platform output
            let path_str = path.to_string_lossy().replace('\\', "/");
            let lang_name = lang_dir.file_name().unwrap().to_string_lossy();

            // Skip feature fixtures — handled in a separate pass below.
            if feature_set.contains(&path_str) {
                continue;
            }

            // Semantic XML
            let xml_path = format!("{}.xml", path_str);
            let output = run_tractor(&tractor_bin, &path_str, &["-f", "xml"]);

            // Raw TreeSitter XML
            let raw_xml_path = format!("{}.raw.xml", path_str);
            let raw_output = run_tractor(&tractor_bin, &path_str, &["-t", "raw", "-f", "xml"]);

            if check_mode {
                match fs::read_to_string(&xml_path) {
                    Ok(existing) if existing != output => {
                        mismatches.push(Mismatch::changed(&xml_path, &existing, &output));
                    }
                    Err(_) => mismatches.push(Mismatch::missing(&xml_path, &output)),
                    _ => {}
                }
                match fs::read_to_string(&raw_xml_path) {
                    Ok(existing) if existing != raw_output => {
                        mismatches.push(Mismatch::changed(&raw_xml_path, &existing, &raw_output));
                    }
                    Err(_) => mismatches.push(Mismatch::missing(&raw_xml_path, &raw_output)),
                    _ => {}
                }
            } else {
                fs::write(&xml_path, &output).expect("cannot write .xml snapshot");
                fs::write(&raw_xml_path, &raw_output).expect("cannot write .raw.xml snapshot");
                println!("  {}/{} -> .xml, .raw.xml", lang_name, file_name);
            }

            processed += 1;
        }
    }

    // --- Feature-invariant fragment snapshots ---
    //
    // Each fixture extracts a focused subtree via XPath and emits a
    // single text snapshot. The regression protection lives in the
    // assertion suite (`tests/semantic_tree.rs`); these fragments are
    // for reviewers who want a scannable visual reference of what
    // each transformed construct looks like.
    for &(source_rel, xpath, depth) in FEATURE_FIXTURES {
        let source_rel = source_rel.replace('\\', "/");
        let source_path = Path::new(&source_rel);
        if !source_path.is_file() {
            eprintln!(
                "error: feature fixture source not found: {} — add the source file before listing it",
                source_rel
            );
            process::exit(1);
        }

        let txt_snap = format!("{}.snapshot.txt", source_rel);
        let depth_str = depth.to_string();
        let mut args: Vec<&str> = vec![
            "query", &source_rel, "-x", xpath, "-p", "tree", "--single",
        ];
        if depth > 0 {
            args.push("--depth");
            args.push(&depth_str);
        }
        let txt_out = run_tractor_args(&tractor_bin, &args);

        if check_mode {
            match fs::read_to_string(&txt_snap) {
                Ok(existing) if existing != txt_out => {
                    mismatches.push(Mismatch::changed(&txt_snap, &existing, &txt_out));
                }
                Err(_) => mismatches.push(Mismatch::missing(&txt_snap, &txt_out)),
                _ => {}
            }
        } else {
            fs::write(&txt_snap, &txt_out).expect("cannot write .snapshot.txt");
            println!("  feature {} -> .snapshot.txt ({})", source_rel, xpath);
        }

        processed += 1;
    }

    // --- Output-format combination snapshots ---

    let output_formats_dir = tests_dir.join("formats");

    // Stable CWD prefix for normalizing absolute paths in gcc snapshots.
    let cwd_prefix = std::env::current_dir()
        .map(|p| p.to_string_lossy().replace('\\', "/") + "/")
        .unwrap_or_default();

    for (name, args) in OUTPUT_FORMAT_CASES {
        let snap_path = output_formats_dir.join(name);
        let snap_path_str = snap_path.to_string_lossy().replace('\\', "/");
        if !check_mode {
            if let Some(parent) = snap_path.parent() {
                fs::create_dir_all(parent).expect("cannot create output-format subdir");
            }
        }
        let raw = run_tractor_args(&tractor_bin, args);
        // Strip the absolute CWD prefix from gcc/text output so snapshots are portable.
        // Replace ANSI color spans with emoji pairs: 🟦text🟦 so you see where each color
        // starts and ends. RESET just closes the current span without opening a new one.
        let output = replace_ansi_with_emoji_spans(
            &raw.replace(&cwd_prefix, "")
                .replace("tractor.exe", "tractor"),
        );

        if check_mode {
            match fs::read_to_string(&snap_path) {
                Ok(existing) if existing != output => {
                    mismatches.push(Mismatch::changed(&snap_path_str, &existing, &output));
                }
                Err(_) => mismatches.push(Mismatch::missing(&snap_path_str, &output)),
                _ => {}
            }
        } else {
            fs::write(&snap_path, &output).expect("cannot write output-format snapshot");
            println!("  formats/{}", name);
        }

        processed += 1;
    }

    if check_mode {
        if mismatches.is_empty() {
            println!("\x1b[32m✓\x1b[0m Snapshots match ({} fixtures checked)", processed);
        } else {
            println!("\x1b[31m✗\x1b[0m Snapshot mismatch ({} file(s)):", mismatches.len());
            println!();
            for m in &mismatches {
                print_mismatch(m);
            }
            println!("Summary:");
            for m in &mismatches {
                let tag = if m.missing { " (missing)" } else { "" };
                println!("  {}{}", m.path, tag);
            }
            println!();
            println!("If intentional, run 'task test:snapshots:update' to regenerate.");
            process::exit(1);
        }
    } else {
        println!("\nUpdated {} fixture(s).", processed);
    }
}

/// Replace ANSI color sequences with emoji-bracketed spans for readable snapshot diffs.
///
/// Tractor's color output uses ANSI escape codes (`\x1b[34m` for blue, etc.) which are
/// invisible bytes that make snapshot files hard to read and review in diffs. We want to
/// test that colors are emitted correctly, but raw escape codes like `\e[34mname\e[0m`
/// are hard to parse visually.
///
/// This function wraps each colored text span with a matching colored emoji on both sides,
/// so `\x1b[34mname\x1b[0m` becomes `🟦name🟦`. This makes it immediately obvious which
/// text is colored and in what color, both in the file and in PR diffs.
///
/// RESET (`\x1b[0m`) just closes the current span without opening a new one.
/// Unknown codes are written as `\e[…m` for visibility.
fn replace_ansi_with_emoji_spans(input: &str) -> String {
    fn code_to_emoji(code: &str) -> Option<&'static str> {
        match code {
            "1" => Some("🟥"),  // BOLD
            "2" => Some("🟫"),  // DIM
            "32" => Some("🟩"), // GREEN
            "33" => Some("🟨"), // YELLOW
            "34" => Some("🟦"), // BLUE
            "36" => Some("🟪"), // CYAN
            "43" => Some("⚠️"), // BG_YELLOW (match highlights)
            "97" => Some("⬜"), // WHITE (bright)
            _ => None,
        }
    }

    let mut out = String::with_capacity(input.len());
    let mut rest = input;

    while let Some(esc_pos) = rest.find('\x1b') {
        // Copy text before this escape
        out.push_str(&rest[..esc_pos]);
        rest = &rest[esc_pos..];

        // Try to parse \x1b[<digits>m
        if rest.len() >= 3 && rest.as_bytes()[1] == b'[' {
            if let Some(m_pos) = rest[2..].find('m') {
                let code = &rest[2..2 + m_pos];
                if code.bytes().all(|b| b.is_ascii_digit()) {
                    rest = &rest[2 + m_pos + 1..]; // skip past 'm'

                    if code == "0" {
                        // RESET: just close current span (handled below as no-op;
                        // the *previous* emoji is already emitted by the next block)
                        continue;
                    }

                    if let Some(emoji) = code_to_emoji(code) {
                        // Find the text this color covers (up to next \x1b or end)
                        let end = rest.find('\x1b').unwrap_or(rest.len());
                        let text = &rest[..end];
                        out.push_str(emoji);
                        out.push_str(text);
                        out.push_str(emoji);
                        rest = &rest[end..];
                        continue;
                    }

                    // Unknown code — write escaped for visibility
                    out.push_str("\\e[");
                    out.push_str(code);
                    out.push('m');
                    continue;
                }
            }
        }

        // Not a recognized escape sequence — escape the ESC byte and move on
        out.push_str("\\e");
        rest = &rest[1..];
    }

    out.push_str(rest);
    out
}

fn find_tractor_bin() -> String {
    // Prefer the most recently built binary so snapshots always reflect the
    // current source, regardless of which profile was used to build.
    let candidates = if cfg!(windows) {
        vec![
            "target/release/tractor.exe".to_string(),
            "target/debug/tractor.exe".to_string(),
        ]
    } else {
        vec![
            "target/release/tractor".to_string(),
            "target/debug/tractor".to_string(),
        ]
    };

    let mut found: Vec<(String, std::time::SystemTime)> = Vec::new();
    for c in &candidates {
        let p = Path::new(c);
        if p.is_file() {
            if let Ok(meta) = p.metadata() {
                if let Ok(mtime) = meta.modified() {
                    found.push((c.clone(), mtime));
                }
            }
        }
    }

    if found.is_empty() {
        eprintln!("error: tractor binary not found — run `cargo build` first");
        process::exit(1);
    }

    // Most recently modified first
    found.sort_by(|a, b| b.1.cmp(&a.1));
    found[0].0.clone()
}

fn run_tractor(bin: &str, fixture: &str, extra_args: &[&str]) -> String {
    let output = Command::new(bin)
        .arg(fixture)
        .args(extra_args)
        .output()
        .unwrap_or_else(|e| {
            eprintln!("error: failed to run {}: {}", bin, e);
            process::exit(1);
        });

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!(
            "warning: tractor failed on {} {:?}: {}",
            fixture, extra_args, stderr
        );
    }

    String::from_utf8(output.stdout).expect("non-UTF8 tractor output")
}

/// Run tractor with an arbitrary list of args (for output-format cases).
/// Stdout is captured as-is. Stderr lines are prefixed with ❌.
fn run_tractor_args(bin: &str, args: &[&str]) -> String {
    let output = Command::new(bin)
        .args(args)
        .output()
        .unwrap_or_else(|e| {
            eprintln!("error: failed to run {}: {}", bin, e);
            process::exit(1);
        });

    let stderr = String::from_utf8(output.stderr).expect("non-UTF8 tractor stderr");
    let stdout = String::from_utf8(output.stdout).expect("non-UTF8 tractor stdout");
    let mut merged = stdout;
    for line in stderr.lines() {
        merged.push_str("❌ ");
        merged.push_str(line);
        merged.push('\n');
    }
    merged
}
