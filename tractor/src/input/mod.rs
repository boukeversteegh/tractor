pub mod git;
pub mod filter;
pub mod file_resolver;

use std::io::{self, BufRead, Read};
use tractor_core::{expand_globs_checked, filter_supported_files};
use crate::cli::SharedArgs;

pub enum InputMode {
    Files(Vec<String>),
    InlineSource { source: String, lang: String },
}

pub fn resolve_input(
    shared: &SharedArgs,
    files: Vec<String>,
    content: Option<String>,
) -> Result<InputMode, Box<dyn std::error::Error>> {
    let expansion_limit = shared.max_files * 10;
    let result = expand_globs_checked(&files, expansion_limit)
        .map_err(|e| format!("{} — use a more specific pattern or increase --max-files", e))?;
    let mut files: Vec<String> = result.files;

    let input = if let Some(ref content_str) = content {
        if shared.lang.is_none() {
            return Err("--string requires --lang to specify the language".into());
        }
        InputMode::InlineSource {
            source: content_str.clone(),
            lang: shared.lang.clone().unwrap(),
        }
    } else if files.is_empty() && shared.lang.is_some() && !atty::is(atty::Stream::Stdin) {
        let mut s = String::new();
        io::stdin().read_to_string(&mut s)?;
        InputMode::InlineSource {
            source: s,
            lang: shared.lang.clone().unwrap(),
        }
    } else {
        if files.is_empty() && shared.lang.is_none() && !atty::is(atty::Stream::Stdin) {
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
        files = filter_supported_files(files);
        InputMode::Files(files)
    };

    Ok(input)
}
