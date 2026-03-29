use serde_json::{json, Value};
use tractor_core::{report::{Report, ReportKind, ReportMatch, ResultItem}, normalize_path, xml_node_to_json, RenderOptions};
use super::options::{GroupBy, ViewField, ViewSet};

pub fn render_json_report(report: &Report, view: &ViewSet, render_opts: &RenderOptions) -> String {
    let mut root = serde_json::Map::new();

    // success + totals: top-level fields on the report.
    // Always present for check/test/set reports (structural, not view-gated).
    // For query reports, only include if explicitly requested via -v summary or -v query.
    let show_totals = if matches!(report.kind, ReportKind::Query) {
        view.has(ViewField::Summary) || view.has(ViewField::Query)
    } else {
        true
    };
    if show_totals {
        emit_report_metadata(&mut root, report);
    }

    // Render results from the new structure if populated, otherwise fall back to old fields
    if !report.results.is_empty() {
        let results_json = render_results_json(&report.results, view, render_opts);
        if !results_json.is_empty() {
            root.insert("results".into(), Value::Array(results_json));
        }
        // Emit group key if present
        if let Some(ref group) = report.group {
            root.insert("group".into(), json!(group));
        }
    } else {
        // Fallback to old fields
        if !report.matches.is_empty() {
            let matches_json: Vec<Value> = report.matches.iter()
                .map(|rm| match_to_value(rm, view, render_opts, GroupBy::None))
                .collect();
            root.insert("matches".into(), Value::Array(matches_json));
        }

        if let Some(ref ops) = report.operations {
            let ops_json: Vec<Value> = ops.iter().map(|sub| {
                let sub_str = render_json_report(sub, view, render_opts);
                let sub_obj: serde_json::Map<String, Value> = serde_json::from_str(&sub_str).unwrap_or_default();
                let mut ordered = serde_json::Map::new();
                ordered.insert("kind".into(), json!(sub.kind.as_str()));
                ordered.extend(sub_obj);
                Value::Object(ordered)
            }).collect();
            root.insert("operations".into(), Value::Array(ops_json));
        }

        if let Some(ref groups) = report.groups {
            let groups_json: Vec<Value> = groups.iter().map(|g| {
                let group_matches: Vec<Value> = g.matches.iter()
                    .map(|rm| match_to_value(rm, view, render_opts, GroupBy::File))
                    .filter(|v| !v.as_object().map(|o| o.is_empty()).unwrap_or(false))
                    .collect();
                let mut group_obj = serde_json::Map::new();
                if !g.file.is_empty() {
                    group_obj.insert("file".into(), json!(g.file));
                }
                if view.has(ViewField::Output) {
                    if let Some(ref content) = g.output {
                        group_obj.insert("output".into(), json!(content));
                    }
                }
                if !group_matches.is_empty() {
                    group_obj.insert("matches".into(), Value::Array(group_matches));
                }
                Value::Object(group_obj)
            }).collect();
            root.insert("groups".into(), Value::Array(groups_json));
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
        t.insert("files".into(),   json!(totals.files));
        if totals.errors > 0 { t.insert("errors".into(), json!(totals.errors)); }
        if totals.warnings > 0 { t.insert("warnings".into(), json!(totals.warnings)); }
        if totals.updated > 0 { t.insert("updated".into(), json!(totals.updated)); }
        if totals.unchanged > 0 { t.insert("unchanged".into(), json!(totals.unchanged)); }
        root.insert("totals".into(), Value::Object(t));
    }
    if let Some(ref expected) = report.expected {
        root.insert("expected".into(), json!(expected));
    }
    if let Some(ref query) = report.query {
        root.insert("query".into(), json!(query));
    }
}

/// Render a results list recursively as JSON.
pub fn render_results_json(items: &[ResultItem], view: &ViewSet, render_opts: &RenderOptions) -> Vec<Value> {
    items.iter().map(|item| {
        match item {
            ResultItem::Match(rm) => match_to_value(rm, view, render_opts, GroupBy::None),
            ResultItem::Group(sub) => {
                let mut obj = serde_json::Map::new();
                // Hoisted file
                if let Some(ref file) = sub.file {
                    obj.insert("file".into(), json!(file));
                }
                // Group-level output (set stdout mode)
                if view.has(ViewField::Output) {
                    if let Some(ref content) = sub.output_content {
                        obj.insert("output".into(), json!(content));
                    }
                }
                // Sub-group metadata
                if let Some(ref group) = sub.group {
                    obj.insert("group".into(), json!(group));
                }
                // Sub-report metadata (success, totals, expected)
                emit_report_metadata(&mut obj, sub);
                // Recurse into sub-results
                let sub_results = render_results_json(&sub.results, view, render_opts);
                if !sub_results.is_empty() {
                    obj.insert("results".into(), Value::Array(sub_results));
                }
                Value::Object(obj)
            }
        }
    }).collect()
}

/// Shared match serialization — reused by yaml.rs.
/// `group_by`: when `File`, omits the `file` field (already on the parent group).
/// Fields are emitted in ViewSet declaration order.
pub fn match_to_value(
    rm: &ReportMatch,
    view: &ViewSet,
    render_opts: &RenderOptions,
    group_by: GroupBy,
) -> Value {
    let mut obj = serde_json::Map::new();

    for field in &view.fields {
        match field {
            ViewField::File => {
                if group_by == GroupBy::None {
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
            // rule_id: emitted if present regardless of ViewSet (it's an annotation)
            // Summary/Count/Schema: handled outside match iteration
            _ => {}
        }
    }

    // command: only emitted when view includes Command (not part of default single-command views)
    if view.has(ViewField::Command) && !rm.command.is_empty() {
        obj.insert("command".into(), json!(rm.command));
    }
    // message and rule_id are always emitted when present (annotations, not view-gated)
    if let Some(ref msg) = rm.message {
        obj.insert("message".into(), json!(msg));
    }
    if let Some(ref rule_id) = rm.rule_id {
        obj.insert("rule_id".into(), json!(rule_id));
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
            line: 1, column: 1, end_line: 1, end_column: 1,
            command: String::new(),
            tree: None,
            value: Some(value.to_string()),
            source: None, lines: None, reason: None, severity: None,
            message: None, rule_id: None, status: None, output: None,
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
            message: None, rule_id: None, status: None, output: None,
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
        let val = match_to_value(&rm, &view, &opts, GroupBy::None);
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
        let val = match_to_value(&rm, &view, &opts, GroupBy::None);
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
        let totals = Totals {
            results: 1, files: 1,
            errors: 0, warnings: 0, updated: 0, unchanged: 0,
        };
        let report = Report::query(vec![rm], totals);
        let view = ViewSet::new(vec![ViewField::File, ViewField::Tree]);
        let opts = RenderOptions::new();
        let output = render_json_report(&report, &view, &opts);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let match_tree = &parsed["matches"][0]["tree"];
        assert!(match_tree.is_object(), "Map tree should be a JSON object in report output, got: {}", match_tree);
        assert_eq!(match_tree["name"], "foo");
        assert_eq!(match_tree["count"], 3.0);
    }
}
