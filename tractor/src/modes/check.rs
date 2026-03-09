use std::collections::HashSet;
use tractor_core::{
    OutputFormat, format_matches, OutputOptions,
    report::{Severity, Summary},
};
use crate::cli::CheckArgs;
use crate::pipeline::{RunContext, InputMode, query_files_batched};

pub fn run_check(args: CheckArgs) -> Result<(), Box<dyn std::error::Error>> {
    let severity = match args.severity.as_str() {
        "error" => Severity::Error,
        "warning" => Severity::Warning,
        s => return Err(format!("invalid severity '{}': use 'error' or 'warning'", s).into()),
    };
    let reason = args.reason.clone().unwrap_or_else(|| "check failed".to_string());

    let ctx = RunContext::build(
        &args.shared, args.files, args.shared.xpath.clone(),
        &args.output, args.message, None, false, false,
    )?;

    let xpath_expr = ctx.xpath.as_ref()
        .ok_or("check requires an XPath query (-x)")?;

    let files = match &ctx.input {
        InputMode::Files(files) => files,
        InputMode::InlineSource { .. } => {
            return Err("check cannot be used with stdin input".into());
        }
    };

    if files.is_empty() {
        return Ok(());
    }

    let (_, matches) = query_files_batched(&ctx, files, xpath_expr, true)?;

    let severity_str = match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };

    // If using gcc (default) or github format, emit per-match lines
    if matches!(ctx.format, OutputFormat::Gcc | OutputFormat::Github) {
        let check_options = OutputOptions {
            message: Some(reason.clone()),
            use_color: false,
            strip_locations: ctx.options.strip_locations,
            max_depth: ctx.options.max_depth,
            pretty_print: ctx.options.pretty_print,
            language: ctx.options.language.clone(),
            warning: matches!(severity, Severity::Warning),
        };
        let output = format_matches(&matches, ctx.format.clone(), &check_options);
        print!("{}", output);
    } else {
        // For other formats (json, etc.), just output matches normally
        let output = format_matches(&matches, ctx.format.clone(), &ctx.options);
        print!("{}", output);
    }

    // Summary
    let mut files_affected = HashSet::new();
    for m in &matches {
        files_affected.insert(&m.file);
    }
    let summary = Summary {
        passed: matches.is_empty(),
        total: matches.len(),
        files_affected: files_affected.len(),
        errors: if matches!(severity, Severity::Error) { matches.len() } else { 0 },
        warnings: if matches!(severity, Severity::Warning) { matches.len() } else { 0 },
        expected: None,
    };

    if summary.total > 0 {
        eprintln!();
        let kind = if summary.errors > 0 {
            format!("{} error{}", summary.errors, if summary.errors == 1 { "" } else { "s" })
        } else {
            format!("{} warning{}", summary.warnings, if summary.warnings == 1 { "" } else { "s" })
        };
        eprintln!("{} in {} file{}", kind, summary.files_affected,
            if summary.files_affected == 1 { "" } else { "s" });
    }

    // Exit code: 1 if any errors, 0 for warnings-only or no matches
    if summary.errors > 0 {
        return Err(format!("{} {} found", summary.errors, severity_str).into());
    }

    Ok(())
}
