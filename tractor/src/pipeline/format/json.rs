use serde_json::{json, Value};
use tractor_core::{report::{Report, ReportKind, ReportMatch}, normalize_path, xml_node_to_json, RenderOptions};
use super::options::{GroupBy, ViewField, ViewSet};

pub fn render_json_report(report: &Report, view: &ViewSet, render_opts: &RenderOptions) -> String {
    let mut root = serde_json::Map::new();

    // Summary: always present for check/test/set reports (structural, not view-gated).
    // For query reports, only include if explicitly requested via -v summary or -v query.
    let show_summary = if matches!(report.kind, ReportKind::Query) {
        view.has(ViewField::Summary) || view.has(ViewField::Query)
    } else {
        true
    };
    if show_summary {
        if let Some(ref summary) = report.summary {
            let mut s = serde_json::Map::new();
            if matches!(report.kind, ReportKind::Set) {
                // For set reports, use "updated"/"unchanged" instead of "errors"/"warnings"
                s.insert("total".into(),     json!(summary.total));
                s.insert("files".into(),     json!(summary.files_affected));
                s.insert("updated".into(),   json!(summary.errors));
                s.insert("unchanged".into(), json!(summary.warnings));
            } else {
                s.insert("passed".into(),   json!(summary.passed));
                s.insert("total".into(),    json!(summary.total));
                s.insert("files".into(),    json!(summary.files_affected));
                s.insert("errors".into(),   json!(summary.errors));
                s.insert("warnings".into(), json!(summary.warnings));
                if let Some(ref expected) = summary.expected {
                    s.insert("expected".into(), json!(expected));
                }
                if let Some(ref query) = summary.query {
                    s.insert("query".into(), json!(query));
                }
            }
            root.insert("summary".into(), Value::Object(s));
        }
    }

    if !report.matches.is_empty() {
        let matches_json: Vec<Value> = report.matches.iter()
            .map(|rm| match_to_value(rm, view, render_opts, GroupBy::None))
            .collect();
        root.insert("matches".into(), Value::Array(matches_json));
    }

    // Run report: emit sub-reports as "operations" array
    if let Some(ref ops) = report.operations {
        let ops_json: Vec<Value> = ops.iter().map(|sub| {
            let sub_str = render_json_report(sub, view, render_opts);
            let sub_obj: serde_json::Map<String, Value> = serde_json::from_str(&sub_str).unwrap_or_default();
            // Put "kind" first by building a new ordered map
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
                // file is on the group — omit it from individual matches
                .map(|rm| match_to_value(rm, view, render_opts, GroupBy::File))
                // skip empty match objects (e.g. stdout mode with only Output in view)
                .filter(|v| !v.as_object().map(|o| o.is_empty()).unwrap_or(false))
                .collect();
            let mut group_obj = serde_json::Map::new();
            if !g.file.is_empty() {
                group_obj.insert("file".into(), json!(g.file));
            }
            // Group-level output (set stdout mode): placed before matches in output
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

    serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_else(|_| "{}".to_string())
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

    // command, message and rule_id are always emitted when present (not ViewFields, but annotations)
    if !rm.command.is_empty() {
        obj.insert("command".into(), json!(rm.command));
    }
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
    use tractor_core::report::{Report, Summary};
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
        let summary = Summary {
            passed: true, total: 1, files_affected: 1,
            errors: 0, warnings: 0, expected: None, query: None,
        };
        let report = Report::query(vec![rm], summary);
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
