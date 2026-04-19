pub mod options;
pub mod gcc;
pub mod github;
pub mod xml;
pub mod json;
pub mod yaml;
pub mod text;
pub mod claude_code;
mod shared;

pub use options::{OutputFormat, GroupDimension, ViewField, ViewSet, Projection, parse_view_set, parse_group_by};
pub use gcc::{render_gcc, render_gcc_report_with_template};
pub use github::render_github;
pub use xml::render_xml_report;
pub use json::render_json_report;
pub use yaml::render_yaml_report;
pub use text::render_text_report;
pub use claude_code::render_claude_code;

use tractor::{
    render_query_tree_node, render_xml_node,
    render_source_precomputed, render_lines,
    report::{Report, ReportMatch},
    RenderOptions,
};
use crate::cli::context::RunContext;
use crate::cli::test::test_colors;

/// Options for test-specific rendering (colored pass/fail, error detail).
/// When None, the report is rendered generically.
pub struct TestRenderOptions {
    pub message: Option<String>,
    pub error_template: Option<String>,
}

/// Render any report to stdout. Unified entry point for all command modes.
///
/// - Dispatches to format-specific renderers (json, yaml, xml, gcc, github, text).
/// - Prints gcc-style summary to stderr when format is gcc and report has totals.
/// - Returns Err(SilentExit) when `success == Some(false)`.
/// - For test reports, `test_opts` enables colored pass/fail rendering.
pub fn render_report(
    report: &Report,
    ctx: &RunContext,
    test_opts: Option<&TestRenderOptions>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Test reports with text/gcc/github get special colored pass/fail rendering.
    if let Some(opts) = test_opts {
        if matches!(ctx.output_format, OutputFormat::Text | OutputFormat::Gcc | OutputFormat::Github) {
            return render_test_text(report, ctx, opts);
        }
    }

    // Projection dispatch: for non-report projections, render the projected element.
    if ctx.projection != Projection::Report {
        let exit_fail = report.success == Some(false);
        let got_output = render_projection(report, ctx)?;
        // --single with 0 matches → empty stdout, non-zero exit.
        if ctx.single && !got_output && ctx.projection.is_per_match() {
            return Err(Box::new(crate::SilentExit));
        }
        if ctx.single && !got_output && ctx.projection == Projection::Results {
            return Err(Box::new(crate::SilentExit));
        }
        if exit_fail {
            return Err(Box::new(crate::SilentExit));
        }
        return Ok(());
    }

    // Standard format dispatch — same for all report types.
    let dims: Vec<&str> = ctx.group_by.iter().map(|d| d.as_str()).collect();
    match ctx.output_format {
        OutputFormat::Json   => print!("{}", render_json_report(report, &ctx.view, &ctx.render_options(), &dims)),
        OutputFormat::Yaml   => print!("{}", render_yaml_report(report, &ctx.view, &ctx.render_options(), &dims)),
        OutputFormat::Xml    => print!("{}", render_xml_report(report, &ctx.view, &ctx.render_options(), &dims)),
        OutputFormat::Gcc    => {
            print!("{}", render_gcc(report, &ctx.render_options(), &dims));
            if let Some(summary) = gcc_summary_string(report) {
                print!("{}", summary);
            }
        }
        OutputFormat::Github => print!("{}", render_github(report, &dims)),
        OutputFormat::ClaudeCode => print!("{}", render_claude_code(report, ctx.hook_type.unwrap_or(options::HookType::PostToolUse), &ctx.render_options(), &dims)),
        OutputFormat::Text   => print!("{}", render_text_report(report, &ctx.view, &ctx.render_options(), &dims)),
    }

    // Exit code: fail when success is explicitly false.
    if report.success == Some(false) {
        return Err(Box::new(crate::SilentExit));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Projection rendering
// ---------------------------------------------------------------------------

/// Render a projected element from the report. Dispatches based on ctx.projection.
/// Returns true if any output was produced (used for --single empty-result detection).
fn render_projection(
    report: &Report,
    ctx: &RunContext,
) -> Result<bool, Box<dyn std::error::Error>> {
    let render_opts = ctx.render_options();
    let single = ctx.single;

    // Warn when --single is used on a projection that is already singular.
    if single && ctx.projection.is_singular() {
        eprintln!(
            "warning: --single has no effect with -p {} (already singular). Drop --single.",
            ctx.projection.name()
        );
    }

    let had_output = match ctx.projection {
        Projection::Count => {
            render_projection_count(report, &ctx.output_format);
            true
        }
        Projection::Schema => {
            render_projection_schema(report, &ctx.output_format, &render_opts);
            true
        }
        Projection::Summary => {
            render_projection_summary(report, &ctx.output_format, &render_opts);
            true
        }
        Projection::Totals => {
            render_projection_totals(report, &ctx.output_format, &render_opts);
            true
        }
        Projection::Results => {
            let dims: Vec<&str> = ctx.group_by.iter().map(|d| d.as_str()).collect();
            render_projection_results(report, ctx, &dims, single)
        }
        Projection::Tree | Projection::Value | Projection::Source | Projection::Lines => {
            render_projection_per_match(report, ctx, single)
        }
        Projection::Report => unreachable!("report projection is handled before this function"),
    };
    Ok(had_output)
}

fn render_projection_count(report: &Report, format: &OutputFormat) {
    let count = report.totals.as_ref().map_or(0, |t| t.results);
    match format {
        OutputFormat::Xml => {
            println!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
            println!("<count>{count}</count>");
        }
        OutputFormat::Json => println!("{count}"),
        OutputFormat::Yaml => println!("{count}"),
        _ => println!("{count}"),
    }
}

fn render_projection_schema(report: &Report, format: &OutputFormat, _render_opts: &RenderOptions) {
    let schema = report.schema.as_deref().unwrap_or("");
    match format {
        OutputFormat::Xml => {
            let escaped = schema.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            println!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
            println!("<schema>{escaped}</schema>");
        }
        OutputFormat::Json => {
            print!("{}", serde_json::to_string_pretty(&serde_json::Value::String(schema.to_string())).unwrap_or_default());
            println!();
        }
        OutputFormat::Yaml => {
            print!("{}", serde_yaml::to_string(&serde_json::Value::String(schema.to_string())).unwrap_or_default());
        }
        _ => print!("{schema}"),
    }
}

fn render_projection_summary(report: &Report, format: &OutputFormat, _render_opts: &RenderOptions) {
    use json::build_summary_json;
    match format {
        OutputFormat::Xml => {
            let mut body = String::new();
            if let Some(passed) = report.success {
                body.push_str(&format!("  <success>{passed}</success>\n"));
            }
            if let Some(ref totals) = report.totals {
                body.push_str("  <totals>\n");
                body.push_str(&format!("    <results>{}</results>\n", totals.results));
                body.push_str(&format!("    <files>{}</files>\n", totals.files));
                if totals.fatals   > 0 { body.push_str(&format!("    <fatals>{}</fatals>\n", totals.fatals)); }
                if totals.errors   > 0 { body.push_str(&format!("    <errors>{}</errors>\n", totals.errors)); }
                if totals.warnings > 0 { body.push_str(&format!("    <warnings>{}</warnings>\n", totals.warnings)); }
                if totals.infos    > 0 { body.push_str(&format!("    <infos>{}</infos>\n", totals.infos)); }
                if totals.updated  > 0 { body.push_str(&format!("    <updated>{}</updated>\n", totals.updated)); }
                if totals.unchanged > 0 { body.push_str(&format!("    <unchanged>{}</unchanged>\n", totals.unchanged)); }
                body.push_str("  </totals>\n");
            }
            if let Some(ref expected) = report.expected {
                let e = expected.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
                body.push_str(&format!("  <expected>{e}</expected>\n"));
            }
            if let Some(ref query) = report.query {
                let q = query.as_str().replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
                body.push_str(&format!("  <query>{q}</query>\n"));
            }
            println!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
            print!("<summary>\n{body}</summary>\n");
        }
        OutputFormat::Json => {
            let summary = build_summary_json(report);
            println!("{}", serde_json::to_string_pretty(&serde_json::Value::Object(summary)).unwrap_or_default());
        }
        OutputFormat::Yaml => {
            let summary = build_summary_json(report);
            print!("{}", serde_yaml::to_string(&serde_json::Value::Object(summary)).unwrap_or_default());
        }
        _ => {
            // Text: render summary line(s)
            if let Some(ref totals) = report.totals {
                use text::format_summary_text;
                print!("{}", format_summary_text(totals, report.success, report.expected.as_deref()));
            }
            if let Some(ref query) = report.query {
                println!("Query: {query}");
            }
        }
    }
}

fn render_projection_totals(report: &Report, format: &OutputFormat, _render_opts: &RenderOptions) {
    let totals = match report.totals.as_ref() {
        Some(t) => t,
        None => return,
    };
    match format {
        OutputFormat::Xml => {
            println!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
            let mut body = String::new();
            body.push_str(&format!("  <results>{}</results>\n", totals.results));
            body.push_str(&format!("  <files>{}</files>\n", totals.files));
            if totals.fatals   > 0 { body.push_str(&format!("  <fatals>{}</fatals>\n", totals.fatals)); }
            if totals.errors   > 0 { body.push_str(&format!("  <errors>{}</errors>\n", totals.errors)); }
            if totals.warnings > 0 { body.push_str(&format!("  <warnings>{}</warnings>\n", totals.warnings)); }
            if totals.infos    > 0 { body.push_str(&format!("  <infos>{}</infos>\n", totals.infos)); }
            if totals.updated  > 0 { body.push_str(&format!("  <updated>{}</updated>\n", totals.updated)); }
            if totals.unchanged > 0 { body.push_str(&format!("  <unchanged>{}</unchanged>\n", totals.unchanged)); }
            print!("<totals>\n{body}</totals>\n");
        }
        OutputFormat::Json => {
            let mut t = serde_json::Map::new();
            t.insert("results".into(), serde_json::json!(totals.results));
            t.insert("files".into(),   serde_json::json!(totals.files));
            if totals.fatals   > 0 { t.insert("fatals".into(),    serde_json::json!(totals.fatals)); }
            if totals.errors   > 0 { t.insert("errors".into(),    serde_json::json!(totals.errors)); }
            if totals.warnings > 0 { t.insert("warnings".into(),  serde_json::json!(totals.warnings)); }
            if totals.infos    > 0 { t.insert("infos".into(),     serde_json::json!(totals.infos)); }
            if totals.updated  > 0 { t.insert("updated".into(),   serde_json::json!(totals.updated)); }
            if totals.unchanged > 0 { t.insert("unchanged".into(), serde_json::json!(totals.unchanged)); }
            println!("{}", serde_json::to_string_pretty(&serde_json::Value::Object(t)).unwrap_or_default());
        }
        OutputFormat::Yaml => {
            let mut t = serde_json::Map::new();
            t.insert("results".into(), serde_json::json!(totals.results));
            t.insert("files".into(),   serde_json::json!(totals.files));
            if totals.fatals   > 0 { t.insert("fatals".into(),    serde_json::json!(totals.fatals)); }
            if totals.errors   > 0 { t.insert("errors".into(),    serde_json::json!(totals.errors)); }
            if totals.warnings > 0 { t.insert("warnings".into(),  serde_json::json!(totals.warnings)); }
            if totals.infos    > 0 { t.insert("infos".into(),     serde_json::json!(totals.infos)); }
            if totals.updated  > 0 { t.insert("updated".into(),   serde_json::json!(totals.updated)); }
            if totals.unchanged > 0 { t.insert("unchanged".into(), serde_json::json!(totals.unchanged)); }
            print!("{}", serde_yaml::to_string(&serde_json::Value::Object(t)).unwrap_or_default());
        }
        _ => {
            println!("results: {}", totals.results);
            println!("files: {}", totals.files);
        }
    }
}

/// Render just the results list (without the report envelope). Returns true if output was produced.
fn render_projection_results(
    report: &Report,
    ctx: &RunContext,
    dims: &[&str],
    single: bool,
) -> bool {
    let render_opts = ctx.render_options();
    let all_matches: Vec<&ReportMatch> = report.all_matches();
    if all_matches.is_empty() && single {
        return false;
    }
    let matches_to_render: &[&ReportMatch] = if single {
        &all_matches[..all_matches.len().min(1)]
    } else {
        &all_matches
    };

    match ctx.output_format {
        OutputFormat::Xml => {
            let xml_body = xml::render_xml_matches_only(matches_to_render, &ctx.view, &render_opts, dims);
            if single {
                if xml_body.is_empty() {
                    return false;
                }
                print!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{xml_body}");
            } else {
                print!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<results>\n{xml_body}</results>\n");
            }
        }
        OutputFormat::Json => {
            let items: Vec<serde_json::Value> = matches_to_render.iter()
                .map(|rm| json::match_to_value(rm, &ctx.view, &render_opts, dims))
                .filter(|v| !v.as_object().is_some_and(|o| o.is_empty()))
                .collect();
            if single {
                if let Some(first) = items.into_iter().next() {
                    println!("{}", serde_json::to_string_pretty(&first).unwrap_or_default());
                } else {
                    return false;
                }
            } else {
                println!("{}", serde_json::to_string_pretty(&serde_json::Value::Array(items)).unwrap_or_default());
            }
        }
        OutputFormat::Yaml => {
            let items: Vec<serde_json::Value> = matches_to_render.iter()
                .map(|rm| json::match_to_value(rm, &ctx.view, &render_opts, dims))
                .filter(|v| !v.as_object().is_some_and(|o| o.is_empty()))
                .collect();
            if single {
                if let Some(first) = items.into_iter().next() {
                    print!("{}", serde_yaml::to_string(&first).unwrap_or_default());
                } else {
                    return false;
                }
            } else {
                print!("{}", serde_yaml::to_string(&serde_json::Value::Array(items)).unwrap_or_default());
            }
        }
        _ => {
            // For text/gcc/github: use normal text rendering without the summary envelope.
            let tmp = make_results_only_report(report);
            print!("{}", render_text_report(&tmp, &ctx.view, &render_opts, dims));
        }
    }
    true
}

/// Render per-match projection (tree, value, source, lines). Returns true if output was produced.
fn render_projection_per_match(
    report: &Report,
    ctx: &RunContext,
    single: bool,
) -> bool {
    let render_opts = ctx.render_options();
    let field = ctx.projection;
    let all_matches: Vec<&ReportMatch> = report.all_matches();
    if all_matches.is_empty() && single {
        return false;
    }
    let matches_to_render: &[&ReportMatch] = if single {
        &all_matches[..all_matches.len().min(1)]
    } else {
        &all_matches
    };

    match ctx.output_format {
        OutputFormat::Xml => {
            let xml_parts: Vec<String> = matches_to_render.iter()
                .filter_map(|rm| extract_field_as_xml(rm, field, &render_opts))
                .collect();
            if single {
                if let Some(bare) = xml_parts.into_iter().next() {
                    // For --single: emit the inner tree content (unwrapped), not the <tree> element.
                    let unwrapped = unwrap_xml_element(&bare);
                    print!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{unwrapped}");
                } else {
                    return false;
                }
            } else {
                let inner: String = xml_parts.into_iter().map(|s| {
                    s.lines().map(|l| format!("  {l}\n")).collect::<String>()
                }).collect();
                print!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<results>\n{inner}</results>\n");
            }
        }
        OutputFormat::Json => {
            let items: Vec<serde_json::Value> = matches_to_render.iter()
                .filter_map(|rm| extract_field_as_json(rm, field, &render_opts))
                .collect();
            if single {
                if let Some(first) = items.into_iter().next() {
                    println!("{}", serde_json::to_string_pretty(&first).unwrap_or_default());
                } else {
                    return false;
                }
            } else {
                println!("{}", serde_json::to_string_pretty(&serde_json::Value::Array(items)).unwrap_or_default());
            }
        }
        OutputFormat::Yaml => {
            let items: Vec<serde_json::Value> = matches_to_render.iter()
                .filter_map(|rm| extract_field_as_json(rm, field, &render_opts))
                .collect();
            if single {
                if let Some(first) = items.into_iter().next() {
                    print!("{}", serde_yaml::to_string(&first).unwrap_or_default());
                } else {
                    return false;
                }
            } else {
                print!("{}", serde_yaml::to_string(&serde_json::Value::Array(items)).unwrap_or_default());
            }
        }
        _ => {
            // Text: re-use existing text renderer, which already outputs bare fields.
            let tmp = make_results_only_report(report);
            print!("{}", render_text_report(&tmp, &ctx.view, &render_opts, &[]));
        }
    }
    true
}

/// Unwrap an outer XML element wrapper, returning just its inner content.
/// Used for `--single` tree/value/source/lines projections: strips the `<tree>`, `<value>`, etc. wrapper.
fn unwrap_xml_element(xml: &str) -> String {
    // Simple approach: strip first line (opening tag) and last non-empty line (closing tag),
    // then de-indent the remaining content.
    let lines: Vec<&str> = xml.lines().collect();
    if lines.len() <= 2 {
        return xml.to_string();
    }
    // Find opening line (first) and closing line (last non-empty).
    let inner_lines = &lines[1..lines.len()-1];
    // De-indent by 2 spaces if all lines have it.
    let all_indented = inner_lines.iter().all(|l| l.is_empty() || l.starts_with("  "));
    let result: String = inner_lines.iter().map(|l| {
        if all_indented && l.starts_with("  ") {
            format!("{}\n", &l[2..])
        } else {
            format!("{l}\n")
        }
    }).collect();
    result
}

/// Extract a per-match field as XML text (the field's content, with its wrapper element).
///
/// For Tree: returns the rendered tree node (bare, without the `<tree>` wrapper when single,
/// but `<tree>content</tree>` when listing).
fn extract_field_as_xml(rm: &ReportMatch, projection: Projection, render_opts: &RenderOptions) -> Option<String> {
    match projection {
        Projection::Tree => {
            rm.tree.as_ref().map(|node| {
                let rendered = render_xml_node(node, render_opts);
                format!("<tree>\n{}</tree>\n",
                    rendered.lines().map(|l| format!("  {l}\n")).collect::<String>()
                )
            })
        }
        Projection::Value => {
            rm.value.as_ref().map(|v| {
                let escaped = v.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
                format!("<value>{escaped}</value>\n")
            })
        }
        Projection::Source => {
            rm.source.as_ref().map(|s| {
                let escaped = s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
                format!("<source>{escaped}</source>\n")
            })
        }
        Projection::Lines => {
            rm.lines.as_ref().map(|ls| {
                let mut out = String::from("<lines>\n");
                for l in ls {
                    let escaped = l.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
                    out.push_str(&format!("  <line>{escaped}</line>\n"));
                }
                out.push_str("</lines>\n");
                out
            })
        }
        _ => None,
    }
}

/// Extract a per-match field as a JSON value.
fn extract_field_as_json(rm: &ReportMatch, projection: Projection, render_opts: &RenderOptions) -> Option<serde_json::Value> {
    match projection {
        Projection::Tree => {
            rm.tree.as_ref().map(|node| tractor::xml_node_to_json(node, render_opts.max_depth))
        }
        Projection::Value => rm.value.as_ref().map(|v| serde_json::json!(v)),
        Projection::Source => rm.source.as_ref().map(|s| serde_json::json!(s)),
        Projection::Lines => rm.lines.as_ref().map(|ls| serde_json::json!(ls)),
        _ => None,
    }
}

/// Create a temporary report with only results (no success/totals) for text rendering
/// of results/per-match projections.
fn make_results_only_report(report: &Report) -> Report {
    Report {
        success: None,
        totals: None,
        expected: None,
        query: None,
        outputs: vec![],
        schema: None,
        results: report.results.clone(),
        group: report.group.clone(),
        file: report.file.clone(),
        command: report.command.clone(),
        rule_id: report.rule_id.clone(),
    }
}

// ---------------------------------------------------------------------------
// Test-specific text rendering (colored pass/fail with error detail)
// ---------------------------------------------------------------------------

fn render_test_text(
    report: &Report,
    ctx: &RunContext,
    opts: &TestRenderOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let success = report.success.unwrap_or(true);
    let totals = report.totals.as_ref().expect("test report must have totals");

    let (symbol, color) = if success {
        ("✓", test_colors::GREEN)
    } else {
        ("✗", test_colors::RED)
    };

    let label        = opts.message.as_deref().unwrap_or("");
    let expected_str = report.expected.as_deref().unwrap_or("?");

    if ctx.use_color {
        if label.is_empty() {
            println!("{}{}{} {} matches{}", test_colors::BOLD, color, symbol, totals.results, test_colors::RESET);
        } else if success {
            println!("{}{}{} {}{}", test_colors::BOLD, color, symbol, label, test_colors::RESET);
        } else {
            println!("{}{}{} {} {}(expected {}, got {}){}", test_colors::BOLD, color, symbol, label, test_colors::RESET, expected_str, totals.results, test_colors::RESET);
        }
    } else if label.is_empty() {
        println!("{} {} matches", symbol, totals.results);
    } else if success {
        println!("{} {}", symbol, label);
    } else {
        println!("{} {} (expected {}, got {})", symbol, label, expected_str, totals.results);
    }

    let all_matches = report.all_matches();
    if !success && !all_matches.is_empty() {
        if let Some(ref error_tmpl) = opts.error_template {
            let flat_matches: Vec<ReportMatch> = all_matches.into_iter().cloned().collect();
            let out = render_gcc_report_with_template(&flat_matches, error_tmpl, false, &ctx.render_options());
            for line in out.lines() {
                if ctx.use_color {
                    println!("  {}{}{}", color, line, test_colors::RESET);
                } else {
                    println!("  {}", line);
                }
            }
        } else {
            let render_opts = ctx.render_options();
            for rm in &all_matches {
                let rendered = if let Some(ref s) = rm.source {
                    render_source_precomputed(
                        s, rm.tree.as_ref(),
                        rm.line, rm.column, rm.end_line, rm.end_column,
                        &render_opts,
                    )
                } else if let Some(ref ls) = rm.lines {
                    render_lines(ls, rm.tree.as_ref(), rm.line, rm.column, rm.end_line, rm.end_column, &render_opts)
                } else if let Some(ref v) = rm.value {
                    format!("{}\n", v)
                } else if let Some(ref node) = rm.tree {
                    let rendered = if ctx.output_format == OutputFormat::Text {
                        render_query_tree_node(node, &render_opts)
                    } else {
                        let rendered = render_xml_node(node, &render_opts);
                        if render_opts.pretty_print && !rendered.ends_with('\n') {
                            format!("{}\n", rendered)
                        } else {
                            rendered
                        }
                    };
                    rendered
                } else {
                    String::new()
                };
                for line in rendered.lines() {
                    println!("  {}", line);
                }
            }
        }
    }

    if !success {
        return Err(Box::new(crate::SilentExit));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Gcc summary (stderr) — printed after gcc format output
// ---------------------------------------------------------------------------

fn gcc_summary_string(report: &Report) -> Option<String> {
    let totals = report.totals.as_ref()?;
    let mut parts = Vec::new();
    let mut updated_files = std::collections::HashSet::new();

    for m in report.all_matches() {
        if m.status.as_deref() == Some("updated") && !m.file.is_empty() {
            updated_files.insert(&m.file);
        }
    }

    if totals.fatals > 0 {
        parts.push(format!("{} fatal{}", totals.fatals, if totals.fatals == 1 { "" } else { "s" }));
    }
    if totals.errors > 0 {
        parts.push(format!("{} error{}", totals.errors, if totals.errors == 1 { "" } else { "s" }));
    }
    if totals.warnings > 0 && totals.errors == 0 && totals.fatals == 0 {
        parts.push(format!("{} warning{}", totals.warnings, if totals.warnings == 1 { "" } else { "s" }));
    }
    if !updated_files.is_empty() {
        let count = updated_files.len();
        parts.push(format!("updated {} file{}", count, if count == 1 { "" } else { "s" }));
    }

    if parts.is_empty() { return None; }

    let file_part = if totals.files > 0 && (totals.fatals > 0 || totals.errors > 0 || totals.warnings > 0) {
        format!(" in {} file{}", totals.files, if totals.files == 1 { "" } else { "s" })
    } else {
        String::new()
    };

    Some(format!("{}{}\n", parts.join(", "), file_part))
}

#[cfg(test)]
mod tests {
    use super::gcc_summary_string;
    use tractor::report::{ReportMatch, ReportBuilder, Severity};

    fn set_match(file: &str, status: &str) -> ReportMatch {
        ReportMatch {
            file: file.to_string(),
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 1,
            command: "set".to_string(),
            tree: None,
            value: None,
            source: None,
            lines: None,
            reason: None,
            severity: None,
            message: None,
            origin: None,
            rule_id: None,
            status: Some(status.to_string()),
            output: None,
        }
    }

    #[test]
    fn gcc_summary_counts_distinct_updated_files() {
        let mut builder = ReportBuilder::new();
        builder.add(set_match("a.yaml", "updated"));
        builder.add(set_match("a.yaml", "updated"));
        builder.add(set_match("b.yaml", "unchanged"));
        let report = builder.build();

        assert_eq!(gcc_summary_string(&report).as_deref(), Some("updated 1 file\n"));
    }

    #[test]
    fn gcc_summary_keeps_error_file_count_separate_from_updated_files() {
        let mut builder = ReportBuilder::new();
        builder.add(set_match("a.yaml", "updated"));
        builder.add(ReportMatch {
            file: "b.yaml".to_string(),
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 1,
            command: "check".to_string(),
            tree: None,
            value: None,
            source: None,
            lines: None,
            reason: Some("bad".to_string()),
            severity: Some(Severity::Error),
            message: None,
            origin: None,
            rule_id: Some("rule".to_string()),
            status: None,
            output: None,
        });
        let report = builder.build();

        assert_eq!(
            gcc_summary_string(&report).as_deref(),
            Some("1 error, updated 1 file in 2 files\n")
        );
    }
}
