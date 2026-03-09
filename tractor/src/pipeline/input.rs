use std::io::{self, BufRead, Read};
use tractor_core::{expand_globs, filter_supported_files};
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
    let mut files: Vec<String> = expand_globs(&files);

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
