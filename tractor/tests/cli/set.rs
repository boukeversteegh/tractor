use crate::common::{repo_root, tractor_run, tractor_run_stdin, tractor_fails, TempFile};

// ==========================================================================
// Set (YAML)
// ==========================================================================

#[test]
fn set_single_yaml_value() {
    let f = TempFile::new("test.yaml", "name: my-app\ndatabase:\n  host: localhost\n  port: 5432");
    let r = tractor_run(
        repo_root().as_path(),
        &["set", &f.path_str(), "-x", "//database/host", "--value", "db.example.com"],
    );
    assert!(r.success, "set should succeed: {}", r.stderr);
    assert_eq!(
        f.read(),
        "name: my-app\ndatabase:\n  host: db.example.com\n  port: 5432",
        "single YAML value should be set"
    );
}

#[test]
fn set_multiple_yaml_values() {
    let f = TempFile::new(
        "test.yaml",
        "servers:\n  - name: web-1\n    port: 8080\n  - name: web-2\n    port: 8080\n  - name: web-3\n    port: 9090",
    );
    let r = tractor_run(
        repo_root().as_path(),
        &["set", &f.path_str(), "-x", "//servers/port[.='8080']", "--value", "3000"],
    );
    assert!(r.success, "set should succeed: {}", r.stderr);
    assert_eq!(
        f.read(),
        "servers:\n  - name: web-1\n    port: 3000\n  - name: web-2\n    port: 3000\n  - name: web-3\n    port: 9090",
        "multiple YAML values should be set"
    );
}

#[test]
fn set_respects_limit() {
    let f = TempFile::new(
        "test.yaml",
        "items:\n  - value: old\n  - value: old\n  - value: old",
    );
    let r = tractor_run(
        repo_root().as_path(),
        &["set", &f.path_str(), "-x", "//items/value[.='old']", "-n", "1", "--value", "new"],
    );
    assert!(r.success, "set should succeed: {}", r.stderr);
    assert_eq!(
        f.read(),
        "items:\n  - value: new\n  - value: old\n  - value: old",
        "set should respect --limit"
    );
}

// ==========================================================================
// Set (JSON)
// ==========================================================================

#[test]
fn set_json_string_value() {
    let f = TempFile::new(
        "test.json",
        "{\n  \"database\": {\n    \"host\": \"localhost\",\n    \"port\": 5432\n  }\n}",
    );
    let r = tractor_run(
        repo_root().as_path(),
        &["set", &f.path_str(), "-x", "//database/host", "--value", "db.example.com"],
    );
    assert!(r.success, "set should succeed: {}", r.stderr);
    assert_eq!(
        f.read(),
        "{\n  \"database\": {\n    \"host\": \"db.example.com\",\n    \"port\": 5432\n  }\n}",
        "JSON string value should be set"
    );
}

// ==========================================================================
// Set (stdout mode)
// ==========================================================================

#[test]
fn set_stdin_writes_to_stdout() {
    let r = tractor_run_stdin(
        repo_root().as_path(),
        &["set", "-l", "yaml", "-x", "//name", "--value", "newvalue"],
        "name: test",
    );
    assert!(r.success, "set with stdin should succeed: {}", r.stderr);
    assert_eq!(r.stdout.trim_end(), "name: newvalue", "set with stdin should write to stdout");
}

#[test]
fn set_stdout_flag_does_not_modify_file() {
    let f = TempFile::new("test.yaml", "host: localhost");
    let r = tractor_run(
        repo_root().as_path(),
        &["set", &f.path_str(), "-x", "//host", "--value", "example.com", "--stdout"],
    );
    assert!(r.success, "--stdout should succeed: {}", r.stderr);
    assert_eq!(r.stdout.trim_end(), "host: example.com", "--stdout should output modified content");
    assert_eq!(f.read(), "host: localhost", "--stdout should not modify the file");
}

// ==========================================================================
// Set (error cases)
// ==========================================================================

#[test]
fn set_without_xpath_fails() {
    let f = TempFile::new("test.json", "{}");
    tractor_fails(repo_root().as_path(), &["set", &f.path_str(), "--value", "foo"]);
}

#[test]
fn set_with_no_matches_succeeds() {
    let f = TempFile::new("test.json", "{}");
    let r = tractor_run(
        repo_root().as_path(),
        &["set", &f.path_str(), "-x", "//nonexistent", "--value", "x"],
    );
    assert!(r.success, "set with no matches should succeed");
}
