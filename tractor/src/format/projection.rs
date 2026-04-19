use super::options::{OutputFormat, ParsedViewSet, ViewField, ViewSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Projection {
    Tree,
    Value,
    Source,
    Lines,
    Schema,
    Count,
    Summary,
    Totals,
    Results,
    Report,
}

impl Projection {
    const ALL: [Projection; 10] = [
        Projection::Tree,
        Projection::Value,
        Projection::Source,
        Projection::Lines,
        Projection::Schema,
        Projection::Count,
        Projection::Summary,
        Projection::Totals,
        Projection::Results,
        Projection::Report,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Projection::Tree => "tree",
            Projection::Value => "value",
            Projection::Source => "source",
            Projection::Lines => "lines",
            Projection::Schema => "schema",
            Projection::Count => "count",
            Projection::Summary => "summary",
            Projection::Totals => "totals",
            Projection::Results => "results",
            Projection::Report => "report",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Projection::Tree => "Project matched trees",
            Projection::Value => "Project matched values",
            Projection::Source => "Project matched source snippets",
            Projection::Lines => "Project matched source lines",
            Projection::Schema => "Project the schema summary",
            Projection::Count => "Project the total result count",
            Projection::Summary => "Project the summary section",
            Projection::Totals => "Project summary totals",
            Projection::Results => "Project the results list",
            Projection::Report => "Project the full report",
        }
    }

    pub fn help_text() -> String {
        let max_name = Self::ALL
            .iter()
            .map(|value| value.name().len())
            .max()
            .unwrap_or(0);
        let mut lines =
            vec!["Choose which report element is emitted [default: report]".to_string()];
        for value in Self::ALL {
            lines.push(format!(
                "  {:width$}  {}",
                value.name(),
                value.description(),
                width = max_name
            ));
        }
        lines.join("\n")
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "tree" => Ok(Projection::Tree),
            "value" => Ok(Projection::Value),
            "source" => Ok(Projection::Source),
            "lines" => Ok(Projection::Lines),
            "schema" => Ok(Projection::Schema),
            "count" => Ok(Projection::Count),
            "summary" => Ok(Projection::Summary),
            "totals" => Ok(Projection::Totals),
            "results" => Ok(Projection::Results),
            "report" => Ok(Projection::Report),
            _ => Err(format!(
                "invalid projection '{}'. Valid values: {}",
                s,
                Self::ALL
                    .iter()
                    .map(|value| value.name())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    pub fn view_replacement_field(&self) -> Option<ViewField> {
        match self {
            Projection::Tree => Some(ViewField::Tree),
            Projection::Value => Some(ViewField::Value),
            Projection::Source => Some(ViewField::Source),
            Projection::Lines => Some(ViewField::Lines),
            Projection::Schema => Some(ViewField::Schema),
            Projection::Count => Some(ViewField::Count),
            Projection::Summary | Projection::Totals | Projection::Results | Projection::Report => {
                None
            }
        }
    }

    pub fn is_sequence(&self) -> bool {
        matches!(
            self,
            Projection::Tree
                | Projection::Value
                | Projection::Source
                | Projection::Lines
                | Projection::Results
        )
    }

    pub fn keeps_match_fields(&self) -> bool {
        matches!(self, Projection::Results | Projection::Report)
    }

    pub fn is_metadata_container(&self) -> bool {
        matches!(self, Projection::Summary | Projection::Totals)
    }
}

#[derive(Debug, Clone)]
pub struct OutputPlan {
    pub projection: Projection,
    pub single: bool,
    pub limit: Option<usize>,
    pub view: ViewSet,
    pub message: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestedItem {
    Field(ViewField),
    Message,
}

impl RequestedItem {
    fn label(&self) -> &'static str {
        match self {
            RequestedItem::Field(field) => field.name(),
            RequestedItem::Message => "message",
        }
    }
}

pub fn normalize_output_plan(
    projection_arg: Option<&str>,
    single_requested: bool,
    limit: Option<usize>,
    view: ParsedViewSet,
    message: Option<String>,
    output_format: OutputFormat,
    has_group_by: bool,
) -> Result<OutputPlan, String> {
    if single_requested && limit.is_some_and(|value| value != 1) {
        return Err("--single contradicts -n/--limit unless the limit is exactly 1".to_string());
    }

    let projection = match projection_arg {
        Some(value) => Projection::from_str(value)?,
        None if single_requested => Projection::Results,
        None => Projection::Report,
    };

    let non_default = projection != Projection::Report || single_requested;
    if non_default && !output_format.supports_projection() {
        return Err(format!(
            "-p/--single is not supported with -f {} (line-oriented format)",
            output_format.name(),
        ));
    }
    if non_default && has_group_by && !projection.keeps_match_fields() {
        return Err(format!(
            "-p {} is not compatible with --group",
            projection.name(),
        ));
    }

    let mut warnings = Vec::new();
    let explicit_items = explicit_items(&view, message.is_some());

    if let Some(replacement) = projection.view_replacement_field() {
        let dropped: Vec<_> = explicit_items
            .into_iter()
            .filter(|item| *item != RequestedItem::Field(replacement))
            .collect();
        if !dropped.is_empty() {
            warnings.push(format_replacement_warning(projection, &dropped));
        }
    } else if projection.is_metadata_container() && !explicit_items.is_empty() {
        warnings.push(format_unreachable_warning(projection, &explicit_items));
    }

    let single = single_requested && projection.is_sequence();
    if single_requested && !projection.is_sequence() {
        warnings.push(format!(
            "warning: --single has no effect with -p {} (already singular). Remove --single to silence this warning.",
            projection.name()
        ));
    }

    let view = match projection.view_replacement_field() {
        Some(field) => ViewSet::single(field),
        None => view.resolved,
    };

    let message = if projection.keeps_match_fields() {
        message
    } else {
        None
    };

    Ok(OutputPlan {
        projection,
        single,
        limit: if single { Some(1) } else { limit },
        view,
        message,
        warnings,
    })
}

fn explicit_items(view: &ParsedViewSet, has_message: bool) -> Vec<RequestedItem> {
    let mut items: Vec<RequestedItem> = view
        .explicit_fields
        .iter()
        .copied()
        .map(RequestedItem::Field)
        .collect();
    if has_message {
        items.push(RequestedItem::Message);
    }
    items
}

fn format_replacement_warning(projection: Projection, dropped: &[RequestedItem]) -> String {
    if dropped == [RequestedItem::Message] {
        return format!(
            "warning: -m message template was discarded because -p {} replaces the view set.\n  To keep -m intact, use `-p results` (respects -v/-m) instead of `-p {}`.",
            projection.name(),
            projection.name()
        );
    }

    format!(
        "warning: requested view items {} were discarded because -p {} replaces the view set.\n  To keep -v/-m intact, use `-p results` (respects -v/-m) instead of `-p {}`.",
        format_requested_items(dropped),
        projection.name(),
        projection.name()
    )
}

fn format_unreachable_warning(projection: Projection, dropped: &[RequestedItem]) -> String {
    if dropped == [RequestedItem::Message] {
        return format!(
            "warning: -m message template has no effect with -p {} (no per-match rendering).",
            projection.name()
        );
    }

    format!(
        "warning: requested view items {} were discarded because -p {} has no per-match rendering.",
        format_requested_items(dropped),
        projection.name()
    )
}

fn format_requested_items(items: &[RequestedItem]) -> String {
    let names = items
        .iter()
        .map(RequestedItem::label)
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{names}}}")
}

#[cfg(test)]
mod tests {
    use super::{normalize_output_plan, Projection};
    use crate::format::options::{OutputFormat, ParsedViewSet, ViewField, ViewSet};

    fn parsed_view(resolved: &[ViewField], explicit: &[ViewField]) -> ParsedViewSet {
        ParsedViewSet {
            resolved: ViewSet::new(resolved.to_vec()),
            explicit_fields: explicit.to_vec(),
        }
    }

    #[test]
    fn single_without_projection_defaults_to_results_and_limit_one() {
        let plan = normalize_output_plan(
            None,
            true,
            None,
            parsed_view(&[ViewField::File, ViewField::Tree], &[]),
            None,
            OutputFormat::Text,
            false,
        )
        .unwrap();

        assert_eq!(plan.projection, Projection::Results);
        assert!(plan.single);
        assert_eq!(plan.limit, Some(1));
    }

    #[test]
    fn view_level_projection_replaces_view_and_drops_message() {
        let plan = normalize_output_plan(
            Some("tree"),
            false,
            None,
            parsed_view(
                &[ViewField::File, ViewField::Tree],
                &[ViewField::File, ViewField::Tree],
            ),
            Some("{file}".to_string()),
            OutputFormat::Text,
            false,
        )
        .unwrap();

        assert_eq!(plan.view.fields, vec![ViewField::Tree]);
        assert!(plan.message.is_none());
        assert_eq!(plan.warnings.len(), 1);
        assert!(plan.warnings[0].contains("{file, message}"));
    }

    #[test]
    fn metadata_projection_warns_about_unreachable_message_template() {
        let plan = normalize_output_plan(
            Some("summary"),
            false,
            None,
            parsed_view(&[ViewField::Reason], &[]),
            Some("hello".to_string()),
            OutputFormat::Text,
            false,
        )
        .unwrap();

        assert_eq!(plan.projection, Projection::Summary);
        assert_eq!(
            plan.warnings,
            vec!["warning: -m message template has no effect with -p summary (no per-match rendering).".to_string()]
        );
    }

    #[test]
    fn single_with_non_singular_limit_is_rejected() {
        let err = normalize_output_plan(
            Some("tree"),
            true,
            Some(2),
            parsed_view(&[ViewField::Tree], &[]),
            None,
            OutputFormat::Text,
            false,
        )
        .unwrap_err();

        assert!(err.contains("--single contradicts -n/--limit"));
    }

    #[test]
    fn single_warns_when_projection_is_already_singular() {
        let plan = normalize_output_plan(
            Some("summary"),
            true,
            Some(1),
            parsed_view(&[ViewField::Totals], &[]),
            None,
            OutputFormat::Text,
            false,
        )
        .unwrap();

        assert!(!plan.single);
        assert_eq!(plan.limit, Some(1));
        assert_eq!(
            plan.warnings,
            vec!["warning: --single has no effect with -p summary (already singular). Remove --single to silence this warning.".to_string()]
        );
    }

    #[test]
    fn rejects_projection_for_line_oriented_formats() {
        let err = normalize_output_plan(
            Some("tree"),
            false,
            None,
            parsed_view(&[ViewField::Tree], &[]),
            None,
            OutputFormat::Gcc,
            false,
        )
        .unwrap_err();

        assert_eq!(
            err,
            "-p/--single is not supported with -f gcc (line-oriented format)"
        );
    }

    #[test]
    fn rejects_grouping_for_non_match_preserving_projection() {
        let err = normalize_output_plan(
            Some("summary"),
            false,
            None,
            parsed_view(&[ViewField::Totals], &[]),
            None,
            OutputFormat::Json,
            true,
        )
        .unwrap_err();

        assert_eq!(err, "-p summary is not compatible with --group");
    }

    #[test]
    fn allows_grouping_for_results_projection() {
        let plan = normalize_output_plan(
            Some("results"),
            false,
            None,
            parsed_view(&[ViewField::File, ViewField::Tree], &[ViewField::Tree]),
            None,
            OutputFormat::Json,
            true,
        )
        .unwrap();

        assert_eq!(plan.projection, Projection::Results);
    }
}
