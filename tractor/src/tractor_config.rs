//! Config file parser for tractor batch execution.
//!
//! Parses YAML or TOML config files into a flat `Vec<Operation>`.
//! Supports two forms:
//!
//! 1. Root-level command keys (one of each type):
//!    ```yaml
//!    check:
//!      files: [...]
//!      rules: [...]
//!    set:
//!      files: [...]
//!      mappings: [...]
//!    ```
//!
//! 2. Explicit operations list (ordered, allows duplicates):
//!    ```yaml
//!    operations:
//!      - check:
//!          files: [...]
//!          rules: [...]
//!      - set:
//!          files: [...]
//!          mappings: [...]
//!    ```
//!
//! Both forms produce the same `Vec<Operation>`. When both are present,
//! root-level keys are expanded first, then the operations list is appended.

use std::path::Path;
use serde::Deserialize;
use tractor_core::report::Severity;
use tractor_core::rule::Rule;
use tractor_core::tree_mode::TreeMode;

use crate::executor::{
    CheckOperation, Operation, SetMapping, SetOperation,
};

// ---------------------------------------------------------------------------
// Serde schema
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
struct ConfigFile {
    /// Root-level check shorthand (single check operation).
    #[serde(default)]
    check: Option<CheckConfig>,

    /// Root-level set shorthand (single set operation).
    #[serde(default)]
    set: Option<SetConfig>,

    /// Explicit ordered list of operations.
    #[serde(default)]
    operations: Vec<OperationEntry>,
}

/// A single entry in the operations list.
/// Deserialized from YAML like:
///   - check:
///       files: [...]
///   - set:
///       files: [...]
#[derive(Deserialize, Debug)]
struct OperationEntry {
    #[serde(default)]
    check: Option<CheckConfig>,
    #[serde(default)]
    set: Option<SetConfig>,
}

#[derive(Deserialize, Debug)]
struct CheckConfig {
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    rules: Vec<CheckRuleConfig>,
    #[serde(default)]
    tree_mode: Option<String>,
    #[serde(default)]
    language: Option<String>,
}

#[derive(Deserialize, Debug)]
struct CheckRuleConfig {
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
}

#[derive(Deserialize, Debug)]
struct SetConfig {
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    mappings: Vec<SetMappingConfig>,
    #[serde(default)]
    tree_mode: Option<String>,
    #[serde(default)]
    language: Option<String>,
}

#[derive(Deserialize, Debug)]
struct SetMappingConfig {
    xpath: String,
    value: String,
}

fn default_severity() -> String {
    "error".to_string()
}

// ---------------------------------------------------------------------------
// Conversion
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

fn convert_check(config: CheckConfig) -> Result<Operation, Box<dyn std::error::Error>> {
    let tree_mode = config.tree_mode.as_deref().map(parse_tree_mode).transpose()?;

    let rules: Vec<Rule> = config.rules.into_iter().map(|r| {
        let severity = parse_severity(&r.severity)?;
        let mut rule = Rule::new(r.id, r.xpath).with_severity(severity);
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
        Ok::<Rule, Box<dyn std::error::Error>>(rule)
    }).collect::<Result<_, _>>()?;

    Ok(Operation::Check(CheckOperation {
        files: config.files,
        exclude: config.exclude,
        rules,
        tree_mode,
        language: config.language,
    }))
}

fn convert_set(config: SetConfig) -> Result<Operation, Box<dyn std::error::Error>> {
    let tree_mode = config.tree_mode.as_deref().map(parse_tree_mode).transpose()?;

    let mappings = config.mappings.into_iter().map(|m| {
        SetMapping {
            xpath: m.xpath,
            value: m.value,
        }
    }).collect();

    Ok(Operation::Set(SetOperation {
        files: config.files,
        exclude: config.exclude,
        mappings,
        tree_mode,
        language: config.language,
    }))
}

fn config_to_operations(config: ConfigFile) -> Result<Vec<Operation>, Box<dyn std::error::Error>> {
    let mut ops = Vec::new();

    // Root-level shorthand keys first
    if let Some(check) = config.check {
        ops.push(convert_check(check)?);
    }
    if let Some(set) = config.set {
        ops.push(convert_set(set)?);
    }

    // Then explicit operations list
    for entry in config.operations {
        if let Some(c) = entry.check {
            ops.push(convert_check(c)?);
        }
        if let Some(s) = entry.set {
            ops.push(convert_set(s)?);
        }
    }

    Ok(ops)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse a tractor config file into a list of operations.
/// Format is detected from the file extension.
pub fn load_tractor_config(path: &Path) -> Result<Vec<Operation>, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read '{}': {}", path.display(), e))?;

    match path.extension().and_then(|e| e.to_str()) {
        Some("yml") | Some("yaml") => parse_config_yaml(&content),
        Some("toml") => parse_config_toml(&content),
        Some(ext) => Err(format!(
            "unsupported config file extension '.{}': use .yaml, .yml, or .toml",
            ext
        ).into()),
        None => Err("config file has no extension: use .yaml, .yml, or .toml".into()),
    }
}

/// Parse a tractor config from a YAML string.
pub fn parse_config_yaml(content: &str) -> Result<Vec<Operation>, Box<dyn std::error::Error>> {
    let config: ConfigFile = serde_yaml::from_str(content)
        .map_err(|e| format!("invalid tractor config YAML: {}", e))?;
    config_to_operations(config)
}

/// Parse a tractor config from a TOML string.
pub fn parse_config_toml(content: &str) -> Result<Vec<Operation>, Box<dyn std::error::Error>> {
    let config: ConfigFile = toml::from_str(content)
        .map_err(|e| format!("invalid tractor config TOML: {}", e))?;
    config_to_operations(config)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_yaml_root_level_check() {
        let yaml = r#"
check:
  files: ["src/**/*.rs"]
  rules:
    - id: no-todo
      xpath: "//comment[contains(.,'TODO')]"
      reason: "TODO found"
"#;
        let ops = parse_config_yaml(yaml).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operation::Check(c) => {
                assert_eq!(c.files, vec!["src/**/*.rs"]);
                assert_eq!(c.rules.len(), 1);
                assert_eq!(c.rules[0].id, "no-todo");
                assert_eq!(c.rules[0].reason.as_deref(), Some("TODO found"));
            }
            _ => panic!("expected Check operation"),
        }
    }

    #[test]
    fn parse_yaml_root_level_set() {
        let yaml = r#"
set:
  files: ["config.json"]
  mappings:
    - xpath: "//database/host"
      value: "localhost"
    - xpath: "//database/port"
      value: "5432"
"#;
        let ops = parse_config_yaml(yaml).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operation::Set(s) => {
                assert_eq!(s.files, vec!["config.json"]);
                assert_eq!(s.mappings.len(), 2);
                assert_eq!(s.mappings[0].xpath, "//database/host");
                assert_eq!(s.mappings[0].value, "localhost");
                assert_eq!(s.mappings[1].xpath, "//database/port");
                assert_eq!(s.mappings[1].value, "5432");
            }
            _ => panic!("expected Set operation"),
        }
    }

    #[test]
    fn parse_yaml_both_root_level_keys() {
        let yaml = r#"
check:
  files: ["src/**/*.rs"]
  rules:
    - id: no-todo
      xpath: "//comment"
set:
  files: ["config.json"]
  mappings:
    - xpath: "//host"
      value: "localhost"
"#;
        let ops = parse_config_yaml(yaml).unwrap();
        assert_eq!(ops.len(), 2);
        assert!(matches!(&ops[0], Operation::Check(_)));
        assert!(matches!(&ops[1], Operation::Set(_)));
    }

    #[test]
    fn parse_yaml_operations_list() {
        let yaml = r#"
operations:
  - check:
      files: ["src/**/*.rs"]
      rules:
        - id: rule-a
          xpath: "//a"
  - set:
      files: ["config.json"]
      mappings:
        - xpath: "//host"
          value: "localhost"
  - check:
      files: ["test/**/*.rs"]
      rules:
        - id: rule-b
          xpath: "//b"
"#;
        let ops = parse_config_yaml(yaml).unwrap();
        assert_eq!(ops.len(), 3);
        assert!(matches!(&ops[0], Operation::Check(_)));
        assert!(matches!(&ops[1], Operation::Set(_)));
        assert!(matches!(&ops[2], Operation::Check(_)));

        // Verify ordering is preserved
        if let Operation::Check(c) = &ops[0] {
            assert_eq!(c.rules[0].id, "rule-a");
        }
        if let Operation::Check(c) = &ops[2] {
            assert_eq!(c.rules[0].id, "rule-b");
        }
    }

    #[test]
    fn parse_yaml_root_plus_operations() {
        let yaml = r#"
check:
  files: ["src/**/*.rs"]
  rules:
    - id: root-check
      xpath: "//a"
operations:
  - set:
      files: ["config.json"]
      mappings:
        - xpath: "//host"
          value: "localhost"
"#;
        let ops = parse_config_yaml(yaml).unwrap();
        assert_eq!(ops.len(), 2);
        // Root-level check comes first
        assert!(matches!(&ops[0], Operation::Check(_)));
        // Then operations list
        assert!(matches!(&ops[1], Operation::Set(_)));
    }

    #[test]
    fn parse_yaml_check_with_severity() {
        let yaml = r#"
check:
  files: ["**/*.rs"]
  rules:
    - id: warn-rule
      xpath: "//x"
      severity: warning
    - id: error-rule
      xpath: "//y"
      severity: error
"#;
        let ops = parse_config_yaml(yaml).unwrap();
        if let Operation::Check(c) = &ops[0] {
            assert_eq!(c.rules[0].severity, Severity::Warning);
            assert_eq!(c.rules[1].severity, Severity::Error);
        }
    }

    #[test]
    fn parse_yaml_set_with_exclude() {
        let yaml = r#"
set:
  files: ["**/*.json"]
  exclude: ["node_modules/**"]
  mappings:
    - xpath: "//version"
      value: "2.0"
"#;
        let ops = parse_config_yaml(yaml).unwrap();
        if let Operation::Set(s) = &ops[0] {
            assert_eq!(s.exclude, vec!["node_modules/**"]);
        }
    }

    #[test]
    fn parse_yaml_empty() {
        let yaml = "{}";
        let ops = parse_config_yaml(yaml).unwrap();
        assert!(ops.is_empty());
    }

    #[test]
    fn parse_yaml_invalid_severity() {
        let yaml = r#"
check:
  rules:
    - id: bad
      xpath: "//x"
      severity: critical
"#;
        let err = parse_config_yaml(yaml).unwrap_err();
        assert!(err.to_string().contains("invalid severity"));
    }

    #[test]
    fn parse_toml_root_level() {
        let toml = r#"
[set]
files = ["config.json"]

[[set.mappings]]
xpath = "//host"
value = "localhost"
"#;
        let ops = parse_config_toml(toml).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operation::Set(s) => {
                assert_eq!(s.mappings.len(), 1);
                assert_eq!(s.mappings[0].value, "localhost");
            }
            _ => panic!("expected Set operation"),
        }
    }
}
