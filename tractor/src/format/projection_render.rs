//! Per-format rendering for non-default `-p` / `--single` projections.
//!
//! The top-level `render_report` dispatches to one of the functions here
//! when `plan.target != Projection::Report` (or `--single` is set on a
//! structural projection). For the default case (`-p report` without
//! `--single`), existing format functions (`render_xml_report`, etc.)
//! still own the envelope.

use serde_json::{json, Value};
use tractor::{
    normalize_path, render_xml_node, xml_node_to_json,
    render_query_tree_node, render_source_precomputed, render_lines,
    report::{Report, ReportMatch},
    RenderOptions,
};

use super::options::{Cardinality, Projection, ViewField, ViewSet};
use super::json::{build_summary_object, match_to_value, render_results_json, totals_to_map};
use super::xml::{append_match, append_summary, append_totals, escape, render_xml_results};
use crate::cli::projection::ProjectionPlan;

// ---------------------------------------------------------------------------
// XML
// ---------------------------------------------------------------------------

pub fn render_xml_projection(
    report: &Report,
    plan: &ProjectionPlan,
    render_opts: &RenderOptions,
) -> String {
    let view = &plan.effective_view;
    let mut tree_opts = render_opts.clone();
    tree_opts.use_color = false;

    let mut body = String::new();

    match (plan.target, plan.target.cardinality(plan.single)) {
        (Projection::Summary, _) => {
            // Unconditionally emit the summary container so structured
            // consumers always see a root element, even when empty.
            append_summary(&mut body, report, "");
            if body.is_empty() {
                body.push_str("<summary/>\n");
            }
        }
        (Projection::Totals, _) => match report.totals.as_ref() {
            Some(t) => append_totals(&mut body, t, ""),
            None => body.push_str("<totals/>\n"),
        },
        (Projection::Schema, _) => {
            let s = report.schema.as_deref().unwrap_or("");
            body.push_str(&format!("<schema>{}</schema>\n", escape(s)));
        }
        (Projection::Count, _) => {
            let n = report.totals.as_ref().map(|t| t.results).unwrap_or(0);
            body.push_str(&format!("<count>{}</count>\n", n));
        }
        (Projection::Results, Cardinality::Sequence) => {
            let mut inner = String::new();
            render_xml_results(&mut inner, &report.results, view, "  ", &tree_opts, &[]);
            body.push_str("<results>\n");
            body.push_str(&inner);
            body.push_str("</results>\n");
        }
        (Projection::Results, Cardinality::Singular) => {
            if let Some(first) = report.all_matches().into_iter().next() {
                append_match(&mut body, first, view, "", &tree_opts, &[]);
            }
        }
        (Projection::Tree, Cardinality::Sequence) => {
            body.push_str("<results>\n");
            for m in report.all_matches() {
                if let Some(ref node) = m.tree {
                    body.push_str("  <tree>\n");
                    let rendered = render_xml_node(node, &tree_opts);
                    for line in rendered.lines() {
                        body.push_str("    ");
                        body.push_str(line);
                        body.push('\n');
                    }
                    body.push_str("  </tree>\n");
                }
            }
            body.push_str("</results>\n");
        }
        (Projection::Tree, Cardinality::Singular) => {
            if let Some(first) = report.all_matches().into_iter().next() {
                if let Some(ref node) = first.tree {
                    let rendered = render_xml_node(node, &tree_opts);
                    body.push_str(&rendered);
                    if !rendered.ends_with('\n') {
                        body.push('\n');
                    }
                }
            }
        }
        (Projection::Value, Cardinality::Sequence) => {
            body.push_str("<results>\n");
            for m in report.all_matches() {
                if let Some(ref v) = m.value {
                    body.push_str(&format!("  <value>{}</value>\n", escape(v)));
                }
            }
            body.push_str("</results>\n");
        }
        (Projection::Value, Cardinality::Singular) => {
            if let Some(first) = report.all_matches().into_iter().next() {
                if let Some(ref v) = first.value {
                    body.push_str(&format!("<value>{}</value>\n", escape(v)));
                }
            }
        }
        (Projection::Source, Cardinality::Sequence) => {
            body.push_str("<results>\n");
            for m in report.all_matches() {
                if let Some(ref s) = m.source {
                    body.push_str(&format!("  <source>{}</source>\n", escape(s)));
                }
            }
            body.push_str("</results>\n");
        }
        (Projection::Source, Cardinality::Singular) => {
            if let Some(first) = report.all_matches().into_iter().next() {
                if let Some(ref s) = first.source {
                    body.push_str(&format!("<source>{}</source>\n", escape(s)));
                }
            }
        }
        (Projection::Lines, Cardinality::Sequence) => {
            body.push_str("<results>\n");
            for m in report.all_matches() {
                if let Some(ref ls) = m.lines {
                    body.push_str("  <lines>\n");
                    for line in ls {
                        body.push_str(&format!("    <line>{}</line>\n", escape(line)));
                    }
                    body.push_str("  </lines>\n");
                }
            }
            body.push_str("</results>\n");
        }
        (Projection::Lines, Cardinality::Singular) => {
            if let Some(first) = report.all_matches().into_iter().next() {
                if let Some(ref ls) = first.lines {
                    body.push_str("<lines>\n");
                    for line in ls {
                        body.push_str(&format!("  <line>{}</line>\n", escape(line)));
                    }
                    body.push_str("</lines>\n");
                }
            }
        }
        (Projection::Report, _) => {
            unreachable!("render_xml_projection: Report is rendered by render_xml_report");
        }
    }

    if body.is_empty() {
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n")
    } else {
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", body)
    }
}

// ---------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------

pub fn render_json_projection(
    report: &Report,
    plan: &ProjectionPlan,
    render_opts: &RenderOptions,
) -> String {
    let view = &plan.effective_view;

    let value = build_json_projection(report, plan, view, render_opts);
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "null".to_string())
}

fn build_json_projection(
    report: &Report,
    plan: &ProjectionPlan,
    view: &ViewSet,
    render_opts: &RenderOptions,
) -> Value {
    match (plan.target, plan.target.cardinality(plan.single)) {
        (Projection::Summary, _) => {
            build_summary_object(report)
                .map(Value::Object)
                .unwrap_or(Value::Object(serde_json::Map::new()))
        }
        (Projection::Totals, _) => match report.totals.as_ref() {
            Some(t) => Value::Object(totals_to_map(t)),
            None => Value::Object(serde_json::Map::new()),
        },
        (Projection::Schema, _) => {
            json!(report.schema.as_deref().unwrap_or(""))
        }
        (Projection::Count, _) => {
            json!(report.totals.as_ref().map(|t| t.results).unwrap_or(0))
        }
        (Projection::Results, Cardinality::Sequence) => {
            Value::Array(render_results_json(&report.results, view, render_opts, &[]))
        }
        (Projection::Results, Cardinality::Singular) => {
            report.all_matches().into_iter().next()
                .map(|m| match_to_value(m, view, render_opts, &[]))
                .unwrap_or(Value::Null)
        }
        (Projection::Tree, Cardinality::Sequence) => {
            Value::Array(
                report.all_matches().iter()
                    .filter_map(|m| m.tree.as_ref())
                    .map(|n| xml_node_to_json(n, render_opts.max_depth))
                    .collect(),
            )
        }
        (Projection::Tree, Cardinality::Singular) => {
            report.all_matches().into_iter().next()
                .and_then(|m| m.tree.as_ref())
                .map(|n| xml_node_to_json(n, render_opts.max_depth))
                .unwrap_or(Value::Null)
        }
        (Projection::Value, Cardinality::Sequence) => {
            Value::Array(
                report.all_matches().iter()
                    .filter_map(|m| m.value.clone())
                    .map(Value::String)
                    .collect(),
            )
        }
        (Projection::Value, Cardinality::Singular) => {
            report.all_matches().into_iter().next()
                .and_then(|m| m.value.clone())
                .map(Value::String)
                .unwrap_or(Value::Null)
        }
        (Projection::Source, Cardinality::Sequence) => {
            Value::Array(
                report.all_matches().iter()
                    .filter_map(|m| m.source.clone())
                    .map(Value::String)
                    .collect(),
            )
        }
        (Projection::Source, Cardinality::Singular) => {
            report.all_matches().into_iter().next()
                .and_then(|m| m.source.clone())
                .map(Value::String)
                .unwrap_or(Value::Null)
        }
        (Projection::Lines, Cardinality::Sequence) => {
            Value::Array(
                report.all_matches().iter()
                    .filter_map(|m| m.lines.as_ref())
                    .map(|ls| json!(ls))
                    .collect(),
            )
        }
        (Projection::Lines, Cardinality::Singular) => {
            report.all_matches().into_iter().next()
                .and_then(|m| m.lines.as_ref())
                .map(|ls| json!(ls))
                .unwrap_or(Value::Null)
        }
        (Projection::Report, _) => {
            unreachable!("render_json_projection: Report is rendered by render_json_report");
        }
    }
}

// ---------------------------------------------------------------------------
// YAML — reuses the JSON value builder; YAML serializer differs.
// ---------------------------------------------------------------------------

pub fn render_yaml_projection(
    report: &Report,
    plan: &ProjectionPlan,
    render_opts: &RenderOptions,
) -> String {
    let view = &plan.effective_view;
    let value = build_json_projection(report, plan, view, render_opts);
    serde_yaml::to_string(&value).unwrap_or_else(|_| "null\n".to_string())
}

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

pub fn render_text_projection(
    report: &Report,
    plan: &ProjectionPlan,
    render_opts: &RenderOptions,
) -> String {
    let mut out = String::new();

    match (plan.target, plan.target.cardinality(plan.single)) {
        (Projection::Summary, _) => {
            // Text summary: render the same fields that would appear at the
            // end of a normal report, but without the matches above them.
            if let Some(ref query) = report.query {
                out.push_str(&format!("Query: {}\n", query));
            }
            if let Some(ref totals) = report.totals {
                out.push_str(&format_totals_line(totals, report.success, report.expected.as_deref()));
            }
        }
        (Projection::Totals, _) => {
            if let Some(ref t) = report.totals {
                out.push_str(&format_totals_line(t, None, None));
            }
        }
        (Projection::Schema, _) => {
            if let Some(ref s) = report.schema {
                out.push_str(s);
                if !s.ends_with('\n') { out.push('\n'); }
            }
        }
        (Projection::Count, _) => {
            let n = report.totals.as_ref().map(|t| t.results).unwrap_or(0);
            out.push_str(&format!("{}\n", n));
        }
        (Projection::Results, Cardinality::Sequence) => {
            for m in report.all_matches() {
                append_match_text(&mut out, m, &plan.effective_view, render_opts);
            }
        }
        (Projection::Results, Cardinality::Singular) => {
            if let Some(first) = report.all_matches().into_iter().next() {
                append_match_text(&mut out, first, &plan.effective_view, render_opts);
            }
        }
        (Projection::Tree, Cardinality::Sequence) => {
            for m in report.all_matches() {
                if let Some(ref node) = m.tree {
                    out.push_str(&render_query_tree_node(node, render_opts));
                }
            }
        }
        (Projection::Tree, Cardinality::Singular) => {
            if let Some(first) = report.all_matches().into_iter().next() {
                if let Some(ref node) = first.tree {
                    out.push_str(&render_query_tree_node(node, render_opts));
                }
            }
        }
        (Projection::Value, Cardinality::Sequence) => {
            for m in report.all_matches() {
                if let Some(ref v) = m.value {
                    out.push_str(v);
                    out.push('\n');
                }
            }
        }
        (Projection::Value, Cardinality::Singular) => {
            if let Some(first) = report.all_matches().into_iter().next() {
                if let Some(ref v) = first.value {
                    out.push_str(v);
                    out.push('\n');
                }
            }
        }
        (Projection::Source, Cardinality::Sequence) => {
            for m in report.all_matches() {
                if let Some(ref s) = m.source {
                    out.push_str(&render_source_precomputed(
                        s, m.tree.as_ref(),
                        m.line, m.column, m.end_line, m.end_column,
                        render_opts,
                    ));
                }
            }
        }
        (Projection::Source, Cardinality::Singular) => {
            if let Some(first) = report.all_matches().into_iter().next() {
                if let Some(ref s) = first.source {
                    out.push_str(&render_source_precomputed(
                        s, first.tree.as_ref(),
                        first.line, first.column, first.end_line, first.end_column,
                        render_opts,
                    ));
                }
            }
        }
        (Projection::Lines, Cardinality::Sequence) => {
            for m in report.all_matches() {
                if let Some(ref ls) = m.lines {
                    out.push_str(&render_lines(
                        ls, m.tree.as_ref(),
                        m.line, m.column, m.end_line, m.end_column,
                        render_opts,
                    ));
                }
            }
        }
        (Projection::Lines, Cardinality::Singular) => {
            if let Some(first) = report.all_matches().into_iter().next() {
                if let Some(ref ls) = first.lines {
                    out.push_str(&render_lines(
                        ls, first.tree.as_ref(),
                        first.line, first.column, first.end_line, first.end_column,
                        render_opts,
                    ));
                }
            }
        }
        (Projection::Report, _) => {
            unreachable!("render_text_projection: Report is rendered by render_text_report");
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Text helpers — match rendering and summary line.
// ---------------------------------------------------------------------------

fn append_match_text(
    out: &mut String,
    rm: &ReportMatch,
    view: &ViewSet,
    render_opts: &RenderOptions,
) {
    if let Some(ref msg) = rm.message {
        out.push_str(msg);
        if !msg.ends_with('\n') { out.push('\n'); }
        return;
    }

    let has_location = view.has(ViewField::File)
        || view.has(ViewField::Line)
        || view.has(ViewField::Column);
    if has_location && !rm.file.is_empty() && rm.file != "<stdin>" {
        let mut loc = String::new();
        if view.has(ViewField::File) {
            loc.push_str(&normalize_path(&rm.file));
        }
        if view.has(ViewField::Line) {
            if !loc.is_empty() { loc.push(':'); }
            loc.push_str(&rm.line.to_string());
        }
        if view.has(ViewField::Column) {
            loc.push(':');
            loc.push_str(&rm.column.to_string());
        }
        out.push_str(&loc);
        out.push('\n');
    }

    if view.has(ViewField::Tree) {
        if let Some(ref node) = rm.tree {
            out.push_str(&render_query_tree_node(node, render_opts));
        }
    }
    if view.has(ViewField::Value) {
        if let Some(ref v) = rm.value {
            out.push_str(v);
            out.push('\n');
        }
    }
    if view.has(ViewField::Source) {
        if let Some(ref s) = rm.source {
            out.push_str(&render_source_precomputed(
                s, rm.tree.as_ref(),
                rm.line, rm.column, rm.end_line, rm.end_column,
                render_opts,
            ));
        }
    }
    if view.has(ViewField::Lines) {
        if let Some(ref ls) = rm.lines {
            out.push_str(&render_lines(
                ls, rm.tree.as_ref(),
                rm.line, rm.column, rm.end_line, rm.end_column,
                render_opts,
            ));
        }
    }
}

fn format_totals_line(
    totals: &tractor::report::Totals,
    success: Option<bool>,
    _expected: Option<&str>,
) -> String {
    let f = totals.files;
    if success.is_none() {
        if f <= 1 {
            return format!("{} matches\n", totals.results);
        }
        return format!("{} matches in {} files\n", totals.results, f);
    }
    let verdict = success.unwrap_or(true);
    if verdict {
        format!("{} matches across {} files\n", totals.results, f)
    } else {
        format!("{} matches, {} errors\n", totals.results, totals.errors)
    }
}
