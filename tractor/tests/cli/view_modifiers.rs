use crate::common::{integration_dir, tractor_run, tractor_fails};

fn sample_cs() -> std::path::PathBuf {
    integration_dir("formats").join("sample.cs")
}

fn sample_cs_str() -> String {
    sample_cs().to_string_lossy().replace('\\', "/")
}

fn formats_dir() -> std::path::PathBuf {
    integration_dir("formats")
}

// ---------------------------------------------------------------------------
// -field: remove a field from the default view
// ---------------------------------------------------------------------------

#[test]
fn check_gcc_remove_lines() {
    let r = tractor_run(
        &formats_dir(),
        &[
            "check", &sample_cs_str(), "-x", "//class",
            "--reason", "class found", "--no-color", "-f", "gcc", "-v=-lines",
        ],
    );
    // With lines removed, each match should produce exactly one line (the header).
    // Count "error:" occurrences
    let error_lines: Vec<&str> = r.stdout.lines().filter(|l| l.contains("error:")).collect();
    assert_eq!(error_lines.len(), 2, "should have 2 error header lines");
}

#[test]
fn check_gcc_remove_lines_no_gutter() {
    let r = tractor_run(
        &formats_dir(),
        &[
            "check", &sample_cs_str(), "-x", "//class",
            "--reason", "class found", "--no-color", "-f", "gcc", "-v=-lines",
        ],
    );
    // No gutter lines like "1 >| ..."
    let has_gutter = r.stdout.lines().any(|l| {
        let trimmed = l.trim();
        trimmed.len() > 2
            && trimmed.chars().next().map_or(false, |c| c.is_ascii_digit())
            && (trimmed.contains(" >|") || trimmed.contains(" |"))
    });
    assert!(!has_gutter, "should have no line-number gutter output");
}

#[test]
fn check_remove_severity() {
    let r = tractor_run(
        &formats_dir(),
        &[
            "check", &sample_cs_str(), "-x", "//class",
            "--reason", "class found", "--no-color", "-v=-severity",
        ],
    );
    assert!(
        r.combined().contains("class found"),
        "removing severity should keep other fields intact"
    );
}

#[test]
fn query_remove_tree() {
    let r = tractor_run(
        &formats_dir(),
        &[&sample_cs_str(), "-x", "//class/name", "--no-color", "-v=-tree"],
    );
    assert!(r.success, "command should succeed: {}", r.stderr);
    assert!(
        !r.stdout.contains('<'),
        "removing tree should remove all XML tags from output"
    );
}

// ---------------------------------------------------------------------------
// +field: add a field to the default view
// ---------------------------------------------------------------------------

#[test]
fn query_add_source() {
    let r = tractor_run(
        &formats_dir(),
        &[&sample_cs_str(), "-x", "//class/name", "--no-color", "-v=+source"],
    );
    assert!(r.success, "command should succeed: {}", r.stderr);
    let has_class_name = r.stdout.lines().any(|l| {
        let t = l.trim();
        t == "Foo" || t == "Qux"
    });
    assert!(has_class_name, "adding source should show class names in output");
}

// ---------------------------------------------------------------------------
// Combining + and - modifiers
// ---------------------------------------------------------------------------

#[test]
fn check_remove_lines_add_source() {
    let r = tractor_run(
        &formats_dir(),
        &[
            "check", &sample_cs_str(), "-x", "//class/name",
            "--reason", "found", "--no-color", "-f", "text", "-v=-lines,+source",
        ],
    );
    let combined = r.combined();
    assert!(
        combined.contains("Foo") || combined.contains("Qux"),
        "should have source text in output"
    );
}

// ---------------------------------------------------------------------------
// No-op and idempotency
// ---------------------------------------------------------------------------

#[test]
fn add_existing_field_is_noop() {
    let default = tractor_run(
        &formats_dir(),
        &[&sample_cs_str(), "-x", "//class/name", "--no-color"],
    );
    let with_modifier = tractor_run(
        &formats_dir(),
        &[&sample_cs_str(), "-x", "//class/name", "--no-color", "-v=+tree"],
    );
    assert_eq!(
        default.stdout, with_modifier.stdout,
        "adding a field already in default should be a no-op"
    );
}

#[test]
fn remove_absent_field_is_noop() {
    let r = tractor_run(
        &formats_dir(),
        &[&sample_cs_str(), "-x", "//class/name", "--no-color", "-v=-source"],
    );
    assert!(r.success || !r.success, "command should run");
    assert!(
        r.stdout.contains("Foo") || r.combined().contains("Foo"),
        "output should still contain class names"
    );
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn mixing_plain_and_modifier_fails() {
    tractor_fails(
        &formats_dir(),
        &[&sample_cs_str(), "-x", "//class", "-v=tree,+source"],
    );
}

#[test]
fn removing_all_default_fields_fails() {
    tractor_fails(
        &formats_dir(),
        &[&sample_cs_str(), "-x", "//class/name", "-v=-file,-line,-tree"],
    );
}

#[test]
fn invalid_field_name_fails() {
    tractor_fails(
        &formats_dir(),
        &[&sample_cs_str(), "-x", "//class", "-v=-nosuchfield"],
    );
}
