use clap::Args;
use tractor::declarative_set::parse_set_expr;
use crate::cli::SharedArgs;

/// Set mode: modify matched node values in-place
///
/// Examples:
///   tractor set config.yaml -x "//database/host" --value "localhost"
///   tractor set config.yaml "database[host='localhost'][port=5432]"
///   tractor set config.yaml "database/host" --value "localhost"
///   tractor set config.yaml "servers[host='localhost']/port" --value "5433"
#[derive(Args, Debug)]
pub struct SetArgs {
    /// Files to process and optional path expression.
    /// When -x is not given, the last argument that isn't an existing file
    /// is treated as the path expression.
    #[arg()]
    pub args: Vec<String>,

    /// Value to set matched nodes to (optional when path expression contains values)
    #[arg(long = "value", help_heading = "Set")]
    pub value: Option<String>,

    /// Write output to stdout instead of modifying files in-place
    #[arg(long = "stdout", help_heading = "Set")]
    pub stdout: bool,

    /// Path to a tractor config file (YAML/TOML) — runs only set operations from it
    #[arg(long = "config", help_heading = "Config")]
    pub config: Option<String>,

    #[command(flatten)]
    pub shared: SharedArgs,

    /// Report fields to include (e.g. tree, value, source) [default: file,line,status,reason]
    #[arg(short = 'v', long = "view", help_heading = "View", allow_hyphen_values = true)]
    pub view: Option<String>,

    /// Output format [default: text]
    #[arg(short = 'f', long = "format", default_value = "text", help_heading = "Format")]
    pub format: String,
}
use crate::executor::{
    self, SetMapping, SetOperation, SetReportMode, SetWriteMode,
};
use crate::cli::context::RunContext;
use crate::input::{plan_single, InputMode, Operation, SingleOpRequest};
use crate::tractor_config::OperationInputs;
use crate::format::{ViewField, GroupDimension, render_report};
use crate::matcher::prepare_report_for_output;
use super::config::{ConfigRunParams, run_from_config};

/// Separate positional args into files and an optional path expression.
///
/// When `-x` is given, all positional args are files.
/// Otherwise, the last arg that looks like a path expression (contains `[` or
/// doesn't resolve to any existing file/glob) is treated as the expression.
fn split_files_and_expr(args: &[String], has_xpath: bool) -> (Vec<String>, Option<String>) {
    if has_xpath || args.is_empty() {
        return (args.to_vec(), None);
    }

    if let Some(last) = args.last() {
        let is_expr = last.contains('[')
            || last.contains('=')
            || (!std::path::Path::new(last).exists() && !last.contains('*') && !last.contains('?'));

        if is_expr {
            return (args[..args.len() - 1].to_vec(), Some(last.clone()));
        }
    }

    (args.to_vec(), None)
}

fn selector_xpath(expr: &str) -> String {
    if expr.starts_with('/') {
        expr.to_string()
    } else {
        format!("//{}", expr)
    }
}

fn normalize_set_mappings(
    xpath: Option<&tractor::NormalizedXpath>,
    expr: Option<&str>,
    explicit_value: Option<&str>,
) -> Result<Vec<SetMapping>, Box<dyn std::error::Error>> {
    if let Some(xpath) = xpath {
        let value = explicit_value
            .ok_or("set with -x requires --value")?;
        return Ok(vec![SetMapping {
            xpath: xpath.to_string(),
            value: value.to_string(),
            value_kind: Some("string".to_string()),
        }]);
    }

    let expr = expr.ok_or("set requires either an XPath query (-x) or a path expression")?;
    if let Some(value) = explicit_value {
        return Ok(vec![SetMapping {
            xpath: selector_xpath(expr),
            value: value.to_string(),
            value_kind: Some("string".to_string()),
        }]);
    }

    Ok(parse_set_expr(expr)?.into_iter().map(|op| SetMapping {
        xpath: op.xpath,
        value: op.value.text().to_string(),
        value_kind: Some(op.value.kind().to_string()),
    }).collect())
}

pub fn run_set(args: SetArgs) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(ref config_path) = args.config {
        return run_from_config(ConfigRunParams {
            config_path,
            shared: &args.shared,
            cli_files: args.args.clone(),
            cli_content: None,
            format: &args.format,
            default_view: &[ViewField::File, ViewField::Line, ViewField::Status, ViewField::Reason],
            view_override: args.view.as_deref(),
            message: None,
            default_group: &[GroupDimension::File],
            op_filter: |kind| matches!(kind, crate::tractor_config::ConfigOperationKind::Set),
            filter_label: "set",
        });
    }

    let has_xpath = args.shared.xpath.is_some();
    let (files, expr) = split_files_and_expr(&args.args, has_xpath);

    let capture = args.stdout
        || (files.is_empty() && args.shared.lang.is_some() && !atty::is(atty::Stream::Stdin));

    let default_view: &[ViewField] = if capture {
        &[ViewField::File, ViewField::Output]
    } else {
        &[ViewField::File, ViewField::Line, ViewField::Status, ViewField::Reason]
    };

    let ctx = RunContext::build(
        &args.shared,
        files,
        args.shared.xpath.clone(),
        &args.format,
        default_view,
        args.view.as_deref(),
        None,
        None,
        false,
        &[GroupDimension::File],
    )?;

    let mappings = normalize_set_mappings(
        ctx.xpath.as_ref(),
        expr.as_deref(),
        args.value.as_deref(),
    )?;

    let (op_files, inline_source, op_language, write_mode): (Vec<String>, Option<crate::input::Source>, Option<String>, SetWriteMode) = match &ctx.input {
        InputMode::Files(files) => {
            if files.is_empty() {
                return Err("set requires at least one file or inline source".into());
            }
            (
                files.clone(),
                None,
                ctx.lang.clone(),
                if capture { SetWriteMode::Capture } else { SetWriteMode::InPlace },
            )
        }
        InputMode::Inline(source) => (
            Vec::new(),
            Some(source.clone()),
            Some(source.language.clone()),
            SetWriteMode::Capture,
        ),
    };

    let inputs = OperationInputs {
        files: op_files,
        exclude: Vec::new(),
        diff_files: Vec::new(),
        diff_lines: Vec::new(),
        language: op_language,
        inline_source,
    };

    let op = Operation::Set(SetOperation {
        mappings,
        tree_mode: ctx.tree_mode,
        limit: ctx.limit,
        ignore_whitespace: ctx.ignore_whitespace,
        write_mode,
        report_mode: SetReportMode::PerMatch,
    });

    let mut builder = tractor::ReportBuilder::new();
    let env = ctx.exec_ctx();
    let plan = plan_single(
        SingleOpRequest { op, inputs, command: "set" },
        args.shared.diff_files.clone(),
        args.shared.diff_lines.clone(),
        args.shared.max_files,
        &env,
        &mut builder,
    )?;

    if let Some(plan) = plan {
        executor::execute(&[plan], &env, &mut builder)?;
    }
    let mut report = builder.build();

    if capture
        && ctx.output_format == crate::format::OutputFormat::Text
        && report.outputs.len() == 1
        && args.view.is_none()
    {
        print!("{}", report.outputs[0].content);
        return Ok(());
    }

    prepare_report_for_output(&mut report, &ctx);
    render_report(&report, &ctx, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_set_mappings_preserves_predicates_with_explicit_value() {
        let mappings = normalize_set_mappings(
            None,
            Some("servers[host='localhost']/port"),
            Some("5433"),
        )
        .unwrap();

        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].xpath, "//servers[host='localhost']/port");
        assert_eq!(mappings[0].value, "5433");
        assert_eq!(mappings[0].value_kind.as_deref(), Some("string"));
    }
}
