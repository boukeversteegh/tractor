//! B1 lint: ensure language modules use `semantic::*` constants — not
//! bare string literals — for the renamed-element side of
//! `map_element_name` arms.
//!
//! Self-bootstrapping: this test runs tractor on tractor's own
//! `src/languages/<lang>.rs` files using an XPath query to locate
//! every bare `string` literal that appears as an argument to a
//! `Some(...)` constructor inside the `map_element_name` function.
//! Those are the right-hand-side strings that should be replaced with
//! a named `semantic::FOO` constant.
//!
//! Run with `cargo test --test source_lint`.
//!
//! ## Allowlist
//!
//! Some short / single-use strings (e.g. shape-marker names like
//! "anonymous", "arrow") may not warrant a named constant. Annotate
//! the line with a trailing `// tractor-lint:allow` comment to skip
//! it. The comment must be on the same source line as the offending
//! string.
//!
//! ## Failure mode
//!
//! Reports each violation with its file:line:col location. The lint
//! is gated (currently a soft warning printed by default; flip
//! `LINT_GATE_HARD` once violations hit zero).

use std::path::{Path, PathBuf};
use std::process::Command;

mod support;

/// Each entry lists the candidate paths in priority order — Phase B2
/// splits each `<lang>.rs` into `<lang>/{mod,semantic,transform}.rs`,
/// so we accept either layout while the migration is in progress.
const LANGUAGE_FILES: &[&[&str]] = &[
    &["tractor/src/languages/csharp/transform.rs", "tractor/src/languages/csharp.rs"],
    &["tractor/src/languages/typescript/transform.rs", "tractor/src/languages/typescript.rs"],
    &["tractor/src/languages/python/transform.rs", "tractor/src/languages/python.rs"],
    &["tractor/src/languages/rust_lang/transform.rs", "tractor/src/languages/rust_lang.rs"],
    &["tractor/src/languages/go/transform.rs", "tractor/src/languages/go.rs"],
    &["tractor/src/languages/java/transform.rs", "tractor/src/languages/java.rs"],
    &["tractor/src/languages/php/transform.rs", "tractor/src/languages/php.rs"],
    &["tractor/src/languages/ruby/transform.rs", "tractor/src/languages/ruby.rs"],
    &["tractor/src/languages/tsql/transform.rs", "tractor/src/languages/tsql.rs"],
];

/// XPath that finds bare `string` literal arguments to `Some(...)`
/// constructors inside the `map_element_name` function body.
///
/// `//function[name='map_element_name']` — narrows to the right fn.
/// `//arm/value//call[name='Some']` — every `Some(...)` call on the
///   value (right-hand side) of a match arm. The `arm/value` step is
///   what excludes left-hand-side patterns like `"foo" =>`.
/// `//string` — bare string literals inside.
const LINT_XPATH: &str =
    "//function[name='map_element_name']//arm/value//call[name='Some']//string";

/// Allowlist comment marker: lines containing this trailing comment
/// are skipped by the lint.
const ALLOW_MARKER: &str = "tractor-lint:allow";

/// Hard gate: when true, violations cause test failure. Flipped on
/// after the initial audit pass drove the count to zero across all
/// nine language files.
const LINT_GATE_HARD: bool = true;

#[derive(Debug)]
struct Violation {
    file: String,
    line: u32,
    column: u32,
    text: String,
}

fn project_root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR must be set when running cargo test");
    let mut path = PathBuf::from(manifest);
    // CARGO_MANIFEST_DIR points to tractor/tractor/; walk up to the
    // workspace root.
    if path.ends_with("tractor") {
        path.pop();
    }
    path
}

fn tractor_binary() -> PathBuf {
    if let Some(p) = std::env::var_os("CARGO_BIN_EXE_tractor") {
        return PathBuf::from(p);
    }
    let exe = std::env::current_exe().expect("test binary path");
    let profile_dir = exe
        .parent()
        .and_then(Path::parent)
        .expect("test binary should live in target/<profile>/deps");
    let mut p = profile_dir.join("tractor");
    if cfg!(windows) {
        p.set_extension("exe");
    }
    p
}

/// Run tractor with the lint XPath on a single source file and return
/// the violations.
fn run_lint(file: &Path) -> Result<Vec<Violation>, String> {
    let bin = tractor_binary();
    if !bin.exists() {
        return Err(format!(
            "tractor binary not found at {}; run `cargo build` first",
            bin.display()
        ));
    }

    let output = Command::new(&bin)
        .arg("query")
        .arg(file)
        .arg("-x")
        .arg(LINT_XPATH)
        .arg("-v")
        .arg("file,line,column,value")
        .arg("-p")
        .arg("results")
        .arg("-f")
        .arg("json")
        .arg("--no-color")
        .output()
        .map_err(|e| format!("failed to spawn tractor: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "tractor exited with {}: stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    let json: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("malformed JSON: {e}"))?;

    let arr = match json.as_array() {
        Some(a) => a,
        None => return Ok(Vec::new()),
    };

    let mut violations = Vec::new();
    for entry in arr {
        let file = entry["file"].as_str().unwrap_or("").to_string();
        let line = entry["line"].as_u64().unwrap_or(0) as u32;
        let column = entry["column"].as_u64().unwrap_or(0) as u32;
        let text = entry["value"].as_str().unwrap_or("").to_string();
        violations.push(Violation { file, line, column, text });
    }
    Ok(violations)
}

/// Read a single line (1-indexed) from the source file, used for
/// allowlist detection.
fn read_line(file: &Path, line_num: u32) -> Option<String> {
    let content = std::fs::read_to_string(file).ok()?;
    content.lines().nth((line_num as usize).saturating_sub(1)).map(|s| s.to_string())
}

#[test]
fn map_element_name_uses_semantic_constants() {
    let bin = tractor_binary();
    if !bin.exists() {
        // Build it on demand so the test is self-contained when run
        // before any prior `cargo build`.
        let _ = Command::new(env!("CARGO"))
            .args(["build", "--bin", "tractor"])
            .status();
    }

    let root = project_root();
    let mut all_violations: Vec<Violation> = Vec::new();
    let mut errors = Vec::new();

    for candidates in LANGUAGE_FILES {
        let mut found: Option<(PathBuf, &str)> = None;
        for rel in *candidates {
            let path = root.join(rel);
            if path.is_file() {
                found = Some((path, *rel));
                break;
            }
        }
        let (path, rel) = match found {
            Some(p) => p,
            None => {
                errors.push(format!(
                    "missing language file: tried {}",
                    candidates.join(", "),
                ));
                continue;
            }
        };
        match run_lint(&path) {
            Ok(violations) => {
                for v in violations {
                    // Skip allowlisted lines.
                    if let Some(line_text) = read_line(&path, v.line) {
                        if line_text.contains(ALLOW_MARKER) {
                            continue;
                        }
                    }
                    all_violations.push(v);
                }
            }
            Err(e) => errors.push(format!("{}: {}", rel, e)),
        }
    }

    if !errors.is_empty() {
        panic!("lint setup errors:\n  - {}", errors.join("\n  - "));
    }

    if all_violations.is_empty() {
        return;
    }

    let mut report = format!(
        "B1 lint: {} bare string literal(s) on the RHS of map_element_name arms.\n\
         These should be replaced with `semantic::FOO` constants.\n\
         Annotate intentional exceptions with `// {}` on the same line.\n\n",
        all_violations.len(),
        ALLOW_MARKER,
    );
    for v in &all_violations {
        report.push_str(&format!("  {}:{}:{}  {}\n", v.file, v.line, v.column, v.text));
    }

    if LINT_GATE_HARD {
        panic!("{}", report);
    } else {
        eprintln!("{}", report);
    }
}
