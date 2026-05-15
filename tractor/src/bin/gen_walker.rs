//! Generate variant-blind XML accessors for `SyntaxTree`.
//!
//! Reads `tractor/src/tree/types.rs`, parses the `SyntaxTree` enum with
//! `syn`, and emits `tractor/src/tree/render_generated.rs` containing
//! two accessors used by the variant-blind walker in `to_xot.rs`:
//!
//! - `element_name_of(&SyntaxTree) -> Option<&'static str>` — the XML
//!   element name for this node (`None` means "no wrapper element;
//!   inline children at parent").
//! - `flags_of(&SyntaxTree) -> Vec<Marker>` — the empty-element marker
//!   children (modifiers, named flags, extra markers).
//!
//! No `#[shape(...)]` attribute DSL: the codegen reads the field types
//! directly. Rules (mechanical, no per-variant overrides):
//!
//! Element name:
//! - If a variant has a field named `element_name`, `kind`, or
//!   `wrapper` of string-like type: use that field's value.
//! - Else if the variant is `Inline` or `Skip`: `None` (no wrapper).
//! - Else: snake_case of the variant identifier.
//!
//! Flags / markers:
//! - `modifiers: Modifiers` → expand via `markers_with_spans()`.
//! - `extra_markers: Vec<Marker>` → emit each.
//! - Any `Flag`-typed field → emit a marker named after the field with
//!   any trailing underscore stripped (`async_` → `async`).
//!
//! Output is committed; CI runs `task verify:gen-walker` to enforce
//! freshness.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use quote::ToTokens;
use syn::{Fields, Item, ItemEnum, Variant};

const INPUT: &str = "tractor/src/tree/types.rs";
const OUTPUT: &str = "tractor/src/tree/render_generated.rs";

fn main() -> Result<()> {
    let src = fs::read_to_string(INPUT)
        .with_context(|| format!("reading {}", INPUT))?;
    let file: syn::File = syn::parse_file(&src)
        .with_context(|| format!("parsing {} as Rust", INPUT))?;
    let enum_item = find_enum(&file, "SyntaxTree")
        .ok_or_else(|| anyhow!("SyntaxTree enum not found in {}", INPUT))?;

    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str(&render_element_name_of(enum_item));
    out.push('\n');
    out.push_str(&render_flags_of(enum_item));

    write_if_changed(OUTPUT, &out)?;
    println!("{} ({} variants)", OUTPUT, enum_item.variants.len());
    Ok(())
}

const HEADER: &str = "\
// DO NOT EDIT — regenerate via `task gen:walker`.
// Source: SyntaxTree enum in tractor/src/tree/types.rs.
//
// Variant-blind accessors that drive the XML renderer's mechanical
// walk in `to_xot.rs`. Rules are derived from field types only — no
// per-variant special cases. See `tractor/src/bin/gen_walker.rs`.

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
/// e.g. `Box<SyntaxTree>`, `Option<Box<SyntaxTree>>`, `Vec<SyntaxTree>`,
/// `Vec<Marker>`, `Modifiers`, `Flag`, `&'staticstr`.
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

/// Look up a named field on a variant.
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
        let arm = element_arm(&name, v);
        out.push_str(&arm);
    }
    out.push_str(
        "    }
}
",
    );
    out
}

/// One `element_name_of` match arm. Picks the rule:
///  - `element_name: &'static str` / `wrapper: &'static str` /
///    `kind: &'static str` / `kind: String` → use field value.
///  - `Inline` / `Skip` → None.
///  - Otherwise → snake_case of variant name.
fn element_arm(name: &str, v: &Variant) -> String {
    if name == "Inline" || name == "Skip" {
        return format!("        SyntaxTree::{} {{ .. }} => None,\n", name);
    }
    // Field-driven element name. Order matters: prefer explicit
    // `element_name`, then `wrapper`, then `kind`.
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
        let arm = flags_arm(v);
        out.push_str(&arm);
    }
    out.push_str(
        "    }
    out
}
",
    );
    out
}

/// Build the bindings/body for one `flags_of` match arm. The arm
/// inspects each field by *type*, not by name:
///  - `Modifiers` → expand `markers_with_spans()`.
///  - `Flag` → emit a `Marker` named after the field.
///  - `Vec<Marker>` → push each entry.
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

    // Empty arm — no flag-bearing fields. Use the wildcard binding
    // form so we don't have a long list of `_` placeholders.
    if bindings.is_empty() {
        return format!("        SyntaxTree::{} {{ .. }} => {{}}\n", name);
    }

    // Bind `span` only if a Modifier or Flag arm needs it as fallback.
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

fn write_if_changed(path: &str, content: &str) -> Result<()> {
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == content {
            return Ok(());
        }
    }
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

