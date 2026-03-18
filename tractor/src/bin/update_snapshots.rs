//! Generate/update or check XML snapshots for integration tests.
//!
//! Walks `tests/integration/*/`, finds source files by known extensions,
//! and runs tractor on each to produce `.xml` and `.raw.xml` snapshots.
//!
//! Also handles output-format combination snapshots in
//! `tests/integration/formats/snapshots/`.
//!
//! Usage:
//!   cargo run --release --bin update-snapshots          # update snapshots
//!   cargo run --release --bin update-snapshots -- --check  # check only (no writes)

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::{self, Command};

/// File extensions to skip (not source fixtures).
const SKIP_EXTENSIONS: &[&str] = &["xml", "sh", "md"];

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
    // -f yaml
    ("yaml/query.yaml", &[
        "query", "tests/integration/formats/sample.cs", "-x", "class", "-f", "yaml",
    ]),
    ("yaml/check.yaml", &[
        "check", "tests/integration/formats/sample.cs", "-x", "class",
        "--reason", "class found", "-f", "yaml",
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
];

fn main() {
    let check_mode = std::env::args().any(|a| a == "--check");

    let tests_dir = Path::new("tests/integration");
    if !tests_dir.is_dir() {
        eprintln!("error: {} not found — run from project root", tests_dir.display());
        process::exit(1);
    }

    let tractor_bin = find_tractor_bin();
    let skip: HashSet<&str> = SKIP_EXTENSIONS.iter().copied().collect();

    let mut processed = 0;
    let mut mismatches: Vec<String> = Vec::new();

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
            let file_name = path.file_name().unwrap().to_string_lossy();

            // Semantic XML
            let xml_path = format!("{}.xml", path_str);
            let output = run_tractor(&tractor_bin, &path_str, &[]);

            // Raw TreeSitter XML
            let raw_xml_path = format!("{}.raw.xml", path_str);
            let raw_output = run_tractor(&tractor_bin, &path_str, &["--raw"]);

            if check_mode {
                if let Ok(existing) = fs::read_to_string(&xml_path) {
                    if existing != output {
                        mismatches.push(xml_path.clone());
                    }
                }
                if let Ok(existing) = fs::read_to_string(&raw_xml_path) {
                    if existing != raw_output {
                        mismatches.push(raw_xml_path.clone());
                    }
                }
            } else {
                fs::write(&xml_path, &output).expect("cannot write .xml snapshot");
                fs::write(&raw_xml_path, &raw_output).expect("cannot write .raw.xml snapshot");
                println!("  {}/{} -> .xml, .raw.xml", lang_name, file_name);
            }

            processed += 1;
        }
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
        // Make ANSI escape codes visible as \e so color snapshots are readable in text editors.
        let output = raw
            .replace(&cwd_prefix, "")
            .replace('\x1b', "\\e");

        if check_mode {
            if let Ok(existing) = fs::read_to_string(&snap_path) {
                if existing != output {
                    mismatches.push(snap_path_str.clone());
                }
            } else {
                mismatches.push(format!("{} (missing)", snap_path_str));
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
            println!("\x1b[31m✗\x1b[0m Snapshot mismatch:");
            println!();
            for m in &mismatches {
                println!("  {}", m);
            }
            println!();
            println!("If intentional, run 'task test:snapshots:update' to regenerate.");
            process::exit(1);
        }
    } else {
        println!("\nUpdated {} fixture(s).", processed);
    }
}

fn find_tractor_bin() -> String {
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

    for c in &candidates {
        if Path::new(c).is_file() {
            return c.clone();
        }
    }

    eprintln!("error: tractor binary not found — run `cargo build --release` first");
    process::exit(1);
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
fn run_tractor_args(bin: &str, args: &[&str]) -> String {
    let output = Command::new(bin)
        .args(args)
        .output()
        .unwrap_or_else(|e| {
            eprintln!("error: failed to run {}: {}", bin, e);
            process::exit(1);
        });

    // Non-zero exit is expected for check commands that find violations — capture stdout anyway.
    String::from_utf8(output.stdout).expect("non-UTF8 tractor output")
}
