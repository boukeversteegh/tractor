use crate::common::{integration_dir, tractor_run, tractor_fails};

fn sample_cs_str() -> String {
    integration_dir("formats").join("sample.cs").to_string_lossy().replace('\\', "/")
}

fn formats_dir() -> std::path::PathBuf {
    integration_dir("formats")
}

// ---------------------------------------------------------------------------
// -field: remove a field from the default view
// ---------------------------------------------------------------------------

#[test]
fn remove_field() {
    let cs = sample_cs_str();
    let dir = formats_dir();

    // -v=-lines removes source line blocks from gcc output
    let r = tractor_run(&dir, &["check", &cs, "-x", "//class", "--reason", "class found", "--no-color", "-f", "gcc", "-v=-lines"]);
    let error_lines: Vec<&str> = r.stdout.lines().filter(|l| l.contains("error:")).collect();
    assert_eq!(error_lines.len(), 2, "should have 2 error header lines");

    // No gutter lines (e.g. "1 >| ...")
    let has_gutter = r.stdout.lines().any(|l| {
        let t = l.trim();
        t.len() > 2 && t.chars().next().map_or(false, |c| c.is_ascii_digit()) && (t.contains(" >|") || t.contains(" |"))
    });
    assert!(!has_gutter, "should have no line-number gutter output");

    // -v=-severity keeps other fields intact
    let r = tractor_run(&dir, &["check", &cs, "-x", "//class", "--reason", "class found", "--no-color", "-v=-severity"]);
    assert!(r.combined().contains("class found"), "removing severity should keep other fields");

    // -v=-tree removes XML tags from query output
    let r = tractor_run(&dir, &[&cs, "-x", "//class/name", "--no-color", "-v=-tree"]);
    assert!(r.success);
    assert!(!r.stdout.contains('<'), "removing tree should remove all XML tags");
}

// ---------------------------------------------------------------------------
// +field: add a field to the default view
// ---------------------------------------------------------------------------

#[test]
fn add_field() {
    let cs = sample_cs_str();
    let dir = formats_dir();

    // +source adds source text to query output
    let r = tractor_run(&dir, &[&cs, "-x", "//class/name", "--no-color", "-v=+source"]);
    assert!(r.success);
    assert!(r.stdout.lines().any(|l| { let t = l.trim(); t == "Foo" || t == "Qux" }), "should show class names");

    // Combining: -lines,+source
    let r = tractor_run(&dir, &["check", &cs, "-x", "//class/name", "--reason", "found", "--no-color", "-f", "text", "-v=-lines,+source"]);
    let c = r.combined();
    assert!(c.contains("Foo") || c.contains("Qux"), "should have source text in output");
}

// ---------------------------------------------------------------------------
// No-op and idempotency
// ---------------------------------------------------------------------------

#[test]
fn noop_modifiers() {
    let cs = sample_cs_str();
    let dir = formats_dir();

    // Adding a field already in default is a no-op
    let default = tractor_run(&dir, &[&cs, "-x", "//class/name", "--no-color"]);
    let with_mod = tractor_run(&dir, &[&cs, "-x", "//class/name", "--no-color", "-v=+tree"]);
    assert_eq!(default.stdout, with_mod.stdout, "+tree should be no-op");

    // Removing a field not in default is a no-op
    let r = tractor_run(&dir, &[&cs, "-x", "//class/name", "--no-color", "-v=-source"]);
    assert!(r.stdout.contains("Foo") || r.combined().contains("Foo"), "output should still contain class names");
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn modifier_errors() {
    let cs = sample_cs_str();
    let dir = formats_dir();
    tractor_fails(&dir, &[&cs, "-x", "//class", "-v=tree,+source"]);          // mixing plain and modifier
    tractor_fails(&dir, &[&cs, "-x", "//class/name", "-v=-file,-line,-tree"]); // removing all defaults
    tractor_fails(&dir, &[&cs, "-x", "//class", "-v=-nosuchfield"]);           // invalid field name
}
