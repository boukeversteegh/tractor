use crate::common::{integration_dir, tractor_run, TempCopy};

fn set_dir() -> std::path::PathBuf {
    integration_dir("formats/set")
}

/// Run `tractor set` on a temp copy of sample.yaml, capture stdout,
/// normalize the temp path to the snapshot-relative path, compare to snapshot.
fn check_set_snapshot(desc: &str, snapshot_name: &str, extra_args: &[&str]) {
    let dir = set_dir();
    let tmp = TempCopy::new(&dir, &["yaml"]);

    let file_str = tmp.file_str("sample.yaml");
    let tmp_display = tmp.path().to_string_lossy().replace('\\', "/");

    let mut args = vec!["set", &file_str];
    args.extend_from_slice(extra_args);

    let r = tractor_run(tmp.path(), &args);

    // Normalize: replace temp path with the repo-relative path used in snapshots
    let mut actual = r.stdout.clone();
    actual = actual.replace(&format!("{}/", tmp_display), "tests/integration/formats/set/");
    let tmp_native = tmp.path().to_string_lossy().to_string();
    actual = actual.replace(&format!("{}/", tmp_native), "tests/integration/formats/set/");
    // Also replace bare temp path for file references without trailing slash
    actual = actual.replace(&file_str, "tests/integration/formats/set/sample.yaml");
    let actual = actual.trim_end();

    let snapshot_path = dir.join(snapshot_name);
    let expected = std::fs::read_to_string(&snapshot_path)
        .unwrap_or_else(|e| panic!("failed to read snapshot {}: {}", snapshot_path.display(), e));
    let expected = expected.trim_end();

    assert_eq!(
        actual, expected,
        "{desc}: snapshot mismatch\n  snapshot: {}\n  expected:\n{expected}\n  actual:\n{actual}",
        snapshot_path.display(),
    );
}

// ==========================================================================
// Text format snapshots
// ==========================================================================

#[test]
fn set_text_default() {
    check_set_snapshot(
        "text default (file:line + status + summary)",
        "set.txt",
        &["-x", "//database/host", "--value", "db.example.com", "--no-color"],
    );
}

#[test]
fn set_text_unchanged() {
    check_set_snapshot(
        "text unchanged (value already set)",
        "set-unchanged.txt",
        &["-x", "//database/host", "--value", "localhost", "--no-color"],
    );
}

// ==========================================================================
// Stdout mode snapshot
// ==========================================================================

#[test]
fn set_stdout_mode() {
    check_set_snapshot(
        "text stdout mode",
        "set-stdout.txt",
        &["-x", "//database/host", "--value", "db.example.com", "--stdout", "--no-color"],
    );
}

// ==========================================================================
// JSON format snapshot
// ==========================================================================

#[test]
fn set_json() {
    check_set_snapshot(
        "json default",
        "set.json",
        &["-x", "//database/host", "--value", "db.example.com", "-f", "json"],
    );
}

// ==========================================================================
// XML format snapshot
// ==========================================================================

#[test]
fn set_xml() {
    check_set_snapshot(
        "xml default",
        "set.xml",
        &["-x", "//database/host", "--value", "db.example.com", "-f", "xml", "--no-color"],
    );
}
