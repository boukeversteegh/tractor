use std::collections::HashSet;

use rayon::prelude::*;
use tractor::{
    Match, NormalizedXpath,
    language_info::parse_language,
    output::{render_document, RenderOptions},
    parse, ParseInput, ParseOptions,
    report::{Report, ReportMatch, Severity, DiagnosticOrigin},
    rule::CompiledRule,
    xpath::validate_xpath,
};
use crate::input::filter::Filters;
use crate::input::Source;

use crate::cli::context::RunContext;
use crate::format::{ViewField, ViewSet};

// ---------------------------------------------------------------------------
// XPath validation
// ---------------------------------------------------------------------------

/// Validate an XPath expression upfront and return a fatal diagnostic if invalid.
///
/// Builds a `ReportMatch` with `Severity::Fatal`, the XPath string as source,
/// and the error position highlighted. Returns `None` if the XPath is valid.
pub fn validate_xpath_diagnostic(xpath_expr: &NormalizedXpath, command: &str) -> Option<ReportMatch> {
    let result = validate_xpath(xpath_expr.as_str());
    if result.valid {
        return None;
    }

    let reason = result.error.as_deref().unwrap_or("invalid XPath expression").to_string();

    // Default: highlight the entire expression
    let line = 1u32;
    let mut col = 1u32;
    let end_line = 1u32;
    let mut end_col = xpath_expr.as_str().len() as u32 + 1;

    // If we have error position info, narrow the highlight to the error span
    if let (Some(start), Some(end)) = (result.error_start, result.error_end) {
        col = start as u32 + 1; // 1-based
        end_col = if end > start { end as u32 + 1 } else { col + 1 };
    }

    Some(ReportMatch {
        file: String::new(),
        line, column: col, end_line, end_column: end_col,
        command: command.to_string(),
        tree: None,
        value: None,
        source: Some(xpath_expr.to_string()),
        lines: Some(vec![xpath_expr.to_string()]),
        reason: Some(reason),
        severity: Some(Severity::Fatal),
        message: None,

        origin: Some(DiagnosticOrigin::Xpath),
        rule_id: None,
        status: None,
        output: None,
    })
}

// ---------------------------------------------------------------------------
// Debug mode
// ---------------------------------------------------------------------------

pub fn run_debug(ctx: &RunContext, files: &[String], xpath_expr: &NormalizedXpath) -> Result<(), Box<dyn std::error::Error>> {
    let mut remaining_limit = ctx.limit;

    for file_path in files {
        if remaining_limit == Some(0) {
            break;
        }

        let mut result = match parse(
            ParseInput::Disk {
                path: std::path::Path::new(file_path),
            },
            ParseOptions {
                language: ctx.lang.as_deref(),
                tree_mode: ctx.tree_mode,
                ignore_whitespace: ctx.ignore_whitespace,
                parse_depth: ctx.parse_depth,
            },
        ) {
            Ok(r) => r,
            Err(e) => {
                if ctx.verbose {
                    eprintln!("warning: {}: {}", file_path, e);
                }
                continue;
            }
        };

        match result.query(xpath_expr.as_str()) {
            Ok(matches) if !matches.is_empty() => {
                let matches: Vec<_> = if let Some(limit) = remaining_limit {
                    let take = limit.min(matches.len());
                    remaining_limit = Some(limit - take);
                    matches.into_iter().take(take).collect()
                } else {
                    matches
                };

                let highlights: HashSet<(u32, u32)> = matches
                    .iter()
                    .map(|m| (m.line, m.column))
                    .collect();

                let doc_node = result.documents.document_node(result.doc_handle).unwrap();
                let render_opts = RenderOptions::new()
                    .with_color(ctx.use_color)
                    .with_meta(true)
                    .with_max_depth(ctx.depth)
                    .with_highlights(highlights)
                    .with_pretty_print(!ctx.no_pretty);
                let output = render_document(result.documents.xot(), doc_node, &render_opts);
                print!("{}", output);
            }
            Ok(_) => {}
            Err(e) => {
                if ctx.verbose {
                    eprintln!("warning: {}: query error: {}", file_path, e);
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Batch rule execution
// ---------------------------------------------------------------------------

/// A match tagged with its originating rule, ready for report building.
pub struct RuleMatch {
    pub rule_index: usize,
    pub m: Match,
}

/// Check if a rule's (already-resolved) language matches the source's
/// pre-resolved language.
///
/// Returns `true` if:
/// - The rule has no explicit language (uses the source's language as-is)
/// - The rule's effective language matches the source's language
///
/// This prevents rules from matching sources of incompatible languages,
/// fixing the mixed-language hang issue where e.g. a markdown rule
/// would incorrectly apply to JavaScript sources. Works for both disk
/// sources (language from extension detection) and inline sources
/// (language from `-l`).
fn rule_language_matches_source(
    rule: &CompiledRule,
    source_language: &str,
) -> bool {
    match rule.language.as_deref() {
        // No language specified → rule uses the source's language as-is
        None => true,
        // Language specified → must match source's language
        Some(rule_lang) => {
            let rule_language = parse_language(rule_lang);
            let source_lang_enum = parse_language(source_language);
            rule_language == source_lang_enum
        }
    }
}

/// Execute all compiled rules against a list of sources.
///
/// The caller has already resolved per-rule glob patterns against the
/// appropriate `base_dir` and compiled them into [`CompiledRule`] entries
/// via `tractor::compile_ruleset`. This function is a pure consumer — it
/// never compiles patterns and never resolves paths.
///
/// Virtual (inline) and disk sources share the same code path: each [`Source`]
/// carries its own path, language, and content accessor, so `run_rules` doesn't
/// need to know where the bytes came from. Rule globs match against
/// `source.path`, letting stdin-with-a-virtual-path participate in `include:` /
/// `exclude:` filtering exactly like a real file.
///
/// For each source, rules are filtered by glob + language, then the source is
/// parsed once using either:
/// - The first applicable rule's language override, or
/// - The source's pre-resolved language (from `-l` or extension detection)
///
/// `verbose` controls whether parse/query warnings are printed to stderr.
pub fn run_rules(
    rules: &[CompiledRule],
    sources: &[Source],
    tree_mode: Option<tractor::TreeMode>,
    ignore_whitespace: bool,
    parse_depth: Option<usize>,
    verbose: bool,
    filters: &Filters,
) -> Result<Vec<RuleMatch>, Box<dyn std::error::Error>> {
    // Process sources in parallel. Each source is parsed once using either:
    // - The source's detected language (when no rules specify a language override)
    // - The effective language from the first applicable rule (when rules specify a language)
    // Note: rule_language_matches_source() ensures all applicable rules are compatible
    // with the source's language, so we won't try to parse a source in multiple languages.
    let results: Vec<Vec<RuleMatch>> = sources
        .par_iter()
        .filter_map(|source| {
            let file_path = &source.path;
            let path_str = file_path.as_str();

            // Determine which rules apply to this source based on globs AND language.
            let applicable: Vec<usize> = rules
                .iter()
                .enumerate()
                .filter(|(_, rule)| {
                    rule.glob.matches(file_path)
                        && rule_language_matches_source(rule, &source.language)
                })
                .map(|(i, _)| i)
                .collect();

            if applicable.is_empty() {
                return None;
            }

            // Resolve language and tree_mode from the first applicable rule.
            // All applicable rules are compatible with this source's language
            // (ensured by rule_language_matches_source filter above).
            let first_rule = &rules[applicable[0]];
            let lang_override = first_rule.language.as_deref();
            let effective_tree_mode = first_rule.tree_mode.or(tree_mode);

            let mut result = match source.parse(
                lang_override,
                effective_tree_mode,
                ignore_whitespace,
                parse_depth,
            ) {
                Ok(r) => r,
                Err(e) => {
                    if verbose {
                        eprintln!("warning: {}: {}", path_str, e);
                    }
                    return None;
                }
            };

            let mut file_matches = Vec::new();

            // Run all applicable rules against the parsed result
            for rule_idx in applicable {
                match result.query(rules[rule_idx].xpath.as_str()) {
                    Ok(matches) => {
                        for m in matches {
                            file_matches.push(RuleMatch {
                                rule_index: rule_idx,
                                m,
                            });
                        }
                    }
                    Err(e) => {
                        if verbose {
                            eprintln!(
                                "warning: {}: rule '{}' query error: {}",
                                file_path, rules[rule_idx].id, e
                            );
                        }
                    }
                }
            }

            // Apply result filters at the query engine level.
            if !filters.is_empty() {
                file_matches.retain(|rm| filters.include(&rm.m));
            }

            if file_matches.is_empty() {
                None
            } else {
                Some(file_matches)
            }
        })
        .collect();

    // Flatten and sort by file, line, column for stable output.
    let mut all_matches: Vec<RuleMatch> = results.into_iter().flatten().collect();
    all_matches.sort_by(|a, b| {
        (&a.m.file, a.m.line, a.m.column).cmp(&(&b.m.file, b.m.line, b.m.column))
    });

    Ok(all_matches)
}

// ---------------------------------------------------------------------------
// Report post-processing helpers
// ---------------------------------------------------------------------------

/// Project a report to only contain the fields requested by the view.
///
/// The executor populates all content fields. This function prunes
/// fields that are not in the view, ensuring renderers see `None`
/// for unselected fields (matching the behaviour of `match_to_report_match`).
pub fn project_report(report: &mut Report, view: &ViewSet) {
    for m in report.all_matches_mut() {
        // Fatal diagnostics (broken XPath, bad config) always keep their fields —
        // the user needs to see why their query failed regardless of -v settings.
        // Error/Warning matches from user rules are normal output, subject to view.
        let is_diagnostic = matches!(m.severity, Some(Severity::Fatal));
        if !is_diagnostic {
            // Map/Array nodes are always kept — they're the only representation for data formats.
            // For other nodes, keep when tree/lines/source is selected (needed for rendering).
            let keep_tree = match m.tree.as_ref().map(|t| t.as_xml_node()) {
                Some(tractor::xpath::XmlNode::Map { .. })
                    | Some(tractor::xpath::XmlNode::Array { .. }) => true,
                _ => view.has(ViewField::Tree) || view.has(ViewField::Lines) || view.has(ViewField::Source),
            };
            if !keep_tree {
                m.tree = None;
            }
            if !view.has(ViewField::Value) {
                m.value = None;
            }
            if !view.has(ViewField::Source) {
                m.source = None;
            }
            if !view.has(ViewField::Lines) {
                m.lines = None;
            }
            if !view.has(ViewField::Reason) {
                m.reason = None;
            }
            if !view.has(ViewField::Severity) {
                m.severity = None;
            }
            if !view.has(ViewField::Status) {
                m.status = None;
            }
        }
    }
}

pub fn attach_report_schema(report: &mut Report, _depth: Option<usize>) {
    if report.schema.is_some() {
        return;
    }

    let mut collector = tractor::SchemaCollector::new();
    for m in report.all_matches() {
        if let Some(node) = m.tree.as_ref().map(|t| t.as_xml_node()) {
            collector.collect_from_xml_node(node);
        }
    }
    report.schema = Some(collector.to_schema_tree());
}

pub fn prepare_report_for_output(report: &mut Report, ctx: &RunContext) {
    if ctx.view.has(ViewField::Query) {
        report.query = ctx.xpath.clone();
    }

    if ctx.view.has(ViewField::Schema) {
        attach_report_schema(report, ctx.schema_depth());
    }

    if let Some(ref template) = ctx.message {
        apply_message_template(report, template);
    }

    project_report(report, &ctx.view);
}

/// Apply a CLI-level message template (`-m`) to all matches in a report.
///
/// This overwrites any existing message (e.g. from rule-level templates).
/// Placeholders: `{file}`, `{line}`, `{col}`, `{value}`.
pub fn apply_message_template(report: &mut Report, template: &str) {
    if !template.contains('{') {
        // Static template — same for every match.
        let msg = template.to_string();
        for m in report.all_matches_mut() {
            m.message = Some(msg.clone());
        }
        return;
    }

    for m in report.all_matches_mut() {
        m.message = Some(
            template
                .replace("{file}", &tractor::output::normalize_path(&m.file))
                .replace("{line}", &m.line.to_string())
                .replace("{col}", &m.column.to_string())
                .replace("{value}", m.value.as_deref().unwrap_or(""))
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{OutputFormat, Projection, ViewSet};
    use crate::input::InputMode;
    use tractor::report::ReportBuilder;
    use tractor::language_info::Language;
    use tractor::NormalizedXpath;

    #[test]
    fn test_language_parsing() {
        // Test that language parsing handles aliases correctly
        assert_eq!(parse_language("js"), Language::JavaScript);
        assert_eq!(parse_language("javascript"), Language::JavaScript);
        assert_eq!(parse_language("ts"), Language::TypeScript);
        assert_eq!(parse_language("typescript"), Language::TypeScript);
        assert_eq!(parse_language("py"), Language::Python);
        assert_eq!(parse_language("python"), Language::Python);
        assert_eq!(parse_language("rb"), Language::Ruby);
        assert_eq!(parse_language("ruby"), Language::Ruby);
        assert_eq!(parse_language("rs"), Language::Rust);
        assert_eq!(parse_language("rust"), Language::Rust);
        assert_eq!(parse_language("cs"), Language::CSharp);
        assert_eq!(parse_language("csharp"), Language::CSharp);
        assert_eq!(parse_language("md"), Language::Markdown);
        assert_eq!(parse_language("markdown"), Language::Markdown);
        assert_eq!(parse_language("yml"), Language::Yaml);
        assert_eq!(parse_language("yaml"), Language::Yaml);
        assert_eq!(parse_language("sh"), Language::Bash);
        assert_eq!(parse_language("bash"), Language::Bash);
        assert_eq!(parse_language("go"), Language::Go);
        assert_eq!(parse_language("java"), Language::Java);
        // Unknown languages return Language::Unknown
        assert_eq!(parse_language("nonexistent"), Language::Unknown);
    }

    /// Compile a single rule into a `CompiledRule` with optional ruleset
    /// defaults, mirroring the old `RuleSet`-based test setup.
    fn compile_for_lang(
        rule: tractor::rule::Rule,
        ruleset_default_language: Option<&str>,
    ) -> CompiledRule {
        tractor::compile_ruleset(&[], &[], ruleset_default_language, None, vec![rule], None)
            .expect("no globs → compile cannot fail")
            .pop()
            .unwrap()
    }

    #[test]
    fn test_rule_language_matches_source_no_language_specified() {
        // When no language is specified, rule should match any source
        let rule = compile_for_lang(tractor::rule::Rule::new("test", "//any"), None);

        assert!(rule_language_matches_source(&rule, "javascript"));
        assert!(rule_language_matches_source(&rule, "rust"));
        assert!(rule_language_matches_source(&rule, "markdown"));
        assert!(rule_language_matches_source(&rule, "unknown"));
    }

    #[test]
    fn test_rule_language_matches_source_with_language() {
        // When language is specified, only matching sources should match
        let rule = compile_for_lang(
            tractor::rule::Rule::new("test", "//any").with_language("javascript"),
            None,
        );

        assert!(rule_language_matches_source(&rule, "javascript"));
        assert!(!rule_language_matches_source(&rule, "rust"));
        assert!(!rule_language_matches_source(&rule, "markdown"));
    }

    #[test]
    fn test_rule_language_matches_source_with_alias() {
        // Language aliases should work on both sides
        let rule = compile_for_lang(
            tractor::rule::Rule::new("test", "//any").with_language("js"), // alias for javascript
            None,
        );

        assert!(rule_language_matches_source(&rule, "javascript"));
        assert!(!rule_language_matches_source(&rule, "rust"));
    }

    #[test]
    fn test_rule_language_matches_source_with_default_language() {
        // Default language on ruleset should be used
        let rule = compile_for_lang(tractor::rule::Rule::new("test", "//any"), Some("markdown"));

        assert!(rule_language_matches_source(&rule, "markdown"));
        assert!(!rule_language_matches_source(&rule, "javascript"));
    }

    #[test]
    fn test_rule_language_overrides_default() {
        // Rule language should override default
        let rule = compile_for_lang(
            tractor::rule::Rule::new("test", "//any").with_language("javascript"),
            Some("markdown"),
        );

        assert!(rule_language_matches_source(&rule, "javascript"));
        assert!(!rule_language_matches_source(&rule, "markdown"));
    }

    #[test]
    fn prepare_report_for_output_attaches_query_and_schema() {
        let mut builder = ReportBuilder::new();
        builder.set_no_verdict();
        builder.add(ReportMatch {
            file: "test.xml".to_string(),
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 1,
            command: "query".to_string(),
            tree: Some(tractor::xpath::Tree::Xml(tractor::xpath::XmlNode::Element {
                name: "a".to_string(),
                attributes: vec![],
                children: vec![],
            })),
            value: Some("1".to_string()),
            source: Some("<a>1</a>".to_string()),
            lines: Some(vec!["<a>1</a>".to_string()]),
            reason: None,
            severity: None,
            message: None,
            origin: None,
            rule_id: None,
            status: None,
            output: None,
        });
        let mut report = builder.build();

        let ctx = RunContext {
            xpath: Some(NormalizedXpath::new("//a")),
            output_format: OutputFormat::Json,
            projection: Projection::Report,
            single: false,
            view: ViewSet::new(vec![ViewField::Tree, ViewField::Query, ViewField::Schema]),
            use_color: false,
            message: None,
            input: InputMode::Files(vec!["test.xml".to_string()]),
            limit: None,
            depth: None,
            parse_depth: None,
            meta: false,
            tree_mode: None,
            no_pretty: false,
            ignore_whitespace: false,
            verbose: false,
            base_dir: None,
            lang: None,
            debug: false,
            group_by: vec![],
            hook_type: None,
        };

        prepare_report_for_output(&mut report, &ctx);

        assert_eq!(report.query.as_ref().map(|q| q.as_str()), Some("//a"));
        assert!(report.schema.is_some());
    }
}

