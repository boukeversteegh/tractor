//! Input planning: the single seam where CLI invocations and config files
//! turn into a list of executor-ready `Operation`s.
//!
//! Every CLI command (single-op) and `run_from_config` (multi-op) funnels
//! through here. This module is the *only* place that:
//!
//! - constructs `ResolverOptions` (inside `build_resolver`)
//! - calls `FileResolver::new` (inside `build_resolver`)
//! - calls `resolver.resolve(...)` (inside `resolve_one`)
//! - constructs `SourceRequest { ... }` (inside `resolve_one`)
//!
//! If you want to know how an operation gets its `sources` + `filters`, read
//! this file and `executor::execute`. Nothing else touches input normalization.
//!
//! ## Shape
//!
//! - `OperationDraft` — op-kind-specific metadata without sources/filters.
//! - `SingleOpRequest` / `plan_single` — single-op CLI path (check/query/...).
//! - `plan_multi` — config-mode path; iterates over `ConfigOperation`s.
//! - `InputPlan` — return envelope (a `Vec<Operation>`, room to grow).
//!
//! The single-op path is a one-op specialization of the multi-op path: both
//! go through the same `resolve_one` helper.

use tractor::report::ReportBuilder;

use crate::cli::context::ExecCtx;
use crate::executor::{
    CheckOperation, Operation, QueryDraft, SetDraft, TestDraft, UpdateDraft,
};
use crate::tractor_config::{CheckDraft, ConfigOperation, OperationInputs};

use super::{FileResolver, ResolverOptions, Source, SourceRequest};

// ---------------------------------------------------------------------------
// OperationDraft — op-kind-specific metadata without sources/filters
// ---------------------------------------------------------------------------

/// Per-kind skeleton that knows everything about an operation except its
/// input list. The planner attaches `sources` + `filters` via each draft's
/// `into_operation` to produce the final `Operation`.
///
/// Every variant is a true draft — it carries only op-specific metadata, so
/// no `*Operation` struct in the system exists in an "unresolved" shape with
/// placeholder empty `sources` / default `filters`. The `Check` variant
/// additionally defers rule-glob compilation until `base_dir` is known — the
/// planner drives that compilation inline during conversion.
pub enum OperationDraft {
    Check(CheckDraft),
    Query(QueryDraft),
    Set(SetDraft),
    Test(TestDraft),
    Update(UpdateDraft),
}

impl OperationDraft {
    /// Attach resolved inputs and produce an `Operation` ready for the executor.
    fn into_operation(
        self,
        sources: Vec<Source>,
        filters: crate::input::filter::Filters,
        base_dir: Option<&std::path::Path>,
    ) -> Result<Operation, Box<dyn std::error::Error>> {
        match self {
            OperationDraft::Check(draft) => {
                let compiled_rules = tractor::compile_ruleset(
                    &draft.ruleset_include,
                    &draft.ruleset_exclude,
                    draft.ruleset_default_language.as_deref(),
                    draft.tree_mode,
                    draft.rules,
                    base_dir,
                )
                .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
                Ok(Operation::Check(CheckOperation {
                    sources,
                    filters,
                    compiled_rules,
                    tree_mode: draft.tree_mode,
                    ignore_whitespace: draft.ignore_whitespace,
                    parse_depth: draft.parse_depth,
                }))
            }
            OperationDraft::Query(draft) => {
                Ok(Operation::Query(draft.into_operation(sources, filters)))
            }
            OperationDraft::Set(draft) => {
                Ok(Operation::Set(draft.into_operation(sources, filters)))
            }
            OperationDraft::Test(draft) => {
                Ok(Operation::Test(draft.into_operation(sources, filters)))
            }
            OperationDraft::Update(draft) => {
                Ok(Operation::Update(draft.into_operation(sources, filters)))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Plan envelope
// ---------------------------------------------------------------------------

/// The plan a CLI invocation produces. Today it's just a `Vec<Operation>`;
/// future slices may carry warnings, a shared language map, or other
/// plan-time facts without changing the call sites.
pub struct InputPlan {
    pub operations: Vec<Operation>,
}

// ---------------------------------------------------------------------------
// Single-op request
// ---------------------------------------------------------------------------

/// A single-op plan request: one `OperationDraft` + its per-op `OperationInputs`.
pub struct SingleOpRequest<'a> {
    pub draft: OperationDraft,
    pub inputs: OperationInputs,
    /// Command name for fatal diagnostics (e.g. "check", "query"). Typically
    /// matches the kind of `draft`, but passed explicitly to preserve the
    /// exact labels each CLI path used pre-refactor.
    pub command: &'a str,
}

// ---------------------------------------------------------------------------
// Shared core: build the FileResolver + resolve one op through it.
// ---------------------------------------------------------------------------

/// The sole call site of `FileResolver::new` and `ResolverOptions { ... }`.
///
/// Construction can fail (bad root glob, etc.); the error surfaces as a
/// `String` so callers can wrap it in the appropriate diagnostic shape.
fn build_resolver(
    shared_diff_files: Option<String>,
    shared_diff_lines: Option<String>,
    max_files: usize,
    cli_files: Vec<String>,
    config_root_files: Option<Vec<String>>,
    env: &ExecCtx<'_>,
) -> Result<FileResolver, String> {
    let resolver_opts = ResolverOptions {
        diff_files: shared_diff_files,
        diff_lines: shared_diff_lines,
        max_files,
        cli_files,
        config_root_files,
    };
    FileResolver::new(&resolver_opts, env)
}

/// The sole call site of `resolver.resolve` and `SourceRequest { ... }`.
///
/// Returns `Ok(None)` when the resolver emitted a fatal diagnostic (the
/// operation should be skipped). Returns `Ok(Some(operation))` otherwise.
fn resolve_one(
    resolver: &FileResolver,
    draft: OperationDraft,
    inputs: &OperationInputs,
    command: &str,
    base_dir: Option<&std::path::Path>,
    report: &mut ReportBuilder,
) -> Result<Option<Operation>, Box<dyn std::error::Error>> {
    let fatal_count_before = report.fatal_count();

    let request = SourceRequest {
        files: &inputs.files,
        exclude: &inputs.exclude,
        diff_files: &inputs.diff_files,
        diff_lines: &inputs.diff_lines,
        command,
        language: inputs.language.as_deref(),
        inline_source: inputs.inline_source.as_ref(),
    };
    let (sources, filters) = resolver.resolve(&request, report);

    // If the resolver surfaced a new fatal diagnostic for this op, drop it
    // from the plan — the executor would only add confusion on top.
    if report.fatal_count() > fatal_count_before {
        return Ok(None);
    }

    let operation = draft.into_operation(sources, filters, base_dir)?;
    Ok(Some(operation))
}

// ---------------------------------------------------------------------------
// Single-op entry point
// ---------------------------------------------------------------------------

/// Plan a single CLI-originated operation. Used by `check`, `query`, `set`,
/// `test`, `update` for their non-config branches.
///
/// Constructs the `FileResolver` for this one-op run (no `cli_files`, no
/// `config_root_files`), runs it, and returns the ready-to-execute
/// `Operation` — or `None` when the resolver emitted a fatal.
pub fn plan_single(
    req: SingleOpRequest<'_>,
    shared_diff_files: Option<String>,
    shared_diff_lines: Option<String>,
    max_files: usize,
    env: &ExecCtx<'_>,
    report: &mut ReportBuilder,
) -> Result<Option<Operation>, Box<dyn std::error::Error>> {
    let resolver = build_resolver(
        shared_diff_files,
        shared_diff_lines,
        max_files,
        Vec::new(),
        None,
        env,
    )
    .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    resolve_one(
        &resolver,
        req.draft,
        &req.inputs,
        req.command,
        env.base_dir,
        report,
    )
}

// ---------------------------------------------------------------------------
// Multi-op entry point (config mode)
// ---------------------------------------------------------------------------

/// Multi-op plan request. Used by `run_from_config` to normalize all config
/// operations through a single shared `FileResolver`.
pub struct MultiOpRequest {
    pub operations: Vec<ConfigOperation>,
    pub cli_files: Vec<String>,
    pub config_root_files: Option<Vec<String>>,
    pub shared_diff_files: Option<String>,
    pub shared_diff_lines: Option<String>,
    pub max_files: usize,
    /// Label threaded into `SourceRequest.command` for fatal diagnostics.
    /// Typically the filter label the command passed in (e.g. "check" for
    /// `tractor check --config ...`, empty for `tractor run`).
    pub command_label: String,
}

/// Plan all operations from a config file. Builds one `FileResolver` shared
/// across every op, then iterates resolving each through `resolve_one`.
/// Operations that hit a fatal diagnostic are dropped from the plan.
pub fn plan_multi(
    req: MultiOpRequest,
    env: &ExecCtx<'_>,
    report: &mut ReportBuilder,
) -> Result<InputPlan, Box<dyn std::error::Error>> {
    let resolver = match build_resolver(
        req.shared_diff_files,
        req.shared_diff_lines,
        req.max_files,
        req.cli_files,
        req.config_root_files,
        env,
    ) {
        Ok(r) => r,
        Err(e) => {
            report.add(super::make_fatal_diagnostic(&req.command_label, e));
            return Ok(InputPlan { operations: Vec::new() });
        }
    };

    let mut operations = Vec::with_capacity(req.operations.len());
    for config_op in req.operations {
        let (inputs, draft) = config_op.into_draft();
        if let Some(op) = resolve_one(
            &resolver,
            draft,
            &inputs,
            &req.command_label,
            env.base_dir,
            report,
        )? {
            operations.push(op);
        }
    }

    Ok(InputPlan { operations })
}
