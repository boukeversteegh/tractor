//! Rule data model for batch execution.
//!
//! A `Rule` is a self-contained unit of work: an XPath query plus metadata
//! describing how matches should be interpreted and reported. Rules are
//! storage-agnostic — they can be constructed programmatically, deserialized
//! from any config format, or even extracted from source files via tractor
//! queries.
//!
//! A `RuleSet` groups rules with shared defaults (file globs, tree mode, etc.)
//! so that callers don't need to repeat common configuration on every rule.

use crate::report::Severity;
use crate::tree_mode::TreeMode;

// ---------------------------------------------------------------------------
// Rule
// ---------------------------------------------------------------------------

/// A single query rule: what to search for and how to report matches.
#[derive(Debug, Clone)]
pub struct Rule {
    /// Unique identifier for this rule (e.g. "no-unwrap", "require-tests").
    /// Used in reports to attribute matches to their originating rule.
    pub id: String,

    /// XPath expression to execute against each parsed document.
    pub xpath: String,

    /// Human-readable explanation shown for each match (the "why").
    pub reason: Option<String>,

    /// Severity of matches. Determines whether a match causes check failure.
    pub severity: Severity,

    /// Custom message template with placeholders ({value}, {line}, {col}, {file}).
    pub message: Option<String>,

    /// File globs to restrict this rule to (e.g. ["**/*.rs", "**/*.ts"]).
    /// Empty means "use the files provided by the caller".
    pub include: Vec<String>,

    /// File globs to exclude from matching.
    pub exclude: Vec<String>,

    /// Language override for parsing (e.g. "rust", "typescript").
    /// None means auto-detect from file extension.
    pub language: Option<String>,

    /// Tree mode override for this rule.
    /// None means use the default (auto-detect per language).
    pub tree_mode: Option<TreeMode>,
}

impl Rule {
    /// Create a rule with just an id and xpath. All other fields use defaults.
    pub fn new(id: impl Into<String>, xpath: impl Into<String>) -> Self {
        Rule {
            id: id.into(),
            xpath: xpath.into(),
            reason: None,
            severity: Severity::Error,
            message: None,
            include: Vec::new(),
            exclude: Vec::new(),
            language: None,
            tree_mode: None,
        }
    }

    /// Set the reason message.
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Set the severity level.
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    /// Set the message template.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Set include globs.
    pub fn with_include(mut self, include: Vec<String>) -> Self {
        self.include = include;
        self
    }

    /// Set exclude globs.
    pub fn with_exclude(mut self, exclude: Vec<String>) -> Self {
        self.exclude = exclude;
        self
    }

    /// Set the language override.
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Set the tree mode override.
    pub fn with_tree_mode(mut self, tree_mode: TreeMode) -> Self {
        self.tree_mode = Some(tree_mode);
        self
    }
}

// ---------------------------------------------------------------------------
// RuleSet
// ---------------------------------------------------------------------------

/// A collection of rules with shared defaults.
///
/// Fields on individual rules take precedence over ruleset defaults.
/// For example, if a rule has `include: ["**/*.rs"]` and the ruleset has
/// `default_include: ["**/*.ts"]`, the rule's include wins.
#[derive(Debug, Clone)]
pub struct RuleSet {
    /// The rules in this set.
    pub rules: Vec<Rule>,

    /// Default include globs applied when a rule's `include` is empty.
    pub default_include: Vec<String>,

    /// Default exclude globs applied when a rule's `exclude` is empty.
    pub default_exclude: Vec<String>,

    /// Default tree mode applied when a rule's `tree_mode` is None.
    pub default_tree_mode: Option<TreeMode>,

    /// Default language applied when a rule's `language` is None.
    pub default_language: Option<String>,
}

impl RuleSet {
    /// Create an empty ruleset.
    pub fn new() -> Self {
        RuleSet {
            rules: Vec::new(),
            default_include: Vec::new(),
            default_exclude: Vec::new(),
            default_tree_mode: None,
            default_language: None,
        }
    }

    /// Create a ruleset from a vec of rules.
    pub fn from_rules(rules: Vec<Rule>) -> Self {
        RuleSet {
            rules,
            ..Self::new()
        }
    }

    /// Add a rule to this set.
    pub fn add(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// Resolve the effective include globs for a rule.
    /// Returns the rule's own include if non-empty, otherwise the ruleset default.
    pub fn effective_include<'a>(&'a self, rule: &'a Rule) -> &'a [String] {
        if rule.include.is_empty() {
            &self.default_include
        } else {
            &rule.include
        }
    }

    /// Resolve the effective exclude globs for a rule.
    pub fn effective_exclude<'a>(&'a self, rule: &'a Rule) -> &'a [String] {
        if rule.exclude.is_empty() {
            &self.default_exclude
        } else {
            &rule.exclude
        }
    }

    /// Resolve the effective tree mode for a rule.
    pub fn effective_tree_mode(&self, rule: &Rule) -> Option<TreeMode> {
        rule.tree_mode.or(self.default_tree_mode)
    }

    /// Resolve the effective language for a rule.
    pub fn effective_language<'a>(&'a self, rule: &'a Rule) -> Option<&'a str> {
        rule.language.as_deref().or(self.default_language.as_deref())
    }
}

impl Default for RuleSet {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_builder() {
        let rule = Rule::new("no-unwrap", "//call[name='unwrap']")
            .with_reason("Prefer ? operator over .unwrap()")
            .with_severity(Severity::Warning)
            .with_include(vec!["**/*.rs".into()]);

        assert_eq!(rule.id, "no-unwrap");
        assert_eq!(rule.xpath, "//call[name='unwrap']");
        assert_eq!(rule.reason.as_deref(), Some("Prefer ? operator over .unwrap()"));
        assert_eq!(rule.severity, Severity::Warning);
        assert_eq!(rule.include, vec!["**/*.rs".to_string()]);
        assert!(rule.exclude.is_empty());
        assert!(rule.language.is_none());
        assert!(rule.tree_mode.is_none());
    }

    #[test]
    fn test_rule_defaults() {
        let rule = Rule::new("test", "//function");

        assert_eq!(rule.severity, Severity::Error);
        assert!(rule.reason.is_none());
        assert!(rule.message.is_none());
        assert!(rule.include.is_empty());
        assert!(rule.exclude.is_empty());
        assert!(rule.language.is_none());
        assert!(rule.tree_mode.is_none());
    }

    #[test]
    fn test_ruleset_effective_include() {
        let mut rs = RuleSet::new();
        rs.default_include = vec!["**/*.ts".into(), "**/*.js".into()];

        let rule_no_include = Rule::new("a", "//x");
        let rule_with_include = Rule::new("b", "//y")
            .with_include(vec!["**/*.rs".into()]);

        // Rule without include inherits from ruleset
        assert_eq!(rs.effective_include(&rule_no_include), &["**/*.ts", "**/*.js"]);
        // Rule with include uses its own
        assert_eq!(rs.effective_include(&rule_with_include), &["**/*.rs"]);
    }

    #[test]
    fn test_ruleset_effective_tree_mode() {
        let mut rs = RuleSet::new();
        rs.default_tree_mode = Some(TreeMode::Data);

        let rule_no_mode = Rule::new("a", "//x");
        let rule_with_mode = Rule::new("b", "//y")
            .with_tree_mode(TreeMode::Raw);

        assert_eq!(rs.effective_tree_mode(&rule_no_mode), Some(TreeMode::Data));
        assert_eq!(rs.effective_tree_mode(&rule_with_mode), Some(TreeMode::Raw));
    }

    #[test]
    fn test_ruleset_effective_language() {
        let mut rs = RuleSet::new();
        rs.default_language = Some("typescript".into());

        let rule_no_lang = Rule::new("a", "//x");
        let rule_with_lang = Rule::new("b", "//y")
            .with_language("rust");

        assert_eq!(rs.effective_language(&rule_no_lang), Some("typescript"));
        assert_eq!(rs.effective_language(&rule_with_lang), Some("rust"));
    }

    #[test]
    fn test_ruleset_from_rules() {
        let rules = vec![
            Rule::new("a", "//x"),
            Rule::new("b", "//y"),
        ];
        let rs = RuleSet::from_rules(rules);
        assert_eq!(rs.rules.len(), 2);
        assert_eq!(rs.rules[0].id, "a");
        assert_eq!(rs.rules[1].id, "b");
    }

    #[test]
    fn test_ruleset_add() {
        let mut rs = RuleSet::new();
        assert_eq!(rs.rules.len(), 0);

        rs.add(Rule::new("a", "//x"));
        rs.add(Rule::new("b", "//y"));
        assert_eq!(rs.rules.len(), 2);
    }
}
