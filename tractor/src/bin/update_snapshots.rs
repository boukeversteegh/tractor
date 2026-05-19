//! Generate/update or check snapshots for integration tests.
//!
//! Two hardcoded lists drive the snapshots:
//!
//! - `BLUEPRINTS`: one kitchen-sink source file per language. Each
//!   produces a `<source>.snapshot.txt` showing the entire transformed
//!   shape. Used to spot any cross-cutting transform regression.
//! - `OUTPUT_FORMAT_CASES`: tractor invocations exercising every
//!   output format / projection / CLI option combination, against a
//!   shared `sample.cs` (and a small `sample2.cs` for multi-file
//!   cases). Each case writes a snapshot under `tests/integration/cli/`.
//!
//! Usage:
//!   cargo run --release --bin update-snapshots          # update snapshots
//!   cargo run --release --bin update-snapshots -- --check  # check only (no writes)

use std::fs;
use std::path::Path;
use std::process::{self, Command};

/// Per-language blueprints. Each entry: (path, xpath, depth, shape_only).
///
/// - `xpath` selects the root element to render.
/// - `depth = 0` means "no limit" (blueprints render at full depth so
///   every transform change shows up as a visible snapshot diff).
/// - `shape_only = true` renders via `-p shape` (tree structure +
///   queryable markers, no source text). Text preservation is enforced
///   separately by `tests/text_preservation.rs`.
///
/// Output: two snapshots per blueprint, both rendering the same tree
/// at the same depth:
/// - `<source>.snapshot.txt` — text shape, primary diff for transform
///   changes (no source text per `-p shape`).
/// - `<source>.snapshot.json` — JSON tree projection (via `-p tree
///   --single -f json`), so cardinality decisions (`list=` →
///   array vs object) are visible in fixtures and shape regressions
///   that only show up in JSON consumers are caught.
const BLUEPRINTS: &[(&str, &str, u32, bool)] = &[
    ("tests/integration/languages/typescript/blueprint.ts", "//program", 0, true),
    ("tests/integration/languages/java/blueprint.java",     "//program", 0, true),
    ("tests/integration/languages/csharp/blueprint.cs",     "//unit",    0, true),
    ("tests/integration/languages/rust/blueprint.rs",       "//file",    0, true),
    ("tests/integration/languages/python/blueprint.py",     "//module",  0, true),
    ("tests/integration/languages/go/blueprint.go",         "//file",    0, true),
    ("tests/integration/languages/php/blueprint.php",       "//program", 0, true),
    ("tests/integration/languages/ruby/blueprint.rb",       "//program", 0, true),
    ("tests/integration/languages/tsql/blueprint.sql",      "//file",    0, true),
];

/// CLI snapshot cases: (relative path under cli/, tractor args).
///
/// Each scenario has its own folder. All format renderings of one scenario
/// sit side-by-side as `{scenario}.snapshot.{txt,json,xml,yaml}`. For
/// non-default-extension formats (gcc, github), the format is encoded in
/// the filename: `{scenario}.snapshot.gcc.txt`. Configs (when needed) live
/// alongside the snapshots as `{scenario}.config.yml`.
const OUTPUT_FORMAT_CASES: &[(&str, &[&str])] = &[
    // ---- query: basic query against sample.cs ----
    ("query/query.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class", "--depth", "2",
    ]),
    ("query/query.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class", "-f", "json", "--depth", "2",
    ]),
    ("query/query.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "class", "-f", "xml", "--depth", "2",
    ]),
    ("query/query.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-x", "class", "-f", "yaml", "--depth", "2",
    ]),

    // ---- query-value / count / message / summary / query / location / meta ----
    ("query-value/query-value.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class", "-v", "value",
    ]),
    ("query-value/query-value.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-v", "value", "-f", "json",
    ]),
    ("query-value/query-value.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "value", "-f", "yaml",
    ]),
    ("query-value/query-value.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "value", "-f", "xml",
    ]),
    ("query-count/query-count.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class", "-v", "count",
    ]),
    ("query-message/query-message.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-m", "{file}:{line}",
    ]),
    ("query-message/query-message.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-m", "{file}:{line}", "-f", "json", "--depth", "2",
    ]),
    ("query-summary/query-summary.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class", "-v", "summary",
    ]),
    ("query-summary/query-summary.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-v", "summary", "-f", "json",
    ]),
    ("query-summary/query-summary.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-v", "summary", "-f", "xml",
    ]),
    ("query-summary/query-summary.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-v", "summary", "-f", "yaml",
    ]),
    ("query-query/query-query.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class", "-v", "query",
    ]),
    ("query-query/query-query.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-v", "query", "-f", "json",
    ]),
    ("query-query/query-query.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-v", "query", "-f", "xml",
    ]),
    ("query-query/query-query.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-v", "query", "-f", "yaml",
    ]),
    ("query-location/query-location.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-v", "file,line",
    ]),
    ("query-meta/query-meta.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "--meta", "--depth", "1",
    ]),

    // ---- explore-tree-source / -color: combined tree + source view ----
    ("explore-tree-source/explore-tree-source.snapshot.txt", &[
        "tests/integration/sample.cs", "-v", "tree,source", "--depth", "2",
    ]),
    ("explore-tree-source-color/explore-tree-source-color.snapshot.txt", &[
        "tests/integration/sample.cs", "-v", "tree,source",
        "--color", "always", "--depth", "2",
    ]),

    // ---- check: simple check rule ----
    ("check/check.snapshot.txt", &[
        "check", "tests/integration/sample.cs", "-x", "class",
        "--reason", "class found", "-f", "text", "--depth", "2",
    ]),
    ("check/check.snapshot.json", &[
        "check", "tests/integration/sample.cs", "-x", "class",
        "--reason", "class found", "-f", "json", "--depth", "2",
    ]),
    ("check/check.snapshot.xml", &[
        "check", "tests/integration/sample.cs", "-x", "class",
        "--reason", "class found", "-f", "xml", "--depth", "2",
    ]),
    ("check/check.snapshot.yaml", &[
        "check", "tests/integration/sample.cs", "-x", "class",
        "--reason", "class found", "-f", "yaml", "--depth", "2",
    ]),
    ("check/check.snapshot.gcc.txt", &[
        "check", "tests/integration/sample.cs", "-x", "class",
        "--reason", "class found",
    ]),
    ("check-composable/check-composable.snapshot.txt", &[
        "check", "tests/integration/sample.cs", "-x", "class",
        "--reason", "class found", "-v", "tree,reason,severity", "-f", "text", "--depth", "2",
    ]),
    ("check-composable/check-composable.snapshot.json", &[
        "check", "tests/integration/sample.cs", "-x", "class",
        "--reason", "class found", "-v", "tree,reason,severity", "-f", "json", "--depth", "2",
    ]),
    ("check-composable-depth1/check-composable-depth1.snapshot.xml", &[
        "check", "tests/integration/sample.cs", "-x", "class",
        "--reason", "class found", "-v", "tree,reason,severity", "-f", "xml", "--depth", "1",
    ]),
    ("check-no-matches/check-no-matches.snapshot.gcc.txt", &[
        "check", "tests/integration/sample.cs", "-x", "interface",
        "--reason", "interface found",
    ]),

    // ---- depth truncation snapshots ----
    ("query-depth1/query-depth1.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class", "--depth", "1",
    ]),
    ("query-depth1/query-depth1.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-v", "tree", "-f", "json", "--depth", "1",
    ]),
    ("query-depth2/query-depth2.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class", "--depth", "2",
    ]),
    ("query-depth2/query-depth2.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-v", "tree", "-f", "json", "--depth", "2",
    ]),

    // ---- projection: tree ----
    ("project-tree/project-tree.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-p", "tree", "--depth", "2",
    ]),
    ("project-tree/project-tree.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-p", "tree", "-f", "json", "--depth", "2",
    ]),
    ("project-tree/project-tree.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-p", "tree", "-f", "yaml", "--depth", "2",
    ]),
    ("project-tree/project-tree.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-p", "tree", "-f", "xml", "--depth", "2",
    ]),
    ("project-tree-single/project-tree-single.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-p", "tree", "--single", "--depth", "2",
    ]),
    ("project-tree-single/project-tree-single.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-p", "tree", "--single", "-f", "json", "--depth", "2",
    ]),
    ("project-tree-single/project-tree-single.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-p", "tree", "--single", "-f", "yaml", "--depth", "2",
    ]),
    ("project-tree-single/project-tree-single.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-p", "tree", "--single", "-f", "xml", "--depth", "2",
    ]),

    // ---- projection: value ----
    ("project-value/project-value.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "value",
    ]),
    ("project-value/project-value.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "value", "-f", "json",
    ]),
    ("project-value/project-value.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "value", "-f", "yaml",
    ]),
    ("project-value/project-value.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "value", "-f", "xml",
    ]),
    ("project-value-single/project-value-single.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "value", "--single",
    ]),
    ("project-value-single/project-value-single.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "value", "--single", "-f", "json",
    ]),
    ("project-value-single/project-value-single.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "value", "--single", "-f", "yaml",
    ]),
    ("project-value-single/project-value-single.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "value", "--single", "-f", "xml",
    ]),

    // ---- projection: source ----
    ("project-source/project-source.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "source",
    ]),
    ("project-source/project-source.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "source", "-f", "json",
    ]),
    ("project-source/project-source.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "source", "-f", "yaml",
    ]),
    ("project-source/project-source.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "source", "-f", "xml",
    ]),
    ("project-source-single/project-source-single.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "source", "--single",
    ]),
    ("project-source-single/project-source-single.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "source", "--single", "-f", "json",
    ]),
    ("project-source-single/project-source-single.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "source", "--single", "-f", "yaml",
    ]),
    ("project-source-single/project-source-single.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "source", "--single", "-f", "xml",
    ]),

    // ---- projection: lines ----
    ("project-lines/project-lines.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "lines",
    ]),
    ("project-lines/project-lines.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "lines", "-f", "json",
    ]),
    ("project-lines/project-lines.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "lines", "-f", "yaml",
    ]),
    ("project-lines/project-lines.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "lines", "-f", "xml",
    ]),
    ("project-lines-single/project-lines-single.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "lines", "--single",
    ]),
    ("project-lines-single/project-lines-single.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "lines", "--single", "-f", "json",
    ]),
    ("project-lines-single/project-lines-single.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "lines", "--single", "-f", "yaml",
    ]),
    ("project-lines-single/project-lines-single.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-p", "lines", "--single", "-f", "xml",
    ]),

    // ---- projection: schema ----
    ("project-schema/project-schema.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-p", "schema", "--depth", "1",
    ]),
    ("project-schema/project-schema.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-p", "schema", "-f", "json", "--depth", "1",
    ]),
    ("project-schema/project-schema.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-p", "schema", "-f", "yaml", "--depth", "1",
    ]),
    ("project-schema/project-schema.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-p", "schema", "-f", "xml", "--depth", "1",
    ]),
    ("project-schema-color/project-schema-color.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-p", "schema", "--color", "always", "--depth", "1",
    ]),

    // ---- projection: results / report ----
    ("project-results/project-results.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-v", "file,value", "-p", "results",
    ]),
    ("project-results/project-results.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-v", "file,value", "-p", "results", "-f", "json",
    ]),
    ("project-results/project-results.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-v", "file,value", "-p", "results", "-f", "yaml",
    ]),
    ("project-results/project-results.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-v", "file,value", "-p", "results", "-f", "xml",
    ]),
    ("project-results-single/project-results-single.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-v", "file,value", "-p", "results", "--single",
    ]),
    ("project-results-single/project-results-single.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-v", "file,value", "-p", "results", "--single", "-f", "json",
    ]),
    ("project-results-single/project-results-single.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-v", "file,value", "-p", "results", "--single", "-f", "yaml",
    ]),
    ("project-results-single/project-results-single.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-v", "file,value", "-p", "results", "--single", "-f", "xml",
    ]),
    ("project-results-message/project-results-message.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-m", "hit", "-p", "results", "-f", "json",
    ]),
    ("project-report/project-report.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-v", "summary,file,value", "-p", "report",
    ]),
    ("project-report/project-report.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-v", "summary,file,value", "-p", "report", "-f", "json",
    ]),
    ("project-report/project-report.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-v", "summary,file,value", "-p", "report", "-f", "yaml",
    ]),
    ("project-report/project-report.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "class/name",
        "-v", "summary,file,value", "-p", "report", "-f", "xml",
    ]),

    // ---- projection: count / summary / totals ----
    ("project-count/project-count.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-p", "count",
    ]),
    ("project-count/project-count.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-p", "count", "-f", "json",
    ]),
    ("project-count/project-count.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-p", "count", "-f", "yaml",
    ]),
    ("project-count/project-count.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-p", "count", "-f", "xml",
    ]),
    ("project-summary/project-summary.snapshot.txt", &[
        "check", "tests/integration/sample.cs", "-x", "class",
        "--reason", "class found", "-p", "summary",
    ]),
    ("project-summary/project-summary.snapshot.json", &[
        "check", "tests/integration/sample.cs", "-x", "class",
        "--reason", "class found", "-p", "summary", "-f", "json",
    ]),
    ("project-summary/project-summary.snapshot.yaml", &[
        "check", "tests/integration/sample.cs", "-x", "class",
        "--reason", "class found", "-p", "summary", "-f", "yaml",
    ]),
    ("project-summary/project-summary.snapshot.xml", &[
        "check", "tests/integration/sample.cs", "-x", "class",
        "--reason", "class found", "-p", "summary", "-f", "xml",
    ]),
    ("project-totals/project-totals.snapshot.txt", &[
        "check", "tests/integration/sample.cs", "-x", "class",
        "--reason", "class found", "-p", "totals",
    ]),
    ("project-totals/project-totals.snapshot.json", &[
        "check", "tests/integration/sample.cs", "-x", "class",
        "--reason", "class found", "-p", "totals", "-f", "json",
    ]),
    ("project-totals/project-totals.snapshot.yaml", &[
        "check", "tests/integration/sample.cs", "-x", "class",
        "--reason", "class found", "-p", "totals", "-f", "yaml",
    ]),
    ("project-totals/project-totals.snapshot.xml", &[
        "check", "tests/integration/sample.cs", "-x", "class",
        "--reason", "class found", "-p", "totals", "-f", "xml",
    ]),

    // ---- query-map / query-array: XPath 3.1 map+array constructors ----
    ("query-map/query-map.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-l", "csharp",
        "-x", r#"/! map { "classes": array { //class ! map { "name": string(name), "methods": array { body/method/name/string(.) } } } }"#,
        "-f", "json",
    ]),
    ("query-map/query-map.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-l", "csharp",
        "-x", r#"/! map { "classes": array { //class ! map { "name": string(name), "methods": array { body/method/name/string(.) } } } }"#,
        "-f", "yaml",
    ]),
    ("query-map/query-map.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-l", "csharp",
        "-x", r#"/! map { "classes": array { //class ! map { "name": string(name), "methods": array { body/method/name/string(.) } } } }"#,
        "-f", "xml",
    ]),
    ("query-array/query-array.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-l", "csharp",
        "-x", r#"//class ! array { string(name) }"#, "-f", "json",
    ]),
    ("query-array/query-array.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-l", "csharp",
        "-x", r#"//class ! array { string(name) }"#, "-f", "yaml",
    ]),
    ("query-array/query-array.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-l", "csharp",
        "-x", r#"//class ! array { string(name) }"#, "-f", "xml",
    ]),
    // Issue #60: map with sequence-valued key (no explicit array{}); the bare
    // sequence is auto-wrapped in an array. Output should match query-map above.
    ("query-map-sequence-value/query-map-sequence-value.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-l", "csharp",
        "-x", r#"/! map { "classes": array { //class ! map { "name": string(name), "methods": body/method/name/string(.) } } }"#,
        "-f", "json",
    ]),
    ("query-map-sequence-value/query-map-sequence-value.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-l", "csharp",
        "-x", r#"/! map { "classes": array { //class ! map { "name": string(name), "methods": body/method/name/string(.) } } }"#,
        "-f", "yaml",
    ]),
    ("query-map-sequence-value/query-map-sequence-value.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-l", "csharp",
        "-x", r#"/! map { "classes": array { //class ! map { "name": string(name), "methods": body/method/name/string(.) } } }"#,
        "-f", "xml",
    ]),

    // ---- group-by: multi-file query and check ----
    ("query-group-file/query-group-file.snapshot.json", &[
        "query", "tests/integration/sample.cs", "tests/integration/sample2.cs",
        "-x", "class", "-g", "file", "-f", "json", "--depth", "2",
    ]),
    ("query-group-file/query-group-file.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "tests/integration/sample2.cs",
        "-x", "class", "-g", "file", "-f", "xml", "--depth", "2",
    ]),
    ("check-no-group/check-no-group.snapshot.json", &[
        "check", "tests/integration/sample.cs", "tests/integration/sample2.cs",
        "-x", "class", "--reason", "class found", "-g", "none", "-f", "json", "--depth", "2",
    ]),

    // ---- color: ANSI codes rendered as emoji-bracketed spans ----
    ("query-color/query-color.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "--color", "always", "--depth", "2",
    ]),
    ("query-color/query-color.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "class",
        "-f", "xml", "--color", "always", "--depth", "2",
    ]),
    ("check-color/check-color.snapshot.xml", &[
        "check", "tests/integration/sample.cs", "-x", "class",
        "--reason", "class found", "-f", "xml", "--color", "always", "--depth", "2",
    ]),
    ("check-color/check-color.snapshot.gcc.txt", &[
        "check", "tests/integration/sample.cs", "-x", "class",
        "--reason", "class found", "--color", "always",
    ]),

    // ---- check-multirule: multi-rule check via --config (config alongside snapshots) ----
    ("check-multirule/check-multirule.snapshot.xml", &[
        "check",
        "--config", "tests/integration/cli/check-multirule/check-multirule.config.yml",
        "-f", "xml", "--depth", "2",
    ]),
    ("check-multirule/check-multirule.snapshot.json", &[
        "check",
        "--config", "tests/integration/cli/check-multirule/check-multirule.config.yml",
        "-f", "json", "--depth", "2",
    ]),
    ("check-multirule/check-multirule.snapshot.gcc.txt", &[
        "check",
        "--config", "tests/integration/cli/check-multirule/check-multirule.config.yml",
    ]),

    // ---- run-multiop: multi-operation run report (check + test in one config) ----
    ("run-multiop/run-multiop.snapshot.xml", &[
        "run",
        "--config", "tests/integration/cli/run-multiop/run-multiop.config.yml",
        "-f", "xml", "--depth", "2",
    ]),
    ("run-multiop/run-multiop.snapshot.json", &[
        "run",
        "--config", "tests/integration/cli/run-multiop/run-multiop.config.yml",
        "-f", "json", "--depth", "2",
    ]),

    // ---- --help snapshots: track CLI help text per subcommand ----
    ("help-query/help-query.snapshot.txt", &["query", "--help"]),
    ("help-check/help-check.snapshot.txt", &["check", "--help"]),
    ("help-test/help-test.snapshot.txt",   &["test",  "--help"]),
    ("help-run/help-run.snapshot.txt",     &["run",   "--help"]),
    ("help-init/help-init.snapshot.txt",   &["init",  "--help"]),

    // ---- xpath-invalid: error/diagnostic snapshots across formats ----
    ("xpath-invalid/xpath-invalid.snapshot.txt", &[
        "query", "tests/integration/sample.cs", "-x", "//class[bad=(",
        "--no-color",
    ]),
    ("xpath-invalid/xpath-invalid.snapshot.json", &[
        "query", "tests/integration/sample.cs", "-x", "//class[bad=(",
        "-f", "json",
    ]),
    ("xpath-invalid/xpath-invalid.snapshot.yaml", &[
        "query", "tests/integration/sample.cs", "-x", "//class[bad=(",
        "-f", "yaml",
    ]),
    ("xpath-invalid/xpath-invalid.snapshot.xml", &[
        "query", "tests/integration/sample.cs", "-x", "//class[bad=(",
        "-f", "xml", "--no-color",
    ]),
    ("xpath-invalid/xpath-invalid.snapshot.gcc.txt", &[
        "query", "tests/integration/sample.cs", "-x", "//class[bad=(",
        "-f", "gcc", "--no-color",
    ]),
    ("xpath-invalid/xpath-invalid.snapshot.github.txt", &[
        "query", "tests/integration/sample.cs", "-x", "//class[bad=(",
        "-f", "github",
    ]),
    ("xpath-invalid-check/xpath-invalid-check.snapshot.txt", &[
        "check", "tests/integration/sample.cs", "-x", "//class[bad=(",
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

    let mut processed = 0;
    let mut mismatches: Vec<Mismatch> = Vec::new();

    // --- Per-language blueprint snapshots ---
    //
    // One kitchen-sink fixture per language; the snapshot shows the
    // full transformed tree (or shape projection). Spot any
    // cross-cutting transform regression at a glance.
    for &(source_rel, xpath, depth, shape_only) in BLUEPRINTS {
        let source_rel = source_rel.replace('\\', "/");
        let source_path = Path::new(&source_rel);
        if !source_path.is_file() {
            eprintln!(
                "error: blueprint source not found: {} — add the source file before listing it",
                source_rel
            );
            process::exit(1);
        }

        let txt_snap = format!("{}.snapshot.txt", source_rel);
        let json_snap = format!("{}.snapshot.json", source_rel);
        let depth_str = depth.to_string();
        let projection = if shape_only { "shape" } else { "tree" };

        // --- text snapshot: shape projection (existing) ---
        let mut txt_args: Vec<&str> = vec![
            "query", &source_rel, "-x", xpath, "-p", projection, "--single",
        ];
        if depth > 0 {
            txt_args.push("--depth");
            txt_args.push(&depth_str);
        }
        let txt_out = run_tractor_args(&tractor_bin, &txt_args);

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
            println!("  blueprint {} -> .snapshot.txt", source_rel);
        }

        processed += 1;

        // --- json snapshot: tree projection (cardinality fixture) ---
        // Always uses `-p tree`: shape would lose source text and
        // primitive values, defeating the purpose of having a JSON
        // fixture that shows what consumers actually receive.
        let mut json_args: Vec<&str> = vec![
            "query", &source_rel, "-x", xpath, "-p", "tree", "--single", "-f", "json",
        ];
        if depth > 0 {
            json_args.push("--depth");
            json_args.push(&depth_str);
        }
        let json_out = run_tractor_args(&tractor_bin, &json_args);

        if check_mode {
            match fs::read_to_string(&json_snap) {
                Ok(existing) if existing != json_out => {
                    mismatches.push(Mismatch::changed(&json_snap, &existing, &json_out));
                }
                Err(_) => mismatches.push(Mismatch::missing(&json_snap, &json_out)),
                _ => {}
            }
        } else {
            fs::write(&json_snap, &json_out).expect("cannot write .snapshot.json");
            println!("  blueprint {} -> .snapshot.json", source_rel);
        }

        processed += 1;
    }

    // --- CLI scenario snapshots ---

    let output_formats_dir = tests_dir.join("cli");

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
            println!("  cli/{}", name);
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
