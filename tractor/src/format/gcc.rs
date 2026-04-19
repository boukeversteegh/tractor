use tractor::{render_lines, report::{Report, ReportMatch, ResultItem}, RenderOptions};
use super::shared::to_absolute_path;

/// Render report matches in gcc format: `file:line:col: severity: reason`
/// GCC is a flat per-line format. Grouping affects ordering only, not field omission.
pub fn render_gcc(report: &Report, opts: &RenderOptions, _dimensions: &[&str]) -> String {
    let mut out = String::new();
    render_gcc_results(&mut out, &report.results, None, opts);
    out
}

/// Walk the results tree recursively, rendering matches in gcc format.
fn render_gcc_results(out: &mut String, items: &[ResultItem], parent_file: Option<&str>, opts: &RenderOptions) {
    for item in items {
        match item {
            ResultItem::Match(rm) => render_gcc_match(out, rm, parent_file, opts),
            ResultItem::Group(g) => {
                let file = g.file.as_deref().or(parent_file);
                render_gcc_results(out, &g.results, file, opts);
            }
        }
    }
}

fn render_gcc_match(out: &mut String, rm: &ReportMatch, group_file: Option<&str>, opts: &RenderOptions) {
    let file = group_file.unwrap_or(&rm.file);
    let severity = gcc_severity(rm);
    let detail = gcc_detail(rm);

    if file.is_empty() {
        let prefix = rm.origin.map_or("tractor", |o| o.as_str());
        out.push_str(&format!("{}: {}: {}\n", prefix, severity, detail));
    } else {
        out.push_str(&format!(
            "{}:{}:{}: {}: {}\n",
            to_absolute_path(file), rm.line, rm.column, severity, detail
        ));
    }

    if rm.severity.is_some() {
        if let Some(ref ls) = rm.lines {
            out.push_str(&render_lines(
                ls, rm.tree.as_ref(),
                rm.line, rm.column, rm.end_line, rm.end_column,
                opts,
            ));
        }
    }
}

fn gcc_severity(rm: &ReportMatch) -> &'static str {
    match rm.severity {
        Some(tractor::report::Severity::Fatal) => "error",
        Some(tractor::report::Severity::Error) => "error",
        Some(tractor::report::Severity::Warning) => "warning",
        Some(tractor::report::Severity::Info) => "note",
        None => "note",
    }
}

fn gcc_detail(rm: &ReportMatch) -> String {
    match (rm.status.as_deref(), rm.reason.as_deref(), rm.value.as_deref()) {
        (Some(status), Some(reason), _) => format!("{} {}", status, reason),
        (Some(status), None, _) => status.to_string(),
        (None, Some(reason), _) => reason.to_string(),
        (None, None, Some(value)) => value.to_string(),
        (None, None, None) => rm.command.clone(),
    }
}

/// Render ReportMatches in gcc format using a message template (for `test --error`).
pub fn render_gcc_report_with_template(matches: &[ReportMatch], template: &str, is_warning: bool, opts: &RenderOptions) -> String {
    let severity = if is_warning { "warning" } else { "error" };
    let mut out = String::new();
    for rm in matches {
        let msg = template
            .replace("{file}", &tractor::normalize_path(&rm.file))
            .replace("{line}", &rm.line.to_string())
            .replace("{col}", &rm.column.to_string())
            .replace("{value}", rm.value.as_deref().unwrap_or(""));
        out.push_str(&format!(
            "{}:{}:{}: {}: {}\n",
            to_absolute_path(&rm.file), rm.line, rm.column, severity, msg
        ));
        if let Some(ref ls) = rm.lines {
            out.push_str(&render_lines(
                ls, rm.tree.as_ref(),
                rm.line, rm.column, rm.end_line, rm.end_column,
                opts,
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::render_gcc;
    use tractor::report::{Report, ReportMatch, ResultItem, Totals};
    use tractor::RenderOptions;

    fn set_match(file: &str, line: u32, column: u32, status: &str, xpath: &str) -> ReportMatch {
        ReportMatch {
            file: file.to_string(),
            line,
            column,
            end_line: line,
            end_column: column + 1,
            command: "set".to_string(),
            tree: None,
            value: None,
            source: None,
            lines: None,
            reason: Some(xpath.to_string()),
            severity: None,
            message: None,
            origin: None,
            rule_id: None,
            status: Some(status.to_string()),
            output: None,
        }
    }

    #[test]
    fn render_gcc_renders_each_set_mapping_as_a_note() {
        let grouped = Report {
            success: Some(true),
            totals: Some(Totals {
                results: 2,
                files: 1,
                fatals: 0,
                errors: 0,
                warnings: 0,
                infos: 0,
                updated: 2,
                unchanged: 0,
            }),
            expected: None,
            query: None,
            outputs: vec![],
            schema: None,
            results: vec![ResultItem::Group(Box::new(Report {
                success: None,
                totals: None,
                expected: None,
                query: None,
                outputs: vec![],
                schema: None,
                results: vec![
                    ResultItem::Match(set_match("app-config.json", 3, 5, "updated", "//database/host")),
                    ResultItem::Match(set_match("app-config.json", 8, 5, "updated", "//cache/ttl")),
                ],
                group: None,
                file: Some("app-config.json".to_string()),
                command: None,
                rule_id: None,
                }))],
            group: Some("file".to_string()),
            file: None,
            command: Some("set".to_string()),
            rule_id: None,
        };

        assert_eq!(
            render_gcc(&grouped, &RenderOptions::default(), &[]),
            format!(
                "{}:3:5: note: updated //database/host\n{}:8:5: note: updated //cache/ttl\n",
                super::to_absolute_path("app-config.json"),
                super::to_absolute_path("app-config.json")
            )
        );
    }

    #[test]
    fn render_gcc_renders_query_from_value_without_command_specific_logic() {
        let report = Report {
            success: None,
            totals: Some(Totals {
                results: 1,
                files: 1,
                fatals: 0,
                errors: 0,
                warnings: 0,
                infos: 0,
                updated: 0,
                unchanged: 0,
            }),
            expected: None,
            query: None,
            outputs: vec![],
            schema: None,
            results: vec![ResultItem::Match(ReportMatch {
                file: "sample.json".to_string(),
                line: 2,
                column: 7,
                end_line: 2,
                end_column: 12,
                command: "query".to_string(),
                tree: None,
                value: Some("alice".to_string()),
                source: None,
                lines: None,
                reason: None,
                severity: None,
                message: None,
                origin: None,
                rule_id: None,
                status: None,
                output: None,
            })],
            group: None,
            file: None,
            command: None,
            rule_id: None,
        };

        assert_eq!(
            render_gcc(&report, &RenderOptions::default(), &[]),
            format!("{}:2:7: note: alice\n", super::to_absolute_path("sample.json"))
        );
    }
}
