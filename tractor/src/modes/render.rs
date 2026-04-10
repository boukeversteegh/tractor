use crate::cli::RenderArgs;
use std::io::Read;
use tractor_core::language_info::{get_language_for_extension, get_language_info};
use tractor_core::render::{parse_input, render, RenderOptions};
use tractor_core::TreeMode;

pub fn run_render(args: RenderArgs) -> Result<(), Box<dyn std::error::Error>> {
    let lang = resolve_language(&args)?;
    let input = read_input(&args)?;
    let node = parse_input(&input)?;
    let opts = RenderOptions::default();
    let source = render(&node, &lang, TreeMode::Data, &opts)?;

    if let Some(file) = &args.file {
        std::fs::write(file, &source)?;
        eprintln!("Rendered to {}", file);
    } else {
        println!("{}", source);
    }

    Ok(())
}

fn resolve_language(args: &RenderArgs) -> Result<String, Box<dyn std::error::Error>> {
    // Explicit --lang takes priority (supports aliases like "cs" → "csharp")
    if let Some(lang) = &args.lang {
        let info = get_language_info(lang).ok_or_else(|| format!("unknown language: {}", lang))?;
        return Ok(info.name.to_string());
    }

    // Derive from file extension
    if let Some(file) = &args.file {
        if let Some(ext) = std::path::Path::new(file)
            .extension()
            .and_then(|e| e.to_str())
        {
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

    // Read from stdin
    if atty::is(atty::Stream::Stdin) {
        return Err("render requires input (XML or JSON) from stdin or --string".into());
    }

    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    Ok(input)
}
