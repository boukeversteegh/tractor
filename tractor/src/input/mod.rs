pub mod git;
pub mod filter;
pub mod file_resolver;
pub mod plan;
pub mod source;

#[allow(unused_imports)]
pub use source::{Source, SourceContent, SourceDisposition, PATHLESS_LABEL};
pub use file_resolver::{FileResolver, SourceRequest, ResolverOptions, make_fatal_diagnostic};
#[allow(unused_imports)]
pub use plan::{plan_multi, plan_single, InputPlan, MultiOpRequest, OperationDraft, SingleOpRequest};

use std::io::{self, BufRead, Read};
use std::sync::Arc;

use tractor::{detect_language, expand_globs_checked, NormalizedPath};

use crate::cli::SharedArgs;

/// What the input boundary resolved the user's invocation into.
///
/// Two mutually-exclusive shapes:
/// - `Files(patterns)`: disk mode. Glob patterns are resolved by `FileResolver`
///   at execution time — unchanged from prior behaviour.
/// - `Inline(source)`: stdin or `-s/--string` mode. A single [`Source`] whose
///   content is already in memory. The executor treats this source like any
///   other: rule globs match against `source.path`, diff-lines can compare
///   against it, diagnostics display it.
pub enum InputMode {
    Files(Vec<String>),
    Inline(Source),
}

/// Resolve CLI inputs into either a file-glob list or a single inline source.
///
/// Behaviour rules:
/// - `-s/--string` + `-l/--lang` → inline source; the positional `files` arg
///   may carry a single virtual path that gets attached to the source.
/// - piped stdin + `-l/--lang` on a non-TTY → same as above.
/// - piped stdin with no `-l` on a non-TTY → stdin is read as a list of paths.
/// - otherwise → disk mode with the given glob patterns.
///
/// Validation: in inline mode we accept **at most one** positional path. That
/// single path becomes the virtual path for glob matching, diff-lines, and
/// diagnostics. Multiple paths would be ambiguous — reject with a clear error.
pub fn resolve_input(
    shared: &SharedArgs,
    files: Vec<String>,
    content: Option<String>,
) -> Result<InputMode, Box<dyn std::error::Error>> {
    let expansion_limit = shared.max_files * 10;

    // Three input shapes:
    //   -s "..."      → inline from the string value (positional may be vpath)
    //   piped stdin   → inline when content is non-empty; stdin-as-paths
    //                   when no -l and files is empty (legacy behaviour)
    //   no stdin/-s   → disk mode
    //
    // We avoid the common test-env pitfall (stdin tied to /dev/null appears
    // "piped" to atty but has no content) by reading the piped bytes up
    // front: if empty, we stay in whichever disk path the CLI asked for.
    let stdin_piped = !atty::is(atty::Stream::Stdin);

    if content.is_some() {
        // -s / --string: always inline, content is already in hand.
        return resolve_inline(shared, files, content);
    }

    if stdin_piped && shared.lang.is_some() {
        // Language override + potential piped stdin → read and decide.
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        if !buf.is_empty() {
            return resolve_inline(shared, files, Some(buf));
        }
        // Empty stdin (typical in test env where stdin is /dev/null):
        // fall through to disk mode with whatever files were provided.
    }

    let result = expand_globs_checked(&files, expansion_limit, None)
        .map_err(|e| format!("{} — use a more specific pattern or increase --max-files", e))?;
    // Output boundary: downstream `InputMode::Files` carries `Vec<String>`,
    // so we convert here and treat stdin-fed paths as raw strings.
    let mut files: Vec<String> = result.files.into_iter().map(|p| p.as_str().to_string()).collect();

    if files.is_empty() && shared.lang.is_none() && stdin_piped {
        // Legacy stdin-as-paths mode: no -l, no positional args, something
        // piped. Read stdin line-by-line as file paths (git diff --name-only
        // style). Any -l invocation is handled above.
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
    files.retain(|f| detect_language(f) != "unknown");
    Ok(InputMode::Files(files))
}

/// Build the single [`Source`] for an inline invocation.
///
/// Handles both `-s "..."` (content provided directly) and piped stdin.
/// The positional `files` arg, if present, must be exactly one entry — it's
/// the virtual path, not a glob pattern. When absent the source is pathless
/// and displays as [`PATHLESS_LABEL`].
fn resolve_inline(
    shared: &SharedArgs,
    files: Vec<String>,
    content: Option<String>,
) -> Result<InputMode, Box<dyn std::error::Error>> {
    if files.len() > 1 {
        return Err(
            "inline source (stdin or --string) accepts at most one path — got multiple".into(),
        );
    }

    let lang = shared
        .lang
        .clone()
        .ok_or("--string / stdin input requires --lang to specify the language")?;

    let content = match content {
        Some(c) => c,
        None => {
            let mut s = String::new();
            io::stdin().read_to_string(&mut s)?;
            s
        }
    };
    let content = Arc::new(content);

    let source = match files.into_iter().next() {
        Some(vpath) => {
            // Anchor the virtual path to cwd the same way disk paths are
            // anchored, so `include:` globs (which are absolutized) match.
            let path = NormalizedPath::absolute(&vpath);
            Source::inline_at(path, lang, content)
        }
        None => Source::inline_pathless(lang, content),
    };

    Ok(InputMode::Inline(source))
}
