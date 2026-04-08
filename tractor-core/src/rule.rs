//! Rule data model for batch execution.
//!
//! A `Rule` is a self-contained unit of work: an XPath query plus metadata
//! describing how matches should be interpreted and reported. Rules are
//! storage-agnostic — they can be constructed programmatically, deserialized
//! from any config format, or even extracted from source files via tractor
//! queries.
//!
//! A `RuleSet` groups rules under a global file scope. The ruleset's own
//! `include`/`exclude` globs act as a hard boundary — individual rule globs
//! can only narrow within that boundary, never widen it.
//!
//! ## Glob intersection semantics
//!
//! A file is eligible for a rule when **all** of the following hold:
//!
//! 1. It matches at least one ruleset `include` pattern (or include is empty → everything allowed)
//! 2. It matches at least one rule `include` pattern (or include is empty → everything allowed)
//! 3. It does **not** match any ruleset `exclude` pattern
//! 4. It does **not** match any rule `exclude` pattern
//!
//! This means `include` lists intersect (both must pass) while `exclude` lists
//! union (either can reject).

use crate::normalized_xpath::NormalizedXpath;
use crate::report::Severity;
use crate::tree_mode::TreeMode;

// ---------------------------------------------------------------------------
// GlobMatcher (native only — needs the glob crate for Pattern)
// ---------------------------------------------------------------------------

#[cfg(feature = "native")]
pub use glob_matcher::GlobMatcher;
#[cfg(feature = "native")]
pub use glob_matcher::GlobError;

#[cfg(feature = "native")]
mod glob_matcher {
    use glob::Pattern;
    use glob::MatchOptions;

    /// Error returned when a glob pattern string is invalid.
    #[derive(Debug, Clone)]
    pub struct GlobError {
        pub pattern: String,
        pub message: String,
    }

    impl std::fmt::Display for GlobError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "invalid glob '{}': {}", self.pattern, self.message)
        }
    }

    impl std::error::Error for GlobError {}

    fn compile(patterns: &[String]) -> Result<Vec<Pattern>, GlobError> {
        patterns
            .iter()
            .map(|p| {
                Pattern::new(p).map_err(|e| GlobError {
                    pattern: p.clone(),
                    message: e.msg.to_string(),
                })
            })
            .collect()
    }

    const OPTS: MatchOptions = MatchOptions {
        case_sensitive: true,
        require_literal_separator: false,
        require_literal_leading_dot: false,
    };

    fn any_matches(patterns: &[Pattern], path: &str) -> bool {
        patterns.iter().any(|p| p.matches_with(path, OPTS))
    }

    /// Compiled glob patterns for path matching.
    ///
    /// Constructed from two layers of include/exclude patterns (ruleset + rule).
    /// Implements intersection semantics: both include layers must pass, either
    /// exclude layer can reject.
    #[derive(Debug, Clone)]
    pub struct GlobMatcher {
        /// Ruleset-level include patterns (hard boundary).
        rs_include: Vec<Pattern>,
        /// Rule-level include patterns (further narrowing).
        r_include: Vec<Pattern>,
        /// All exclude patterns (ruleset + rule, unioned).
        exclude: Vec<Pattern>,
    }

    impl GlobMatcher {
        /// Compile a matcher from two layers of glob pattern strings.
        ///
        /// - Both include layers must pass (intersection).
        /// - Either exclude layer can reject (union).
        /// - An empty include layer is permissive (matches everything).
        pub fn new(
            ruleset_include: &[String],
            ruleset_exclude: &[String],
            rule_include: &[String],
            rule_exclude: &[String],
        ) -> Result<Self, GlobError> {
            Ok(GlobMatcher {
                rs_include: compile(ruleset_include)?,
                r_include: compile(rule_include)?,
                exclude: compile(ruleset_exclude)?
                    .into_iter()
                    .chain(compile(rule_exclude)?)
                    .collect(),
            })
        }

        /// Does `path` pass through the include/exclude filter?
        ///
        /// A path matches when:
        /// 1. It matches at least one ruleset include (or that list is empty)
        /// 2. It matches at least one rule include (or that list is empty)
        /// 3. It does not match any exclude pattern
        pub fn matches(&self, path: &str) -> bool {
            // Include intersection: both layers must pass
            if !self.rs_include.is_empty() && !any_matches(&self.rs_include, path) {
                return false;
            }
            if !self.r_include.is_empty() && !any_matches(&self.r_include, path) {
                return false;
            }
            // Exclude union: any match rejects
            if any_matches(&self.exclude, path) {
                return false;
            }
            true
        }

        /// Returns true if both include layers are empty and there are no excludes.
        /// In this case every path matches.
        pub fn is_empty(&self) -> bool {
            self.rs_include.is_empty() && self.r_include.is_empty() && self.exclude.is_empty()
        }
    }
}

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
    pub xpath: NormalizedXpath,

    /// Human-readable explanation shown for each match (the "why").
    pub reason: Option<String>,

    /// Severity of matches. Determines whether a match causes check failure.
    pub severity: Severity,

    /// Custom message template with placeholders ({value}, {line}, {col}, {file}).
    pub message: Option<String>,

    /// File globs to further narrow this rule within the ruleset boundary.
    /// Empty means "all files allowed by the ruleset".
    pub include: Vec<String>,

    /// File globs to exclude (in addition to ruleset excludes).
    pub exclude: Vec<String>,

    /// Language override for parsing (e.g. "rust", "typescript").
    /// None means auto-detect from file extension.
    pub language: Option<String>,

    /// Tree mode override for this rule.
    /// None means use the default (auto-detect per language).
    pub tree_mode: Option<TreeMode>,

    /// Code examples that should pass the check (no matches expected).
    /// In config files: `expect: [{valid: "..."}]`, CLI: `--expect-valid`.
    pub valid_examples: Vec<String>,

    /// Code examples that should fail the check (1+ matches expected).
    /// In config files: `expect: [{invalid: "..."}]`, CLI: `--expect-invalid`.
    pub invalid_examples: Vec<String>,
}

impl Rule {
    /// Create a rule with just an id and xpath. All other fields use defaults.
    pub fn new(id: impl Into<String>, xpath: impl Into<NormalizedXpath>) -> Self {
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
            valid_examples: Vec::new(),
            invalid_examples: Vec::new(),
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

    /// Set valid examples (code that should NOT trigger the check).
    pub fn with_valid_examples(mut self, examples: Vec<String>) -> Self {
        self.valid_examples = examples;
        self
    }

    /// Set invalid examples (code that SHOULD trigger the check).
    pub fn with_invalid_examples(mut self, examples: Vec<String>) -> Self {
        self.invalid_examples = examples;
        self
    }

    /// Returns true if this rule has any examples to validate.
    pub fn has_examples(&self) -> bool {
        !self.valid_examples.is_empty() || !self.invalid_examples.is_empty()
    }
}

// ---------------------------------------------------------------------------
// RuleSet
// ---------------------------------------------------------------------------

/// A collection of rules under a shared file scope.
///
/// The ruleset's `include`/`exclude` define the hard boundary. Individual
/// rule globs can only narrow within that boundary — they cannot widen it.
///
/// For non-glob fields (`default_tree_mode`, `default_language`), the ruleset
/// provides fallback defaults that rules can override.
#[derive(Debug, Clone)]
pub struct RuleSet {
    /// The rules in this set.
    pub rules: Vec<Rule>,

    /// Global include boundary. Only files matching at least one of these
    /// patterns are eligible for any rule. Empty means no restriction.
    pub include: Vec<String>,

    /// Global exclude boundary. Files matching any of these patterns are
    /// excluded from all rules.
    pub exclude: Vec<String>,

    /// Default tree mode applied when a rule's `tree_mode` is None.
    pub default_tree_mode: Option<TreeMode>,

    /// Default language applied when a rule's `language` is None.
    pub default_language: Option<String>,
}

impl RuleSet {
    /// Create an empty ruleset with no restrictions.
    pub fn new() -> Self {
        RuleSet {
            rules: Vec::new(),
            include: Vec::new(),
            exclude: Vec::new(),
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

    /// Compile a [`GlobMatcher`] for a specific rule, combining the ruleset's
    /// global boundary with the rule's own globs.
    #[cfg(feature = "native")]
    pub fn glob_matcher(&self, rule: &Rule) -> Result<GlobMatcher, GlobError> {
        GlobMatcher::new(&self.include, &self.exclude, &rule.include, &rule.exclude)
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

    // -----------------------------------------------------------------------
    // GlobMatcher tests (native only)
    // -----------------------------------------------------------------------

    #[cfg(feature = "native")]
    mod glob_tests {
        use super::*;

        #[test]
        fn test_empty_matcher_matches_everything() {
            let m = GlobMatcher::new(&[], &[], &[], &[]).unwrap();
            assert!(m.matches("anything.rs"));
            assert!(m.matches("deeply/nested/path.ts"));
            assert!(m.is_empty());
        }

        #[test]
        fn test_ruleset_include_only() {
            let m = GlobMatcher::new(
                &["**/*.rs".into()],
                &[],
                &[],
                &[],
            ).unwrap();

            assert!(m.matches("src/main.rs"));
            assert!(m.matches("lib.rs"));
            assert!(!m.matches("index.ts"));
        }

        #[test]
        fn test_rule_include_only() {
            let m = GlobMatcher::new(
                &[],
                &[],
                &["**/*.rs".into()],
                &[],
            ).unwrap();

            assert!(m.matches("src/main.rs"));
            assert!(!m.matches("index.ts"));
        }

        #[test]
        fn test_include_intersection() {
            // Ruleset allows all .rs and .ts files
            // Rule further narrows to only src/ directory
            let m = GlobMatcher::new(
                &["**/*.rs".into(), "**/*.ts".into()],
                &[],
                &["src/**".into()],
                &[],
            ).unwrap();

            assert!(m.matches("src/main.rs"));       // .rs + in src/
            assert!(m.matches("src/index.ts"));       // .ts + in src/
            assert!(!m.matches("test/main.rs"));      // .rs but not in src/
            assert!(!m.matches("src/data.json"));     // in src/ but not .rs/.ts
        }

        #[test]
        fn test_rule_cannot_widen_beyond_ruleset() {
            // Ruleset only allows src/**
            // Rule asks for **/*.rs (including test/)
            let m = GlobMatcher::new(
                &["src/**".into()],
                &[],
                &["**/*.rs".into()],
                &[],
            ).unwrap();

            assert!(m.matches("src/main.rs"));        // in src/ + .rs
            assert!(!m.matches("test/main.rs"));       // .rs but outside src/ boundary
            assert!(!m.matches("src/data.json"));      // in src/ but not .rs
        }

        #[test]
        fn test_exclude_union() {
            // Ruleset excludes vendor/
            // Rule excludes generated/
            let m = GlobMatcher::new(
                &[],
                &["vendor/**".into()],
                &[],
                &["generated/**".into()],
            ).unwrap();

            assert!(m.matches("src/main.rs"));
            assert!(!m.matches("vendor/lib.rs"));       // ruleset exclude
            assert!(!m.matches("generated/code.rs"));   // rule exclude
        }

        #[test]
        fn test_exclude_overrides_include() {
            let m = GlobMatcher::new(
                &["**/*.rs".into()],
                &["test/**".into()],
                &[],
                &[],
            ).unwrap();

            assert!(m.matches("src/main.rs"));
            assert!(!m.matches("test/main.rs"));  // included by glob, but excluded
        }

        #[test]
        fn test_full_intersection() {
            // Ruleset: all rust/ts in src/, exclude vendor/
            // Rule: only test files, exclude snapshots
            let m = GlobMatcher::new(
                &["src/**/*.rs".into(), "src/**/*.ts".into()],
                &["src/vendor/**".into()],
                &["**/*_test.*".into()],
                &["**/*_snapshot*".into()],
            ).unwrap();

            assert!(m.matches("src/foo_test.rs"));
            assert!(!m.matches("src/foo.rs"));                  // not a test file
            assert!(!m.matches("test/foo_test.rs"));            // outside src/
            assert!(!m.matches("src/vendor/foo_test.rs"));      // vendored
            assert!(!m.matches("src/foo_test_snapshot.rs"));    // snapshot
        }

        #[test]
        fn test_invalid_pattern() {
            let result = GlobMatcher::new(&["[invalid".into()], &[], &[], &[]);
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(err.pattern, "[invalid");
        }

        #[test]
        fn test_glob_matcher_via_ruleset() {
            let mut rs = RuleSet::new();
            rs.include = vec!["src/**".into()];
            rs.exclude = vec!["src/vendor/**".into()];

            let rule = Rule::new("a", "//x")
                .with_include(vec!["**/*.rs".into()]);

            let matcher = rs.glob_matcher(&rule).unwrap();
            assert!(matcher.matches("src/main.rs"));
            assert!(!matcher.matches("test/main.rs"));       // outside ruleset boundary
            assert!(!matcher.matches("src/vendor/lib.rs"));   // excluded by ruleset
            assert!(!matcher.matches("src/index.ts"));         // excluded by rule include
        }

        #[test]
        fn test_glob_matcher_no_restrictions() {
            let rs = RuleSet::new();
            let rule = Rule::new("a", "//x");

            let matcher = rs.glob_matcher(&rule).unwrap();
            assert!(matcher.is_empty());
            assert!(matcher.matches("literally/anything"));
        }

        #[test]
        fn relative_include_does_not_match_absolute_path() {
            // Rule include "src/**/*.rs" should match relative paths...
            let m = GlobMatcher::new(&[], &[], &["src/**/*.rs".into()], &[]).unwrap();
            assert!(m.matches("src/main.rs"));
            assert!(m.matches("src/deep/nested/lib.rs"));

            // ...but does NOT match absolute paths (glob::Pattern does full-string matching)
            assert!(!m.matches("/home/user/project/src/main.rs"));
            assert!(!m.matches("/home/user/project/src/deep/nested/lib.rs"));
        }

        #[test]
        fn relative_exclude_does_not_match_absolute_path() {
            // Rule exclude "test/**" rejects relative paths...
            let m = GlobMatcher::new(&[], &[], &[], &["test/**".into()]).unwrap();
            assert!(!m.matches("test/foo.rs"));

            // ...but fails to reject the same file via absolute path
            assert!(m.matches("/home/user/project/test/foo.rs")); // limitation: should be rejected
        }
    }
}
