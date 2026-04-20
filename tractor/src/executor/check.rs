//! Check operation: run XPath rules against sources, report violations.

use tractor::report::{ReportBuilder, ReportMatch, Severity};
use tractor::tree_mode::TreeMode;
use tractor::rule::CompiledRule;
use tractor::{parse, ParseInput, ParseOptions};

use crate::matcher::validate_xpath_diagnostic;
use crate::matcher::run_rules;
use crate::input::filter::Filters;
use crate::input::Source;

use crate::cli::context::ExecCtx;

use super::match_to_report_match;

// ---------------------------------------------------------------------------
// Operation type
// ---------------------------------------------------------------------------

/// A check operation plan: run XPath rules against sources, report violations.
///
/// Construction site has already resolved file globs, CLI intersection,
/// diff-files filter, inline-source wiring, and language detection into
/// `sources`. Per-rule glob patterns have also been resolved against the
/// known `base_dir` and compiled into [`CompiledRule`] entries — so the
/// executor just runs already-prepared matchers. Downstream is a plain
/// `Vec<Source>` × `Vec<CompiledRule>` loop.
#[derive(Debug, Clone)]
pub struct CheckOperationPlan {
    /// Pre-resolved unified input list (disk and/or inline).
    pub sources: Vec<Source>,
    /// Pre-built result filters (diff-lines, etc.). Applied inside
    /// `run_rules` to every match.
    pub filters: Filters,
    /// Rules with globs already resolved + compiled against `base_dir`.
    /// The ruleset-level `include`/`exclude` boundary is folded into
    /// each rule's `GlobMatcher`, and the effective language / tree mode
    /// are pre-resolved using the ruleset defaults.
    pub compiled_rules: Vec<CompiledRule>,
    /// Default tree mode for all rules (rules can override, and then
    /// this value is the final fallback if neither the rule nor the
    /// ruleset specify one).
    pub tree_mode: Option<TreeMode>,
    /// Ignore whitespace-only text nodes during parsing.
    pub ignore_whitespace: bool,
    /// Maximum parse depth.
    pub parse_depth: Option<usize>,
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

pub(crate) fn execute_check(
    op: &CheckOperationPlan,
    ctx: &ExecCtx<'_>,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    if op.compiled_rules.is_empty() {
        return Ok(());
    }

    // --- Phase 0: Validate XPath expressions upfront ---
    let diagnostics: Vec<_> = op.compiled_rules.iter()
        .filter_map(|rule| validate_xpath_diagnostic(&rule.xpath, "check"))
        .collect();
    if !diagnostics.is_empty() {
        report.add_all(diagnostics);
        return Ok(());
    }

    // --- Phase 1: Validate rule examples inline ---
    validate_rule_examples(&op.compiled_rules, op.tree_mode, report)?;

    if op.sources.is_empty() {
        return Ok(());
    }

    let rule_matches = run_rules(
        &op.compiled_rules,
        &op.sources,
        op.tree_mode,
        op.ignore_whitespace,
        op.parse_depth,
        ctx.verbose,
        &op.filters,
    )?;

    for rm in rule_matches {
        let rule = &op.compiled_rules[rm.rule_index];
        let reason = rule
            .reason
            .clone()
            .unwrap_or_else(|| format!("[{}] check failed", rule.id));
        let severity = rule.severity;

        // Apply rule-level message template (if the rule defines one)
        let message = rule
            .message
            .as_deref()
            .map(|t| tractor::format_message(t, &rm.m));

        let mut report_match = match_to_report_match(rm.m, "check");
        report_match.reason = Some(reason);
        report_match.severity = Some(severity);
        report_match.rule_id = Some(rule.id.clone());
        report_match.message = message;
        report.add(report_match);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Example validation
// ---------------------------------------------------------------------------

/// Validate rule examples by parsing and querying inline.
///
/// For each rule with examples, parses the example source and runs the rule's
/// XPath query. Adds failure matches to the builder for any unmet expectations.
fn validate_rule_examples(
    rules: &[CompiledRule],
    default_tree_mode: Option<TreeMode>,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    for rule in rules {
        if !rule.has_examples() {
            continue;
        }

        let lang = rule.language.as_deref()
            .ok_or_else(|| format!(
                "rule '{}' has examples but no language specified (set language on the rule or check operation)",
                rule.id
            ))?;

        let tree_mode = rule.tree_mode.or(default_tree_mode);

        // Validate valid examples: expect "none" (query should NOT match valid code)
        for (i, example) in rule.valid_examples.iter().enumerate() {
            let mut result = parse(
                ParseInput::Inline {
                    content: example,
                    file_label: tractor::PATHLESS_LABEL,
                },
                ParseOptions {
                    language: Some(lang),
                    tree_mode,
                    ignore_whitespace: false,
                    parse_depth: None,
                },
            )?;
            let matches = result.query(rule.xpath.as_str())?;
            if !super::check_expectation("none", matches.len())? {
                report.add(example_failure_match(
                    &rule.id,
                    &format!(
                        "[{}] valid example {} unexpectedly matched query",
                        rule.id, i + 1
                    ),
                ));
            }
        }

        // Validate invalid examples: expect "some" (query SHOULD match invalid code)
        for (i, example) in rule.invalid_examples.iter().enumerate() {
            let mut result = parse(
                ParseInput::Inline {
                    content: example,
                    file_label: tractor::PATHLESS_LABEL,
                },
                ParseOptions {
                    language: Some(lang),
                    tree_mode,
                    ignore_whitespace: false,
                    parse_depth: None,
                },
            )?;
            let matches = result.query(rule.xpath.as_str())?;
            if !super::check_expectation("some", matches.len())? {
                report.add(example_failure_match(
                    &rule.id,
                    &format!(
                        "[{}] invalid example {} did not match query",
                        rule.id, i + 1
                    ),
                ));
            }
        }
    }

    Ok(())
}

/// Build a synthetic ReportMatch for a failed example validation.
fn example_failure_match(rule_id: &str, reason: &str) -> ReportMatch {
    ReportMatch {
        file: String::new(),
        line: 0,
        column: 0,
        end_line: 0,
        end_column: 0,
        command: "test".to_string(),
        tree: None,
        value: None,
        source: None,
        lines: None,
        reason: Some(reason.to_string()),
        severity: Some(Severity::Error),
        message: None,
        origin: None,
        rule_id: Some(rule_id.to_string()),
        status: None,
        output: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tractor::report::ReportBuilder;
    use tractor::rule::Rule;

    /// Compile a single rule into a `CompiledRule`, optionally with a
    /// fallback default language. Used by the example-validation tests
    /// below to mirror the old `default_language` argument.
    fn compile(rule: Rule, default_language: Option<&str>) -> CompiledRule {
        tractor::compile_ruleset(&[], &[], default_language, None, vec![rule], None)
            .expect("compile_ruleset should not fail on no-glob rules")
            .pop()
            .unwrap()
    }

    #[test]
    fn test_validate_examples_pass_and_fail_correct() {
        let rules = vec![compile(
            Rule::new("no-comments", "//line_comment")
                .with_language("rust")
                .with_valid_examples(vec!["fn main() {}".to_string()])
                .with_invalid_examples(vec!["// hello\nfn main() {}".to_string()]),
            None,
        )];
        let mut builder = ReportBuilder::new();
        validate_rule_examples(&rules, None, &mut builder).unwrap();
        let report = builder.build();
        assert!(report.all_matches().is_empty(), "expected no failures: {:?}", report.all_matches());
    }

    #[test]
    fn test_validate_examples_valid_unexpectedly_matches() {
        let rules = vec![compile(
            Rule::new("no-comments", "//line_comment")
                .with_language("rust")
                .with_valid_examples(vec!["// oops this is a comment".to_string()]),
            None,
        )];
        let mut builder = ReportBuilder::new();
        validate_rule_examples(&rules, None, &mut builder).unwrap();
        let report = builder.build();
        let matches = report.all_matches();
        assert_eq!(matches.len(), 1);
        assert!(matches[0].reason.as_ref().unwrap().contains("valid example 1 unexpectedly matched"));
    }

    #[test]
    fn test_validate_examples_invalid_does_not_match() {
        let rules = vec![compile(
            Rule::new("no-comments", "//line_comment")
                .with_language("rust")
                .with_invalid_examples(vec!["fn main() {}".to_string()]),
            None,
        )];
        let mut builder = ReportBuilder::new();
        validate_rule_examples(&rules, None, &mut builder).unwrap();
        let report = builder.build();
        let matches = report.all_matches();
        assert_eq!(matches.len(), 1);
        assert!(matches[0].reason.as_ref().unwrap().contains("invalid example 1 did not match"));
    }

    #[test]
    fn test_validate_examples_language_from_operation() {
        let rules = vec![compile(
            Rule::new("no-comments", "//line_comment")
                .with_valid_examples(vec!["fn main() {}".to_string()]),
            Some("rust"),
        )];
        let mut builder = ReportBuilder::new();
        validate_rule_examples(&rules, None, &mut builder).unwrap();
        let report = builder.build();
        assert!(report.all_matches().is_empty());
    }

    #[test]
    fn test_validate_examples_no_language_errors() {
        let rules = vec![compile(
            Rule::new("no-comments", "//line_comment")
                .with_valid_examples(vec!["fn main() {}".to_string()]),
            None,
        )];
        let mut builder = ReportBuilder::new();
        let err = validate_rule_examples(&rules, None, &mut builder).unwrap_err();
        assert!(err.to_string().contains("no language specified"));
    }

    #[test]
    fn test_validate_examples_no_examples_is_noop() {
        let rules = vec![compile(Rule::new("simple", "//function"), None)];
        let mut builder = ReportBuilder::new();
        validate_rule_examples(&rules, None, &mut builder).unwrap();
        let report = builder.build();
        assert!(report.all_matches().is_empty());
    }
}
