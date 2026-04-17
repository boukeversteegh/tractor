//! Projection rendering (-p/--project flag).
//!
//! The `-p` flag selects a specific element of the built report to emit.
//! `--single` drops the list wrapper from sequence projections and takes
//! only the first match.
//!
//! Design contract (see `docs/design-p-projection-flag.md`):
//! - Every projection value corresponds to an element in the report.
//! - Output shape depends on flags alone — never on result cardinality.
//! - XML always emits a single root (valid XML).

use serde_json::{json, Value};
use tractor::{
    render_query_tree_node, render_xml_node, render_source_precomputed, render_lines,
    report::{Report, ReportMatch, ResultItem},
    RenderOptions,
};

use super::options::{OutputFormat, Projection, ViewField, ViewSet};
use super::json::{match_to_value, summary_to_json, totals_to_json};
use super::xml::{append_summary_xml, append_totals_xml, escape_xml};
use super::text::render_text_report;

/// Entry point: render `report` as the requested `projection`. Returns
/// pre-serialized text for the given format.
pub fn render_projection(
    report: &Report,
    projection: Projection,
    single: bool,
    format: OutputFormat,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
) -> String {
    // Per-match sequence projections: collect the matches (flattened), then
    // render as list-or-bare depending on `single`.
    if projection.is_view_level() || projection == Projection::Results {
        return render_sequence_projection(report, projection, single, format, view, render_opts, dimensions);
    }

    // Metadata / structural singular projections.
    match projection {
        Projection::Summary => render_summary_projection(report, format),
        Projection::Totals  => render_totals_projection(report, format),
        Projection::Report  => render_full_report(report, format, view, render_opts, dimensions),
        // Already handled above, but keep the match exhaustive.
        _ => render_sequence_projection(report, projection, single, format, view, render_opts, dimensions),
    }
}

// ---------------------------------------------------------------------------
// Sequence projections (tree/value/source/lines/results) + singular
// view-level projections (schema, count).
// ---------------------------------------------------------------------------

fn render_sequence_projection(
    report: &Report,
    projection: Projection,
    single: bool,
    format: OutputFormat,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
) -> String {
    match projection {
        Projection::Count => render_count_projection(report, format),
        Projection::Schema => render_schema_projection(report, format),
        Projection::Tree | Projection::Value | Projection::Source | Projection::Lines => {
            render_per_match_field_projection(report, projection, single, format, render_opts)
        }
        Projection::Results => {
            render_results_projection(report, single, format, view, render_opts, dimensions)
        }
        _ => String::new(),
    }
}

fn render_count_projection(report: &Report, format: OutputFormat) -> String {
    let n = report.totals.as_ref().map(|t| t.results).unwrap_or(0);
    match format {
        OutputFormat::Xml => format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<count>{}</count>\n", n),
        OutputFormat::Json => format!("{}\n", n),
        OutputFormat::Yaml => format!("{}\n", n),
        _ => format!("{}\n", n),
    }
}

fn render_schema_projection(report: &Report, format: OutputFormat) -> String {
    let text = report.schema.as_deref().unwrap_or("");
    match format {
        OutputFormat::Xml => format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<schema>{}</schema>\n",
            escape_xml(text),
        ),
        OutputFormat::Json => format!("{}\n", serde_json::to_string(text).unwrap_or_default()),
        OutputFormat::Yaml => {
            let v: Value = json!(text);
            serde_yaml::to_string(&v).unwrap_or_else(|_| format!("{}\n", text))
        }
        _ => text.to_string(),
    }
}

/// Render `-p tree|value|source|lines`: one entry per match, list-wrapped
/// unless `single` is set.
fn render_per_match_field_projection(
    report: &Report,
    projection: Projection,
    single: bool,
    format: OutputFormat,
    render_opts: &RenderOptions,
) -> String {
    let matches: Vec<&ReportMatch> = collect_matches(&report.results);
    let selected: Vec<&ReportMatch> = if single {
        matches.into_iter().take(1).collect()
    } else {
        matches
    };

    match format {
        OutputFormat::Xml => render_per_match_xml(&selected, projection, single, render_opts),
        OutputFormat::Json => render_per_match_json(&selected, projection, single, render_opts),
        OutputFormat::Yaml => render_per_match_yaml(&selected, projection, single, render_opts),
        OutputFormat::Text => render_per_match_text(&selected, projection, render_opts),
        // Other formats (gcc/github/claude-code) don't meaningfully support
        // these projections — fall back to per-match field text.
        _ => render_per_match_text(&selected, projection, render_opts),
    }
}

fn render_per_match_xml(
    matches: &[&ReportMatch],
    projection: Projection,
    single: bool,
    render_opts: &RenderOptions,
) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    if single {
        if let Some(rm) = matches.first() {
            out.push_str(&render_match_field_xml(rm, projection, render_opts, ""));
        } else {
            out.push_str("<results/>\n");
        }
        return out;
    }
    if matches.is_empty() {
        out.push_str("<results/>\n");
        return out;
    }
    out.push_str("<results>\n");
    for rm in matches {
        out.push_str(&render_match_field_xml(rm, projection, render_opts, "  "));
    }
    out.push_str("</results>\n");
    out
}

fn render_match_field_xml(
    rm: &ReportMatch,
    projection: Projection,
    render_opts: &RenderOptions,
    indent: &str,
) -> String {
    match projection {
        Projection::Tree => {
            let body = rm.tree.as_ref()
                .map(|n| render_xml_node(n, render_opts))
                .unwrap_or_default();
            let body = body.trim_end();
            let inner = if body.is_empty() { String::new() } else { format!("\n{}{}\n{}", indent, body.replace('\n', &format!("\n{}", indent)), indent) };
            format!("{}<tree>{}</tree>\n", indent, inner)
        }
        Projection::Value => {
            let v = rm.value.as_deref().unwrap_or("");
            format!("{}<value>{}</value>\n", indent, escape_xml(v))
        }
        Projection::Source => {
            let s = rm.source.as_deref().unwrap_or("");
            format!("{}<source>{}</source>\n", indent, escape_xml(s))
        }
        Projection::Lines => {
            let inner = rm.lines.as_deref().map(|ls| {
                ls.iter()
                    .map(|l| format!("{}  <line>{}</line>\n", indent, escape_xml(l)))
                    .collect::<String>()
            }).unwrap_or_default();
            if inner.is_empty() {
                format!("{}<lines/>\n", indent)
            } else {
                format!("{}<lines>\n{}{}</lines>\n", indent, inner, indent)
            }
        }
        _ => String::new(),
    }
}

fn render_per_match_json(
    matches: &[&ReportMatch],
    projection: Projection,
    single: bool,
    render_opts: &RenderOptions,
) -> String {
    if single {
        let v = matches.first()
            .map(|rm| match_field_json(rm, projection, render_opts))
            .unwrap_or(Value::Null);
        return format!("{}\n", serde_json::to_string_pretty(&v).unwrap_or_default());
    }
    let arr: Vec<Value> = matches.iter()
        .map(|rm| match_field_json(rm, projection, render_opts))
        .collect();
    format!("{}\n", serde_json::to_string_pretty(&Value::Array(arr)).unwrap_or("[]".to_string()))
}

fn match_field_json(rm: &ReportMatch, projection: Projection, render_opts: &RenderOptions) -> Value {
    match projection {
        Projection::Tree => rm.tree.as_ref()
            .map(|n| tractor::xml_node_to_json(n, render_opts.max_depth))
            .unwrap_or(Value::Null),
        Projection::Value  => rm.value.as_deref().map(|s| json!(s)).unwrap_or(Value::Null),
        Projection::Source => rm.source.as_deref().map(|s| json!(s)).unwrap_or(Value::Null),
        Projection::Lines  => rm.lines.as_deref().map(|ls| json!(ls)).unwrap_or(Value::Null),
        _ => Value::Null,
    }
}

fn render_per_match_yaml(
    matches: &[&ReportMatch],
    projection: Projection,
    single: bool,
    render_opts: &RenderOptions,
) -> String {
    let v = if single {
        matches.first()
            .map(|rm| match_field_json(rm, projection, render_opts))
            .unwrap_or(Value::Null)
    } else {
        Value::Array(matches.iter()
            .map(|rm| match_field_json(rm, projection, render_opts))
            .collect())
    };
    serde_yaml::to_string(&v).unwrap_or_else(|_| String::new())
}

fn render_per_match_text(
    matches: &[&ReportMatch],
    projection: Projection,
    render_opts: &RenderOptions,
) -> String {
    let mut out = String::new();
    for rm in matches {
        match projection {
            Projection::Tree => {
                if let Some(ref node) = rm.tree {
                    let rendered = render_query_tree_node(node, render_opts);
                    out.push_str(&rendered);
                    if !rendered.ends_with('\n') { out.push('\n'); }
                }
            }
            Projection::Value => {
                if let Some(ref v) = rm.value {
                    out.push_str(v);
                    out.push('\n');
                }
            }
            Projection::Source => {
                if let Some(ref s) = rm.source {
                    let rendered = render_source_precomputed(
                        s, rm.tree.as_ref(),
                        rm.line, rm.column, rm.end_line, rm.end_column,
                        render_opts,
                    );
                    out.push_str(&rendered);
                }
            }
            Projection::Lines => {
                if let Some(ref ls) = rm.lines {
                    out.push_str(&render_lines(
                        ls, rm.tree.as_ref(),
                        rm.line, rm.column, rm.end_line, rm.end_column,
                        render_opts,
                    ));
                }
            }
            _ => {}
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Structural projections (results, report) and metadata (summary, totals).
// ---------------------------------------------------------------------------

/// `-p results`: the list of matches with `-v`-driven fields. `--single` takes
/// the first match bare.
fn render_results_projection(
    report: &Report,
    single: bool,
    format: OutputFormat,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
) -> String {
    let matches: Vec<&ReportMatch> = collect_matches(&report.results);
    let selected: Vec<&ReportMatch> = if single {
        matches.into_iter().take(1).collect()
    } else {
        matches
    };

    match format {
        OutputFormat::Json => {
            let arr: Vec<Value> = selected.iter()
                .map(|rm| match_to_value(rm, view, render_opts, dimensions))
                .collect();
            if single {
                let v = arr.into_iter().next().unwrap_or(Value::Null);
                format!("{}\n", serde_json::to_string_pretty(&v).unwrap_or_default())
            } else {
                format!("{}\n", serde_json::to_string_pretty(&Value::Array(arr)).unwrap_or_default())
            }
        }
        OutputFormat::Yaml => {
            let arr: Vec<Value> = selected.iter()
                .map(|rm| match_to_value(rm, view, render_opts, dimensions))
                .collect();
            let v = if single {
                arr.into_iter().next().unwrap_or(Value::Null)
            } else {
                Value::Array(arr)
            };
            serde_yaml::to_string(&v).unwrap_or_else(|_| String::new())
        }
        OutputFormat::Xml => {
            let mut body = String::new();
            body.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
            if single {
                match selected.first() {
                    Some(rm) => {
                        super::xml::append_match_xml(&mut body, rm, view, "", render_opts, dimensions);
                    }
                    None => body.push_str("<results/>\n"),
                }
            } else {
                if selected.is_empty() {
                    body.push_str("<results/>\n");
                } else {
                    body.push_str("<results>\n");
                    for rm in &selected {
                        super::xml::append_match_xml(&mut body, rm, view, "  ", render_opts, dimensions);
                    }
                    body.push_str("</results>\n");
                }
            }
            body
        }
        OutputFormat::Text => {
            // Text doesn't carry a wrapper — just emit matches. We reuse
            // `render_text_report` after stripping non-match content.
            let mut stripped = Report::empty();
            stripped.results = selected.into_iter()
                .map(|rm| ResultItem::Match(rm.clone()))
                .collect();
            render_text_report(&stripped, view, render_opts, dimensions)
        }
        _ => {
            let mut stripped = Report::empty();
            stripped.results = selected.into_iter()
                .map(|rm| ResultItem::Match(rm.clone()))
                .collect();
            render_text_report(&stripped, view, render_opts, dimensions)
        }
    }
}

fn render_summary_projection(report: &Report, format: OutputFormat) -> String {
    match format {
        OutputFormat::Json => {
            let v = summary_to_json(report).unwrap_or(Value::Object(Default::default()));
            format!("{}\n", serde_json::to_string_pretty(&v).unwrap_or_default())
        }
        OutputFormat::Yaml => {
            let v = summary_to_json(report).unwrap_or(Value::Object(Default::default()));
            serde_yaml::to_string(&v).unwrap_or_else(|_| String::new())
        }
        OutputFormat::Xml => {
            let mut body = String::new();
            body.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
            append_summary_xml(&mut body, report, "");
            body
        }
        OutputFormat::Text => {
            let mut out = String::new();
            if let Some(ref query) = report.query {
                out.push_str(&format!("Query: {}\n", query));
            }
            if let Some(ref totals) = report.totals {
                out.push_str(&super::text::format_summary_public(totals, report.success, report.expected.as_deref()));
            }
            out
        }
        _ => String::new(),
    }
}

fn render_totals_projection(report: &Report, format: OutputFormat) -> String {
    let Some(ref totals) = report.totals else { return String::new(); };
    match format {
        OutputFormat::Json => {
            format!("{}\n", serde_json::to_string_pretty(&totals_to_json(totals)).unwrap_or_default())
        }
        OutputFormat::Yaml => {
            serde_yaml::to_string(&totals_to_json(totals)).unwrap_or_else(|_| String::new())
        }
        OutputFormat::Xml => {
            let mut body = String::new();
            body.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
            append_totals_xml(&mut body, totals, "");
            body
        }
        OutputFormat::Text => {
            super::text::format_summary_public(totals, report.success, report.expected.as_deref())
        }
        _ => String::new(),
    }
}

fn render_full_report(
    report: &Report,
    format: OutputFormat,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
) -> String {
    // For `-p report`, invoke the normal full-envelope renderer.
    match format {
        OutputFormat::Json => super::render_json_report(report, view, render_opts, dimensions),
        OutputFormat::Yaml => super::render_yaml_report(report, view, render_opts, dimensions),
        OutputFormat::Xml  => super::render_xml_report(report, view, render_opts, dimensions),
        OutputFormat::Text => super::render_text_report(report, view, render_opts, dimensions),
        _ => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Depth-first flatten of `results` into a list of `ReportMatch` references.
fn collect_matches(items: &[ResultItem]) -> Vec<&ReportMatch> {
    let mut out = Vec::new();
    collect_matches_rec(items, &mut out);
    out
}

fn collect_matches_rec<'a>(items: &'a [ResultItem], out: &mut Vec<&'a ReportMatch>) {
    for item in items {
        match item {
            ResultItem::Match(rm) => out.push(rm),
            ResultItem::Group(g) => collect_matches_rec(&g.results, out),
        }
    }
}

/// Emit a warning on stderr when `--single` is used with a singular
/// projection (no-op case). Called once from render_report.
pub fn warn_single_on_singular(projection: Projection) {
    eprintln!(
        "warning: --single has no effect with -p {} (already a single element). \
         Drop --single.",
        projection.name(),
    );
}

// The unused-imports guard — these are pulled in only for specific arms.
#[allow(dead_code)]
fn _unused(_: ViewField) {}
