use std::collections::{HashSet, HashMap};
use tractor_core::{apply_replacements, apply_set_to_string, compute_set_output};
use tractor_core::report::{Report, ReportMatch, Totals};
use tractor_core::xpath_upsert::upsert;
use tractor_core::declarative_set::declarative_set;
use tractor_core::detect_language;
use crate::cli::SetArgs;
use crate::pipeline::{RunContext, ViewField, InputMode, query_files_batched, query_inline_source, render_report, GroupDimension};
use crate::pipeline::git;

/// Separate positional args into files and an optional path expression.
///
/// When -x is given, all positional args are files.
/// Otherwise, the last arg that looks like a path expression (contains `[`
/// or doesn't resolve to any existing file/glob) is the expression.
fn split_files_and_expr(args: &[String], has_xpath: bool) -> (Vec<String>, Option<String>) {
    if has_xpath || args.is_empty() {
        return (args.to_vec(), None);
    }

    // Check if the last arg looks like a declarative expression
    if let Some(last) = args.last() {
        let is_expr = last.contains('[')
            || last.contains('=')
            || (!std::path::Path::new(last).exists() && !last.contains('*') && !last.contains('?'));

        if is_expr {
            let files = args[..args.len() - 1].to_vec();
            return (files, Some(last.clone()));
        }
    }

    (args.to_vec(), None)
}

pub fn run_set(args: SetArgs) -> Result<(), Box<dyn std::error::Error>> {
    let has_xpath = args.shared.xpath.is_some();
    let diff_files_spec = args.shared.diff_files.clone();
    let diff_lines_spec = args.shared.diff_lines.clone();
    let (files, expr) = split_files_and_expr(&args.args, has_xpath);

    // Declarative mode: path expression without -x
    if let Some(expr) = &expr {
        let ctx = RunContext::build(
            &args.shared, files, None,
            "text", &[ViewField::Tree], None, None, None, false, &[GroupDimension::File],
        )?;

        let file_list = match &ctx.input {
            InputMode::Files(files) => files,
            InputMode::InlineSource { .. } => {
                return Err("set cannot be used with stdin input (no file to modify)".into());
            }
        };

        let file_list = apply_file_filters(file_list.clone(), diff_files_spec.as_deref(), diff_lines_spec.as_deref());
        let lang_override = ctx.lang.as_deref();
        let mut files_modified = 0;
        let mut total_ops = 0;

        for file_path in &file_list {
            let lang = lang_override
                .unwrap_or_else(|| detect_language(file_path));

            let source = std::fs::read_to_string(file_path)?;
            let result = declarative_set(
                &source, lang, expr, args.value.as_deref(),
            )?;

            if result.source != source {
                std::fs::write(file_path, &result.source)?;
                files_modified += 1;
                total_ops += result.ops_applied;
                for desc in &result.descriptions {
                    eprintln!("  {} in {}", desc, file_path);
                }
            }
        }

        eprintln!(
            "Set {} value{} in {} file{}",
            total_ops,
            if total_ops == 1 { "" } else { "s" },
            files_modified,
            if files_modified == 1 { "" } else { "s" },
        );
        return Ok(());
    }

    // XPath mode (-x): with rendering system integration.
    //
    // Early normalization: if stdin is provided as source input (--lang set, no files,
    // stdin is not a TTY), implicitly enable stdout mode — there is no file to modify.
    let stdin_source = files.is_empty()
        && args.shared.lang.is_some()
        && !atty::is(atty::Stream::Stdin);
    let stdout = args.stdout || stdin_source;

    // Default view depends on mode:
    //   stdout mode: only the raw output content (per file, at group level)
    //   in-place mode: file path + line + set status per match
    let default_view: &[ViewField] = if stdout {
        &[ViewField::Output]
    } else {
        &[ViewField::File, ViewField::Line, ViewField::Status]
    };

    let ctx = RunContext::build(
        &args.shared, files, args.shared.xpath.clone(),
        &args.format, default_view, args.view.as_deref(), None, None, false, &[GroupDimension::File],
    )?;

    let xpath_expr = ctx.xpath.as_ref()
        .ok_or("set requires either an XPath query (-x) or a path expression")?;

    let value = args.value.as_ref()
        .ok_or("set with -x requires --value")?;

    match &ctx.input {
        InputMode::Files(files) => {
            let files = apply_file_filters(files.clone(), diff_files_spec.as_deref(), diff_lines_spec.as_deref());
            let (_, matches) = query_files_batched(&ctx, &files, xpath_expr, true)?;

            if stdout {
                // Stdout mode: compute modified content per file without writing to disk.
                let file_outputs = compute_set_output(&files, &matches, value)?;
                let output_map: HashMap<String, String> = file_outputs.into_iter().collect();
                let report = build_set_report_matches(&matches, value, &ctx);
                let report = report.with_grouping(&["file"]).with_file_outputs(&output_map);
                render_report(&report, &ctx, None)?;
            } else {
                // In-place mode: try upsert (language-aware) for each file; fall back to
                // apply_replacements for languages without a renderer.
                let lang_override = ctx.lang.as_deref();
                let mut fallback_files: Vec<String> = Vec::new();

                for file_path in &files {
                    let lang = lang_override
                        .unwrap_or_else(|| detect_language(file_path));
                    let source = std::fs::read_to_string(file_path)?;
                    match upsert(&source, lang, xpath_expr, value, ctx.limit) {
                        Ok(result) => {
                            if result.source != source {
                                std::fs::write(file_path, &result.source)?;
                            }
                        }
                        Err(tractor_core::xpath_upsert::UpsertError::UnsupportedLanguage(_)) => {
                            fallback_files.push(file_path.clone());
                        }
                        Err(e) => return Err(e.into()),
                    }
                }

                // Legacy fallback for languages without renderers
                if !fallback_files.is_empty() {
                    let fallback_matches: Vec<_> = matches.iter()
                        .filter(|m| fallback_files.contains(&m.file))
                        .cloned()
                        .collect();
                    if !fallback_matches.is_empty() {
                        apply_replacements(&fallback_matches, value)?;
                    }
                }

                let report = build_set_report_matches(&matches, value, &ctx);
                let dims: Vec<&str> = ctx.group_by.iter().map(|d| d.as_str()).collect();
                let report = report.with_grouping(&dims);
                render_report(&report, &ctx, None)?;
            }
        }
        InputMode::InlineSource { source, lang } => {
            // Inline source is always stdout mode — one group (no file path), output = modified string.
            let matches = query_inline_source(&ctx, source, lang, xpath_expr)?;
            let modified = apply_set_to_string(source, &matches, value)?;
            let report = build_set_inline_report(modified, &ctx);
            render_report(&report, &ctx, None)?;
        }
    }
    Ok(())
}

/// Build per-match `ReportMatch` entries annotated with status.
/// Does NOT write any files. Returns a flat (non-grouped) `Report`.
fn build_set_report_matches(
    matches: &[tractor_core::Match],
    new_value: &str,
    ctx: &RunContext,
) -> Report {
    let mut files_affected = HashSet::new();
    let mut updated_count = 0usize;
    let mut unchanged_count = 0usize;

    let report_matches: Vec<ReportMatch> = matches.iter().map(|m| {
        files_affected.insert(m.file.clone());
        let is_unchanged = m.value == new_value;
        let status_str = if is_unchanged { "unchanged" } else { "updated" };
        if is_unchanged { unchanged_count += 1; } else { updated_count += 1; }

        ReportMatch {
            file: m.file.clone(),
            line: m.line,
            column: m.column,
            end_line: m.end_line,
            end_column: m.end_column,
            command: "set".to_string(),
            tree: None,
            value: if ctx.view.has(ViewField::Value) { Some(m.value.clone()) } else { None },
            source: if ctx.view.has(ViewField::Source) { Some(m.extract_source_snippet()) } else { None },
            lines: if ctx.view.has(ViewField::Lines) {
                Some(m.get_source_lines_range().into_iter()
                    .map(|l| l.trim_end_matches('\r').to_owned())
                    .collect())
            } else { None },
            reason: None,
            severity: None,
            message: None,
            hint: None,
            origin: None,
            rule_id: None,
            status: if ctx.view.has(ViewField::Status) { Some(status_str.to_string()) } else { None },
            output: None, // output is at group level for stdout mode
        }
    }).collect();

    let totals = Totals {
        results: matches.len(),
        files: files_affected.len(),
        fatals: 0,
        errors: 0,
        warnings: 0,
        infos: 0,
        updated: updated_count,
        unchanged: unchanged_count,
    };
    Report::set(report_matches, true, totals)
}

/// Build a set report for inline (stdin) stdout mode.
/// Creates a single group with no file path and `output` = the modified string.
fn build_set_inline_report(modified: String, ctx: &RunContext) -> Report {
    use tractor_core::report::ResultItem;

    let output_content = if ctx.view.has(ViewField::Output) { Some(modified) } else { None };

    let totals = Totals {
        results: 1,
        files: 0,
        fatals: 0,
        errors: 0,
        warnings: 0,
        infos: 0,
        updated: 0,
        unchanged: 0,
    };
    let mut report = Report::set(vec![], true, totals);
    report.results = vec![ResultItem::Group(Box::new(Report {
        success: None,
        totals: None,
        expected: None,
        query: None,
        results: vec![],
        group: None,
        file: Some(String::new()),
        command: None,
        rule_id: None,
        output_content,
    }))];
    report.group = Some("file".to_string());
    report
}

/// Apply --diff-files and --diff-lines file-level filters (set mode bypasses the executor).
fn apply_file_filters(files: Vec<String>, diff_files_spec: Option<&str>, diff_lines_spec: Option<&str>) -> Vec<String> {
    let cwd = std::path::Path::new(".");

    let files = match diff_files_spec {
        Some(spec) => {
            match git::git_changed_files(spec, cwd) {
                Ok(changed) => git::intersect_changed(files, &changed),
                Err(e) => {
                    eprintln!("warning: --diff-files filter failed: {}", e);
                    files
                }
            }
        }
        None => files,
    };

    match diff_lines_spec {
        Some(spec) => {
            match git::DiffHunkFilter::from_spec(spec, cwd) {
                Ok(filter) => {
                    use crate::filter::ResultFilter;
                    files.into_iter().filter(|f| filter.include_file(f)).collect()
                }
                Err(e) => {
                    eprintln!("warning: --diff-lines filter failed: {}", e);
                    files
                }
            }
        }
        None => files,
    }
}
