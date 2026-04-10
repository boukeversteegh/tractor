//! Language metadata and information
//!
//! Centralized language definitions shared between CLI and WASM.
//!
//! The `LANGUAGES` array is the **single source of truth** for all language metadata:
//! - Canonical names, extensions, aliases, transforms, grammar files
//! - The `Language` enum variants map 1:1 with entries in `LANGUAGES`
//! - All lookups (by name, alias, extension) derive from `LANGUAGES`

use serde::Serialize;
use std::fmt;
use std::str::FromStr;

/// Enum representing all supported languages.
///
/// Using an enum instead of magic strings prevents typos and ensures
/// compile-time checking of language identifiers.
///
/// Each variant corresponds to exactly one entry in `LANGUAGES`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    TypeScript,
    Tsx,
    JavaScript,
    CSharp,
    Rust,
    Python,
    Go,
    Java,
    Ruby,
    Cpp,
    C,
    Json,
    Html,
    Css,
    Bash,
    Yaml,
    Toml,
    Ini,
    Env,
    Php,
    Scala,
    Lua,
    Haskell,
    OCaml,
    R,
    Julia,
    Markdown,
    Xml,
    TSql,
    /// Unknown language (for unsupported extensions)
    Unknown,
}

impl Language {
    /// Get the canonical string name for this language.
    /// Derives from `LANGUAGES` - the single source of truth.
    pub fn as_str(&self) -> &'static str {
        self.info().map(|i| i.name).unwrap_or("unknown")
    }

    /// Get LanguageInfo for this language variant.
    /// Returns None only for Language::Unknown.
    pub fn info(&self) -> Option<&'static LanguageInfo> {
        if *self == Language::Unknown {
            return None;
        }
        LANGUAGES.iter().find(|l| l.language == *self)
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Language {
    type Err = ();

    /// Parse a language name or alias into a Language enum.
    /// Looks up in `LANGUAGES` - the single source of truth.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // First check canonical names
        if let Some(info) = LANGUAGES.iter().find(|l| l.name == s) {
            return Ok(info.language);
        }
        // Then check aliases
        if let Some(info) = LANGUAGES.iter().find(|l| l.aliases.contains(&s)) {
            return Ok(info.language);
        }
        Err(())
    }
}

/// Information about a supported language.
///
/// This struct is the **single source of truth** for language metadata.
/// All language lookups and conversions derive from the `LANGUAGES` array.
#[derive(Debug, Clone, Serialize)]
pub struct LanguageInfo {
    /// The Language enum variant for type-safe language comparisons.
    /// Skipped during serialization since `name` already provides the string identifier.
    #[serde(skip)]
    pub language: Language,
    /// Canonical language identifier (e.g., "typescript", "csharp")
    pub name: &'static str,
    /// File extensions (without dots)
    pub extensions: &'static [&'static str],
    /// Alternative names/aliases for this language (e.g., "ts" for typescript)
    #[serde(skip_serializing_if = "is_empty_slice")]
    pub aliases: &'static [&'static str],
    /// Whether this language has semantic transforms
    pub has_transforms: bool,
    /// Grammar file name for web-tree-sitter (e.g., "tree-sitter-typescript.wasm")
    pub grammar_file: Option<&'static str>,
}

/// Helper for serde skip_serializing_if
fn is_empty_slice(s: &[&str]) -> bool {
    s.is_empty()
}

/// All supported languages with their metadata.
///
/// This is the **single source of truth** for all language information.
/// The `Language` enum, `FromStr`, `as_str()`, and all lookups derive from this array.
pub static LANGUAGES: &[LanguageInfo] = &[
    LanguageInfo {
        language: Language::TypeScript,
        name: "typescript",
        extensions: &["ts"],
        aliases: &["ts"],
        has_transforms: true,
        grammar_file: Some("tree-sitter-typescript.wasm"),
    },
    LanguageInfo {
        language: Language::Tsx,
        name: "tsx",
        extensions: &["tsx"],
        aliases: &[],
        has_transforms: true,
        grammar_file: Some("tree-sitter-tsx.wasm"),
    },
    LanguageInfo {
        language: Language::JavaScript,
        name: "javascript",
        extensions: &["js", "mjs", "cjs", "jsx"],
        aliases: &["js", "jsx"],
        has_transforms: true,
        grammar_file: Some("tree-sitter-javascript.wasm"),
    },
    LanguageInfo {
        language: Language::CSharp,
        name: "csharp",
        extensions: &["cs"],
        aliases: &["cs"],
        has_transforms: true,
        grammar_file: Some("tree-sitter-c_sharp.wasm"),
    },
    LanguageInfo {
        language: Language::Rust,
        name: "rust",
        extensions: &["rs"],
        aliases: &["rs"],
        has_transforms: true,
        grammar_file: Some("tree-sitter-rust.wasm"),
    },
    LanguageInfo {
        language: Language::Python,
        name: "python",
        extensions: &["py", "pyw", "pyi"],
        aliases: &["py"],
        has_transforms: true,
        grammar_file: Some("tree-sitter-python.wasm"),
    },
    LanguageInfo {
        language: Language::Go,
        name: "go",
        extensions: &["go"],
        aliases: &[],
        has_transforms: true,
        grammar_file: Some("tree-sitter-go.wasm"),
    },
    LanguageInfo {
        language: Language::Java,
        name: "java",
        extensions: &["java"],
        aliases: &[],
        has_transforms: true,
        grammar_file: Some("tree-sitter-java.wasm"),
    },
    LanguageInfo {
        language: Language::Ruby,
        name: "ruby",
        extensions: &["rb", "rake", "gemspec"],
        aliases: &["rb"],
        has_transforms: true,
        grammar_file: Some("tree-sitter-ruby.wasm"),
    },
    LanguageInfo {
        language: Language::Cpp,
        name: "cpp",
        extensions: &["cpp", "cc", "cxx", "hpp", "hxx", "hh"],
        aliases: &[],
        has_transforms: false,
        grammar_file: Some("tree-sitter-cpp.wasm"),
    },
    LanguageInfo {
        language: Language::C,
        name: "c",
        extensions: &["c", "h"],
        aliases: &[],
        has_transforms: false,
        grammar_file: Some("tree-sitter-c.wasm"),
    },
    LanguageInfo {
        language: Language::Json,
        name: "json",
        extensions: &["json"],
        aliases: &[],
        has_transforms: true,
        grammar_file: Some("tree-sitter-json.wasm"),
    },
    LanguageInfo {
        language: Language::Html,
        name: "html",
        extensions: &["html", "htm"],
        aliases: &[],
        has_transforms: false,
        grammar_file: Some("tree-sitter-html.wasm"),
    },
    LanguageInfo {
        language: Language::Css,
        name: "css",
        extensions: &["css"],
        aliases: &[],
        has_transforms: false,
        grammar_file: Some("tree-sitter-css.wasm"),
    },
    LanguageInfo {
        language: Language::Bash,
        name: "bash",
        extensions: &["sh", "bash"],
        aliases: &["sh"],
        has_transforms: false,
        grammar_file: Some("tree-sitter-bash.wasm"),
    },
    LanguageInfo {
        language: Language::Yaml,
        name: "yaml",
        extensions: &["yml", "yaml"],
        aliases: &["yml"],
        has_transforms: true,
        grammar_file: Some("tree-sitter-yaml.wasm"),
    },
    LanguageInfo {
        language: Language::Toml,
        name: "toml",
        extensions: &["toml"],
        aliases: &[],
        has_transforms: true,
        grammar_file: Some("tree-sitter-toml.wasm"),
    },
    LanguageInfo {
        language: Language::Ini,
        name: "ini",
        extensions: &["ini", "cfg", "inf"],
        aliases: &[],
        has_transforms: true,
        grammar_file: Some("tree-sitter-ini.wasm"),
    },
    LanguageInfo {
        language: Language::Env,
        name: "env",
        extensions: &["env"],
        aliases: &[],
        has_transforms: true,
        grammar_file: Some("tree-sitter-bash.wasm"),
    },
    LanguageInfo {
        language: Language::Php,
        name: "php",
        extensions: &["php"],
        aliases: &[],
        has_transforms: false,
        grammar_file: Some("tree-sitter-php.wasm"),
    },
    LanguageInfo {
        language: Language::Scala,
        name: "scala",
        extensions: &["scala", "sc"],
        aliases: &[],
        has_transforms: false,
        grammar_file: None,
    },
    LanguageInfo {
        language: Language::Lua,
        name: "lua",
        extensions: &["lua"],
        aliases: &[],
        has_transforms: false,
        grammar_file: None,
    },
    LanguageInfo {
        language: Language::Haskell,
        name: "haskell",
        extensions: &["hs", "lhs"],
        aliases: &[],
        has_transforms: false,
        grammar_file: None,
    },
    LanguageInfo {
        language: Language::OCaml,
        name: "ocaml",
        extensions: &["ml", "mli"],
        aliases: &[],
        has_transforms: false,
        grammar_file: None,
    },
    LanguageInfo {
        language: Language::R,
        name: "r",
        extensions: &["r"],
        aliases: &[],
        has_transforms: false,
        grammar_file: None,
    },
    LanguageInfo {
        language: Language::Julia,
        name: "julia",
        extensions: &["jl"],
        aliases: &[],
        has_transforms: false,
        grammar_file: None,
    },
    LanguageInfo {
        language: Language::Markdown,
        name: "markdown",
        extensions: &["md", "markdown", "mdx"],
        aliases: &["md", "mdx"],
        has_transforms: true,
        grammar_file: Some("tree-sitter-markdown.wasm"),
    },
    LanguageInfo {
        language: Language::Xml,
        name: "xml",
        extensions: &["xml"],
        aliases: &[],
        has_transforms: false,
        grammar_file: None, // Pass-through, no parsing needed
    },
    // SQL dialects - multiple languages share the .sql extension
    LanguageInfo {
        language: Language::TSql,
        name: "tsql",
        extensions: &["sql"],
        aliases: &["mssql"],
        has_transforms: true,
        grammar_file: Some("tree-sitter-sql.wasm"),
    },
];

/// Get language info by name or alias
///
/// Uses `Language::from_str` to handle aliases, then looks up the LanguageInfo.
pub fn get_language_info(name: &str) -> Option<&'static LanguageInfo> {
    Language::from_str(name).ok().and_then(|lang| lang.info())
}

/// Parse a language string into a Language enum
///
/// Handles aliases like "js" → JavaScript, "ts" → TypeScript, etc.
/// Returns Language::Unknown for unrecognized strings.
pub fn parse_language(name: &str) -> Language {
    Language::from_str(name).unwrap_or(Language::Unknown)
}

/// Get language by file extension
pub fn get_language_for_extension(ext: &str) -> Option<&'static LanguageInfo> {
    let ext_lower = ext.to_lowercase();
    LANGUAGES
        .iter()
        .find(|l| l.extensions.iter().any(|e| *e == ext_lower))
}

/// Get all languages that match a file extension
///
/// Returns all languages that claim a given extension. If more than one
/// language matches, the extension is considered ambiguous and `--lang`
/// should be required.
pub fn get_all_languages_for_extension(ext: &str) -> Vec<&'static LanguageInfo> {
    let ext_lower = ext.to_lowercase();
    LANGUAGES
        .iter()
        .filter(|l| l.extensions.iter().any(|e| *e == ext_lower))
        .collect()
}

/// Get list of all language names
pub fn get_language_names() -> Vec<&'static str> {
    LANGUAGES.iter().map(|l| l.name).collect()
}

/// Get languages that are available in web (have grammar files)
pub fn get_web_languages() -> Vec<&'static LanguageInfo> {
    LANGUAGES
        .iter()
        .filter(|l| l.grammar_file.is_some())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_language_info() {
        let ts = get_language_info("typescript").unwrap();
        assert_eq!(ts.name, "typescript");
        assert!(ts.has_transforms);
        assert!(ts.extensions.contains(&"ts"));
    }

    #[test]
    fn test_language_aliases() {
        assert_eq!(get_language_info("ts").unwrap().name, "typescript");
        assert_eq!(get_language_info("cs").unwrap().name, "csharp");
        assert_eq!(get_language_info("py").unwrap().name, "python");
        assert_eq!(get_language_info("rb").unwrap().name, "ruby");
        assert_eq!(get_language_info("rs").unwrap().name, "rust");
        assert_eq!(get_language_info("md").unwrap().name, "markdown");
        assert_eq!(get_language_info("yml").unwrap().name, "yaml");
        assert_eq!(get_language_info("sh").unwrap().name, "bash");
        assert_eq!(get_language_info("js").unwrap().name, "javascript");
        assert_eq!(get_language_info("jsx").unwrap().name, "javascript");
        assert_eq!(get_language_info("mdx").unwrap().name, "markdown");
        assert_eq!(get_language_info("mssql").unwrap().name, "tsql");
    }

    #[test]
    fn test_tsx_language() {
        let tsx = get_language_info("tsx").unwrap();
        assert_eq!(tsx.name, "tsx");
        assert!(tsx.has_transforms);
        assert!(tsx.extensions.contains(&"tsx"));
    }

    #[test]
    fn test_get_language_for_extension() {
        let lang = get_language_for_extension("ts").unwrap();
        assert_eq!(lang.name, "typescript");

        let lang = get_language_for_extension("tsx").unwrap();
        assert_eq!(lang.name, "tsx");

        let lang = get_language_for_extension("cs").unwrap();
        assert_eq!(lang.name, "csharp");
    }

    #[test]
    fn test_tsql_language() {
        let tsql = get_language_info("tsql").unwrap();
        assert_eq!(tsql.name, "tsql");
        assert!(tsql.extensions.contains(&"sql"));
        assert!(tsql.has_transforms);

        // Alias
        assert_eq!(get_language_info("mssql").unwrap().name, "tsql");
    }

    #[test]
    fn test_get_all_languages_for_extension() {
        // Unambiguous extension
        let ts_langs = get_all_languages_for_extension("ts");
        assert_eq!(ts_langs.len(), 1);
        assert_eq!(ts_langs[0].name, "typescript");

        // SQL extension - currently just tsql
        let sql_langs = get_all_languages_for_extension("sql");
        assert!(sql_langs.iter().any(|l| l.name == "tsql"));
    }

    #[test]
    fn test_web_languages() {
        let web_langs = get_web_languages();
        assert!(web_langs.iter().any(|l| l.name == "typescript"));
        assert!(web_langs.iter().any(|l| l.name == "yaml"));
        assert!(web_langs.iter().any(|l| l.name == "markdown"));
    }

    #[test]
    fn test_language_enum_from_str() {
        assert_eq!(
            Language::from_str("typescript").unwrap(),
            Language::TypeScript
        );
        assert_eq!(Language::from_str("ts").unwrap(), Language::TypeScript);
        assert_eq!(
            Language::from_str("javascript").unwrap(),
            Language::JavaScript
        );
        assert_eq!(Language::from_str("js").unwrap(), Language::JavaScript);
        assert_eq!(Language::from_str("csharp").unwrap(), Language::CSharp);
        assert_eq!(Language::from_str("cs").unwrap(), Language::CSharp);
        assert_eq!(Language::from_str("markdown").unwrap(), Language::Markdown);
        assert_eq!(Language::from_str("md").unwrap(), Language::Markdown);
        assert!(Language::from_str("nonexistent").is_err());
    }

    #[test]
    fn test_language_enum_as_str() {
        assert_eq!(Language::TypeScript.as_str(), "typescript");
        assert_eq!(Language::JavaScript.as_str(), "javascript");
        assert_eq!(Language::CSharp.as_str(), "csharp");
        assert_eq!(Language::Markdown.as_str(), "markdown");
    }

    #[test]
    fn test_language_enum_info() {
        let ts_info = Language::TypeScript.info().unwrap();
        assert_eq!(ts_info.name, "typescript");
        assert!(ts_info.extensions.contains(&"ts"));

        assert!(Language::Unknown.info().is_none());
    }

    #[test]
    fn test_parse_language() {
        assert_eq!(parse_language("typescript"), Language::TypeScript);
        assert_eq!(parse_language("ts"), Language::TypeScript);
        assert_eq!(parse_language("nonexistent"), Language::Unknown);
    }
}
