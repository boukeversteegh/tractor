//! Normalized projection plan for the `-p` / `--project` / `--single` flags.
//!
//! The plan is built once at `RunContext::build` and is the single source of
//! truth downstream. Raw CLI inputs (`view`, `project`, `single`, `limit`,
//! `message`) are consumed here; the rest of the pipeline reads the plan.
//!
//! Key invariants:
//! - The effective `ViewSet` already has the `-p` replacement rule applied.
//!   Downstream code must not re-derive view policy from `target`.
//! - `limit` is already reconciled with `--single` (Some(1) when set).
//! - Warnings are collected but not yet emitted — the entry point flushes
//!   them once to stderr before rendering.
//! - Validation errors (e.g. `--single -n 2`) surface as `Err` from
//!   `ProjectionPlan::from_inputs` — they are CLI errors, not warnings.

use crate::format::{
    parse_projection, parse_view_set, Projection, ProjectionKind,
    ViewField, ViewSet, OutputFormat,
};

/// Normalized projection state shared by CLI, executor, and renderer stages.
#[derive(Debug, Clone)]
pub struct ProjectionPlan {
    /// Which element to emit. `Projection::Report` when `-p` is omitted.
    pub target: Projection,
    /// Whether `--single` is in effect. Collapses sequence projections to a
    /// bare element; for already-singular targets this is redundant and
    /// triggers a warning during plan construction.
    pub single: bool,
    /// The `ViewSet` to use for rendering, with `-p` replacement applied.
    pub effective_view: ViewSet,
    /// Match limit (`-n`), reconciled with `--single`.
    pub limit: Option<usize>,
    /// Warnings to emit on stderr before rendering. Owned by the plan so
    /// the entry point has one flush site and downstream code never
    /// rediscovers them.
    pub warnings: Vec<String>,
}

/// Raw inputs to the plan — the flag values as they arrived from clap.
pub struct ProjectionInputs<'a> {
    /// Raw `-p` string; `None` means the flag was omitted.
    pub project_raw: Option<&'a str>,
    /// `--single` value.
    pub single: bool,
    /// Raw `-v` string; `None` means the user did not set `-v`.
    pub view_raw: Option<&'a str>,
    /// The default view set for this subcommand (used when `-v` is omitted
    /// or with the `+/-` modifier syntax).
    pub default_view: &'a [ViewField],
    /// Whether `-m` was explicitly passed. A template value is held
    /// separately by `RunContext`; the plan only needs to know presence
    /// to compute warnings.
    pub explicit_message: bool,
    /// `-n` limit, or `None` when unset.
    pub limit: Option<usize>,
    /// The resolved output format — used to reject incoherent combinations
    /// like `-p tree -f gcc`.
    pub output_format: OutputFormat,
    /// Whether the user passed `-g/--group`. Projection with grouping is
    /// out of scope per the design; the combination is rejected.
    pub has_group_by: bool,
}

impl ProjectionPlan {
    /// Build a plan from raw CLI inputs. Returns `Err` on contradictory
    /// flag combinations (CLI errors); warnings accumulate on the plan.
    pub fn from_inputs(inputs: &ProjectionInputs<'_>) -> Result<Self, String> {
        // -- Parse the explicit view (only when the user actually set -v).
        let explicit_view_fields: Vec<ViewField> = match inputs.view_raw {
            Some(s) => parse_view_set(s, inputs.default_view)?.fields,
            None => Vec::new(),
        };
        let user_set_view = inputs.view_raw.is_some() && !explicit_view_fields.is_empty();

        // -- Parse the projection target. Default is `Report`.
        let target = match inputs.project_raw {
            Some(s) => parse_projection(s)?,
            None => {
                // `--single` with -p omitted implies -p results — emitting a
                // single whole-report is a no-op, which can never be what
                // the user meant.
                if inputs.single {
                    Projection::Results
                } else {
                    Projection::Report
                }
            }
        };

        // -- Reject incoherent format × projection combinations.
        let is_default_report = target == Projection::Report && !inputs.single;
        let explicit_projection = inputs.project_raw.is_some() || inputs.single;
        if !is_default_report && explicit_projection {
            match inputs.output_format {
                OutputFormat::Gcc | OutputFormat::Github | OutputFormat::ClaudeCode => {
                    return Err(format!(
                        "-p/--single is not supported with -f {} (line-oriented format)",
                        inputs.output_format.name(),
                    ));
                }
                _ => {}
            }
        }

        // -- Reject projection + grouping (out of scope).
        if explicit_projection && inputs.has_group_by {
            return Err(
                "-p/--single with --group is not yet supported".to_string(),
            );
        }

        // -- Reconcile `-n` with `--single`. `--single -n 1` is redundant
        //    but accepted; any other `-n` contradicts --single.
        let limit = match (inputs.single, inputs.limit) {
            (true, Some(n)) if n != 1 => {
                return Err(format!(
                    "--single is incompatible with -n {} (--single implies -n 1)",
                    n,
                ));
            }
            (true, _) => Some(1),
            (false, n) => n,
        };

        // -- Compute the effective ViewSet using the projection's kind.
        //    View-level projections replace the view set with a single field;
        //    structural projections respect the user's view; metadata
        //    projections leave the view intact but irrelevant.
        let effective_view = match target.kind() {
            ProjectionKind::ViewLevel => {
                ViewSet::single(
                    target.replacement_view_field().expect(
                        "ViewLevel projections must supply a replacement ViewField",
                    ),
                )
            }
            ProjectionKind::Structural | ProjectionKind::Metadata => {
                match inputs.view_raw {
                    Some(s) => parse_view_set(s, inputs.default_view)?,
                    None => ViewSet::from_fields(inputs.default_view.to_vec()),
                }
            }
        };

        // -- Collect warnings. Rules:
        //    1. Already-singular `--single` — the flag has nothing to flatten.
        //    2. View-level `-p X` with explicit `-v` fields other than X —
        //       those extras are replaced away.
        //    3. Metadata `-p summary|totals` with explicit `-v`/`-m` —
        //       those fields have no per-match rendering to appear in.
        let mut warnings = Vec::new();

        if inputs.single && target.is_already_singular() {
            warnings.push(format!(
                "warning: --single has no effect with -p {} (already singular). Drop --single.",
                target.name(),
            ));
        }

        match target.kind() {
            ProjectionKind::ViewLevel => {
                let keeper = target.replacement_view_field();
                let dropped_fields: Vec<&str> = explicit_view_fields.iter()
                    .filter(|f| Some(**f) != keeper)
                    .map(|f| f.name())
                    .collect();
                // -m adds `message`; `-p message` doesn't exist today so any
                // view-level projection drops it when explicitly set.
                let drop_message = inputs.explicit_message;
                if !dropped_fields.is_empty() || drop_message {
                    let mut names: Vec<String> = dropped_fields.iter().map(|s| s.to_string()).collect();
                    if drop_message {
                        names.push("message".to_string());
                    }
                    let joined = names.join(", ");
                    warnings.push(format!(
                        "warning: -v fields {{{}}} were discarded because -p {} replaces the view set.\n  To keep -v intact, use `-p results` (respects -v) instead of `-p {}`.",
                        joined,
                        target.name(),
                        target.name(),
                    ));
                }
            }
            ProjectionKind::Metadata => {
                let has_explicit_view = user_set_view;
                if has_explicit_view || inputs.explicit_message {
                    let mut parts: Vec<String> = Vec::new();
                    if has_explicit_view {
                        let names: Vec<&str> = explicit_view_fields.iter().map(|f| f.name()).collect();
                        parts.push(format!("-v fields {{{}}}", names.join(", ")));
                    }
                    if inputs.explicit_message {
                        parts.push("-m message template".to_string());
                    }
                    warnings.push(format!(
                        "warning: {} ha{} no effect with -p {} (no per-match rendering).",
                        parts.join(" and "),
                        if parts.len() == 1 { "s" } else { "ve" },
                        target.name(),
                    ));
                }
            }
            ProjectionKind::Structural => {
                // No warning — structural projections respect -v/-m in full.
            }
        }

        Ok(ProjectionPlan {
            target,
            single: inputs.single,
            effective_view,
            limit,
            warnings,
        })
    }

    /// Emit any collected warnings to stderr. Safe to call multiple times;
    /// warnings are moved out so subsequent calls are no-ops.
    pub fn flush_warnings(&mut self) {
        for w in self.warnings.drain(..) {
            eprintln!("{}", w);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(project: Option<&str>, single: bool, view: Option<&str>, msg: bool, limit: Option<usize>) -> ProjectionInputs<'static> {
        ProjectionInputs {
            project_raw: project.map(|s| Box::leak(s.to_string().into_boxed_str()) as &str),
            single,
            view_raw: view.map(|s| Box::leak(s.to_string().into_boxed_str()) as &str),
            default_view: &[ViewField::Tree],
            explicit_message: msg,
            limit,
            output_format: OutputFormat::Text,
            has_group_by: false,
        }
    }

    #[test]
    fn default_is_report() {
        let plan = ProjectionPlan::from_inputs(&inputs(None, false, None, false, None)).unwrap();
        assert_eq!(plan.target, Projection::Report);
        assert!(!plan.single);
        assert!(plan.warnings.is_empty());
    }

    #[test]
    fn single_with_no_project_implies_results() {
        let plan = ProjectionPlan::from_inputs(&inputs(None, true, None, false, None)).unwrap();
        assert_eq!(plan.target, Projection::Results);
        assert_eq!(plan.limit, Some(1));
    }

    #[test]
    fn view_level_replaces_view_set() {
        let plan = ProjectionPlan::from_inputs(&inputs(Some("tree"), false, Some("tree,file"), false, None)).unwrap();
        assert_eq!(plan.effective_view.fields, vec![ViewField::Tree]);
        assert_eq!(plan.warnings.len(), 1);
        assert!(plan.warnings[0].contains("file"));
        assert!(plan.warnings[0].contains("replaces the view set"));
    }

    #[test]
    fn view_level_same_view_no_warning() {
        let plan = ProjectionPlan::from_inputs(&inputs(Some("tree"), false, Some("tree"), false, None)).unwrap();
        assert!(plan.warnings.is_empty());
    }

    #[test]
    fn view_level_default_view_no_warning() {
        // User did not explicitly set -v — no surprise to warn about.
        let plan = ProjectionPlan::from_inputs(&inputs(Some("tree"), false, None, false, None)).unwrap();
        assert!(plan.warnings.is_empty());
    }

    #[test]
    fn metadata_with_explicit_view_warns() {
        let plan = ProjectionPlan::from_inputs(&inputs(Some("summary"), false, Some("tree,file"), false, None)).unwrap();
        assert_eq!(plan.target, Projection::Summary);
        assert_eq!(plan.warnings.len(), 1);
        assert!(plan.warnings[0].contains("no per-match rendering"));
    }

    #[test]
    fn metadata_with_explicit_message_warns() {
        let plan = ProjectionPlan::from_inputs(&inputs(Some("summary"), false, None, true, None)).unwrap();
        assert_eq!(plan.warnings.len(), 1);
        assert!(plan.warnings[0].contains("message template"));
    }

    #[test]
    fn structural_respects_view_no_warning() {
        let plan = ProjectionPlan::from_inputs(&inputs(Some("results"), false, Some("tree,file"), true, None)).unwrap();
        assert_eq!(plan.target, Projection::Results);
        assert!(plan.warnings.is_empty());
        assert!(plan.effective_view.has(ViewField::Tree));
        assert!(plan.effective_view.has(ViewField::File));
    }

    #[test]
    fn already_singular_with_single_warns() {
        let plan = ProjectionPlan::from_inputs(&inputs(Some("summary"), true, None, false, None)).unwrap();
        assert_eq!(plan.warnings.len(), 1);
        assert!(plan.warnings[0].contains("already singular"));
    }

    #[test]
    fn single_with_n_2_errors() {
        let err = ProjectionPlan::from_inputs(&inputs(Some("tree"), true, None, false, Some(2))).unwrap_err();
        assert!(err.contains("incompatible"));
    }

    #[test]
    fn single_with_n_1_redundant_but_ok() {
        let plan = ProjectionPlan::from_inputs(&inputs(Some("tree"), true, None, false, Some(1))).unwrap();
        assert_eq!(plan.limit, Some(1));
    }

    #[test]
    fn projection_with_gcc_errors() {
        let mut i = inputs(Some("tree"), false, None, false, None);
        i.output_format = OutputFormat::Gcc;
        let err = ProjectionPlan::from_inputs(&i).unwrap_err();
        assert!(err.contains("-f gcc"));
    }

    #[test]
    fn projection_with_grouping_errors() {
        let mut i = inputs(Some("tree"), false, None, false, None);
        i.has_group_by = true;
        let err = ProjectionPlan::from_inputs(&i).unwrap_err();
        assert!(err.contains("--group"));
    }

    #[test]
    fn view_level_drops_message_field_in_warning() {
        let plan = ProjectionPlan::from_inputs(&inputs(Some("tree"), false, Some("tree,file"), true, None)).unwrap();
        assert_eq!(plan.warnings.len(), 1);
        assert!(plan.warnings[0].contains("file"));
        assert!(plan.warnings[0].contains("message"));
    }

    #[test]
    fn count_is_view_level() {
        let plan = ProjectionPlan::from_inputs(&inputs(Some("count"), false, None, false, None)).unwrap();
        assert_eq!(plan.effective_view.fields, vec![ViewField::Count]);
    }

    #[test]
    fn schema_is_view_level() {
        let plan = ProjectionPlan::from_inputs(&inputs(Some("schema"), false, None, false, None)).unwrap();
        assert_eq!(plan.effective_view.fields, vec![ViewField::Schema]);
    }

    #[test]
    fn default_report_with_gcc_is_ok() {
        let mut i = inputs(None, false, None, false, None);
        i.output_format = OutputFormat::Gcc;
        assert!(ProjectionPlan::from_inputs(&i).is_ok());
    }
}
