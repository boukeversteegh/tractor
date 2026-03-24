//! TOML config loader for rules.
//!
//! This module converts a TOML config file into the storage-agnostic
//! `RuleSet` / `Rule` data model defined in tractor-core. It is one
//! possible source — rules can equally be constructed programmatically
//! or loaded from any other format.

use std::path::Path;
use serde::Deserialize;
use tractor_core::report::Severity;
use tractor_core::rule::{Rule, RuleSet};
use tractor_core::tree_mode::TreeMode;

// ---------------------------------------------------------------------------
// TOML schema (private — only used for deserialization)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct TomlConfig {
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    tree_mode: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    rules: Vec<TomlRule>,
}

#[derive(Deserialize)]
struct TomlRule {
    id: String,
    xpath: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default = "default_severity")]
    severity: String,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    tree_mode: Option<String>,
}

fn default_severity() -> String {
    "error".to_string()
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

fn parse_severity(s: &str) -> Result<Severity, String> {
    match s {
        "error" => Ok(Severity::Error),
        "warning" => Ok(Severity::Warning),
        other => Err(format!("invalid severity '{}': use 'error' or 'warning'", other)),
    }
}

fn parse_tree_mode(s: &str) -> Result<TreeMode, String> {
    match s {
        "raw" => Ok(TreeMode::Raw),
        "structure" => Ok(TreeMode::Structure),
        "data" => Ok(TreeMode::Data),
        other => Err(format!(
            "invalid tree_mode '{}': use 'raw', 'structure', or 'data'",
            other
        )),
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Load a `RuleSet` from a TOML config file.
pub fn load_rules_toml(path: &Path) -> Result<RuleSet, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read '{}': {}", path.display(), e))?;
    parse_rules_toml(&content)
}

/// Parse a `RuleSet` from a TOML string.
pub fn parse_rules_toml(content: &str) -> Result<RuleSet, Box<dyn std::error::Error>> {
    let config: TomlConfig = toml::from_str(content)
        .map_err(|e| format!("invalid rules TOML: {}", e))?;

    let default_tree_mode = config
        .tree_mode
        .as_deref()
        .map(parse_tree_mode)
        .transpose()?;

    let mut rules = Vec::with_capacity(config.rules.len());
    for r in config.rules {
        let severity = parse_severity(&r.severity)?;
        let tree_mode = r.tree_mode.as_deref().map(parse_tree_mode).transpose()?;

        let mut rule = Rule::new(r.id, r.xpath)
            .with_severity(severity);

        if let Some(reason) = r.reason {
            rule = rule.with_reason(reason);
        }
        if let Some(message) = r.message {
            rule = rule.with_message(message);
        }
        if !r.include.is_empty() {
            rule = rule.with_include(r.include);
        }
        if !r.exclude.is_empty() {
            rule = rule.with_exclude(r.exclude);
        }
        if let Some(lang) = r.language {
            rule = rule.with_language(lang);
        }
        if let Some(tm) = tree_mode {
            rule = rule.with_tree_mode(tm);
        }

        rules.push(rule);
    }

    Ok(RuleSet {
        rules,
        include: config.include,
        exclude: config.exclude,
        default_tree_mode,
        default_language: config.language,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal() {
        let toml = r#"
[[rules]]
id = "no-todo"
xpath = "//comment[contains(text(), 'TODO')]"
"#;
        let rs = parse_rules_toml(toml).unwrap();
        assert_eq!(rs.rules.len(), 1);
        assert_eq!(rs.rules[0].id, "no-todo");
        assert_eq!(rs.rules[0].severity, Severity::Error);
        assert!(rs.include.is_empty());
    }

    #[test]
    fn test_parse_full() {
        let toml = r#"
include = ["src/**/*.rs"]
exclude = ["src/vendor/**"]
tree_mode = "structure"
language = "rust"

[[rules]]
id = "no-unwrap"
xpath = "//call_expression[function='unwrap']"
reason = "Use ? instead of .unwrap()"
severity = "warning"
message = "{file}:{line}: unwrap found"

[[rules]]
id = "no-panic"
xpath = "//macro_invocation[macro='panic']"
reason = "Do not panic in library code"
severity = "error"
include = ["src/lib/**"]
"#;
        let rs = parse_rules_toml(toml).unwrap();
        assert_eq!(rs.include, vec!["src/**/*.rs"]);
        assert_eq!(rs.exclude, vec!["src/vendor/**"]);
        assert_eq!(rs.default_tree_mode, Some(TreeMode::Structure));
        assert_eq!(rs.default_language, Some("rust".to_string()));

        assert_eq!(rs.rules.len(), 2);

        let r0 = &rs.rules[0];
        assert_eq!(r0.id, "no-unwrap");
        assert_eq!(r0.severity, Severity::Warning);
        assert_eq!(r0.reason.as_deref(), Some("Use ? instead of .unwrap()"));
        assert!(r0.message.is_some());
        assert!(r0.include.is_empty()); // inherits from ruleset

        let r1 = &rs.rules[1];
        assert_eq!(r1.id, "no-panic");
        assert_eq!(r1.severity, Severity::Error);
        assert_eq!(r1.include, vec!["src/lib/**"]);
    }

    #[test]
    fn test_parse_invalid_severity() {
        let toml = r#"
[[rules]]
id = "bad"
xpath = "//x"
severity = "critical"
"#;
        let err = parse_rules_toml(toml).unwrap_err();
        assert!(err.to_string().contains("invalid severity"));
    }

    #[test]
    fn test_parse_invalid_tree_mode() {
        let toml = r#"
tree_mode = "fancy"

[[rules]]
id = "a"
xpath = "//x"
"#;
        let err = parse_rules_toml(toml).unwrap_err();
        assert!(err.to_string().contains("invalid tree_mode"));
    }

    #[test]
    fn test_parse_empty_rules() {
        let toml = "";
        let rs = parse_rules_toml(toml).unwrap();
        assert!(rs.rules.is_empty());
    }
}
