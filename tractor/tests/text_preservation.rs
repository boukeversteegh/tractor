//! Advisory test: verify that tractor's semantic transform preserves
//! source text character-for-character.
//!
//! For every fixture in `tests/integration/languages/<lang>/`, parse
//! twice — once in raw tree-sitter mode, once in the default
//! (transformed) mode — and concatenate all text nodes from each
//! resulting `Xot` tree. The two strings must be equal; if they're
//! not, the transform has either dropped or synthesized characters
//! that weren't in the source.
//!
//! This is the mechanical enforcement of the **source-reversibility
//! goal** documented in
//! `specs/tractor-parse/semantic-tree/design.md`: stripping element
//! tags from the tree and concatenating text in document order
//! should reproduce what tree-sitter saw.
//!
//! **Advisory mode (today)**: the test always passes. Violations are
//! printed so we can see the damage without blocking CI. Flip the
//! `ASSERT_INVARIANT` constant to `true` to make this a hard gate.

use std::path::PathBuf;
use tractor::{parse, ParseInput, ParseOptions, TreeMode};
use xot::{Xot, Node};

/// Flip to `true` when every fixture passes and we're ready to
/// enforce the invariant.
const ASSERT_INVARIANT: bool = false;

/// Data-language extensions: the default tree mode for these
/// languages is `Data`, which is an intentional shape projection
/// (keys → elements, scalars → text). The invariant would fail
/// trivially. We compare `Structure` ↔ `Raw` for these, which
/// today is a near-identity pass but puts the harness in place
/// for when data-language structure transforms land.
const DATA_LANG_EXTS: &[&str] = &["json", "yaml", "yml", "toml", "ini", "env"];

/// Max number of violations to print per run; full list is summarised.
const MAX_SHOWN: usize = 30;

fn concat_text(xot: &Xot, root: Node) -> String {
    let mut out = String::new();
    collect_text(xot, root, &mut out);
    out
}

fn collect_text(xot: &Xot, node: Node, out: &mut String) {
    if let Some(text) = xot.text_str(node) {
        out.push_str(text);
    }
    for child in xot.children(node) {
        collect_text(xot, child, out);
    }
}

fn languages_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/integration/languages")
}

/// Walk the languages tree, returning every source fixture —
/// skipping generated snapshots and markdown / shell helpers.
fn iter_fixtures() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = vec![languages_dir()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy();
            if name.contains(".snapshot.") {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(ext, "xml" | "md" | "sh" | "json") {
                continue;
            }
            out.push(path);
        }
    }
    out.sort();
    out
}

struct Violation {
    path: PathBuf,
    raw_len: usize,
    xfm_len: usize,
    first_delta_at: Option<usize>,
    raw_around: String,
    xfm_around: String,
}

fn find_first_delta(raw: &str, xfm: &str) -> Option<usize> {
    raw.chars()
        .zip(xfm.chars())
        .position(|(a, b)| a != b)
        .or_else(|| if raw.len() == xfm.len() { None } else { Some(raw.len().min(xfm.len())) })
}

fn slice_around(s: &str, pos: usize, radius: usize) -> String {
    let start = pos.saturating_sub(radius);
    let end = (pos + radius).min(s.len());
    s[start..end].replace('\n', "⏎").to_string()
}

#[test]
fn transform_preserves_source_text() {
    let fixtures = iter_fixtures();
    let mut violations: Vec<Violation> = Vec::new();
    let mut checked = 0usize;
    let mut skipped_parse = 0usize;

    for path in &fixtures {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let is_data = DATA_LANG_EXTS.contains(&ext);
        // For data languages the default tree mode is Data (a
        // projection), so force Structure for both sides to compare
        // semantic transform against raw — see DATA_LANG_EXTS doc.
        let xfm_mode = if is_data { Some(TreeMode::Structure) } else { None };

        let raw = match parse(
            ParseInput::Disk { path },
            ParseOptions {
                tree_mode: Some(TreeMode::Raw),
                ..ParseOptions::default()
            },
        ) {
            Ok(r) => r,
            Err(_) => {
                skipped_parse += 1;
                continue;
            }
        };
        let xfm = match parse(
            ParseInput::Disk { path },
            ParseOptions {
                tree_mode: xfm_mode,
                ..ParseOptions::default()
            },
        ) {
            Ok(r) => r,
            Err(_) => {
                skipped_parse += 1;
                continue;
            }
        };

        let raw_root = raw.documents.document_node(raw.doc_handle).unwrap();
        let xfm_root = xfm.documents.document_node(xfm.doc_handle).unwrap();

        let raw_text = concat_text(raw.documents.xot(), raw_root);
        let xfm_text = concat_text(xfm.documents.xot(), xfm_root);

        checked += 1;

        if raw_text != xfm_text {
            let pos = find_first_delta(&raw_text, &xfm_text).unwrap_or(0);
            violations.push(Violation {
                path: path.clone(),
                raw_len: raw_text.len(),
                xfm_len: xfm_text.len(),
                first_delta_at: Some(pos),
                raw_around: slice_around(&raw_text, pos, 40),
                xfm_around: slice_around(&xfm_text, pos, 40),
            });
        }
    }

    if violations.is_empty() {
        eprintln!(
            "\n✓ source-text preservation: all {} fixtures OK (skipped {})",
            checked, skipped_parse
        );
        return;
    }

    // Print report (advisory).
    eprintln!();
    eprintln!(
        "⚠ source-text preservation: {}/{} fixtures altered source text",
        violations.len(),
        checked
    );
    eprintln!();
    for v in violations.iter().take(MAX_SHOWN) {
        eprintln!("  {}", v.path.display());
        eprintln!(
            "    raw {} chars, transformed {} chars (Δ {})",
            v.raw_len,
            v.xfm_len,
            v.xfm_len as i64 - v.raw_len as i64
        );
        if let Some(pos) = v.first_delta_at {
            eprintln!("    first delta at byte {}:", pos);
            eprintln!("      raw: …{}…", v.raw_around);
            eprintln!("      xfm: …{}…", v.xfm_around);
        }
    }
    if violations.len() > MAX_SHOWN {
        eprintln!("  … and {} more", violations.len() - MAX_SHOWN);
    }
    eprintln!();
    eprintln!(
        "(Advisory mode — this test passes. Flip ASSERT_INVARIANT=true \
         in tests/text_preservation.rs to make it a hard gate.)"
    );

    if ASSERT_INVARIANT {
        panic!("source-text preservation failed for {} fixture(s)", violations.len());
    }
}
