//! `tractor docs languages` command - list supported languages with their extensions and aliases

use tractor_core::language_info::{LANGUAGES, LANGUAGE_ALIASES};

/// Run the `docs languages` command, printing a table of all supported languages.
pub fn run_languages() -> Result<(), Box<dyn std::error::Error>> {
    println!("Supported languages:\n");
    println!("{:<15} {:<30} {}", "Language", "Extensions", "Aliases");
    println!("{}", "-".repeat(70));

    for lang_info in LANGUAGES.iter() {
        let extensions_str = lang_info.extensions.join(", ");

        // Find aliases that map to this language (using the canonical LANGUAGE_ALIASES from language_info)
        let lang_aliases: Vec<&str> = LANGUAGE_ALIASES
            .iter()
            .filter(|(_, lang)| *lang == lang_info.language)
            .map(|(alias, _)| *alias)
            .collect();
        let aliases_str = if lang_aliases.is_empty() {
            String::new()
        } else {
            lang_aliases.join(", ")
        };

        println!("{:<15} {:<30} {}", lang_info.name, extensions_str, aliases_str);
    }

    println!("\nUse -l/--lang with the language name or any alias.");
    println!("Extensions are used for auto-detection when no language is specified.");

    Ok(())
}
