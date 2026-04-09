use crate::common::{repo_root, tractor_run, tractor_run_stdin, tractor_fails, TempFile};

// ==========================================================================
// Update (YAML)
// ==========================================================================

#[test]
fn update_single_yaml_value() {
    let f = TempFile::new("test.yaml", "name: my-app\ndatabase:\n  host: localhost\n  port: 5432");
    let r = tractor_run(
        repo_root().as_path(),
        &["update", &f.path_str(), "-x", "//database/host", "--value", "db.example.com"],
    );
    assert!(r.success, "update should succeed: {}", r.stderr);
    assert_eq!(
        f.read(),
        "name: my-app\ndatabase:\n  host: db.example.com\n  port: 5432",
    );
}

#[test]
fn update_multiple_yaml_values() {
    let f = TempFile::new(
        "test.yaml",
        "servers:\n  - name: web-1\n    port: 8080\n  - name: web-2\n    port: 8080\n  - name: web-3\n    port: 9090",
    );
    let r = tractor_run(
        repo_root().as_path(),
        &["update", &f.path_str(), "-x", "//servers/port[.='8080']", "--value", "3000"],
    );
    assert!(r.success, "update should succeed: {}", r.stderr);
    assert_eq!(
        f.read(),
        "servers:\n  - name: web-1\n    port: 3000\n  - name: web-2\n    port: 3000\n  - name: web-3\n    port: 9090",
    );
}

#[test]
fn update_respects_limit() {
    let f = TempFile::new(
        "test.yaml",
        "items:\n  - value: old\n  - value: old\n  - value: old",
    );
    let r = tractor_run(
        repo_root().as_path(),
        &["update", &f.path_str(), "-x", "//items/value[.='old']", "-n", "1", "--value", "new"],
    );
    assert!(r.success, "update should succeed: {}", r.stderr);
    assert_eq!(
        f.read(),
        "items:\n  - value: new\n  - value: old\n  - value: old",
    );
}

#[test]
fn update_no_match_fails_and_does_not_create() {
    let f = TempFile::new("test.yaml", "name: my-app");
    let r = tractor_run(
        repo_root().as_path(),
        &["update", &f.path_str(), "-x", "//database/host", "--value", "localhost"],
    );
    assert!(!r.success, "update with no match should fail");
    assert_eq!(f.read(), "name: my-app", "file should not be modified");
}

#[test]
fn update_partial_path_fails_and_does_not_create() {
    let f = TempFile::new("test.yaml", "database:\n  host: localhost");
    let r = tractor_run(
        repo_root().as_path(),
        &["update", &f.path_str(), "-x", "//database/port", "--value", "5432"],
    );
    assert!(!r.success, "update with partial path should fail");
    assert_eq!(f.read(), "database:\n  host: localhost", "file should not be modified");
}

// ==========================================================================
// Update (JSON)
// ==========================================================================

#[test]
fn update_json_string_value() {
    let f = TempFile::new(
        "test.json",
        "{\n  \"database\": {\n    \"host\": \"localhost\",\n    \"port\": 5432\n  }\n}",
    );
    let r = tractor_run(
        repo_root().as_path(),
        &["update", &f.path_str(), "-x", "//database/host", "--value", "db.example.com"],
    );
    assert!(r.success, "update should succeed: {}", r.stderr);
    assert_eq!(
        f.read(),
        "{\n  \"database\": {\n    \"host\": \"db.example.com\",\n    \"port\": 5432\n  }\n}",
    );
}

#[test]
fn update_json_no_match_fails_and_does_not_create() {
    let f = TempFile::new(
        "test.json",
        "{\n  \"name\": \"my-app\"\n}",
    );
    let r = tractor_run(
        repo_root().as_path(),
        &["update", &f.path_str(), "-x", "//database/host", "--value", "localhost"],
    );
    assert!(!r.success, "update with no match should fail");
    assert_eq!(
        f.read(),
        "{\n  \"name\": \"my-app\"\n}",
        "file should not be modified",
    );
}

// ==========================================================================
// Update (error cases)
// ==========================================================================

#[test]
fn update_without_xpath_fails() {
    let f = TempFile::new("test.json", "{}");
    tractor_fails(repo_root().as_path(), &["update", &f.path_str(), "--value", "foo"]);
}

#[test]
fn update_with_stdin_fails() {
    let r = tractor_run_stdin(
        repo_root().as_path(),
        &["update", "--lang", "yaml", "-x", "//name", "--value", "new"],
        "name: test",
    );
    assert!(!r.success, "update with stdin should fail");
}

#[test]
fn update_with_no_matches_fails() {
    let f = TempFile::new("test.json", "{}");
    let r = tractor_run(
        repo_root().as_path(),
        &["update", &f.path_str(), "-x", "//nonexistent", "--value", "x"],
    );
    assert!(!r.success, "update with no matches should fail");
}
