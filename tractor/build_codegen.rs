//! Build-time codegen for the variant-blind reflection metadata over
//! `SyntaxTree`. Invoked from `build.rs`.
//!
//! Reads `tractor/src/tree/syntax/types.rs`, parses the `SyntaxTree`
//! enum with `syn`, and writes
//! `tractor/src/tree/syntax/metadata.generated.rs` containing two
//! accessors consumed by the variant-blind walkers in `to_xot.rs`
//! and `to_json.rs`.
//!
//! Output is committed to source control so reviewers can see exactly
//! what the codegen produced and debuggers can step into real line
//! numbers. `write_if_changed` keeps cargo idempotent — a build is a
//! no-op when the enum hasn't changed.
//!
//! Rules (mechanical, no per-variant overrides):
//!
//! Element name:
//! - If a variant has a field named `element_name`, `wrapper`, or
//!   `kind` of `&'static str` / `String` type: use that field's
//!   value.
//! - Else if the variant is `Inline` or `Skip`: `None` (no wrapper).
//! - Else: snake_case of the variant identifier.
//!
//! Flags / markers:
//! - `modifiers: Modifiers` → expand via `markers_with_spans()`.
//! - `extra_markers: Vec<Marker>` → emit each.
//! - Any `Flag`-typed field → emit a marker named after the field with
//!   any trailing underscore stripped (`async_` → `async`).

use std::fs;
use std::path::Path;

use quote::ToTokens;
use syn::{Fields, Item, ItemEnum, Variant};

const INPUT: &str = "src/tree/syntax/types.rs";
const OUTPUT: &str = "src/tree/syntax/metadata.generated.rs";

/// Run the codegen. Called from `build.rs::main`.
pub fn generate() {
    // build.rs runs with CWD = tractor/, so the relative paths above
    // resolve correctly. Tell cargo to re-run only when the source
    // enum changes — keeps incremental builds fast.
    println!("cargo:rerun-if-changed={}", INPUT);
    println!("cargo:rerun-if-changed=build_codegen.rs");

    let src = fs::read_to_string(INPUT)
        .unwrap_or_else(|e| panic!("reading {}: {}", INPUT, e));
    let file: syn::File = syn::parse_file(&src)
        .unwrap_or_else(|e| panic!("parsing {}: {}", INPUT, e));
    let enum_item = find_enum(&file, "SyntaxTree")
        .unwrap_or_else(|| panic!("SyntaxTree enum not found in {}", INPUT));

    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str(&render_element_name_of(enum_item));
    out.push('\n');
    out.push_str(&render_flags_of(enum_item));

    write_if_changed(OUTPUT, &out);
}

const HEADER: &str = "\
// DO NOT EDIT — emitted by `tractor/build.rs` on every build.
// Source: SyntaxTree enum in tractor/src/tree/syntax/types.rs.
//
// Variant-blind reflection metadata that drives the XML and JSON
// renderers' mechanical walks (`to_xot.rs`, `to_json.rs`). Rules
// are derived from field types only — no per-variant special cases.
// See `tractor/build_codegen.rs`.

#![cfg(feature = \"native\")]

#[allow(unused_imports)]
use super::types::{ByteRange, Flag, Marker, Span, SyntaxTree};

";

fn find_enum<'a>(file: &'a syn::File, name: &str) -> Option<&'a ItemEnum> {
    file.items.iter().find_map(|item| match item {
        Item::Enum(e) if e.ident == name => Some(e),
        _ => None,
    })
}

/// Stringify a `syn::Type` with no whitespace, for pattern matching.
fn type_str(ty: &syn::Type) -> String {
    let s = ty.to_token_stream().to_string();
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

/// `Function` → `function`; `ElseIf` → `else_if`; `CFor` → `c_for`.
fn snake_case(pascal: &str) -> String {
    let mut out = String::new();
    for (i, ch) in pascal.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn find_field<'a>(v: &'a Variant, name: &str) -> Option<&'a syn::Field> {
    if let Fields::Named(named) = &v.fields {
        named.named.iter().find(|f| {
            f.ident.as_ref().map(|i| i == name).unwrap_or(false)
        })
    } else {
        None
    }
}

fn render_element_name_of(en: &ItemEnum) -> String {
    let mut out = String::new();
    out.push_str(
        "/// The XML element name for this tree node, or `None` if the
/// node renders no wrapper (`Inline`, `Skip`).
///
/// Borrowed lifetime: most arms return `&'static str` literals or
/// `&'static str` field values, but `Unknown` / `Raw` carry `String`
/// kinds so the return type ties to the tree.
pub fn element_name_of(tree: &SyntaxTree) -> Option<&str> {
    match tree {
",
    );
    for v in &en.variants {
        let name = v.ident.to_string();
        out.push_str(&element_arm(&name, v));
    }
    out.push_str(
        "    }
}
",
    );
    out
}

fn element_arm(name: &str, v: &Variant) -> String {
    if name == "Inline" || name == "Skip" {
        return format!("        SyntaxTree::{} {{ .. }} => None,\n", name);
    }
    for cand in ["element_name", "wrapper", "kind"] {
        if let Some(field) = find_field(v, cand) {
            let ty = type_str(&field.ty);
            return match ty.as_str() {
                "&'staticstr" => format!(
                    "        SyntaxTree::{} {{ {}, .. }} => Some(*{}),\n",
                    name, cand, cand
                ),
                "String" => format!(
                    "        SyntaxTree::{} {{ {}, .. }} => Some({}.as_str()),\n",
                    name, cand, cand
                ),
                _ => format!(
                    "        SyntaxTree::{} {{ .. }} => Some({:?}),\n",
                    name,
                    snake_case(name)
                ),
            };
        }
    }
    format!(
        "        SyntaxTree::{} {{ .. }} => Some({:?}),\n",
        name,
        snake_case(name)
    )
}

fn render_flags_of(en: &ItemEnum) -> String {
    let mut out = String::new();
    out.push_str(
        "/// Empty-element marker children for this tree node. Drawn from
/// `Modifiers::markers_with_spans()`, any `Flag` field (named after
/// the field with trailing `_` stripped), and any `Vec<Marker>` field.
///
/// Each entry's `span` lets the XML renderer emit
/// `line` / `column` attributes at the keyword position when the flag
/// is anchored, falling back to a default for implicit flags.
pub fn flags_of(tree: &SyntaxTree) -> Vec<Marker> {
    let mut out: Vec<Marker> = Vec::new();
    match tree {
",
    );
    for v in &en.variants {
        out.push_str(&flags_arm(v));
    }
    out.push_str(
        "    }
    out
}
",
    );
    out
}

fn flags_arm(v: &Variant) -> String {
    let name = v.ident.to_string();
    let mut bindings: Vec<String> = Vec::new();
    let mut body = String::new();
    let mut needs_span = false;

    if let Fields::Named(named) = &v.fields {
        for field in &named.named {
            let Some(ident) = &field.ident else { continue };
            let fname = ident.to_string();
            let ty = type_str(&field.ty);
            match ty.as_str() {
                "Modifiers" => {
                    bindings.push(fname.clone());
                    needs_span = true;
                    body.push_str(&format!(
                        "            for (mname, mspan) in {}.markers_with_spans() {{
                out.push(Marker {{
                    name: mname,
                    range: ByteRange::synthetic_empty(),
                    span: mspan.unwrap_or(*span),
                }});
            }}
",
                        fname
                    ));
                }
                "Flag" => {
                    bindings.push(fname.clone());
                    needs_span = true;
                    let marker_name = strip_trailing_underscore(&fname);
                    body.push_str(&format!(
                        "            if let Flag::On {{ range: frange, span: fspan }} = {} {{
                out.push(Marker {{ name: {:?}, range: *frange, span: *fspan }});
            }}
",
                        fname, marker_name
                    ));
                }
                "Vec<Marker>" => {
                    bindings.push(fname.clone());
                    body.push_str(&format!(
                        "            for m in {} {{ out.push(*m); }}\n",
                        fname
                    ));
                }
                _ => {}
            }
        }
    }

    if bindings.is_empty() {
        return format!("        SyntaxTree::{} {{ .. }} => {{}}\n", name);
    }

    if needs_span {
        bindings.push("span".to_string());
    }

    let binding_list = bindings.join(", ");
    format!(
        "        SyntaxTree::{} {{ {}, .. }} => {{
{}
        }}
",
        name, binding_list, body
    )
}

fn strip_trailing_underscore(s: &str) -> String {
    if let Some(stripped) = s.strip_suffix('_') {
        stripped.to_string()
    } else {
        s.to_string()
    }
}

fn write_if_changed(path: &str, content: &str) {
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == content {
            return;
        }
    }
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent).unwrap_or_else(|e| {
            panic!("creating parent dir for {}: {}", path, e)
        });
    }
    fs::write(path, content)
        .unwrap_or_else(|e| panic!("writing {}: {}", path, e));
}
