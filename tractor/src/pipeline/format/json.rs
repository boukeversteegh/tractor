use super::options::{ViewField, ViewSet};
use super::shared::{
    render_fields_for_match, should_emit_command, should_emit_file, should_emit_rule_id,
    should_show_totals,
};
use serde_json::{json, Value};
use tractor_core::{
    normalize_path,
    report::{Report, ReportMatch, ResultItem},
    xml_node_to_json, RenderOptions,
};

pub fn render_json_report(
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
    dimensions: &[&str],
) -> String {
    let mut root = serde_json::Map::new();

    if should_show_totals(report, view) {
        emit_report_metadata(&mut root, report);
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

    serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_else(|_| "{}".to_string())
}

/// Emit success, totals, expected, query as top-level fields.
pub fn emit_report_metadata(root: &mut serde_json::Map<String, Value>, report: &Report) {
    if let Some(success) = report.success {
        root.insert("success".into(), json!(success));
    }
    if let Some(ref totals) = report.totals {
        let mut t = serde_json::Map::new();
        t.insert("results".into(), json!(totals.results));
        t.insert("files".into(), json!(totals.files));
        if totals.fatals > 0 {
            t.insert("fatals".into(), json!(totals.fatals));
        }
        if totals.errors > 0 {
            t.insert("errors".into(), json!(totals.errors));
        }
        if totals.warnings > 0 {
            t.insert("warnings".into(), json!(totals.warnings));
        }
        if totals.infos > 0 {
            t.insert("infos".into(), json!(totals.infos));
        }
        if totals.updated > 0 {
            t.insert("updated".into(), json!(totals.updated));
        }
        if totals.unchanged > 0 {
            t.insert("unchanged".into(), json!(totals.unchanged));
        }
        root.insert("totals".into(), Value::Object(t));
    }
    if let Some(ref expected) = report.expected {
        root.insert("expected".into(), json!(expected));
    }
    if let Some(ref query) = report.query {
        root.insert("query".into(), json!(query));
    }
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
    items
        .iter()
        .map(|item| {
            match item {
                ResultItem::Match(rm) => match_to_value(rm, view, render_opts, dimensions),
                ResultItem::Group(sub) => {
                    let mut obj = serde_json::Map::new();
                    // Hoisted group key
                    if let Some(ref file) = sub.file {
                        obj.insert("file".into(), json!(file));
                    }
                    if let Some(ref command) = sub.command {
                        obj.insert("command".into(), json!(command));
                    }
                    if let Some(ref rule_id) = sub.rule_id {
                        obj.insert("rule_id".into(), json!(rule_id));
                    }
                    emit_report_metadata(&mut obj, sub);
                    // Sub-grouping dimension (before nested results)
                    if let Some(ref group) = sub.group {
                        obj.insert("group".into(), json!(group));
                    }
                    // Group-level output (set stdout mode)
                    if view.has(ViewField::Output) {
                        if let Some(ref content) = sub.output_content {
                            obj.insert("output".into(), json!(content));
                        }
                    }
                    // Recurse
                    let sub_results =
                        render_results_json(&sub.results, view, render_opts, dimensions);
                    if !sub_results.is_empty() {
                        obj.insert("results".into(), Value::Array(sub_results));
                    }
                    Value::Object(obj)
                }
            }
        })
        .collect()
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
            ViewField::Line => {
                obj.insert("line".into(), json!(rm.line));
            }
            ViewField::Column => {
                obj.insert("column".into(), json!(rm.column));
            }
            ViewField::Value => {
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
    use tractor_core::report::{Report, Totals};
    use tractor_core::xpath::XmlNode;

    fn make_plain_match(value: &str) -> ReportMatch {
        ReportMatch {
            file: "test.xml".to_string(),
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 1,
            command: String::new(),
            tree: None,
            value: Some(value.to_string()),
            source: None,
            lines: None,
            reason: None,
            severity: None,
            message: None,
            origin: None,
            rule_id: None,
            status: None,
            output: None,
        }
    }

    fn make_map_match(entries: Vec<(&str, XmlNode)>) -> ReportMatch {
        let tree = XmlNode::Map {
            entries: entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        };
        ReportMatch {
            file: "test.xml".to_string(),
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 1,
            command: String::new(),
            tree: Some(tree),
            value: None, // maps have no value — data is in tree
            source: None,
            lines: None,
            reason: None,
            severity: None,
            message: None,
            origin: None,
            rule_id: None,
            status: None,
            output: None,
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
        assert!(
            v.is_object(),
            "Map tree should be a JSON object, got: {}",
            v
        );
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
        assert!(
            v.is_string(),
            "Regular value should be a string, got: {}",
            v
        );
        assert_eq!(v.as_str().unwrap(), "hello world");
    }

    #[test]
    fn test_map_tree_in_full_json_report() {
        let rm = make_map_match(vec![
            ("name", XmlNode::Text("foo".into())),
            ("count", XmlNode::Number(3.0)),
        ]);
        let mut builder = tractor_core::ReportBuilder::new();
        builder.set_no_verdict();
        builder.add(rm);
        let report = builder.build();
        let view = ViewSet::new(vec![ViewField::File, ViewField::Tree]);
        let opts = RenderOptions::new();
        let output = render_json_report(&report, &view, &opts, &[]);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let match_tree = &parsed["results"][0]["tree"];
        assert!(
            match_tree.is_object(),
            "Map tree should be a JSON object in report output, got: {}",
            match_tree
        );
        assert_eq!(match_tree["name"], "foo");
        assert_eq!(match_tree["count"], 3.0);
    }
}
