use std::io::Read;
use clap::Args;
use tractor::language_info::{get_language_info, get_language_for_extension};
use tractor::parser::{parse, ParseInput, ParseOptions};
use tractor::xpath::Tree;

/// Render mode: round-trip parse source → tree → source, OR
/// reconstruct from a JSON tree projection and render to source.
///
/// **Source input** (default): parses source through the typed-tree
/// pipeline and re-emits it via the tree-aware source renderer
/// (`tree::render::render`). In anchored mode this is byte-for-byte
/// identity — useful for verifying lossless parse and as the
/// substrate that `tractor set` / `tractor update` build on.
///
/// **JSON input** (`--from json`, or auto-detected by leading `{` /
/// `[`): deserialises the JSON to a `SyntaxTree` via `tree_from_json`
/// and renders to source. Synthetic ranges; no anchor available, so
/// output is canonical-form (the language's `render_source` syntax
/// config drives spacing / keywords).
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

    /// Input format: `source` (parse-then-render round-trip; the
    /// default) or `json` (reconstruct a tree from its `$type`-
    /// discriminated JSON projection and render to source). When
    /// omitted, the input is sniffed — a leading `{` or `[` after
    /// trimming whitespace is treated as JSON.
    #[arg(long = "from")]
    pub from: Option<String>,
}

pub fn run_render(args: RenderArgs) -> Result<(), Box<dyn std::error::Error>> {
    let lang = resolve_language(&args)?;
    let input = read_input(&args)?;
    let file_label = args.file.clone().unwrap_or_else(|| "<stdin>".to_string());

    if input_is_json(&args, &input) {
        let rendered = render_from_json(&input, &lang)
            .map_err(|e| format!("render from json: {e}"))?;
        write_output(&args, &rendered)?;
        return Ok(());
    }

    let parsed = parse(
        ParseInput::Inline { content: &input, file_label: &file_label },
        ParseOptions { language: Some(&lang), ..Default::default() },
    ).map_err(|e| format!("parse failed: {e}"))?;

    let rendered = match parsed.root_tree.as_ref() {
        Some(Tree::SyntaxTree { tree, source, .. }) => {
            // Anchored render slices the source verbatim, so the
            // round-trip is byte-identical.
            tractor::tree::render::render(tree, &lang, Some(source))
        }
        Some(Tree::DataTree { tree, source, .. }) => {
            tree.to_source(source).to_string()
        }
        Some(Tree::Sql { tree, source, .. }) => {
            tractor::tree::render::render_sql(tree, Some(source))
        }
        _ => {
            return Err(format!(
                "language '{}' is not on the tree pipeline; render is only available \
                 for tree-supported languages",
                lang,
            ).into());
        }
    };

    write_output(&args, &rendered)?;

    Ok(())
}

/// Choose JSON vs source input mode. Explicit `--from json`/`source`
/// wins; otherwise sniff a non-whitespace leading `{` or `[` to
/// auto-detect a JSON tree projection.
fn input_is_json(args: &RenderArgs, input: &str) -> bool {
    match args.from.as_deref() {
        Some("json") => return true,
        Some("source") => return false,
        Some(_) => {} // unknown value falls through to sniffing
        None => {}
    }
    input
        .trim_start()
        .chars()
        .next()
        .is_some_and(|c| c == '{' || c == '[')
}

/// Deserialise the JSON tree projection, reconstruct a `SyntaxTree`
/// via the codegen'd `tree_from_json`, and render to `lang`. Uses
/// canonical (no-anchor) rendering — the tree carries synthetic
/// ranges after the JSON hop.
fn render_from_json(input: &str, lang: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(input)
        .map_err(|e| format!("invalid JSON: {e}"))?;
    if lang == "tsql" {
        return Err("tsql tree-from-json not implemented (SqlTree has no from_json yet)".into());
    }
    let tree = tractor::tree::tree_from_json(&value, Some(lang));
    Ok(tractor::tree::render::render(&tree, lang, None))
}

fn write_output(args: &RenderArgs, rendered: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(file) = &args.file {
        std::fs::write(file, rendered)?;
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
