//! Build script to capture git information and dependency versions at compile time

use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;

fn main() {
    // Re-run build script if git HEAD changes or Cargo.lock changes
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/index");
    println!("cargo:rerun-if-changed=../Cargo.lock");
    println!("cargo:rerun-if-changed=../.grammar-build");

    // Get git commit hash (short)
    let commit_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Get git commit datetime in ISO format
    let commit_date = Command::new("git")
        .args(["log", "-1", "--format=%ci"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Check if working directory is dirty
    let is_dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);

    let dirty_suffix = if is_dirty { "-dirty" } else { "" };

    println!(
        "cargo:rustc-env=TRACTOR_GIT_HASH={}{}",
        commit_hash, dirty_suffix
    );
    println!("cargo:rustc-env=TRACTOR_GIT_DATE={}", commit_date);

    // Allow overriding the version via TRACTOR_VERSION env var (set in CI release builds)
    println!("cargo:rerun-if-env-changed=TRACTOR_VERSION");
    if let Ok(release_version) = env::var("TRACTOR_VERSION") {
        println!("cargo:rustc-env=TRACTOR_VERSION={}", release_version);
    }

    // Extract dependency versions from Cargo.lock
    let versions = get_dependency_versions();

    // Generate version info file
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("versions.rs");
    let mut f = File::create(&dest_path).unwrap();

    // Write the versions as a const array
    writeln!(f, "/// Auto-generated dependency versions from Cargo.lock").unwrap();
    writeln!(f, "pub const DEPENDENCY_VERSIONS: &[(&str, &str)] = &[").unwrap();

    // Core libraries first
    for name in ["tree-sitter", "xot", "xee-xpath"] {
        if let Some(info) = versions.get(name) {
            writeln!(f, "    (\"{}\", \"{}\"),", name, info.display_version()).unwrap();
        }
    }
    writeln!(f, "];").unwrap();

    writeln!(f).unwrap();
    writeln!(
        f,
        "/// Auto-generated grammar versions from Cargo.lock"
    )
    .unwrap();
    writeln!(
        f,
        "/// Format: (crate_name, version_string, language_name)"
    )
    .unwrap();
    writeln!(f, "pub const GRAMMAR_VERSIONS: &[(&str, &str, &str)] = &[").unwrap();

    // Grammar crates (sorted alphabetically)
    let mut grammar_names: Vec<_> = versions
        .keys()
        .filter(|k| k.starts_with("tree-sitter-") && *k != "tree-sitter-language")
        .collect();
    grammar_names.sort();

    for name in grammar_names {
        if let Some(info) = versions.get(name) {
            let lang_name = crate_to_language_name(name);
            writeln!(
                f,
                "    (\"{}\", \"{}\", \"{}\"),",
                name,
                info.display_version(),
                lang_name
            )
            .unwrap();
        }
    }
    writeln!(f, "];").unwrap();
}

/// Map tree-sitter crate name to tractor language name
fn crate_to_language_name(crate_name: &str) -> &'static str {
    match crate_name {
        "tree-sitter-c-sharp" => "csharp",
        "tree-sitter-c" => "c",
        "tree-sitter-cpp" => "cpp",
        "tree-sitter-rust" => "rust",
        "tree-sitter-typescript" => "typescript",
        "tree-sitter-javascript" => "javascript",
        "tree-sitter-python" => "python",
        "tree-sitter-go" => "go",
        "tree-sitter-java" => "java",
        "tree-sitter-ruby" => "ruby",
        "tree-sitter-json" => "json",
        "tree-sitter-html" => "html",
        "tree-sitter-css" => "css",
        "tree-sitter-bash" => "bash",
        "tree-sitter-yaml" => "yaml",
        "tree-sitter-php" => "php",
        "tree-sitter-scala" => "scala",
        "tree-sitter-lua" => "lua",
        "tree-sitter-haskell" => "haskell",
        "tree-sitter-ocaml" => "ocaml",
        "tree-sitter-r" => "r",
        "tree-sitter-julia" => "julia",
        _ => "unknown",
    }
}

/// Package version info with source detection
struct PackageInfo {
    version: String,
    is_patched: bool,
    source_type: String,         // "registry", "git", "path"
    patch_commit: Option<String>, // For patched crates: commit hash
    patch_date: Option<String>,   // For patched crates: commit date
}

impl PackageInfo {
    fn display_version(&self) -> String {
        if self.is_patched {
            match (&self.patch_commit, &self.patch_date) {
                (Some(commit), Some(date)) => {
                    // Format date as YYYY-MM-DD for brevity
                    let short_date = date.split(' ').next().unwrap_or(date);
                    format!("{} (patched: {} {})", self.version, commit, short_date)
                }
                (Some(commit), None) => {
                    format!("{} (patched: {})", self.version, commit)
                }
                _ => format!("{} (patched)", self.version),
            }
        } else if self.source_type == "git" {
            format!("{} (git)", self.version)
        } else {
            self.version.clone()
        }
    }
}

/// Extract dependency versions from Cargo.lock
fn get_dependency_versions() -> HashMap<String, PackageInfo> {
    let mut versions = HashMap::new();

    if let Ok(lockfile) = std::fs::read_to_string("../Cargo.lock") {
        parse_cargo_lock(&lockfile, &mut versions);
    }

    // For patched crates, try to get git info from their local directories
    let patched_crates: Vec<String> = versions
        .iter()
        .filter(|(_, info)| info.is_patched)
        .map(|(name, _)| name.clone())
        .collect();

    for crate_name in patched_crates {
        let patch_dir = format!("../.grammar-build/{}", crate_name);
        if let Some((commit, date)) = get_git_info_for_dir(&patch_dir) {
            if let Some(info) = versions.get_mut(&crate_name) {
                info.patch_commit = Some(commit);
                info.patch_date = Some(date);
            }
        }
    }

    versions
}

/// Get git commit hash and date for a directory
fn get_git_info_for_dir(dir: &str) -> Option<(String, String)> {
    let commit = Command::new("git")
        .args(["-C", dir, "rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;

    let date = Command::new("git")
        .args(["-C", dir, "log", "-1", "--format=%ci"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    Some((commit, date))
}

/// Parse Cargo.lock to extract package versions
/// Detects patched crates (no source field) vs registry crates
fn parse_cargo_lock(content: &str, versions: &mut HashMap<String, PackageInfo>) {
    // Parse package blocks from TOML
    let mut current_name: Option<String> = None;
    let mut current_version: Option<String> = None;
    let mut current_source: Option<String> = None;
    let mut in_package_block = false;

    for line in content.lines() {
        let line = line.trim();

        if line == "[[package]]" {
            // Save previous package if we have one
            if let (Some(name), Some(version)) = (current_name.take(), current_version.take()) {
                if should_track(&name) {
                    let (is_patched, source_type) = match &current_source {
                        None => (true, "path".to_string()), // No source = patched/path dep
                        Some(s) if s.starts_with("git+") => (false, "git".to_string()),
                        Some(_) => (false, "registry".to_string()),
                    };
                    versions.insert(
                        name,
                        PackageInfo {
                            version,
                            is_patched,
                            source_type,
                            patch_commit: None,
                            patch_date: None,
                        },
                    );
                }
            }
            current_source = None;
            in_package_block = true;
        } else if in_package_block {
            if line.starts_with("name = ") {
                current_name = Some(
                    line.trim_start_matches("name = ")
                        .trim_matches('"')
                        .to_string(),
                );
            } else if line.starts_with("version = ") {
                current_version = Some(
                    line.trim_start_matches("version = ")
                        .trim_matches('"')
                        .to_string(),
                );
            } else if line.starts_with("source = ") {
                current_source = Some(
                    line.trim_start_matches("source = ")
                        .trim_matches('"')
                        .to_string(),
                );
            }
        }
    }

    // Don't forget the last package
    if let (Some(name), Some(version)) = (current_name, current_version) {
        if should_track(&name) {
            let (is_patched, source_type) = match &current_source {
                None => (true, "path".to_string()),
                Some(s) if s.starts_with("git+") => (false, "git".to_string()),
                Some(_) => (false, "registry".to_string()),
            };
            versions.insert(
                name,
                PackageInfo {
                    version,
                    is_patched,
                    source_type,
                    patch_commit: None,
                    patch_date: None,
                },
            );
        }
    }
}

/// Check if we should track this package
fn should_track(name: &str) -> bool {
    name == "tree-sitter"
        || name == "xot"
        || name == "xee-xpath"
        || name.starts_with("tree-sitter-")
}
