use serde_json::Value;
use tractor::{report::Report, RenderOptions};
use super::options::{ViewSet, Projection};
use super::json::{emit_report_metadata, outputs_to_json, render_results_json};
use super::shared::should_show_totals;

pub fn render_yaml_report(
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
    projection: Projection,
    single: bool,
) -> String {
    let value = match projection {
        Projection::Report  => render_full_report_yaml(report, view, render_opts, dimensions),
        Projection::Results => render_results_projection_yaml(report, view, render_opts, dimensions, single),
        Projection::Summary => render_summary_projection_yaml(report),
        Projection::Totals  => render_totals_projection_yaml(report),
        Projection::Count   => render_count_projection_yaml(report),
        Projection::Schema  => render_schema_projection_yaml(report),
        Projection::Tree | Projection::Value | Projection::Source | Projection::Lines => {
            render_per_match_projection_yaml(report, render_opts, projection, single)
        }
    };
    serde_yaml::to_string(&value).unwrap_or_else(|_| "null\n".to_string())
}

fn render_full_report_yaml(
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
) -> Value {
    let mut root = serde_json::Map::new();

    if should_show_totals(report, view) {
        let mut summary = serde_json::Map::new();
        emit_report_metadata(&mut summary, report);
        if !summary.is_empty() {
            root.insert("summary".into(), Value::Object(summary));
        }
    }

    if let Some(ref schema) = report.schema {
        root.insert("schema".into(), serde_json::json!(schema));
    }

    if !report.outputs.is_empty() {
        root.insert("outputs".into(), outputs_to_json(&report.outputs));
    }

    if let Some(ref group) = report.group {
        root.insert("group".into(), serde_json::json!(group));
    }

    if !report.results.is_empty() {
        let results_json = render_results_json(&report.results, view, render_opts, dimensions);
        if !results_json.is_empty() {
            root.insert("results".into(), Value::Array(results_json));
        }
    }

    Value::Object(root)
}

fn render_results_projection_yaml(
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
    single: bool,
) -> Value {
    let results = render_results_json(&report.results, view, render_opts, dimensions);
    if single {
        results.into_iter().next().unwrap_or(Value::Null)
    } else {
        Value::Array(results)
    }
}

fn render_summary_projection_yaml(report: &Report) -> Value {
    let mut obj = serde_json::Map::new();
    emit_report_metadata(&mut obj, report);
    Value::Object(obj)
}

fn render_totals_projection_yaml(report: &Report) -> Value {
    if let Some(ref totals) = report.totals {
        let mut t = serde_json::Map::new();
        t.insert("results".into(), serde_json::json!(totals.results));
        t.insert("files".into(),   serde_json::json!(totals.files));
        if totals.fatals > 0    { t.insert("fatals".into(),    serde_json::json!(totals.fatals));    }
        if totals.errors > 0    { t.insert("errors".into(),    serde_json::json!(totals.errors));    }
        if totals.warnings > 0  { t.insert("warnings".into(),  serde_json::json!(totals.warnings));  }
        if totals.infos > 0     { t.insert("infos".into(),     serde_json::json!(totals.infos));     }
        if totals.updated > 0   { t.insert("updated".into(),   serde_json::json!(totals.updated));   }
        if totals.unchanged > 0 { t.insert("unchanged".into(), serde_json::json!(totals.unchanged)); }
        Value::Object(t)
    } else {
        Value::Null
    }
}

fn render_count_projection_yaml(report: &Report) -> Value {
    serde_json::json!(report.all_matches().len())
}

fn render_schema_projection_yaml(report: &Report) -> Value {
    if let Some(ref schema) = report.schema {
        serde_json::json!(schema)
    } else {
        Value::Null
    }
}

fn render_per_match_projection_yaml(
    report: &Report,
    render_opts: &RenderOptions,
    projection: Projection,
    single: bool,
) -> Value {
    use tractor::xml_node_to_json;
    let values: Vec<Value> = report.all_matches().iter().filter_map(|rm| {
        match projection {
            Projection::Tree   => rm.tree.as_ref().map(|node| xml_node_to_json(node, render_opts.max_depth)),
            Projection::Value  => rm.value.as_ref().map(|v| serde_json::json!(v)),
            Projection::Source => rm.source.as_ref().map(|s| serde_json::json!(s)),
            Projection::Lines  => rm.lines.as_ref().map(|ls| serde_json::json!(ls)),
            _ => None,
        }
    }).collect();

    if single {
        values.into_iter().next().unwrap_or(Value::Null)
    } else {
        Value::Array(values)
    }
}
