//! Semantic tests for the `-p` / `--project` / `--single` flags.
//!
//! Snapshots cover shape; these tests cover behaviour that a diff
//! wouldn't explain: CLI-level errors, content-independence across
//! cardinalities, parseability of every structured format, and the
//! exact warning text the design promises.

#[macro_use]
mod support;

use support::command;

// ---------------------------------------------------------------------------
// CLI errors — flag combinations that must fail early with a clear message
// ---------------------------------------------------------------------------

#[test]
fn single_with_n_two_is_a_cli_error() {
    let result = command([
        "query",
        "-s", "<root><a/></root>",
        "-l", "xml",
        "-x", "//a",
        "--single",
        "-n", "2",
    ])
    .capture();
    assert_ne!(0, result.status, "expected non-zero exit for --single -n 2");
    let complaint = format!("{}\n{}", result.stderr, result.combined);
    assert!(
        complaint.contains("--single") && complaint.contains("-n"),
        "expected error to mention --single and -n, got: {}",
        complaint,
    );
}

#[test]
fn invalid_projection_value_is_a_cli_error() {
    let result = command([
        "query",
        "-s", "<root><a/></root>",
        "-l", "xml",
        "-x", "//a",
        "-p", "bogus",
    ])
    .capture();
    assert_ne!(0, result.status, "expected non-zero exit for -p bogus");
    let complaint = format!("{}\n{}", result.stderr, result.combined);
    assert!(
        complaint.contains("bogus") || complaint.contains("invalid"),
        "expected error naming the invalid value, got: {}",
        complaint,
    );
}

#[test]
fn projection_with_gcc_is_a_cli_error() {
    let result = command([
        "query",
        "-s", "<root><a/></root>",
        "-l", "xml",
        "-x", "//a",
        "-p", "tree",
        "-f", "gcc",
    ])
    .capture();
    assert_ne!(0, result.status);
    let complaint = format!("{}\n{}", result.stderr, result.combined);
    assert!(
        complaint.contains("gcc"),
        "expected error mentioning gcc, got: {}",
        complaint,
    );
}

#[test]
fn projection_with_grouping_is_a_cli_error() {
    let result = command([
        "query",
        "-s", "<root><a/></root>",
        "-l", "xml",
        "-x", "//a",
        "-p", "tree",
        "-g", "file",
    ])
    .capture();
    assert_ne!(0, result.status);
    let complaint = format!("{}\n{}", result.stderr, result.combined);
    assert!(
        complaint.contains("group"),
        "expected error mentioning --group, got: {}",
        complaint,
    );
}

// ---------------------------------------------------------------------------
// Content-independence — same flags → same shape regardless of match count
// ---------------------------------------------------------------------------

#[test]
fn p_tree_f_xml_always_wraps_in_results_root() {
    for doc in [
        "<root></root>",                                       // 0 matches
        "<root><a>1</a></root>",                               // 1 match
        "<root><a>1</a><a>2</a><a>3</a></root>",               // 3 matches
    ] {
        let r = command([
            "query",
            "-s", doc,
            "-l", "xml",
            "-x", "//a",
            "-p", "tree",
            "-f", "xml",
        ])
        .capture();
        assert_eq!(0, r.status, "unexpected exit for {:?}: {}", doc, r.combined);
        assert!(
            r.stdout.contains("<results>") || r.stdout.contains("<results/>"),
            "missing <results> root for {} matches:\n{}",
            doc, r.stdout,
        );
    }
}

#[test]
fn p_tree_f_json_always_produces_a_json_array() {
    for doc in [
        "<root></root>",
        "<root><a>1</a></root>",
        "<root><a>1</a><a>2</a><a>3</a></root>",
    ] {
        let r = command([
            "query",
            "-s", doc,
            "-l", "xml",
            "-x", "//a",
            "-p", "tree",
            "-f", "json",
        ])
        .capture();
        assert_eq!(0, r.status);
        let parsed: serde_json::Value = serde_json::from_str(r.stdout.trim())
            .unwrap_or_else(|e| panic!("invalid JSON for {:?}: {e}\n{}", doc, r.stdout));
        assert!(parsed.is_array(), "expected array, got {:?}", parsed);
    }
}

#[test]
fn p_tree_single_empty_exits_non_zero_with_empty_stdout() {
    let r = command([
        "query",
        "-s", "<root></root>",
        "-l", "xml",
        "-x", "//a",
        "-p", "tree",
        "--single",
    ])
    .capture();
    assert_ne!(0, r.status);
    assert_eq!("", r.stdout.trim(), "expected empty stdout, got {:?}", r.stdout);
}

// ---------------------------------------------------------------------------
// Parseability — every -f xml/json/yaml projection stays machine-readable
// ---------------------------------------------------------------------------

#[test]
fn structured_projections_parse() {
    let doc = "<root><a>1</a><a>2</a></root>";
    let projections = ["tree", "value", "source", "lines", "results", "report",
                       "summary", "totals", "schema", "count"];

    // XML — shallow parseability: must have the <?xml prolog and exactly
    // one root element. Full DOM parsing would need an extra dev-dep.
    for p in projections {
        let r = command([
            "query", "-s", doc, "-l", "xml", "-x", "//a",
            "-p", p, "-f", "xml",
        ]).capture();
        assert_eq!(0, r.status, "xml -p {} failed: {}", p, r.combined);
        let trimmed = r.stdout.trim();
        assert!(
            trimmed.starts_with("<?xml"),
            "xml -p {} missing prolog:\n{}", p, r.stdout,
        );
        let after_prolog = trimmed.find("?>").map(|i| &trimmed[i + 2..]).unwrap_or("").trim();
        assert!(
            after_prolog.starts_with('<') && after_prolog.ends_with('>'),
            "xml -p {} missing root element:\n{}", p, r.stdout,
        );
    }

    // JSON
    for p in projections {
        let r = command([
            "query", "-s", doc, "-l", "xml", "-x", "//a",
            "-p", p, "-f", "json",
        ]).capture();
        assert_eq!(0, r.status, "json -p {} failed: {}", p, r.combined);
        serde_json::from_str::<serde_json::Value>(r.stdout.trim())
            .unwrap_or_else(|e| panic!("json -p {} did not parse: {e}\n{}", p, r.stdout));
    }

    // YAML
    for p in projections {
        let r = command([
            "query", "-s", doc, "-l", "xml", "-x", "//a",
            "-p", p, "-f", "yaml",
        ]).capture();
        assert_eq!(0, r.status, "yaml -p {} failed: {}", p, r.combined);
        serde_yaml::from_str::<serde_yaml::Value>(r.stdout.trim())
            .unwrap_or_else(|e| panic!("yaml -p {} did not parse: {e}\n{}", p, r.stdout));
    }
}

// ---------------------------------------------------------------------------
// Warning contracts — exact design text for the three canonical cases
// ---------------------------------------------------------------------------

#[test]
fn replacement_warning_names_dropped_fields_and_suggests_p_results() {
    let r = command([
        "query",
        "-s", "<root><a>1</a></root>",
        "-l", "xml",
        "-x", "//a",
        "-v", "tree,file",
        "-p", "tree",
    ])
    .capture();
    assert_eq!(0, r.status);
    assert!(r.stderr.starts_with("warning:"), "stderr: {}", r.stderr);
    assert!(r.stderr.contains("file"),        "stderr: {}", r.stderr);
    assert!(r.stderr.contains("-p tree"),     "stderr: {}", r.stderr);
    assert!(r.stderr.contains("-p results"),  "stderr: {}", r.stderr);
}

#[test]
fn unreachable_warning_for_p_summary_with_explicit_v() {
    let r = command([
        "query",
        "-s", "<root><a>1</a></root>",
        "-l", "xml",
        "-x", "//a",
        "-v", "tree,file",
        "-p", "summary",
    ])
    .capture();
    assert_eq!(0, r.status);
    assert!(r.stderr.contains("warning:"));
    assert!(r.stderr.contains("no per-match rendering"));
    assert!(r.stderr.contains("tree") && r.stderr.contains("file"));
}

#[test]
fn no_warning_when_p_overlaps_with_v() {
    let r = command([
        "query",
        "-s", "<root><a>1</a></root>",
        "-l", "xml",
        "-x", "//a",
        "-v", "tree",
        "-p", "tree",
    ])
    .capture();
    assert_eq!(0, r.status);
    assert!(r.stderr.is_empty(), "unexpected stderr: {}", r.stderr);
}

#[test]
fn no_warning_for_default_view_with_projection() {
    let r = command([
        "query",
        "-s", "<root><a>1</a></root>",
        "-l", "xml",
        "-x", "//a",
        "-p", "tree",
    ])
    .capture();
    assert_eq!(0, r.status);
    assert!(r.stderr.is_empty(), "unexpected stderr: {}", r.stderr);
}

#[test]
fn already_singular_warning_for_single_on_summary() {
    let r = command([
        "query",
        "-s", "<root><a>1</a></root>",
        "-l", "xml",
        "-x", "//a",
        "-p", "summary",
        "--single",
    ])
    .capture();
    assert_eq!(0, r.status);
    assert!(r.stderr.contains("warning:"));
    assert!(r.stderr.contains("already singular"));
}
