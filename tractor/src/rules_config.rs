//! Config loader for rule files (TOML and YAML).
//!
//! This module converts a config file into the storage-agnostic
//! `RuleSet` / `Rule` data model defined in tractor-core. The format
//! is auto-detected from the file extension (.toml, .yml, .yaml).

use std::path::Path;
use serde::Deserialize;
use tractor_core::report::Severity;
use tractor_core::rule::{Rule, RuleSet};
use tractor_core::tree_mode::TreeMode;

// ---------------------------------------------------------------------------
// Serde schema (shared across TOML and YAML)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RulesConfig {
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    tree_mode: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    rules: Vec<ConfigRule>,
}

#[derive(Deserialize)]
struct ConfigRule {
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
    #[serde(default)]
    expect: Vec<ExpectEntry>,
}

/// A single expectation entry: an optional valid and/or invalid code example.
#[derive(Deserialize)]
struct ExpectEntry {
    #[serde(default)]
    valid: Option<String>,
    #[serde(default)]
    invalid: Option<String>,
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

/// Convert a deserialized config into a `RuleSet`.
fn config_to_ruleset(config: RulesConfig) -> Result<RuleSet, Box<dyn std::error::Error>> {
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

        let valid_examples: Vec<String> = r.expect.iter().filter_map(|e| e.valid.clone()).collect();
        let invalid_examples: Vec<String> = r.expect.iter().filter_map(|e| e.invalid.clone()).collect();
        if !valid_examples.is_empty() {
            rule = rule.with_valid_examples(valid_examples);
        }
        if !invalid_examples.is_empty() {
            rule = rule.with_invalid_examples(invalid_examples);
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
// Public API
// ---------------------------------------------------------------------------

/// Load a `RuleSet` from a config file. Format is detected from extension:
/// `.toml` for TOML, `.yml` or `.yaml` for YAML.
pub fn load_rules(path: &Path) -> Result<RuleSet, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read '{}': {}", path.display(), e))?;

    match path.extension().and_then(|e| e.to_str()) {
        Some("toml") => parse_rules_toml(&content),
        Some("yml") | Some("yaml") => parse_rules_yaml(&content),
        Some(ext) => Err(format!(
            "unsupported rules file extension '.{}': use .toml, .yml, or .yaml",
            ext
        ).into()),
        None => Err("rules file has no extension: use .toml, .yml, or .yaml".into()),
    }
}

/// Parse a `RuleSet` from a TOML string.
pub fn parse_rules_toml(content: &str) -> Result<RuleSet, Box<dyn std::error::Error>> {
    let config: RulesConfig = toml::from_str(content)
        .map_err(|e| format!("invalid rules TOML: {}", e))?;
    config_to_ruleset(config)
}

/// Parse a `RuleSet` from a YAML string.
pub fn parse_rules_yaml(content: &str) -> Result<RuleSet, Box<dyn std::error::Error>> {
    let config: RulesConfig = serde_yaml::from_str(content)
        .map_err(|e| format!("invalid rules YAML: {}", e))?;
    config_to_ruleset(config)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- TOML tests --

    #[test]
    fn test_parse_toml_minimal() {
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
    fn test_parse_toml_full() {
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
        assert!(r0.include.is_empty());

        let r1 = &rs.rules[1];
        assert_eq!(r1.id, "no-panic");
        assert_eq!(r1.severity, Severity::Error);
        assert_eq!(r1.include, vec!["src/lib/**"]);
    }

    #[test]
    fn test_parse_toml_invalid_severity() {
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
    fn test_parse_toml_invalid_tree_mode() {
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
    fn test_parse_toml_empty() {
        let toml = "";
        let rs = parse_rules_toml(toml).unwrap();
        assert!(rs.rules.is_empty());
    }

    // -- YAML tests --

    #[test]
    fn test_parse_yaml_minimal() {
        let yaml = r#"
rules:
  - id: no-todo
    xpath: "//comment[contains(text(), 'TODO')]"
"#;
        let rs = parse_rules_yaml(yaml).unwrap();
        assert_eq!(rs.rules.len(), 1);
        assert_eq!(rs.rules[0].id, "no-todo");
        assert_eq!(rs.rules[0].severity, Severity::Error);
        assert!(rs.include.is_empty());
    }

    #[test]
    fn test_parse_yaml_full() {
        let yaml = r#"
include:
  - "src/**/*.rs"
exclude:
  - "src/vendor/**"
tree_mode: structure
language: rust

rules:
  - id: no-unwrap
    xpath: "//call_expression[function='unwrap']"
    reason: "Use ? instead of .unwrap()"
    severity: warning
    message: "{file}:{line}: unwrap found"

  - id: no-panic
    xpath: "//macro_invocation[macro='panic']"
    reason: Do not panic in library code
    severity: error
    include:
      - "src/lib/**"
"#;
        let rs = parse_rules_yaml(yaml).unwrap();
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
        assert!(r0.include.is_empty());

        let r1 = &rs.rules[1];
        assert_eq!(r1.id, "no-panic");
        assert_eq!(r1.severity, Severity::Error);
        assert_eq!(r1.include, vec!["src/lib/**"]);
    }

    #[test]
    fn test_parse_yaml_invalid_severity() {
        let yaml = r#"
rules:
  - id: bad
    xpath: "//x"
    severity: critical
"#;
        let err = parse_rules_yaml(yaml).unwrap_err();
        assert!(err.to_string().contains("invalid severity"));
    }

    #[test]
    fn test_parse_yaml_empty() {
        let yaml = "{}";
        let rs = parse_rules_yaml(yaml).unwrap();
        assert!(rs.rules.is_empty());
    }

    // -- Format equivalence --

    #[test]
    fn test_toml_yaml_produce_same_ruleset() {
        let toml = r#"
include = ["**/*.rs"]
exclude = ["vendor/**"]

[[rules]]
id = "test-rule"
xpath = "//function"
reason = "found a function"
severity = "warning"
"#;
        let yaml = r#"
include:
  - "**/*.rs"
exclude:
  - "vendor/**"

rules:
  - id: test-rule
    xpath: "//function"
    reason: found a function
    severity: warning
"#;
        let from_toml = parse_rules_toml(toml).unwrap();
        let from_yaml = parse_rules_yaml(yaml).unwrap();

        assert_eq!(from_toml.include, from_yaml.include);
        assert_eq!(from_toml.exclude, from_yaml.exclude);
        assert_eq!(from_toml.rules.len(), from_yaml.rules.len());
        assert_eq!(from_toml.rules[0].id, from_yaml.rules[0].id);
        assert_eq!(from_toml.rules[0].xpath, from_yaml.rules[0].xpath);
        assert_eq!(from_toml.rules[0].reason, from_yaml.rules[0].reason);
        assert_eq!(from_toml.rules[0].severity, from_yaml.rules[0].severity);
    }

    // -- Expect examples --

    #[test]
    fn test_parse_yaml_expect_examples() {
        let yaml = r#"
rules:
  - id: no-todo
    xpath: "//comment[contains(.,'TODO')]"
    language: rust
    expect:
      - valid: "fn main() {}"
      - invalid: "// TODO: fix this"
      - invalid: "// TODO: refactor"
"#;
        let rs = parse_rules_yaml(yaml).unwrap();
        assert_eq!(rs.rules.len(), 1);
        assert_eq!(rs.rules[0].valid_examples, vec!["fn main() {}"]);
        assert_eq!(rs.rules[0].invalid_examples, vec!["// TODO: fix this", "// TODO: refactor"]);
    }

    #[test]
    fn test_parse_toml_expect_examples() {
        let toml = r#"
[[rules]]
id = "no-todo"
xpath = "//comment[contains(.,'TODO')]"
language = "rust"

[[rules.expect]]
valid = "fn main() {}"

[[rules.expect]]
invalid = "// TODO: fix this"
"#;
        let rs = parse_rules_toml(toml).unwrap();
        assert_eq!(rs.rules.len(), 1);
        assert_eq!(rs.rules[0].valid_examples, vec!["fn main() {}"]);
        assert_eq!(rs.rules[0].invalid_examples, vec!["// TODO: fix this"]);
    }

    #[test]
    fn test_parse_yaml_no_expect() {
        let yaml = r#"
rules:
  - id: simple
    xpath: "//function"
"#;
        let rs = parse_rules_yaml(yaml).unwrap();
        assert!(rs.rules[0].valid_examples.is_empty());
        assert!(rs.rules[0].invalid_examples.is_empty());
    }
}
