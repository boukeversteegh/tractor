use clap::Parser;
use std::io::{self, BufRead, Write};
use std::fs;

/// TreeSitter-based multi-language parser for tractor toolchain.
/// Reads file paths from stdin, outputs XML AST to stdout.
#[derive(Parser, Debug)]
#[command(name = "tractor-parse")]
#[command(about = "Parse source files into XML AST using TreeSitter")]
struct Args {
    /// Files to parse (also accepts file paths on stdin, one per line)
    #[arg()]
    files: Vec<String>,

    /// Language to use (auto-detect from extension if not specified)
    #[arg(short, long)]
    lang: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Collect files from args and stdin
    let mut files: Vec<String> = args.files;

    // If no files provided as args, read from stdin
    if files.is_empty() && !atty::is(atty::Stream::Stdin) {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            if let Ok(path) = line {
                let path = path.trim().to_string();
                if !path.is_empty() {
                    files.push(path);
                }
            }
        }
    }

    if files.is_empty() {
        eprintln!("Usage: tractor-parse <files...>");
        eprintln!("   or: echo 'file.cs' | tractor-parse");
        std::process::exit(1);
    }

    // Output XML header
    let stdout = io::stdout();
    let mut out = stdout.lock();
    writeln!(out, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    writeln!(out, "<Files>")?;

    for file_path in &files {
        parse_file(&mut out, file_path, args.lang.as_deref())?;
    }

    writeln!(out, "</Files>")?;

    Ok(())
}

fn parse_file(out: &mut impl Write, file_path: &str, lang_override: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(file_path)?;
    let lang = lang_override.unwrap_or_else(|| detect_language(file_path));

    let mut parser = tree_sitter::Parser::new();

    // Set language based on detection
    match lang {
        "csharp" | "cs" => {
            parser.set_language(&tree_sitter_c_sharp::LANGUAGE.into())?;
        }
        // Add more languages here as they're enabled in Cargo.toml
        _ => {
            eprintln!("Unsupported language: {}", lang);
            return Ok(());
        }
    }

    let tree = parser.parse(&source, None)
        .ok_or("Failed to parse")?;

    writeln!(out, r#"  <File path="{}">"#, escape_xml(file_path))?;
    write_node(out, tree.root_node(), &source, 2)?;
    writeln!(out, "  </File>")?;

    Ok(())
}

fn detect_language(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("cs") => "csharp",
        Some("js") => "javascript",
        Some("ts") => "typescript",
        Some("py") => "python",
        Some("go") => "go",
        Some("rs") => "rust",
        _ => "unknown"
    }
}

fn write_node(out: &mut impl Write, node: tree_sitter::Node, source: &str, indent: usize) -> Result<(), Box<dyn std::error::Error>> {
    let indent_str = "  ".repeat(indent);
    let kind = node.kind();

    // Skip anonymous nodes (punctuation, etc.) - focus on named nodes
    if !node.is_named() {
        return Ok(());
    }

    let start = node.start_position();
    let end = node.end_position();

    // Check if this is a leaf node (no named children)
    let named_child_count = node.named_child_count();

    if named_child_count == 0 {
        // Leaf node - include text content
        let text = node.utf8_text(source.as_bytes()).unwrap_or("");
        writeln!(out,
            r#"{}<{} startLine="{}" startCol="{}" endLine="{}" endCol="{}">{}</{}>"#,
            indent_str,
            escape_xml(kind),
            start.row + 1,
            start.column + 1,
            end.row + 1,
            end.column + 1,
            escape_xml(text),
            escape_xml(kind)
        )?;
    } else {
        // Node with children
        writeln!(out,
            r#"{}<{} startLine="{}" startCol="{}" endLine="{}" endCol="{}">"#,
            indent_str,
            escape_xml(kind),
            start.row + 1,
            start.column + 1,
            end.row + 1,
            end.column + 1
        )?;

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            write_node(out, child, source, indent + 1)?;
        }

        writeln!(out, "{}</{}>", indent_str, escape_xml(kind))?;
    }

    Ok(())
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}
