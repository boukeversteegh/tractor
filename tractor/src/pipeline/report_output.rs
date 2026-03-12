//! Report output stage — renders a Report to stdout/stderr.
//!
//! gcc and github rendering live here (not in tractor-core) because they
//! operate on ReportMatch fields (reason, severity) that only exist in the
//! tractor pipeline layer, not in the core library.

use std::path::Path;

use tractor_core::{
    format_matches, format_message,
    report::Report,
    normalize_path,
    Match,
};
use super::context::{RunContext, SerFormat, ViewField, ViewSet};
use crate::modes::test::test_colors;

// ---------------------------------------------------------------------------
// Path helpers (mirrored from the old tractor-core format_gcc)
// ---------------------------------------------------------------------------

fn to_absolute_path(path: &str) -> String {
    let p = Path::new(path);
    let absolute = if p.is_absolute() {
        p.to_path_buf()
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(p)
    } else {
        p.to_path_buf()
    };
    normalize_path(&absolute.to_string_lossy())
}

// ---------------------------------------------------------------------------
// Source context rendering (used by gcc renderers)
// ---------------------------------------------------------------------------

fn append_source_context(output: &mut String, m: &Match) {
    if m.source_lines.is_empty() || m.line == 0 {
        return;
    }
    let start_line = m.line as usize;
    let end_line = (m.end_line as usize).min(m.source_lines.len());
    let line_count = end_line.saturating_sub(start_line) + 1;
    let line_num_width = end_line.to_string().len();

    if line_count == 1 && start_line <= m.source_lines.len() {
        let source_line = m.source_lines[start_line - 1].trim_end_matches('\r');
        output.push_str(&format!("{:>width$} | {}\n", start_line, source_line, width = line_num_width));
        let caret_col = (m.column as usize).saturating_sub(1);
        let underline_len = (m.end_column as usize).saturating_sub(m.column as usize).max(1);
        let padding = " ".repeat(line_num_width + 3 + caret_col);
        let underline = format!("^{}", "~".repeat(underline_len.saturating_sub(1)));
        output.push_str(&format!("{}{}\n", padding, underline));
    } else if line_count <= 6 {
        for i in start_line..=end_line {
            if i <= m.source_lines.len() {
                let source_line = m.source_lines[i - 1].trim_end_matches('\r');
                let marker = if i == start_line || i == end_line { ">" } else { " " };
                output.push_str(&format!("{:>width$} {}| {}\n", i, marker, source_line, width = line_num_width));
            }
        }
    } else {
        for i in start_line..start_line + 2 {
            if i <= m.source_lines.len() {
                let source_line = m.source_lines[i - 1].trim_end_matches('\r');
                output.push_str(&format!("{:>width$} >| {}\n", i, source_line, width = line_num_width));
            }
        }
        output.push_str(&format!("{:>width$}  | ... ({} more lines)\n", "...", line_count - 4, width = line_num_width));
        for i in (end_line - 1)..=end_line {
            if i <= m.source_lines.len() {
                let source_line = m.source_lines[i - 1].trim_end_matches('\r');
                output.push_str(&format!("{:>width$} >| {}\n", i, source_line, width = line_num_width));
            }
        }
    }
    output.push('\n');
}

// ---------------------------------------------------------------------------
// Gcc/Github renderers — operate on ReportMatch (use reason + severity fields)
// ---------------------------------------------------------------------------

/// Render report matches in gcc format: `file:line:col: severity: reason`
pub fn render_gcc(report: &Report) -> String {
    let mut output = String::new();
    for rm in &report.matches {
        let reason = rm.reason.as_deref().unwrap_or("violation");
        let severity = rm.severity.map_or("error", |s| s.as_str());
        let m = &rm.inner;
        output.push_str(&format!(
            "{}:{}:{}: {}: {}\n",
            to_absolute_path(&m.file), m.line, m.column, severity, reason
        ));
        append_source_context(&mut output, m);
    }
    output
}

/// Render report matches as GitHub Actions annotations: `::error file=...,line=...::reason`
pub fn render_github(report: &Report) -> String {
    let mut output = String::new();
    for rm in &report.matches {
        let reason = rm.reason.as_deref().unwrap_or("violation");
        let level = rm.severity.map_or("error", |s| s.as_str());
        let m = &rm.inner;
        let file = normalize_path(&m.file);
        output.push_str(&format!(
            "::{level} file={file},line={line},endLine={end_line},col={col},endColumn={end_col}::{reason}\n",
            level = level,
            file = file,
            line = m.line,
            end_line = m.end_line,
            col = m.column,
            end_col = m.end_column,
            reason = reason,
        ));
    }
    output
}

/// Render matches in gcc format using a message template (for test --error flag).
/// Each match's message is interpolated from the template then used as the reason.
pub fn render_gcc_with_template(matches: &[Match], template: &str, is_warning: bool) -> String {
    let severity = if is_warning { "warning" } else { "error" };
    let mut output = String::new();
    for m in matches {
        let msg = format_message(template, m);
        output.push_str(&format!(
            "{}:{}:{}: {}: {}\n",
            to_absolute_path(&m.file), m.line, m.column, severity, msg
        ));
        append_source_context(&mut output, m);
    }
    output
}

// ---------------------------------------------------------------------------
// XML report renderer
// ---------------------------------------------------------------------------

/// Render a Report as an XML document, respecting `view` field selection.
pub fn render_xml_report(report: &Report, view: &ViewSet) -> String {
    use tractor_core::report::ReportKind;

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<report>\n");

    // Summary section — included if view has Summary (or it's a non-query report with no explicit -v)
    let show_summary = view.has(ViewField::Summary);
    if show_summary {
        if let Some(ref summary) = report.summary {
            if !matches!(report.kind, ReportKind::Query) {
                out.push_str("  <summary>\n");
                out.push_str(&format!("    <passed>{}</passed>\n", summary.passed));
                out.push_str(&format!("    <total>{}</total>\n", summary.total));
                out.push_str(&format!("    <files>{}</files>\n", summary.files_affected));
                out.push_str(&format!("    <errors>{}</errors>\n", summary.errors));
                out.push_str(&format!("    <warnings>{}</warnings>\n", summary.warnings));
                if let Some(ref expected) = summary.expected {
                    out.push_str(&format!("    <expected>{}</expected>\n", xml_escape(expected)));
                }
                out.push_str("  </summary>\n");
            }
        }
    }

    // Matches section
    let show_value = view.has(ViewField::Value);
    let show_reason = view.has(ViewField::Reason);
    let show_severity = view.has(ViewField::Severity);
    if !report.matches.is_empty() {
        out.push_str("  <matches>\n");
        for rm in &report.matches {
            append_xml_match(&mut out, rm, show_value, show_reason, show_severity, "    ");
        }
        out.push_str("  </matches>\n");
    }
    if let Some(ref groups) = report.groups {
        out.push_str("  <groups>\n");
        for g in groups {
            let file = xml_attr_escape(&g.file);
            out.push_str(&format!("    <group file=\"{}\">\n", file));
            for rm in &g.matches {
                append_xml_match(&mut out, rm, show_value, show_reason, show_severity, "      ");
            }
            out.push_str("    </group>\n");
        }
        out.push_str("  </groups>\n");
    }

    out.push_str("</report>\n");
    out
}

fn append_xml_match(
    out: &mut String,
    rm: &tractor_core::report::ReportMatch,
    show_value: bool,
    show_reason: bool,
    show_severity: bool,
    indent: &str,
) {
    let m = &rm.inner;
    let file = xml_attr_escape(&normalize_path(&m.file));
    out.push_str(&format!(
        "{}<match file=\"{}\" line=\"{}\" column=\"{}\"",
        indent, file, m.line, m.column
    ));
    if m.end_line != m.line || m.end_column != m.column {
        out.push_str(&format!(" end_line=\"{}\" end_column=\"{}\"", m.end_line, m.end_column));
    }
    out.push_str(">\n");

    let inner = &format!("{}  ", indent);
    if show_value && !m.value.is_empty() {
        out.push_str(&format!("{}<value>{}</value>\n", inner, xml_escape(&m.value)));
    }
    if let Some(ref message) = rm.message {
        out.push_str(&format!("{}<message>{}</message>\n", inner, xml_escape(message)));
    }
    if show_reason {
        if let Some(ref reason) = rm.reason {
            out.push_str(&format!("{}<reason>{}</reason>\n", inner, xml_escape(reason)));
        }
    }
    if show_severity {
        if let Some(severity) = rm.severity {
            out.push_str(&format!("{}<severity>{}</severity>\n", inner, severity.as_str()));
        }
    }
    if let Some(ref rule_id) = rm.rule_id {
        out.push_str(&format!("{}<rule-id>{}</rule-id>\n", inner, xml_escape(rule_id)));
    }

    out.push_str(&format!("{}</match>\n", indent));
}

fn report_match_to_json(rm: &tractor_core::report::ReportMatch, show_value: bool, show_reason: bool, show_severity: bool) -> serde_json::Value {
    use serde_json::{json, Value};
    let m = &rm.inner;
    let mut obj = serde_json::Map::new();
    obj.insert("file".into(), json!(normalize_path(&m.file)));
    obj.insert("line".into(), json!(m.line));
    obj.insert("column".into(), json!(m.column));
    if show_value && !m.value.is_empty() {
        obj.insert("value".into(), json!(m.value));
    }
    if let Some(ref message) = rm.message {
        obj.insert("message".into(), json!(message));
    }
    if show_reason {
        if let Some(ref reason) = rm.reason {
            obj.insert("reason".into(), json!(reason));
        }
    }
    if show_severity {
        if let Some(severity) = rm.severity {
            obj.insert("severity".into(), json!(severity.as_str()));
        }
    }
    if let Some(ref rule_id) = rm.rule_id {
        obj.insert("rule_id".into(), json!(rule_id));
    }
    Value::Object(obj)
}

/// Render a Report as a JSON document, respecting `view` field selection.
pub fn render_json_report(report: &Report, view: &ViewSet) -> String {
    use serde_json::{json, Value};
    use tractor_core::report::ReportKind;

    let show_summary = view.has(ViewField::Summary);
    let show_value = view.has(ViewField::Value);
    let show_reason = view.has(ViewField::Reason);
    let show_severity = view.has(ViewField::Severity);

    let matches_json: Vec<Value> = report.matches.iter()
        .map(|rm| report_match_to_json(rm, show_value, show_reason, show_severity))
        .collect();

    let mut root = serde_json::Map::new();
    root.insert("kind".into(), json!(format!("{:?}", report.kind).to_lowercase()));

    if show_summary {
        if let Some(ref summary) = report.summary {
            if !matches!(report.kind, ReportKind::Query) {
                root.insert("summary".into(), json!({
                    "passed": summary.passed,
                    "total": summary.total,
                    "files": summary.files_affected,
                    "errors": summary.errors,
                    "warnings": summary.warnings,
                }));
            }
        }
    }

    if !matches_json.is_empty() {
        root.insert("matches".into(), Value::Array(matches_json));
    }
    if let Some(ref groups) = report.groups {
        let groups_json: Vec<Value> = groups.iter().map(|g| {
            let group_matches: Vec<Value> = g.matches.iter()
                .map(|rm| report_match_to_json(rm, show_value, show_reason, show_severity))
                .collect();
            json!({ "file": g.file, "matches": group_matches })
        }).collect();
        root.insert("groups".into(), Value::Array(groups_json));
    }

    serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_else(|_| "{}".to_string())
}

/// Render a Report as a YAML document, respecting `view` field selection.
/// Reuses the same JSON-value structure as `render_json_report`, then
/// serializes with serde_yaml for consistent field ordering and types.
pub fn render_yaml_report(report: &Report, view: &ViewSet) -> String {
    use serde_json::Value;
    use tractor_core::report::ReportKind;

    let show_summary = view.has(ViewField::Summary);
    let show_value = view.has(ViewField::Value);
    let show_reason = view.has(ViewField::Reason);
    let show_severity = view.has(ViewField::Severity);

    let matches_json: Vec<Value> = report.matches.iter()
        .map(|rm| report_match_to_json(rm, show_value, show_reason, show_severity))
        .collect();

    let mut root = serde_json::Map::new();
    root.insert("kind".into(), serde_json::json!(format!("{:?}", report.kind).to_lowercase()));

    if show_summary {
        if let Some(ref summary) = report.summary {
            if !matches!(report.kind, ReportKind::Query) {
                root.insert("summary".into(), serde_json::json!({
                    "passed": summary.passed,
                    "total": summary.total,
                    "files": summary.files_affected,
                    "errors": summary.errors,
                    "warnings": summary.warnings,
                }));
            }
        }
    }

    if !matches_json.is_empty() {
        root.insert("matches".into(), Value::Array(matches_json));
    }
    if let Some(ref groups) = report.groups {
        let groups_json: Vec<Value> = groups.iter().map(|g| {
            let group_matches: Vec<Value> = g.matches.iter()
                .map(|rm| report_match_to_json(rm, show_value, show_reason, show_severity))
                .collect();
            serde_json::json!({ "file": g.file, "matches": group_matches })
        }).collect();
        root.insert("groups".into(), Value::Array(groups_json));
    }

    serde_yaml::to_string(&Value::Object(root)).unwrap_or_else(|_| "{}\n".to_string())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
}

fn xml_attr_escape(s: &str) -> String {
    xml_escape(s).replace('"', "&quot;")
}

// ---------------------------------------------------------------------------
// Check report renderer
// ---------------------------------------------------------------------------

/// Render a check report. Returns Err if there are error-severity violations.
pub fn render_check_report(
    report: &Report,
    ctx: &RunContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let summary = report.summary.as_ref().expect("check report must have summary");

    match ctx.ser_format {
        SerFormat::Json => {
            print!("{}", render_json_report(report, &ctx.view));
        }
        SerFormat::Yaml => {
            print!("{}", render_yaml_report(report, &ctx.view));
        }
        SerFormat::Xml => {
            print!("{}", render_xml_report(report, &ctx.view));
        }
        SerFormat::Gcc => {
            print!("{}", render_gcc(report));
            print_check_summary(summary);
        }
        SerFormat::Github => {
            print!("{}", render_github(report));
        }
        SerFormat::Text => {
            let inner_matches: Vec<_> = report.matches.iter().map(|rm| rm.inner.clone()).collect();
            let output = format_matches(&inner_matches, ctx.view.primary_output_format(), &ctx.options);
            print!("{}", output);
            print_check_summary(summary);
        }
    }

    if summary.errors > 0 {
        return Err(Box::new(crate::SilentExit));
    }
    Ok(())
}

fn print_check_summary(summary: &tractor_core::report::Summary) {
    if summary.total > 0 {
        eprintln!();
        let kind = if summary.errors > 0 {
            format!("{} error{}", summary.errors, if summary.errors == 1 { "" } else { "s" })
        } else {
            format!("{} warning{}", summary.warnings, if summary.warnings == 1 { "" } else { "s" })
        };
        eprintln!("{} in {} file{}", kind, summary.files_affected,
            if summary.files_affected == 1 { "" } else { "s" });
    }
}

// ---------------------------------------------------------------------------
// Test report renderer
// ---------------------------------------------------------------------------

/// Render a test report. Returns Err if the test failed and `warning` is false.
pub fn render_test_report(
    report: &Report,
    ctx: &RunContext,
    message: &Option<String>,
    error_template: &Option<String>,
    warning: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let summary = report.summary.as_ref().expect("test report must have summary");

    match ctx.ser_format {
        SerFormat::Json => {
            print!("{}", render_json_report(report, &ctx.view));
            if !summary.passed && !warning {
                return Err(Box::new(crate::SilentExit));
            }
            return Ok(());
        }
        SerFormat::Yaml => {
            print!("{}", render_yaml_report(report, &ctx.view));
            if !summary.passed && !warning {
                return Err(Box::new(crate::SilentExit));
            }
            return Ok(());
        }
        SerFormat::Xml => {
            print!("{}", render_xml_report(report, &ctx.view));
            if !summary.passed && !warning {
                return Err(Box::new(crate::SilentExit));
            }
            return Ok(());
        }
        _ => {}
    }

    // Text/gcc/github output: colored pass/fail
    let (symbol, color) = if summary.passed {
        ("✓", test_colors::GREEN)
    } else if warning {
        ("⚠", test_colors::YELLOW)
    } else {
        ("✗", test_colors::RED)
    };

    let label = message.as_deref().unwrap_or("");
    let expected_str = summary.expected.as_deref().unwrap_or("?");

    if ctx.use_color {
        if label.is_empty() {
            println!("{}{}{} {} matches{}",
                test_colors::BOLD, color, symbol, summary.total, test_colors::RESET);
        } else if summary.passed {
            println!("{}{}{} {}{}",
                test_colors::BOLD, color, symbol, label, test_colors::RESET);
        } else {
            println!("{}{}{} {} {}(expected {}, got {}){}",
                test_colors::BOLD, color, symbol, label, test_colors::RESET,
                expected_str, summary.total, test_colors::RESET);
        }
    } else if label.is_empty() {
        println!("{} {} matches", symbol, summary.total);
    } else if summary.passed {
        println!("{} {}", symbol, label);
    } else {
        println!("{} {} (expected {}, got {})", symbol, label, expected_str, summary.total);
    }

    // Error details when test failed
    if !summary.passed && !report.matches.is_empty() {
        let inner_matches: Vec<_> = report.matches.iter().map(|rm| rm.inner.clone()).collect();

        if let Some(ref error_tmpl) = error_template {
            let output = render_gcc_with_template(&inner_matches, error_tmpl, ctx.options.warning);
            for line in output.lines() {
                if ctx.use_color {
                    println!("  {}{}{}", color, line, test_colors::RESET);
                } else {
                    println!("  {}", line);
                }
            }
        } else {
            let output = format_matches(&inner_matches, ctx.view.primary_output_format(), &ctx.options);
            for line in output.lines() {
                println!("  {}", line);
            }
        }
    }

    if !summary.passed && !warning {
        return Err(Box::new(crate::SilentExit));
    }

    Ok(())
}
