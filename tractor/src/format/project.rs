//! Projection-aware rendering.
//!
//! When `-p` picks an element other than the full report, this module emits
//! just that element. The three parseable formats (xml/json/yaml) each route
//! through here so shape decisions live in one place; text mode has its own
//! cases because it has no envelope to begin with.
//!
//! Design invariant: output shape is determined entirely by `ProjectionPlan`.
//! Result cardinality never changes the shape — zero matches under `-p tree`
//! still emit an empty `<results/>` (XML) or `[]` (JSON). `--single` strips
//! one layer of list wrapping; it never inspects how many matches exist.

use serde_json::{json, Value};
use tractor::{
    report::{Report, ReportMatch, ResultItem, Summary},
    xml_node_to_json, RenderOptions,
};

use super::options::{OutputFormat, Projection, ViewField, ViewSet};
use super::json::{match_to_value, summary_to_json, totals_to_json};
use super::xml::{append_match, append_summary_xml, append_totals_xml, escape, render_xml_results};

// ---------------------------------------------------------------------------
// Dispatch — called from format::render_report when projection != Report.
// ---------------------------------------------------------------------------

/// Render a report as the user-selected projection. Returns the stdout payload.
///
/// The caller decides what to do with the result (print to stdout, wrap in
/// pager, etc.). No ANSI coloring is applied here — projections are primarily
/// intended for scripting / pipes where color would corrupt downstream parsers.
pub fn render_projected(
    report: &Report,
    projection: Projection,
    single: bool,
    view: &ViewSet,
    render_opts: &RenderOptions,
    format: OutputFormat,
    dimensions: &[&str],
) -> String {
    match format {
        OutputFormat::Xml    => render_projected_xml(report, projection, single, view, render_opts, dimensions),
        OutputFormat::Json   => to_json_pretty(render_projected_json(report, projection, single, view, render_opts, dimensions)),
        OutputFormat::Yaml   => to_yaml(render_projected_json(report, projection, single, view, render_opts, dimensions)),
        OutputFormat::Text   => render_projected_text(report, projection, single, view, render_opts, dimensions),
        // Non-textual formats (gcc, github, claude-code) don't have envelopes
        // that -p can meaningfully project into — fall back to the default
        // text projection for now.
        _ => render_projected_text(report, projection, single, view, render_opts, dimensions),
    }
}

// ---------------------------------------------------------------------------
// JSON / YAML — shared value builder.
// ---------------------------------------------------------------------------

fn render_projected_json(
    report: &Report,
    projection: Projection,
    single: bool,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
) -> Value {
    match projection {
        Projection::Report => {
            // Handled by the non-projection path; included for completeness.
            Value::Null
        }
        Projection::Results => {
            let items = per_match_json_items(report, view, render_opts, dimensions);
            if single {
                items.into_iter().next().unwrap_or(Value::Null)
            } else {
                Value::Array(items)
            }
        }
        Projection::Summary => summary_to_json(&Summary::from_report(report)),
        Projection::Totals => report.totals.as_ref()
            .map(totals_to_json)
            .unwrap_or_else(|| Value::Object(Default::default())),
        Projection::Schema => report.schema.as_ref()
            .map(|s| json!(s))
            .unwrap_or(Value::Null),
        Projection::Count => report.totals.as_ref()
            .map(|t| json!(t.results))
            .unwrap_or(json!(0)),
        Projection::Tree | Projection::Value | Projection::Source | Projection::Lines => {
            let field = projection.as_view_field()
                .expect("per-match projection must map to a ViewField");
            let items = per_match_field_json(report, field, render_opts);
            if single {
                items.into_iter().next().unwrap_or(Value::Null)
            } else {
                Value::Array(items)
            }
        }
    }
}

/// Emit each match as a JSON object using the current view set. Reuses
/// `match_to_value` so structural projections inherit every format-adjustment
/// the full-report renderer makes (file/line, diagnostic extras, etc.).
fn per_match_json_items(
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
) -> Vec<Value> {
    report.all_matches().into_iter()
        .map(|rm| match_to_value(rm, view, render_opts, dimensions))
        .collect()
}

/// Emit each match as a bare field value (tree object, value string, …).
/// Used for view-level projections where the user wants the raw field, not
/// a match wrapper.
fn per_match_field_json(
    report: &Report,
    field: ViewField,
    render_opts: &RenderOptions,
) -> Vec<Value> {
    report.all_matches().into_iter().filter_map(|rm| {
        match field {
            ViewField::Tree => rm.tree.as_ref()
                .map(|n| xml_node_to_json(n, render_opts.max_depth)),
            ViewField::Value => rm.value.as_ref().map(|v| json!(v)),
            ViewField::Source => rm.source.as_ref().map(|s| json!(s)),
            ViewField::Lines => rm.lines.as_ref().map(|ls| json!(ls)),
            _ => None,
        }
    }).collect()
}

fn to_json_pretty(v: Value) -> String {
    let mut out = serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".to_string());
    if !out.ends_with('\n') { out.push('\n'); }
    out
}

fn to_yaml(v: Value) -> String {
    serde_yaml::to_string(&v).unwrap_or_else(|_| "{}\n".to_string())
}

// ---------------------------------------------------------------------------
// XML — string builder (manual, matches the full-report renderer's style).
// ---------------------------------------------------------------------------

fn render_projected_xml(
    report: &Report,
    projection: Projection,
    single: bool,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
) -> String {
    let mut tree_opts = render_opts.clone();
    tree_opts.use_color = false;

    let mut body = String::new();
    match projection {
        Projection::Report => {
            // Fall through to the full-report renderer.
            return String::new();
        }
        Projection::Results => {
            render_xml_results_projection(&mut body, report, view, &tree_opts, dimensions, single);
        }
        Projection::Summary => {
            // The summary element IS the root when projected.
            append_summary_xml(&mut body, report, "");
            if body.is_empty() {
                body.push_str("<summary/>\n");
            }
        }
        Projection::Totals => {
            if let Some(ref totals) = report.totals {
                append_totals_xml(&mut body, totals, "");
            } else {
                body.push_str("<totals/>\n");
            }
        }
        Projection::Schema => {
            let content = report.schema.as_deref().unwrap_or("");
            body.push_str(&format!("<schema>{}</schema>\n", escape(content)));
        }
        Projection::Count => {
            let n = report.totals.as_ref().map(|t| t.results).unwrap_or(0);
            body.push_str(&format!("<count>{}</count>\n", n));
        }
        Projection::Tree | Projection::Value | Projection::Source | Projection::Lines => {
            let field = projection.as_view_field().unwrap();
            render_xml_field_projection(&mut body, report, field, &tree_opts, single);
        }
    }

    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", body)
}

fn render_xml_results_projection(
    out: &mut String,
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
    single: bool,
) {
    if single {
        // Bare first match — XML needs one root, which is the <match> element.
        if let Some(rm) = report.all_matches().into_iter().next() {
            append_match(out, rm, view, "", render_opts, dimensions);
        }
        return;
    }
    let mut inner = String::new();
    render_xml_results(&mut inner, &report.results, view, "  ", render_opts, dimensions);
    if inner.is_empty() {
        out.push_str("<results/>\n");
    } else {
        out.push_str("<results>\n");
        out.push_str(&inner);
        out.push_str("</results>\n");
    }
}

fn render_xml_field_projection(
    out: &mut String,
    report: &Report,
    field: ViewField,
    render_opts: &RenderOptions,
    single: bool,
) {
    let rendered: Vec<String> = report.all_matches().into_iter().filter_map(|rm| {
        render_xml_field_element(rm, field, render_opts)
    }).collect();

    if single {
        // Strip the outer <results> — emit the first element bare, as the XML root.
        if let Some(first) = rendered.into_iter().next() {
            out.push_str(&first);
            if !out.ends_with('\n') { out.push('\n'); }
        }
        return;
    }

    if rendered.is_empty() {
        out.push_str("<results/>\n");
        return;
    }
    out.push_str("<results>\n");
    for elem in rendered {
        for line in elem.lines() {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str("</results>\n");
}

fn render_xml_field_element(
    rm: &ReportMatch,
    field: ViewField,
    render_opts: &RenderOptions,
) -> Option<String> {
    match field {
        ViewField::Tree => {
            let node = rm.tree.as_ref()?;
            let inner = tractor::render_xml_node(node, render_opts);
            let mut s = String::from("<tree>\n");
            for line in inner.lines() {
                s.push_str("  ");
                s.push_str(line);
                s.push('\n');
            }
            s.push_str("</tree>");
            Some(s)
        }
        ViewField::Value => {
            let v = rm.value.as_ref()?;
            Some(format!("<value>{}</value>", escape(v)))
        }
        ViewField::Source => {
            let s = rm.source.as_ref()?;
            Some(format!("<source>{}</source>", escape(s)))
        }
        ViewField::Lines => {
            let ls = rm.lines.as_ref()?;
            let mut s = String::from("<lines>\n");
            for line in ls {
                s.push_str("  <line>");
                s.push_str(&escape(line));
                s.push_str("</line>\n");
            }
            s.push_str("</lines>");
            Some(s)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Text — no envelope to begin with; projections differ mainly in what content
// is printed and in the summary/count paths.
// ---------------------------------------------------------------------------

fn render_projected_text(
    report: &Report,
    projection: Projection,
    single: bool,
    view: &ViewSet,
    render_opts: &RenderOptions,
    _dimensions: &[&str],
) -> String {
    match projection {
        Projection::Report | Projection::Results => {
            // Delegate back to the normal text renderer — for text, projecting
            // to `results` is the same as projecting to the report (no envelope).
            // `--single` is handled by the limit-to-1 applied at the CLI.
            super::text::render_text_report(report, view, render_opts, &[])
        }
        Projection::Summary => {
            let summary = Summary::from_report(report);
            let mut out = String::new();
            if let Some(v) = summary.success {
                out.push_str(&format!("success: {}\n", v));
            }
            if let Some(t) = summary.totals {
                out.push_str(&format!("results: {}\n", t.results));
                out.push_str(&format!("files: {}\n", t.files));
                if t.fatals > 0    { out.push_str(&format!("fatals: {}\n", t.fatals)); }
                if t.errors > 0    { out.push_str(&format!("errors: {}\n", t.errors)); }
                if t.warnings > 0  { out.push_str(&format!("warnings: {}\n", t.warnings)); }
                if t.infos > 0     { out.push_str(&format!("infos: {}\n", t.infos)); }
                if t.updated > 0   { out.push_str(&format!("updated: {}\n", t.updated)); }
                if t.unchanged > 0 { out.push_str(&format!("unchanged: {}\n", t.unchanged)); }
            }
            if let Some(q) = summary.query {
                out.push_str(&format!("query: {}\n", q));
            }
            if let Some(e) = summary.expected {
                out.push_str(&format!("expected: {}\n", e));
            }
            out
        }
        Projection::Totals => {
            let mut out = String::new();
            if let Some(t) = report.totals.as_ref() {
                out.push_str(&format!("results: {}\n", t.results));
                out.push_str(&format!("files: {}\n", t.files));
                if t.fatals > 0    { out.push_str(&format!("fatals: {}\n", t.fatals)); }
                if t.errors > 0    { out.push_str(&format!("errors: {}\n", t.errors)); }
                if t.warnings > 0  { out.push_str(&format!("warnings: {}\n", t.warnings)); }
                if t.infos > 0     { out.push_str(&format!("infos: {}\n", t.infos)); }
                if t.updated > 0   { out.push_str(&format!("updated: {}\n", t.updated)); }
                if t.unchanged > 0 { out.push_str(&format!("unchanged: {}\n", t.unchanged)); }
            }
            out
        }
        Projection::Schema => {
            // Preserve today's bare-text schema rendering.
            match report.schema.as_ref() {
                Some(s) => {
                    let mut out = s.clone();
                    if !out.ends_with('\n') { out.push('\n'); }
                    out
                }
                None => String::new(),
            }
        }
        Projection::Count => {
            let n = report.totals.as_ref().map(|t| t.results).unwrap_or(0);
            format!("{}\n", n)
        }
        Projection::Tree | Projection::Value | Projection::Source | Projection::Lines => {
            // Use the normal text renderer with a view that's been pinned to
            // the projected field. This also gets us text's match-separator
            // behavior (blank lines between per-match blocks).
            let pinned = ViewSet::single(projection.as_view_field().unwrap());
            let mut r = report.clone();
            if single {
                // Keep only the first match so the renderer emits one block.
                truncate_to_first_match(&mut r);
            }
            super::text::render_text_report(&r, &pinned, render_opts, &[])
        }
    }
}

/// Prune `report.results` down to at most one leaf match (the first). Used
/// by text-mode projections where `--single` otherwise relies on `-n 1`.
fn truncate_to_first_match(report: &mut Report) {
    let mut seen = false;
    fn walk(items: &mut Vec<ResultItem>, seen: &mut bool) {
        items.retain_mut(|item| {
            if *seen { return false; }
            match item {
                ResultItem::Match(_) => {
                    *seen = true;
                    true
                }
                ResultItem::Group(g) => {
                    walk(&mut g.results, seen);
                    !g.results.is_empty()
                }
            }
        });
    }
    walk(&mut report.results, &mut seen);
}
