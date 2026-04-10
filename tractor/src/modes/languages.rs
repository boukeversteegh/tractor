//! `tractor docs languages` command - list supported languages with their extensions and aliases

use tractor_core::language_info::LANGUAGES;

/// Run the `docs languages` command, printing a table of all supported languages.
pub fn run_languages() -> Result<(), Box<dyn std::error::Error>> {
    println!("Supported languages:\n");
    println!("{:<15} {:<30} {}", "Language", "Extensions", "Aliases");
    println!("{}", "-".repeat(70));

    for lang_info in LANGUAGES.iter() {
        let extensions_str = lang_info.extensions.join(", ");
        let aliases_str = lang_info.aliases.join(", ");

        println!(
            "{:<15} {:<30} {}",
            lang_info.name, extensions_str, aliases_str
        );
    }

    println!("\nUse -l/--lang with the language name or any alias.");
    println!("Extensions are used for auto-detection when no language is specified.");

    Ok(())
}
