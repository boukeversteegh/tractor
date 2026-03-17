use serde_json::{json, Value};
use tractor_core::{report::{Report, ReportKind, ReportMatch}, normalize_path, xml_node_to_json, RenderOptions};
use super::options::{GroupBy, ViewField, ViewSet};

pub fn render_json_report(report: &Report, view: &ViewSet, render_opts: &RenderOptions) -> String {
    let mut root = serde_json::Map::new();

    // Summary: always present for check/test reports (structural, not view-gated).
    // For query reports, only include if explicitly requested via -v summary.
    let show_summary = if matches!(report.kind, ReportKind::Query) {
        view.has(ViewField::Summary)
    } else {
        true
    };
    if show_summary {
        if let Some(ref summary) = report.summary {
            let mut s = serde_json::Map::new();
            s.insert("passed".into(),   json!(summary.passed));
            s.insert("total".into(),    json!(summary.total));
            s.insert("files".into(),    json!(summary.files_affected));
            s.insert("errors".into(),   json!(summary.errors));
            s.insert("warnings".into(), json!(summary.warnings));
            if let Some(ref expected) = summary.expected {
                s.insert("expected".into(), json!(expected));
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

    if let Some(ref groups) = report.groups {
        let groups_json: Vec<Value> = groups.iter().map(|g| {
            let group_matches: Vec<Value> = g.matches.iter()
                // file is on the group — omit it from individual matches
                .map(|rm| match_to_value(rm, view, render_opts, GroupBy::File))
                .collect();
            json!({ "file": g.file, "matches": group_matches })
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
                    if rm.is_json_value {
                        // Parse JSON map/array values into real JSON objects
                        // instead of double-escaping them as strings.
                        if let Ok(parsed) = serde_json::from_str::<Value>(v) {
                            obj.insert("value".into(), parsed);
                        } else {
                            obj.insert("value".into(), json!(v));
                        }
                    } else {
                        obj.insert("value".into(), json!(v));
                    }
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

    // message and rule_id are always emitted when present (not ViewFields, but annotations)
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

    fn make_match(value: &str, is_json: bool) -> ReportMatch {
        ReportMatch {
            file: "test.xml".to_string(),
            line: 1, column: 1, end_line: 1, end_column: 1,
            tree: None,
            value: Some(value.to_string()),
            source: None, lines: None, reason: None, severity: None,
            message: None, rule_id: None,
            is_json_value: is_json,
        }
    }

    #[test]
    fn test_map_value_rendered_as_json_object() {
        let rm = make_match(r#"{"name":"foo","val":"1"}"#, true);
        let view = ViewSet::single(ViewField::Value);
        let opts = RenderOptions::new();
        let val = match_to_value(&rm, &view, &opts, GroupBy::None);
        // The value should be a real JSON object, not a string
        let v = val.get("value").unwrap();
        assert!(v.is_object(), "Map value should be a JSON object, got: {}", v);
        assert_eq!(v["name"], "foo");
        assert_eq!(v["val"], "1");
    }

    #[test]
    fn test_non_json_value_rendered_as_string() {
        let rm = make_match("hello world", false);
        let view = ViewSet::single(ViewField::Value);
        let opts = RenderOptions::new();
        let val = match_to_value(&rm, &view, &opts, GroupBy::None);
        // Regular value should remain a string
        let v = val.get("value").unwrap();
        assert!(v.is_string(), "Regular value should be a string, got: {}", v);
        assert_eq!(v.as_str().unwrap(), "hello world");
    }

    #[test]
    fn test_map_value_in_full_json_report() {
        let rm = make_match(r#"{"name":"foo","count":3}"#, true);
        let summary = Summary {
            passed: true, total: 1, files_affected: 1,
            errors: 0, warnings: 0, expected: None,
        };
        let report = Report::query(vec![rm], summary);
        let view = ViewSet::new(vec![ViewField::File, ViewField::Value]);
        let opts = RenderOptions::new();
        let output = render_json_report(&report, &view, &opts);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        // The map value should be embedded as a real JSON object
        let match_value = &parsed["matches"][0]["value"];
        assert!(match_value.is_object(), "Map value should be a JSON object in report output, got: {}", match_value);
        assert_eq!(match_value["name"], "foo");
        assert_eq!(match_value["count"], 3);
    }

    #[test]
    fn test_string_that_looks_like_json_stays_string() {
        // A string value that happens to look like JSON but is_json_value=false
        // should remain a string (no accidental parsing)
        let rm = make_match(r#"{"key":"val"}"#, false);
        let view = ViewSet::single(ViewField::Value);
        let opts = RenderOptions::new();
        let val = match_to_value(&rm, &view, &opts, GroupBy::None);
        let v = val.get("value").unwrap();
        assert!(v.is_string(), "Non-json-flagged value should remain a string even if it looks like JSON");
    }
}
