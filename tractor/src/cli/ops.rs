//! Running a command's operations and reporting the outcome.
//!
//! Every command follows the same sequence: execute the planned operations,
//! build the report, project it for output, render it. This module owns the
//! part that must not differ between commands — what happens when an
//! operation fails partway through.
//!
//! The executor deliberately knows nothing about presentation: it takes an
//! [`ExecCtx`] (environment only) and pushes into a `ReportBuilder`. Deciding
//! *how* a run is reported is the CLI's job, so the failure path lives here
//! rather than wrapping the executor.

use tractor::report::{ReportBuilder, ReportMatch, Severity, DiagnosticOrigin};

use crate::cli::context::{ExecCtx, RunContext};
use crate::executor::{self, OperationPlan};
use crate::format::render_report;
use crate::matcher::prepare_report_for_output;

/// Execute operations, reporting a mid-run failure as part of the run's own
/// report instead of discarding it.
///
/// On success the caller renders as usual — nothing here has run yet.
///
/// On failure the diagnostics accumulated so far are exactly what explains
/// the failure (an advisory warning that a variable lookup was empty, say),
/// and dropping them leaves the user with a bare error. So the failure is
/// folded in as a fatal diagnostic and the whole thing is rendered once,
/// through the same formatter the successful path uses. The returned
/// `SilentExit` tells `main` the run has already reported itself, which is
/// what keeps a machine-readable format to a **single** document: previously
/// a partial report and `main`'s separate error report were both printed,
/// producing two concatenated JSON objects and a `"success": true` summary
/// next to a non-zero exit.
pub fn execute_and_report_failures(
    operations: &[OperationPlan],
    env: &ExecCtx<'_>,
    builder: &mut ReportBuilder,
    ctx: &RunContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let Err(error) = executor::execute(operations, env, builder) else {
        return Ok(());
    };

    builder.add(fatal_diagnostic(error.to_string()));
    let mut report = std::mem::replace(builder, ReportBuilder::new()).build();
    prepare_report_for_output(&mut report, ctx);

    // Rendering a failed report already yields SilentExit; the explicit
    // error covers formats that don't (and a render error must not mask the
    // failure being reported).
    render_report(&report, ctx, None)?;
    Err(Box::new(crate::SilentExit))
}

/// The run-ending error, as a report entry.
fn fatal_diagnostic(reason: String) -> ReportMatch {
    ReportMatch {
        file: String::new(),
        line: 0,
        column: 0,
        end_line: 0,
        end_column: 0,
        command: String::new(),
        tree: None,
        value: None,
        source: None,
        lines: None,
        reason: Some(reason),
        severity: Some(Severity::Fatal),
        message: None,
        origin: Some(DiagnosticOrigin::Cli),
        rule_id: None,
        status: None,
        output: None,
    }
}
