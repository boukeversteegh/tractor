//! Language metadata and information
//!
//! Centralized language definitions shared between CLI and WASM.

use serde::Serialize;

/// Information about a supported language
#[derive(Debug, Clone, Serialize)]
pub struct LanguageInfo {
    /// Language identifier (e.g., "typescript", "csharp")
    pub name: &'static str,
    /// File extensions (without dots)
    pub extensions: &'static [&'static str],
    /// Whether this language has semantic transforms
    pub has_transforms: bool,
    /// Grammar file name for web-tree-sitter (e.g., "tree-sitter-typescript.wasm")
    pub grammar_file: Option<&'static str>,
}

/// All supported languages with their metadata
pub static LANGUAGES: &[LanguageInfo] = &[
    LanguageInfo {
        name: "typescript",
        extensions: &["ts", "tsx"],
        has_transforms: true,
        grammar_file: Some("tree-sitter-typescript.wasm"),
    },
    LanguageInfo {
        name: "javascript",
        extensions: &["js", "mjs", "cjs", "jsx"],
        has_transforms: true,
        grammar_file: Some("tree-sitter-javascript.wasm"),
    },
    LanguageInfo {
        name: "csharp",
        extensions: &["cs"],
        has_transforms: true,
        grammar_file: Some("tree-sitter-c_sharp.wasm"),
    },
    LanguageInfo {
        name: "rust",
        extensions: &["rs"],
        has_transforms: true,
        grammar_file: Some("tree-sitter-rust.wasm"),
    },
    LanguageInfo {
        name: "python",
        extensions: &["py", "pyw", "pyi"],
        has_transforms: true,
        grammar_file: Some("tree-sitter-python.wasm"),
    },
    LanguageInfo {
        name: "go",
        extensions: &["go"],
        has_transforms: true,
        grammar_file: Some("tree-sitter-go.wasm"),
    },
    LanguageInfo {
        name: "java",
        extensions: &["java"],
        has_transforms: true,
        grammar_file: Some("tree-sitter-java.wasm"),
    },
    LanguageInfo {
        name: "ruby",
        extensions: &["rb", "rake", "gemspec"],
        has_transforms: true,
        grammar_file: Some("tree-sitter-ruby.wasm"),
    },
    LanguageInfo {
        name: "cpp",
        extensions: &["cpp", "cc", "cxx", "hpp", "hxx", "hh"],
        has_transforms: false,
        grammar_file: Some("tree-sitter-cpp.wasm"),
    },
    LanguageInfo {
        name: "c",
        extensions: &["c", "h"],
        has_transforms: false,
        grammar_file: Some("tree-sitter-c.wasm"),
    },
    LanguageInfo {
        name: "json",
        extensions: &["json"],
        has_transforms: false,
        grammar_file: Some("tree-sitter-json.wasm"),
    },
    LanguageInfo {
        name: "html",
        extensions: &["html", "htm"],
        has_transforms: false,
        grammar_file: Some("tree-sitter-html.wasm"),
    },
    LanguageInfo {
        name: "css",
        extensions: &["css"],
        has_transforms: false,
        grammar_file: Some("tree-sitter-css.wasm"),
    },
    LanguageInfo {
        name: "bash",
        extensions: &["sh", "bash"],
        has_transforms: false,
        grammar_file: Some("tree-sitter-bash.wasm"),
    },
    LanguageInfo {
        name: "yaml",
        extensions: &["yml", "yaml"],
        has_transforms: false,
        grammar_file: None, // Not available in web yet
    },
    LanguageInfo {
        name: "php",
        extensions: &["php"],
        has_transforms: false,
        grammar_file: Some("tree-sitter-php.wasm"),
    },
    LanguageInfo {
        name: "scala",
        extensions: &["scala", "sc"],
        has_transforms: false,
        grammar_file: None,
    },
    LanguageInfo {
        name: "lua",
        extensions: &["lua"],
        has_transforms: false,
        grammar_file: None,
    },
    LanguageInfo {
        name: "haskell",
        extensions: &["hs", "lhs"],
        has_transforms: false,
        grammar_file: None,
    },
    LanguageInfo {
        name: "ocaml",
        extensions: &["ml", "mli"],
        has_transforms: false,
        grammar_file: None,
    },
    LanguageInfo {
        name: "r",
        extensions: &["r"],
        has_transforms: false,
        grammar_file: None,
    },
    LanguageInfo {
        name: "julia",
        extensions: &["jl"],
        has_transforms: false,
        grammar_file: None,
    },
    LanguageInfo {
        name: "xml",
        extensions: &["xml"],
        has_transforms: false,
        grammar_file: None, // Pass-through, no parsing needed
    },
];

/// Get language info by name
pub fn get_language_info(name: &str) -> Option<&'static LanguageInfo> {
    // Normalize common aliases
    let normalized = match name {
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "cs" => "csharp",
        "rs" => "rust",
        "py" => "python",
        "rb" => "ruby",
        _ => name,
    };
    LANGUAGES.iter().find(|l| l.name == normalized)
}

/// Get language by file extension
pub fn get_language_for_extension(ext: &str) -> Option<&'static LanguageInfo> {
    let ext_lower = ext.to_lowercase();
    LANGUAGES
        .iter()
        .find(|l| l.extensions.iter().any(|e| *e == ext_lower))
}

/// Get list of all language names
pub fn get_language_names() -> Vec<&'static str> {
    LANGUAGES.iter().map(|l| l.name).collect()
}

/// Get languages that are available in web (have grammar files)
pub fn get_web_languages() -> Vec<&'static LanguageInfo> {
    LANGUAGES.iter().filter(|l| l.grammar_file.is_some()).collect()
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
    }

    #[test]
    fn test_get_language_for_extension() {
        let lang = get_language_for_extension("ts").unwrap();
        assert_eq!(lang.name, "typescript");

        let lang = get_language_for_extension("cs").unwrap();
        assert_eq!(lang.name, "csharp");
    }

    #[test]
    fn test_web_languages() {
        let web_langs = get_web_languages();
        assert!(web_langs.iter().any(|l| l.name == "typescript"));
        assert!(!web_langs.iter().any(|l| l.name == "yaml")); // No grammar file
    }
}
