use crate::common::{integration_dir, tractor_run, TempCopy};

fn run_dir() -> std::path::PathBuf {
    integration_dir("run")
}

/// Run `tractor run <args>` in the run fixture dir, capture output,
/// normalize absolute paths to fixture-relative, and compare.
fn run_and_check(desc: &str, expected_exit: bool, expected_output: &str, args: &[&str]) {
    let dir = run_dir();
    let dir_display = dir.to_string_lossy().replace('\\', "/");

    let mut full_args = vec!["run"];
    full_args.extend_from_slice(args);
    full_args.push("--no-color");

    let r = tractor_run(&dir, &full_args);

    // Merge stdout and stderr, then normalize paths
    let mut actual = r.combined();
    actual = actual.replace(&format!("{}/", dir_display), "");
    // Also handle native path separators on Windows
    let dir_native = dir.to_string_lossy().to_string();
    actual = actual.replace(&format!("{}/", dir_native), "");
    // Trim trailing whitespace
    let actual = actual.trim_end();

    assert_eq!(
        r.success, expected_exit,
        "{desc}: expected exit {}, got {}\n  output: {actual}",
        if expected_exit { "success" } else { "failure" },
        if r.success { "success" } else { "failure" },
    );

    assert_eq!(
        actual, expected_output,
        "{desc}: output mismatch\n  expected:\n{expected_output}\n  actual:\n{actual}",
    );
}

/// Run `tractor run` on a temporary copy of fixtures (for set operations that modify files).
fn run_set_and_check(
    desc: &str,
    expected_exit: bool,
    expected_output: &str,
    config: &str,
    extra_args: &[&str],
) {
    let dir = run_dir();
    let tmp = TempCopy::new(&dir, &["json", "yaml", "yml"]);

    // Also copy the config file
    let config_src = dir.join(config);
    let config_dest = tmp.path().join(config);
    if let Some(parent) = config_dest.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::copy(&config_src, &config_dest).expect("failed to copy config");

    let config_str = config_dest.to_string_lossy().replace('\\', "/");
    let tmp_display = tmp.path().to_string_lossy().replace('\\', "/");

    let mut full_args = vec!["run", &config_str];
    full_args.extend_from_slice(extra_args);
    full_args.push("--no-color");

    let r = tractor_run(tmp.path(), &full_args);

    // Normalize temp paths
    let mut actual = r.combined();
    actual = actual.replace(&format!("{}/", tmp_display), "");
    let tmp_native = tmp.path().to_string_lossy().to_string();
    actual = actual.replace(&format!("{}/", tmp_native), "");
    let actual = actual.trim_end();

    assert_eq!(
        r.success, expected_exit,
        "{desc}: expected exit {}, got {}\n  output: {actual}",
        if expected_exit { "success" } else { "failure" },
        if r.success { "success" } else { "failure" },
    );

    assert_eq!(
        actual, expected_output,
        "{desc}: output mismatch\n  expected:\n{expected_output}\n  actual:\n{actual}",
    );
}

// ==========================================================================
// Check operations
// ==========================================================================

#[test]
fn multirule_check() {
    run_and_check(
        "multirule check finds violations with correct severity",
        false,
        "settings.yaml:3:10: error: debug should be disabled in production\n\
         3 |   debug: true\n\
         \x20            ^~~~\n\
         \n\
         settings.yaml:4:14: warning: log level should not be debug in production\n\
         4 |   log_level: debug\n\
         \x20                ^~~~~\n\
         \n\
         1 error in 1 file",
        &["check-multirule.yaml"],
    );
}

#[test]
fn multifile_check() {
    run_and_check(
        "multifile check scans multiple files",
        false,
        "settings.yaml:3:10: error: debug mode must be disabled\n\
         3 |   debug: true\n\
         \x20            ^~~~\n\
         \n\
         1 error in 1 file",
        &["check-multifile.yaml"],
    );
}

// ==========================================================================
// Set operations
// ==========================================================================

#[test]
fn set_applies_mappings() {
    run_set_and_check(
        "set applies mappings to files",
        true,
        "app-config.json: updated\nupdated 1 file",
        "set-config.yaml",
        &[],
    );
}

// ==========================================================================
// Scope intersection
// ==========================================================================

#[test]
fn scope_intersection_narrow() {
    run_and_check(
        "root ∩ operation narrows to intersection",
        true,
        "scope-intersection/frontend/config.yml:1:8: warning: debug must be disabled\n\
         1 | debug: true\n\
         \x20          ^~~~\n\
         \n\
         1 warning in 1 file",
        &["scope-intersection/intersect-narrow.yaml"],
    );
}

#[test]
fn scope_intersection_fallback() {
    run_and_check(
        "root used as base when operation has no files",
        true,
        "",
        &["scope-intersection/intersect-fallback.yaml"],
    );
}

#[test]
fn scope_intersection_disjoint() {
    run_and_check(
        "disjoint root and operation yields empty set",
        true,
        "",
        &["scope-intersection/intersect-disjoint.yaml"],
    );
}

// ==========================================================================
// Mixed operations
// ==========================================================================

#[test]
fn mixed_check_set() {
    run_set_and_check(
        "mixed check+set succeeds when check passes",
        true,
        "app-config.json: updated\nupdated 1 file",
        "mixed-ops.yaml",
        &[],
    );
}

// ==========================================================================
// Absolute CLI paths
// ==========================================================================

fn abs_path_test(desc: &str, config: &str, file_path: &str, expected_output: &str) {
    let dir = run_dir();
    let dir_display = dir.to_string_lossy().replace('\\', "/");

    let full_args = vec!["run", config, file_path, "--no-color"];
    let r = tractor_run(&dir, &full_args);

    let mut actual = r.combined();
    actual = actual.replace(&format!("{}/", dir_display), "");
    let dir_native = dir.to_string_lossy().to_string();
    actual = actual.replace(&format!("{}/", dir_native), "");
    let actual = actual.trim_end();

    assert_eq!(actual, expected_output, "{desc}: output mismatch");
}

#[test]
fn absolute_cli_path_per_rule_include() {
    let dir = run_dir();
    let abs_file = dir.join("absolute-paths/config.yml").to_string_lossy().replace('\\', "/");
    abs_path_test(
        "absolute CLI path + per-rule include matches",
        "absolute-paths/check-per-rule-include.yaml",
        &abs_file,
        "absolute-paths/config.yml:1:8: warning: debug must be disabled\n\
         1 | debug: true\n\
         \x20          ^~~~\n\
         \n\
         1 warning in 1 file",
    );
}

#[test]
fn absolute_cli_path_per_rule_exclude() {
    let dir = run_dir();
    let abs_file = dir.join("absolute-paths/config.yml").to_string_lossy().replace('\\', "/");
    abs_path_test(
        "absolute CLI path + per-rule exclude filters out",
        "absolute-paths/check-per-rule-exclude.yaml",
        &abs_file,
        "",
    );
}

#[test]
fn absolute_cli_path_root_files() {
    let dir = run_dir();
    let abs_file = dir.join("absolute-paths/config.yml").to_string_lossy().replace('\\', "/");
    abs_path_test(
        "absolute CLI path + root files intersection works",
        "absolute-paths/check-root-files.yaml",
        &abs_file,
        "absolute-paths/config.yml:1:8: warning: debug must be disabled\n\
         1 | debug: true\n\
         \x20          ^~~~\n\
         \n\
         1 warning in 1 file",
    );
}

#[test]
fn absolute_cli_path_root_exclude() {
    let dir = run_dir();
    let abs_file = dir.join("absolute-paths/config.yml").to_string_lossy().replace('\\', "/");
    abs_path_test(
        "absolute CLI path + root exclude filters out",
        "absolute-paths/check-root-exclude.yaml",
        &abs_file,
        "",
    );
}

#[test]
fn dot_relative_cli_path_per_rule_include() {
    abs_path_test(
        "dot-relative CLI path + per-rule include matches",
        "absolute-paths/check-per-rule-include.yaml",
        "./absolute-paths/config.yml",
        "absolute-paths/config.yml:1:8: warning: debug must be disabled\n\
         1 | debug: true\n\
         \x20          ^~~~\n\
         \n\
         1 warning in 1 file",
    );
}

#[test]
fn dot_relative_cli_path_per_rule_exclude() {
    abs_path_test(
        "dot-relative CLI path + per-rule exclude filters out",
        "absolute-paths/check-per-rule-exclude.yaml",
        "./absolute-paths/config.yml",
        "",
    );
}

#[test]
fn dot_relative_cli_path_root_files() {
    abs_path_test(
        "dot-relative CLI path + root files intersection works",
        "absolute-paths/check-root-files.yaml",
        "./absolute-paths/config.yml",
        "absolute-paths/config.yml:1:8: warning: debug must be disabled\n\
         1 | debug: true\n\
         \x20          ^~~~\n\
         \n\
         1 warning in 1 file",
    );
}

#[test]
fn dot_relative_cli_path_root_exclude() {
    abs_path_test(
        "dot-relative CLI path + root exclude filters out",
        "absolute-paths/check-root-exclude.yaml",
        "./absolute-paths/config.yml",
        "",
    );
}

// ==========================================================================
// Mixed language rules
// ==========================================================================

#[test]
fn mixed_language_both_js_and_md() {
    run_and_check(
        "mixed-language: both JS and MD rules find violations",
        false,
        "mixed-language/sample.js:1:1: error: TODO comment found\n\
         1 | // TODO: Fix this code\n\
         \x20   ^~~~~~~~~~~~~~~~~~~~~~\n\
         \n\
         mixed-language/todo-doc.md:3:1: warning: TODO comment found\n\
         3 >| <!-- TODO: Complete this section -->\n\
         4 >| \n\
         \n\
         1 error in 2 files",
        &["mixed-language/mixed-rules.yaml"],
    );
}

#[test]
fn mixed_language_js_only() {
    run_and_check(
        "mixed-language: JS-only rules skip MD files",
        false,
        "mixed-language/sample.js:1:1: error: TODO comment found\n\
         1 | // TODO: Fix this code\n\
         \x20   ^~~~~~~~~~~~~~~~~~~~~~\n\
         \n\
         1 error in 1 file",
        &["mixed-language/js-only-rules.yaml"],
    );
}

#[test]
fn mixed_language_md_only() {
    run_and_check(
        "mixed-language: MD-only rules skip JS files",
        true,
        "mixed-language/todo-doc.md:3:1: warning: TODO comment found\n\
         3 >| <!-- TODO: Complete this section -->\n\
         4 >| \n\
         \n\
         1 warning in 1 file",
        &["mixed-language/md-only-rules.yaml"],
    );
}

#[test]
fn mixed_language_auto_detect() {
    run_and_check(
        "mixed-language: auto-detect uses file extension",
        false,
        "mixed-language/sample.js:1:1: error: TODO comment found\n\
         1 | // TODO: Fix this code\n\
         \x20   ^~~~~~~~~~~~~~~~~~~~~~\n\
         \n\
         1 error in 1 file",
        &["mixed-language/auto-detect.yaml"],
    );
}

#[test]
fn mixed_language_same_lang_rules() {
    run_and_check(
        "mixed-language: multiple rules for same language",
        false,
        "mixed-language/sample.js:1:1: error: TODO comment found\n\
         1 | // TODO: Fix this code\n\
         \x20   ^~~~~~~~~~~~~~~~~~~~~~\n\
         \n\
         mixed-language/sample.js:3:5: warning: No console.log calls allowed\n\
         3 |     console.log(\"Hello\");\n\
         \x20       ^~~~~~~~~~~~~~~~~~~~\n\
         \n\
         mixed-language/sample.js:7:5: warning: No console.log calls allowed\n\
         7 |     console.log(\"Goodbye\");\n\
         \x20       ^~~~~~~~~~~~~~~~~~~~~~\n\
         \n\
         1 error in 1 file",
        &["mixed-language/same-lang-rules.yaml"],
    );
}

#[test]
fn mixed_language_alias() {
    run_and_check(
        "mixed-language: language alias (js -> javascript)",
        false,
        "mixed-language/sample.js:1:1: error: TODO comment found\n\
         1 | // TODO: Fix this code\n\
         \x20   ^~~~~~~~~~~~~~~~~~~~~~\n\
         \n\
         1 error in 1 file",
        &["mixed-language/lang-alias.yaml"],
    );
}

#[test]
fn mixed_language_three_langs() {
    run_and_check(
        "mixed-language: three different languages",
        false,
        "mixed-language/config.yaml:3:10: error: Debug mode must be disabled\n\
         3 |   debug: true\n\
         \x20            ^~~~\n\
         \n\
         mixed-language/sample.js:1:1: error: TODO comment found\n\
         1 | // TODO: Fix this code\n\
         \x20   ^~~~~~~~~~~~~~~~~~~~~~\n\
         \n\
         mixed-language/todo-doc.md:3:1: warning: TODO comment found\n\
         3 >| <!-- TODO: Complete this section -->\n\
         4 >| \n\
         \n\
         2 errors in 3 files",
        &["mixed-language/three-langs.yaml"],
    );
}
