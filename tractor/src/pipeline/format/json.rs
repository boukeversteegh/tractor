use serde_json::{json, Value};
use tractor_core::{report::Report, report::ReportKind, normalize_path, xml_fragment_to_json, RenderOptions};
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

    let match_flags = MatchFlags::from_view(view);

    if !report.matches.is_empty() {
        let matches_json: Vec<Value> = report.matches.iter()
            .map(|rm| match_to_value(rm, &match_flags, render_opts, GroupBy::None))
            .collect();
        root.insert("matches".into(), Value::Array(matches_json));
    }

    if let Some(ref groups) = report.groups {
        let groups_json: Vec<Value> = groups.iter().map(|g| {
            let group_matches: Vec<Value> = g.matches.iter()
                // file is on the group — omit it from individual matches
                .map(|rm| match_to_value(rm, &match_flags, render_opts, GroupBy::File))
                .collect();
            json!({ "file": g.file, "matches": group_matches })
        }).collect();
        root.insert("groups".into(), Value::Array(groups_json));
    }

    serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_else(|_| "{}".to_string())
}

/// Shared match serialization — reused by yaml.rs.
/// `group_by`: when `File`, omits the `file` field (already on the parent group).
pub fn match_to_value(
    rm: &tractor_core::report::ReportMatch,
    flags: &MatchFlags,
    render_opts: &RenderOptions,
    group_by: GroupBy,
) -> Value {
    let m = &rm.inner;
    let mut obj = serde_json::Map::new();
    if group_by == GroupBy::None {
        obj.insert("file".into(), json!(normalize_path(&m.file)));
    }
    obj.insert("line".into(),   json!(m.line));
    obj.insert("column".into(), json!(m.column));

    if flags.value {
        obj.insert("value".into(), json!(m.value));
    }
    if flags.source {
        obj.insert("source".into(), json!(m.extract_source_snippet()));
    }
    if flags.lines {
        let lines: Vec<&str> = m.get_source_lines_range()
            .into_iter()
            .map(|l| l.trim_end_matches('\r'))
            .collect();
        obj.insert("lines".into(), json!(lines));
    }
    if let Some(ref message) = rm.message {
        obj.insert("message".into(), json!(message));
    }
    if flags.reason {
        if let Some(ref reason) = rm.reason {
            obj.insert("reason".into(), json!(reason));
        }
    }
    if flags.severity {
        if let Some(severity) = rm.severity {
            obj.insert("severity".into(), json!(severity.as_str()));
        }
    }
    if let Some(ref rule_id) = rm.rule_id {
        obj.insert("rule_id".into(), json!(rule_id));
    }
    // Tree is always last — it's the bulkiest field
    if flags.tree {
        if let Some(ref frag) = m.xml_fragment {
            obj.insert("tree".into(), xml_fragment_to_json(frag, render_opts.max_depth));
        }
    }
    Value::Object(obj)
}

/// Pre-computed view flags for match serialization.
pub struct MatchFlags {
    pub tree:     bool,
    pub value:    bool,
    pub source:   bool,
    pub lines:    bool,
    pub reason:   bool,
    pub severity: bool,
}

impl MatchFlags {
    pub fn from_view(view: &ViewSet) -> Self {
        MatchFlags {
            tree:     view.has(ViewField::Tree),
            value:    view.has(ViewField::Value),
            source:   view.has(ViewField::Source),
            lines:    view.has(ViewField::Lines),
            reason:   view.has(ViewField::Reason),
            severity: view.has(ViewField::Severity),
        }
    }
}
