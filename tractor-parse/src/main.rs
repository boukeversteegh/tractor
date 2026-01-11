use clap::Parser;
use std::io::{self, BufRead, Read, Write};
use std::fs;

// Modern Agri-Tech color palette (ANSI escape codes)
mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const DIM: &str = "\x1b[2;37m";        // Punctuation: < > = / (dim white)
    pub const GREEN: &str = "\x1b[32m";        // Element names (fresh/growth)
    pub const CYAN: &str = "\x1b[36m";         // Attribute names (tech accent)
    pub const YELLOW: &str = "\x1b[33m";       // Attribute values (harvest gold)
    pub const WHITE: &str = "\x1b[97m";        // Text content (clean)
}

/// TreeSitter-based multi-language parser for tractor toolchain.
/// Outputs XML AST to stdout.
#[derive(Parser, Debug)]
#[command(name = "tractor-parse")]
#[command(about = "Parse source files into XML AST using TreeSitter")]
struct Args {
    /// Files to parse
    #[arg()]
    files: Vec<String>,

    /// Language to use (auto-detect from extension if not specified)
    /// When specified with no files, reads source code from stdin
    #[arg(short, long)]
    lang: Option<String>,

    /// Color output: auto (default), always, never
    #[arg(long, default_value = "auto")]
    color: String,

    /// List supported languages
    #[arg(long)]
    list_languages: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.list_languages {
        println!("Supported languages (22):");
        println!("  csharp     (.cs)");
        println!("  rust       (.rs)");
        println!("  javascript (.js, .mjs, .cjs)");
        println!("  typescript (.ts, .tsx)");
        println!("  python     (.py)");
        println!("  go         (.go)");
        println!("  java       (.java)");
        println!("  ruby       (.rb)");
        println!("  cpp        (.cpp, .cc, .cxx, .hpp, .hxx)");
        println!("  c          (.c, .h)");
        println!("  json       (.json)");
        println!("  html       (.html, .htm)");
        println!("  css        (.css)");
        println!("  bash       (.sh, .bash)");
        println!("  yaml       (.yml, .yaml)");
        println!("  php        (.php)");
        println!("  scala      (.scala, .sc)");
        println!("  lua        (.lua)");
        println!("  haskell    (.hs)");
        println!("  ocaml      (.ml, .mli)");
        println!("  r          (.r, .R)");
        println!("  julia      (.jl)");
        return Ok(());
    }

    // Collect files from args
    let mut files: Vec<String> = args.files;

    // Determine input mode based on args
    let stdin_source = files.is_empty() && args.lang.is_some() && !atty::is(atty::Stream::Stdin);

    if files.is_empty() && !stdin_source && !atty::is(atty::Stream::Stdin) {
        // No files, no --lang, but stdin available - read file paths from stdin
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

    if files.is_empty() && !stdin_source {
        eprintln!("Usage: tractor-parse <files...>");
        eprintln!("   or: cat source.rs | tractor-parse --lang rust");
        eprintln!("   or: echo 'file.rs' | tractor-parse");
        eprintln!("   or: tractor-parse --list-languages");
        std::process::exit(1);
    }

    // Determine if we should use color
    let use_color = match args.color.as_str() {
        "always" => true,
        "never" => false,
        _ => atty::is(atty::Stream::Stdout) && std::env::var("NO_COLOR").is_err(),
    };

    // Output XML header
    let stdout = io::stdout();
    let mut out = stdout.lock();
    writeln!(out, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    write_tag_open(&mut out, "Files", use_color)?;
    writeln!(out)?;

    // Process stdin source if --lang specified with no files
    if stdin_source {
        let mut source = String::new();
        io::stdin().read_to_string(&mut source)?;
        parse_source(&mut out, "<stdin>", &source, args.lang.as_deref().unwrap(), use_color)?;
    }

    // Process file paths
    for file_path in &files {
        parse_file(&mut out, file_path, args.lang.as_deref(), use_color)?;
    }

    write_tag_close(&mut out, "Files", 0, use_color)?;

    Ok(())
}

fn parse_file(out: &mut impl Write, file_path: &str, lang_override: Option<&str>, use_color: bool) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(file_path)?;
    let lang = lang_override.unwrap_or_else(|| detect_language(file_path));
    parse_source(out, file_path, &source, lang, use_color)
}

fn parse_source(out: &mut impl Write, path: &str, source: &str, lang: &str, use_color: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut parser = tree_sitter::Parser::new();

    // Set language based on detection
    let language = match lang {
        "csharp" | "cs" => tree_sitter_c_sharp::LANGUAGE.into(),
        "rust" | "rs" => tree_sitter_rust::LANGUAGE.into(),
        "javascript" | "js" => tree_sitter_javascript::LANGUAGE.into(),
        "typescript" | "ts" | "tsx" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "python" | "py" => tree_sitter_python::LANGUAGE.into(),
        "go" => tree_sitter_go::LANGUAGE.into(),
        "java" => tree_sitter_java::LANGUAGE.into(),
        "ruby" | "rb" => tree_sitter_ruby::LANGUAGE.into(),
        "cpp" | "c++" => tree_sitter_cpp::LANGUAGE.into(),
        "c" => tree_sitter_c::LANGUAGE.into(),
        "json" => tree_sitter_json::LANGUAGE.into(),
        "html" | "htm" => tree_sitter_html::LANGUAGE.into(),
        "css" => tree_sitter_css::LANGUAGE.into(),
        "bash" | "sh" => tree_sitter_bash::LANGUAGE.into(),
        "yaml" | "yml" => tree_sitter_yaml::LANGUAGE.into(),
        "php" => tree_sitter_php::LANGUAGE_PHP.into(),
        "scala" => tree_sitter_scala::LANGUAGE.into(),
        "lua" => tree_sitter_lua::LANGUAGE.into(),
        "haskell" | "hs" => tree_sitter_haskell::LANGUAGE.into(),
        "ocaml" | "ml" => tree_sitter_ocaml::LANGUAGE_OCAML.into(),
        "r" => tree_sitter_r::LANGUAGE.into(),
        "julia" | "jl" => tree_sitter_julia::LANGUAGE.into(),
        _ => {
            eprintln!("Unsupported language: {} (path: {})", lang, path);
            eprintln!("Use --list-languages to see supported languages");
            return Ok(());
        }
    };

    parser.set_language(&language)?;

    let tree = parser.parse(source, None)
        .ok_or("Failed to parse")?;

    write_file_tag_open(out, path, use_color)?;
    write_node(out, tree.root_node(), source, 2, use_color)?;
    write_tag_close(out, "File", 1, use_color)?;

    Ok(())
}

fn detect_language(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext.to_lowercase().as_str() {
        // C#
        "cs" => "csharp",
        // Rust
        "rs" => "rust",
        // JavaScript
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        // TypeScript
        "ts" | "tsx" => "typescript",
        // Python
        "py" | "pyw" | "pyi" => "python",
        // Go
        "go" => "go",
        // Java
        "java" => "java",
        // Ruby
        "rb" | "rake" | "gemspec" => "ruby",
        // C++
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => "cpp",
        // C
        "c" | "h" => "c",
        // JSON
        "json" => "json",
        // HTML
        "html" | "htm" => "html",
        // CSS
        "css" => "css",
        // Bash
        "sh" | "bash" => "bash",
        // YAML
        "yml" | "yaml" => "yaml",
        // PHP
        "php" => "php",
        // Scala
        "scala" | "sc" => "scala",
        // Lua
        "lua" => "lua",
        // Haskell
        "hs" | "lhs" => "haskell",
        // OCaml
        "ml" | "mli" => "ocaml",
        // R
        "r" => "r",
        // Julia
        "jl" => "julia",
        // Unknown
        _ => "unknown"
    }
}

fn write_node(out: &mut impl Write, node: tree_sitter::Node, source: &str, indent: usize, use_color: bool) -> Result<(), Box<dyn std::error::Error>> {
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
        write!(out, "{}", indent_str)?;
        write_element_with_attrs_and_text(
            out, kind,
            &[
                ("startLine", &(start.row + 1).to_string()),
                ("startCol", &(start.column + 1).to_string()),
                ("endLine", &(end.row + 1).to_string()),
                ("endCol", &(end.column + 1).to_string()),
            ],
            Some(text),
            use_color,
        )?;
        writeln!(out)?;
    } else {
        // Node with children
        write!(out, "{}", indent_str)?;
        write_element_open_with_attrs(
            out, kind,
            &[
                ("startLine", &(start.row + 1).to_string()),
                ("startCol", &(start.column + 1).to_string()),
                ("endLine", &(end.row + 1).to_string()),
                ("endCol", &(end.column + 1).to_string()),
            ],
            use_color,
        )?;
        writeln!(out)?;

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            write_node(out, child, source, indent + 1, use_color)?;
        }

        write_tag_close(out, kind, indent, use_color)?;
    }

    Ok(())
}

// Color helper functions for Modern Agri-Tech palette

fn write_tag_open(out: &mut impl Write, name: &str, use_color: bool) -> io::Result<()> {
    if use_color {
        // DIM for <, then RESET before GREEN element name, then DIM for >, then RESET
        write!(out, "{}<{}{}{}{}>{}",
            colors::DIM, colors::RESET, colors::GREEN, name, colors::DIM, colors::RESET)
    } else {
        write!(out, "<{}>", name)
    }
}

fn write_tag_close(out: &mut impl Write, name: &str, indent: usize, use_color: bool) -> io::Result<()> {
    let indent_str = "  ".repeat(indent);
    if use_color {
        writeln!(out, "{}{}<{}/{}{}{}>{}",
            indent_str, colors::DIM, colors::RESET, colors::GREEN, name, colors::DIM, colors::RESET)
    } else {
        writeln!(out, "{}</{}>", indent_str, name)
    }
}

fn write_file_tag_open(out: &mut impl Write, path: &str, use_color: bool) -> io::Result<()> {
    if use_color {
        // Format: <File path="value">
        // Colors: DIM< RESET GREEN(File) RESET CYAN(path) DIM(=) RESET YELLOW("value") DIM(>) RESET
        writeln!(out, "  {}<{}{}File{} {}path{}={}{}\"{}\"{}>{}",
            colors::DIM, colors::RESET, colors::GREEN, colors::RESET,
            colors::CYAN, colors::DIM, colors::RESET, colors::YELLOW, escape_xml(path),
            colors::DIM, colors::RESET)
    } else {
        writeln!(out, "  <File path=\"{}\">", escape_xml(path))
    }
}

fn write_element_open_with_attrs(out: &mut impl Write, name: &str, attrs: &[(&str, &str)], use_color: bool) -> io::Result<()> {
    if use_color {
        write!(out, "{}<{}{}{}", colors::DIM, colors::RESET, colors::GREEN, escape_xml(name))?;
        for (attr_name, attr_value) in attrs {
            write!(out, "{} {}{}{}={}{}\"{}\"",
                colors::RESET, colors::CYAN, attr_name, colors::DIM, colors::RESET, colors::YELLOW, attr_value)?;
        }
        write!(out, "{}>{}",  colors::DIM, colors::RESET)?;
    } else {
        write!(out, "<{}", escape_xml(name))?;
        for (attr_name, attr_value) in attrs {
            write!(out, " {}=\"{}\"", attr_name, attr_value)?;
        }
        write!(out, ">")?;
    }
    Ok(())
}

fn write_element_with_attrs_and_text(out: &mut impl Write, name: &str, attrs: &[(&str, &str)], text: Option<&str>, use_color: bool) -> io::Result<()> {
    let escaped_name = escape_xml(name);
    if use_color {
        write!(out, "{}<{}{}{}", colors::DIM, colors::RESET, colors::GREEN, escaped_name)?;
        for (attr_name, attr_value) in attrs {
            write!(out, "{} {}{}{}={}{}\"{}\"",
                colors::RESET, colors::CYAN, attr_name, colors::DIM, colors::RESET, colors::YELLOW, attr_value)?;
        }
        write!(out, "{}>{}",  colors::DIM, colors::RESET)?;
        if let Some(t) = text {
            write!(out, "{}{}{}", colors::WHITE, escape_xml(t), colors::RESET)?;
        }
        write!(out, "{}<{}/{}{}{}>{}",
            colors::DIM, colors::RESET, colors::GREEN, escaped_name, colors::DIM, colors::RESET)?;
    } else {
        write!(out, "<{}", escaped_name)?;
        for (attr_name, attr_value) in attrs {
            write!(out, " {}=\"{}\"", attr_name, attr_value)?;
        }
        write!(out, ">")?;
        if let Some(t) = text {
            write!(out, "{}", escape_xml(t))?;
        }
        write!(out, "</{}>", escaped_name)?;
    }
    Ok(())
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}
