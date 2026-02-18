//! Generate/update or check XML snapshots for integration tests.
//!
//! Walks `tests/integration/*/`, finds source files by known extensions,
//! and runs tractor on each to produce `.xml` and `.raw.xml` snapshots.
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

    let mut dirs: Vec<_> = fs::read_dir(tests_dir)
        .expect("cannot read tests/integration")
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
                // Compare against existing snapshots
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
                // Write snapshots
                fs::write(&xml_path, &output).expect("cannot write .xml snapshot");
                fs::write(&raw_xml_path, &raw_output).expect("cannot write .raw.xml snapshot");
                println!("  {}/{} -> .xml, .raw.xml", lang_name, file_name);
            }

            processed += 1;
        }
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
