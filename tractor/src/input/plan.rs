//! Input planning: the single seam where CLI invocations and config files
//! turn into a list of executor-ready `OperationPlan`s.
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
//! - `Operation` — op-kind-specific metadata without sources/filters.
//! - `SingleOpRequest` / `plan_single` — single-op CLI path (check/query/...).
//! - `plan_multi` — config-mode path; iterates over `ConfigOperation`s.
//! - `ExecutionPlan` — return envelope (a `Vec<OperationPlan>`, room to grow).
//!
//! The single-op path is a one-op specialization of the multi-op path: both
//! go through the same `resolve_one` helper.

use tractor::report::ReportBuilder;

use crate::cli::context::ExecCtx;
use crate::executor::{
    CheckOperationPlan, OperationPlan, QueryOperation, SetOperation, TestOperation, UpdateOperation,
};
use crate::tractor_config::{CheckOperation, ConfigOperation, OperationInputs};

use super::{FileResolver, ResolverOptions, Source, SourceRequest};

// ---------------------------------------------------------------------------
// Operation — op-kind-specific metadata without sources/filters
// ---------------------------------------------------------------------------

/// Per-kind skeleton that knows everything about an operation except its
/// input list. The planner attaches `sources` + `filters` via each variant's
/// `into_plan` to produce the final `OperationPlan`.
///
/// Every variant is a true pre-resolution shape — it carries only op-specific
/// metadata, so no `*OperationPlan` struct in the system exists in an
/// "unresolved" shape with placeholder empty `sources` / default `filters`.
/// The `Check` variant additionally defers rule-glob compilation until
/// `base_dir` is known — the planner drives that compilation inline during
/// conversion.
pub enum Operation {
    Check(CheckOperation),
    Query(QueryOperation),
    Set(SetOperation),
    Test(TestOperation),
    Update(UpdateOperation),
}

impl Operation {
    /// Attach resolved inputs and produce an `OperationPlan` ready for the executor.
    fn into_plan(
        self,
        sources: Vec<Source>,
        filters: crate::input::filter::Filters,
        base_dir: Option<&std::path::Path>,
    ) -> Result<OperationPlan, Box<dyn std::error::Error>> {
        match self {
            Operation::Check(op) => {
                let compiled_rules = tractor::compile_ruleset(
                    &op.ruleset_include,
                    &op.ruleset_exclude,
                    op.ruleset_default_language.as_deref(),
                    op.tree_mode,
                    op.rules,
                    base_dir,
                )
                .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
                Ok(OperationPlan::Check(CheckOperationPlan {
                    sources,
                    filters,
                    compiled_rules,
                    tree_mode: op.tree_mode,
                    ignore_whitespace: op.ignore_whitespace,
                    parse_depth: op.parse_depth,
                }))
            }
            Operation::Query(op) => {
                Ok(OperationPlan::Query(op.into_plan(sources, filters)))
            }
            Operation::Set(op) => {
                Ok(OperationPlan::Set(op.into_plan(sources, filters)))
            }
            Operation::Test(op) => {
                Ok(OperationPlan::Test(op.into_plan(sources, filters)))
            }
            Operation::Update(op) => {
                Ok(OperationPlan::Update(op.into_plan(sources, filters)))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ExecutionPlan envelope
// ---------------------------------------------------------------------------

/// The plan a CLI invocation produces. Today it's just a `Vec<OperationPlan>`;
/// future slices may carry warnings, a shared language map, or other
/// plan-time facts without changing the call sites.
pub struct ExecutionPlan {
    pub operations: Vec<OperationPlan>,
}

// ---------------------------------------------------------------------------
// Single-op request
// ---------------------------------------------------------------------------

/// A single-op plan request: one `Operation` + its per-op `OperationInputs`.
pub struct SingleOpRequest<'a> {
    pub op: Operation,
    pub inputs: OperationInputs,
    /// Command name for fatal diagnostics (e.g. "check", "query"). Typically
    /// matches the kind of `op`, but passed explicitly to preserve the
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
/// operation should be skipped). Returns `Ok(Some(plan))` otherwise.
fn resolve_one(
    resolver: &FileResolver,
    op: Operation,
    inputs: &OperationInputs,
    command: &str,
    base_dir: Option<&std::path::Path>,
    report: &mut ReportBuilder,
) -> Result<Option<OperationPlan>, Box<dyn std::error::Error>> {
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

    let plan = op.into_plan(sources, filters, base_dir)?;
    Ok(Some(plan))
}

// ---------------------------------------------------------------------------
// Single-op entry point
// ---------------------------------------------------------------------------

/// Plan a single CLI-originated operation. Used by `check`, `query`, `set`,
/// `test`, `update` for their non-config branches.
///
/// Constructs the `FileResolver` for this one-op run (no `cli_files`, no
/// `config_root_files`), runs it, and returns the ready-to-execute
/// `OperationPlan` — or `None` when the resolver emitted a fatal.
pub fn plan_single(
    req: SingleOpRequest<'_>,
    shared_diff_files: Option<String>,
    shared_diff_lines: Option<String>,
    max_files: usize,
    env: &ExecCtx<'_>,
    report: &mut ReportBuilder,
) -> Result<Option<OperationPlan>, Box<dyn std::error::Error>> {
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
        req.op,
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
) -> Result<ExecutionPlan, Box<dyn std::error::Error>> {
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
            return Ok(ExecutionPlan { operations: Vec::new() });
        }
    };

    let mut operations = Vec::with_capacity(req.operations.len());
    for config_op in req.operations {
        let (inputs, op) = config_op.into_parts();
        if let Some(plan) = resolve_one(
            &resolver,
            op,
            &inputs,
            &req.command_label,
            env.base_dir,
            report,
        )? {
            operations.push(plan);
        }
    }

    Ok(ExecutionPlan { operations })
}
