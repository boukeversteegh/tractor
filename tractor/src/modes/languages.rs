//! `tractor languages` command - list supported languages with their extensions and aliases

use tractor_core::language_info::LANGUAGES;

/// Run the `languages` command, printing a table of all supported languages.
pub fn run_languages() -> Result<(), Box<dyn std::error::Error>> {
    println!("Supported languages:\n");
    println!("{:<15} {:<30} {}", "Language", "Extensions", "Aliases");
    println!("{}", "-".repeat(70));

    // Build a map of aliases for display
    let aliases: &[(&str, &str)] = &[
        ("ts", "typescript"),
        ("js", "javascript"),
        ("jsx", "javascript"),
        ("cs", "csharp"),
        ("rs", "rust"),
        ("py", "python"),
        ("rb", "ruby"),
        ("md", "markdown"),
        ("mdx", "markdown"),
        ("yml", "yaml"),
        ("sh", "bash"),
        ("mssql", "tsql"),
    ];

    for lang in LANGUAGES.iter() {
        let extensions_str = lang.extensions.join(", ");

        // Find aliases that map to this language
        let lang_aliases: Vec<&str> = aliases
            .iter()
            .filter(|(_, canonical)| *canonical == lang.name)
            .map(|(alias, _)| *alias)
            .collect();
        let aliases_str = if lang_aliases.is_empty() {
            String::new()
        } else {
            lang_aliases.join(", ")
        };

        println!("{:<15} {:<30} {}", lang.name, extensions_str, aliases_str);
    }

    println!("\nUse -l/--lang with the language name or any alias.");
    println!("Extensions are used for auto-detection when no language is specified.");

    Ok(())
}
