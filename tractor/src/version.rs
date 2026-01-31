//! Version information display

use std::collections::HashMap;

// Include the auto-generated versions from build.rs
include!(concat!(env!("OUT_DIR"), "/versions.rs"));

/// Package version from Cargo.toml
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Git commit hash (set by build.rs)
pub const GIT_HASH: &str = env!("TRACTOR_GIT_HASH");

/// Git commit date (set by build.rs)
pub const GIT_DATE: &str = env!("TRACTOR_GIT_DATE");

/// Print basic version information
pub fn print_version() {
    println!("tractor {} ({} {})", VERSION, GIT_HASH, GIT_DATE);
    println!();
    println!("Core libraries:");

    // Find max name length for alignment
    let max_len = DEPENDENCY_VERSIONS.iter().map(|(n, _)| n.len()).max().unwrap_or(0);

    for (name, version) in DEPENDENCY_VERSIONS {
        println!("  {:width$}  {}", name, version, width = max_len);
    }
}

/// Print verbose version information including all grammar versions
pub fn print_version_verbose() {
    println!("tractor {} ({} {})", VERSION, GIT_HASH, GIT_DATE);
    println!();
    println!("Core libraries:");

    // Find max name length for alignment
    let max_dep_len = DEPENDENCY_VERSIONS.iter().map(|(n, _)| n.len()).max().unwrap_or(0);

    for (name, version) in DEPENDENCY_VERSIONS {
        println!("  {:width$}  {}", name, version, width = max_dep_len);
    }

    // Get runtime ABI versions
    let abi_versions: HashMap<&str, usize> = tractor_core::get_language_abi_versions()
        .into_iter()
        .map(|info| (info.name, info.abi_version))
        .collect();

    println!();
    println!("Language grammars ({} languages):", GRAMMAR_VERSIONS.len());

    // Calculate column widths
    let max_lang_len = GRAMMAR_VERSIONS.iter()
        .map(|(crate_name, _, lang_name)| {
            let crate_suffix = crate_name.strip_prefix("tree-sitter-").unwrap_or(crate_name);
            if crate_suffix == *lang_name {
                lang_name.len()
            } else {
                // "csharp (c-sharp)" format
                lang_name.len() + 3 + crate_suffix.len()
            }
        })
        .max()
        .unwrap_or(0);

    let max_version_len = GRAMMAR_VERSIONS.iter()
        .map(|(_, v, _)| v.len())
        .max()
        .unwrap_or(0);

    for (crate_name, version, lang_name) in GRAMMAR_VERSIONS {
        let crate_suffix = crate_name.strip_prefix("tree-sitter-").unwrap_or(crate_name);
        let display_name = if crate_suffix == *lang_name {
            lang_name.to_string()
        } else {
            format!("{} ({})", lang_name, crate_suffix)
        };

        let abi = abi_versions.get(lang_name)
            .map(|v| format!("[ABI {}]", v))
            .unwrap_or_default();

        println!(
            "  {:name_width$}  {:ver_width$}  {}",
            display_name,
            version,
            abi,
            name_width = max_lang_len,
            ver_width = max_version_len,
        );
    }
}
