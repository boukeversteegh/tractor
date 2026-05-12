use std::io::Read;
use clap::Args;
use tractor::language_info::{get_language_info, get_language_for_extension};
use tractor::parser::parse_string_to_xot;

/// Render mode: round-trip parse source → IR → source.
///
/// The `render` command parses source through the typed-IR pipeline
/// and re-emits it via the IR-aware source renderer
/// (`tree::source::render`). In anchored mode (the default) this is a
/// byte-for-byte identity — useful for verifying lossless parse and as
/// the substrate that `tractor set` / `tractor update` build on.
#[derive(Args, Debug)]
pub struct RenderArgs {
    /// Target file (determines language from extension). When given,
    /// reads the source from that file. If omitted, source is read
    /// from `--string` or stdin.
    #[arg()]
    pub file: Option<String>,

    /// Language (required when no file is given, e.g., csharp, rust)
    #[arg(short = 'l', long = "lang")]
    pub lang: Option<String>,

    /// Source string (alternative to stdin / file)
    #[arg(short = 's', long = "string")]
    pub input: Option<String>,
}

pub fn run_render(args: RenderArgs) -> Result<(), Box<dyn std::error::Error>> {
    let lang = resolve_language(&args)?;
    let input = read_input(&args)?;
    let file_label = args.file.clone().unwrap_or_else(|| "<stdin>".to_string());

    let parsed = parse_string_to_xot(&input, &lang, file_label, None)
        .map_err(|e| format!("parse failed: {e}"))?;

    let rendered = if let Some(ir) = &parsed.ir {
        // Programming-language IR: anchored render slices the source
        // verbatim, so the round-trip is byte-identical.
        tractor::tree::source::render(ir, &lang, Some(&parsed.source))
    } else if let Some(data_ir) = &parsed.data_ir {
        // Data-language IR: anchored slice via DataTree::to_source.
        data_ir.to_source(&parsed.source).to_string()
    } else if let Some(sql_ir) = &parsed.sql_ir {
        tractor::tree::source::render_sql(sql_ir, Some(&parsed.source))
    } else {
        return Err(format!(
            "language '{}' is not on the IR pipeline; render is only available \
             for IR-supported languages",
            lang,
        ).into());
    };

    if let Some(file) = &args.file {
        std::fs::write(file, &rendered)?;
        eprintln!("Rendered to {}", file);
    } else {
        print!("{}", rendered);
    }

    Ok(())
}

fn resolve_language(args: &RenderArgs) -> Result<String, Box<dyn std::error::Error>> {
    // Explicit --lang takes priority (supports aliases like "cs" → "csharp")
    if let Some(lang) = &args.lang {
        let info = get_language_info(lang)
            .ok_or_else(|| format!("unknown language: {}", lang))?;
        return Ok(info.name.to_string());
    }

    // Derive from file extension
    if let Some(file) = &args.file {
        if let Some(ext) = std::path::Path::new(file).extension().and_then(|e| e.to_str()) {
            let info = get_language_for_extension(ext)
                .ok_or_else(|| format!("unrecognized extension: .{}", ext))?;
            return Ok(info.name.to_string());
        }
    }

    Err("render requires --lang or a file with a recognized extension".into())
}

fn read_input(args: &RenderArgs) -> Result<String, Box<dyn std::error::Error>> {
    // Explicit -s/--string input
    if let Some(input) = &args.input {
        return Ok(input.clone());
    }

    // File argument: read source from disk
    if let Some(file) = &args.file {
        return Ok(std::fs::read_to_string(file)?);
    }

    // Read from stdin
    if atty::is(atty::Stream::Stdin) {
        return Err("render requires source input from --string, a file, or stdin".into());
    }

    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    Ok(input)
}
