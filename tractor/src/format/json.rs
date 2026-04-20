use serde_json::{json, Value};
use tractor::{report::{Report, ReportMatch, ResultItem, Summary}, format_schema_tree, normalize_path, xml_node_to_json, RenderOptions};
use super::options::{ViewField, ViewSet};
use super::shared::{render_fields_for_match, should_emit_command, should_emit_file, should_emit_rule_id, should_show_totals};
use super::{Projection, ProjectionRenderError};

pub fn render_json_report(report: &Report, view: &ViewSet, render_opts: &RenderOptions, dimensions: &[&str]) -> String {
    render_json_output(report, view, render_opts, dimensions, Projection::Report, false)
        .expect("report rendering should not fail")
}

pub fn render_json_output(
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
    projection: Projection,
    single: bool,
) -> Result<String, ProjectionRenderError> {
    let value = project_json_value(report, view, render_opts, dimensions, projection, single)?;
    Ok(serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string()))
}

pub(crate) fn project_json_value(
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
    projection: Projection,
    single: bool,
) -> Result<Value, ProjectionRenderError> {
    if projection == Projection::Report && !single {
        return Ok(report_to_json_value(report, view, render_opts, dimensions));
    }

    match projection {
        Projection::Report => Ok(report_to_json_value(report, view, render_opts, dimensions)),
        Projection::Results => {
            let results = render_results_json(&report.results, view, render_opts, dimensions);
            if single {
                Ok(first_or_empty(results.into_iter())?)
            } else {
                Ok(Value::Array(results))
            }
        }
        Projection::Summary => Ok(summary_to_json(report)),
        Projection::Totals => Ok(totals_to_json(report)),
        Projection::Count => Ok(json!(report.totals.as_ref().map(|totals| totals.results).unwrap_or(0))),
        Projection::Schema => Ok(json!(schema_to_string(report, render_opts))),
        Projection::Tree | Projection::Value | Projection::Source | Projection::Lines => {
            let projected: Vec<Value> = report
                .all_matches()
                .into_iter()
                .filter_map(|rm| project_match_field_to_json(rm, projection, render_opts))
                .collect();
            if single {
                Ok(first_or_empty(projected.into_iter())?)
            } else {
                Ok(Value::Array(projected))
            }
        }
    }
}

fn report_to_json_value(
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
) -> Value {
    let mut root = serde_json::Map::new();

    if should_show_totals(report, view) {
        if let Some(summary) = summary_value(report) {
            root.insert("summary".into(), summary);
        }
    }

    if report.schema.is_some() {
        root.insert("schema".into(), json!(schema_to_string(report, render_opts)));
    }

    // Top-level captured outputs — honest view of the report model.
    // Any file-bound outputs that matched a file-group will have already
    // been moved into their group during `with_grouping`; what remains
    // here is genuinely ungrouped output (stdin payloads or orphans).
    if !report.outputs.is_empty() {
        root.insert("outputs".into(), outputs_to_json(&report.outputs));
    }

    // Group dimension (before results, so readers see the grouping context first)
    if let Some(ref group) = report.group {
        root.insert("group".into(), json!(group));
    }

    // Render results
    if !report.results.is_empty() {
        let results_json = render_results_json(&report.results, view, render_opts, dimensions);
        if !results_json.is_empty() {
            root.insert("results".into(), Value::Array(results_json));
        }
    }

    Value::Object(root)
}

/// Serialize a list of captured outputs as a JSON array of objects.
/// Each object has `content` and, if set, `file`.
pub fn outputs_to_json(outputs: &[tractor::report::ReportOutput]) -> Value {
    Value::Array(outputs.iter().map(|o| {
        output_to_json(o)
    }).collect())
}

fn output_to_json(output: &tractor::report::ReportOutput) -> Value {
    let mut obj = serde_json::Map::new();
    if let Some(ref file) = output.file {
        obj.insert("file".into(), json!(file));
    }
    obj.insert("content".into(), json!(output.content));
    Value::Object(obj)
}

fn group_outputs_to_json(
    outputs: &[tractor::report::ReportOutput],
    is_file_group: bool,
) -> (&'static str, Value) {
    if is_file_group && outputs.len() == 1 {
        let output = &outputs[0];
        if output.file.is_none() {
            return ("output", json!(output.content));
        }
        return ("output", output_to_json(output));
    }

    ("outputs", outputs_to_json(outputs))
}

fn summary_value(report: &Report) -> Option<Value> {
    let summary = Summary::from_report(report);
    if summary.is_empty() {
        None
    } else {
        serde_json::to_value(summary).ok()
    }
}

fn summary_to_json(report: &Report) -> Value {
    summary_value(report).unwrap_or_else(|| Value::Object(serde_json::Map::new()))
}

fn totals_to_json(report: &Report) -> Value {
    if let Some(ref totals) = report.totals {
        let mut t = serde_json::Map::new();
        t.insert("results".into(), json!(totals.results));
        t.insert("files".into(), json!(totals.files));
        if totals.fatals > 0 { t.insert("fatals".into(), json!(totals.fatals)); }
        if totals.errors > 0 { t.insert("errors".into(), json!(totals.errors)); }
        if totals.warnings > 0 { t.insert("warnings".into(), json!(totals.warnings)); }
        if totals.infos > 0 { t.insert("infos".into(), json!(totals.infos)); }
        if totals.updated > 0 { t.insert("updated".into(), json!(totals.updated)); }
        if totals.unchanged > 0 { t.insert("unchanged".into(), json!(totals.unchanged)); }
        Value::Object(t)
    } else {
        Value::Object(serde_json::Map::new())
    }
}

fn schema_to_string(report: &Report, render_opts: &RenderOptions) -> String {
    report
        .schema
        .as_ref()
        .map(|schema| format_schema_tree(schema, render_opts.max_depth.or(Some(4)), false))
        .unwrap_or_default()
}

fn project_match_field_to_json(
    rm: &ReportMatch,
    projection: Projection,
    render_opts: &RenderOptions,
) -> Option<Value> {
    match projection {
        Projection::Tree => rm.tree.as_ref().map(|node| xml_node_to_json(node, render_opts.max_depth)),
        Projection::Value => rm.value.as_ref().map(|value| json!(value)),
        Projection::Source => rm.source.as_ref().map(|source| json!(source)),
        Projection::Lines => rm.lines.as_ref().map(|lines| json!(lines)),
        _ => None,
    }
}

fn first_or_empty<I>(mut values: I) -> Result<Value, ProjectionRenderError>
where
    I: Iterator<Item = Value>,
{
    values.next().ok_or(ProjectionRenderError::EmptySingle)
}

/// Render a results list as JSON.
/// `dimensions`: the grouping chain (e.g. ["command", "file"]). Level 0
/// groups carry dimension[0] as their key. Leaf matches skip all dimensions.
pub fn render_results_json(
    items: &[ResultItem],
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
) -> Vec<Value> {
    items.iter().filter_map(|item| {
        match item {
            ResultItem::Match(rm) => {
                let value = match_to_value(rm, view, render_opts, dimensions);
                if value.as_object().is_some_and(|obj| obj.is_empty()) {
                    None
                } else {
                    Some(value)
                }
            }
            ResultItem::Group(sub) => {
                let mut obj = serde_json::Map::new();
                // Hoisted group key
                if let Some(ref file) = sub.file { obj.insert("file".into(), json!(file)); }
                if let Some(ref command) = sub.command { obj.insert("command".into(), json!(command)); }
                if let Some(ref rule_id) = sub.rule_id { obj.insert("rule_id".into(), json!(rule_id)); }
                if let Some(summary) = summary_value(sub) {
                    obj.insert("summary".into(), summary);
                }
                if let Some(ref schema) = sub.schema {
                    obj.insert("schema".into(), json!(schema));
                }
                // Sub-grouping dimension (before nested results)
                if let Some(ref group) = sub.group {
                    obj.insert("group".into(), json!(group));
                }
                // Group-level captured outputs — honest view of the report model.
                // Rendered unconditionally when non-empty, independent of ViewField::Output.
                if !sub.outputs.is_empty() {
                    let (key, value) = group_outputs_to_json(&sub.outputs, sub.file.is_some());
                    obj.insert(key.into(), value);
                }
                // Recurse
                let sub_results = render_results_json(&sub.results, view, render_opts, dimensions);
                if !sub_results.is_empty() {
                    obj.insert("results".into(), Value::Array(sub_results));
                }
                Some(Value::Object(obj))
            }
        }
    }).collect()
}

/// Shared match serialization — reused by yaml.rs.
/// `skip_dims`: all grouping dimensions — these fields are omitted from the match
/// since they're hoisted to ancestor groups.
pub fn match_to_value(
    rm: &ReportMatch,
    view: &ViewSet,
    render_opts: &RenderOptions,
    skip_dims: &[&str],
) -> Value {
    let mut obj = serde_json::Map::new();

    let (view_fields, extra_fields) = render_fields_for_match(view, rm);
    let all_fields: Vec<ViewField> = view_fields.into_iter().chain(extra_fields).collect();

    for field in &all_fields {
        match field {
            ViewField::File => {
                if should_emit_file(rm, skip_dims) {
                    obj.insert("file".into(), json!(normalize_path(&rm.file)));
                }
            }
            ViewField::Line   => { obj.insert("line".into(),   json!(rm.line)); }
            ViewField::Column => { obj.insert("column".into(), json!(rm.column)); }
            ViewField::Value  => {
                if let Some(ref v) = rm.value {
                    obj.insert("value".into(), json!(v));
                }
            }
            ViewField::Source => {
                if let Some(ref s) = rm.source {
                    obj.insert("source".into(), json!(s));
                }
            }
            ViewField::Lines => {
                if let Some(ref ls) = rm.lines {
                    obj.insert("lines".into(), json!(ls));
                }
            }
            ViewField::Reason => {
                if let Some(ref r) = rm.reason {
                    obj.insert("reason".into(), json!(r));
                }
            }
            ViewField::Severity => {
                if let Some(sv) = rm.severity {
                    obj.insert("severity".into(), json!(sv.as_str()));
                }
            }
            ViewField::Status => {
                if let Some(ref st) = rm.status {
                    obj.insert("status".into(), json!(st));
                }
            }
            ViewField::Output => {
                if let Some(ref output) = rm.output {
                    obj.insert("output".into(), json!(output));
                }
            }
            ViewField::Tree => {
                if let Some(ref node) = rm.tree {
                    obj.insert("tree".into(), xml_node_to_json(node, render_opts.max_depth));
                }
            }
            ViewField::Origin => {
                if rm.file.is_empty() {
                    if let Some(origin) = rm.origin {
                        obj.insert("origin".into(), json!(origin.as_str()));
                    }
                }
            }
            _ => {}
        }
    }

    if should_emit_command(rm, view, skip_dims) {
        obj.insert("command".into(), json!(rm.command));
    }
    if let Some(ref msg) = rm.message {
        obj.insert("message".into(), json!(msg));
    }
    if should_emit_rule_id(rm, skip_dims) {
        obj.insert("rule_id".into(), json!(rm.rule_id.as_deref().unwrap()));
    }

    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tractor::xpath::XmlNode;

    fn make_plain_match(value: &str) -> ReportMatch {
        ReportMatch {
            file: "test.xml".to_string(),
            line: 1, column: 1, end_line: 1, end_column: 1,
            command: String::new(),
            tree: None,
            value: Some(value.to_string()),
            source: None, lines: None, reason: None, severity: None,
            message: None, origin: None, rule_id: None, status: None, output: None,
        }
    }

    fn make_map_match(entries: Vec<(&str, XmlNode)>) -> ReportMatch {
        let tree = XmlNode::Map {
            entries: entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        };
        ReportMatch {
            file: "test.xml".to_string(),
            line: 1, column: 1, end_line: 1, end_column: 1,
            command: String::new(),
            tree: Some(tree),
            value: None, // maps have no value — data is in tree
            source: None, lines: None, reason: None, severity: None,
            message: None, origin: None, rule_id: None, status: None, output: None,
        }
    }

    #[test]
    fn test_map_tree_rendered_as_json_object() {
        let rm = make_map_match(vec![
            ("name", XmlNode::Text("foo".into())),
            ("val", XmlNode::Text("1".into())),
        ]);
        let view = ViewSet::single(ViewField::Tree);
        let opts = RenderOptions::new();
        let val = match_to_value(&rm, &view, &opts, &[]);
        let v = val.get("tree").unwrap();
        assert!(v.is_object(), "Map tree should be a JSON object, got: {}", v);
        assert_eq!(v["name"], "foo");
        assert_eq!(v["val"], "1");
    }

    #[test]
    fn test_plain_value_rendered_as_string() {
        let rm = make_plain_match("hello world");
        let view = ViewSet::single(ViewField::Value);
        let opts = RenderOptions::new();
        let val = match_to_value(&rm, &view, &opts, &[]);
        let v = val.get("value").unwrap();
        assert!(v.is_string(), "Regular value should be a string, got: {}", v);
        assert_eq!(v.as_str().unwrap(), "hello world");
    }

    #[test]
    fn test_map_tree_in_full_json_report() {
        let rm = make_map_match(vec![
            ("name", XmlNode::Text("foo".into())),
            ("count", XmlNode::Number(3.0)),
        ]);
        let mut builder = tractor::ReportBuilder::new();
        builder.set_no_verdict();
        builder.add(rm);
        let report = builder.build();
        let view = ViewSet::new(vec![ViewField::File, ViewField::Tree]);
        let opts = RenderOptions::new();
        let output = render_json_report(&report, &view, &opts, &[]);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let match_tree = &parsed["results"][0]["tree"];
        assert!(match_tree.is_object(), "Map tree should be a JSON object in report output, got: {}", match_tree);
        assert_eq!(match_tree["name"], "foo");
        assert_eq!(match_tree["count"], 3.0);
    }

    #[test]
    fn file_group_with_single_output_renders_singular_output() {
        let report = Report {
            success: Some(true),
            totals: None,
            expected: None,
            query: None,
            schema: None,
            outputs: vec![],
            results: vec![ResultItem::Group(Box::new(Report {
                success: None,
                totals: None,
                expected: None,
                query: None,
                schema: None,
                outputs: vec![tractor::report::ReportOutput {
                    file: None,
                    content: "hello\n".to_string(),
                }],
                results: vec![],
                group: None,
                file: Some("test.xml".to_string()),
                command: None,
                rule_id: None,
            }))],
            group: Some("file".to_string()),
            file: None,
            command: None,
            rule_id: None,
        };

        let rendered = render_json_report(&report, &ViewSet::new(vec![]), &RenderOptions::new(), &[]);
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(parsed["results"][0]["output"], "hello\n");
        assert!(parsed["results"][0].get("outputs").is_none());
    }

    #[test]
    fn metadata_renders_under_summary_key() {
        let mut builder = tractor::ReportBuilder::new();
        builder.set_expected("1".to_string());
        let report = builder.build();

        let rendered = render_json_report(&report, &ViewSet::new(vec![ViewField::Totals]), &RenderOptions::new(), &[]);
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();

        assert_eq!(parsed["summary"]["success"], true);
        assert_eq!(parsed["summary"]["totals"]["results"], 0);
        assert_eq!(parsed["summary"]["expected"], "1");
        assert!(parsed.get("totals").is_none());
    }
}
