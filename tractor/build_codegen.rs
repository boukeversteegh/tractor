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

/// Per-tree codegen spec. Each tree (`SyntaxTree`, `DataTree`,
/// `SqlTree`, ...) plugs into the same render pipeline by supplying
/// one of these. The render functions below thread the `tree_name`
/// through every template so the generated code references the right
/// enum.
pub struct TreeSpec {
    /// Identifier of the enum in the source file (e.g. `"SyntaxTree"`).
    /// Drives every `match`-arm prefix in the generated code.
    pub tree_name: &'static str,
    /// Source file holding the enum declaration, relative to `tractor/`.
    pub input_path: &'static str,
    /// Output path for the generated metadata file, relative to `tractor/`.
    pub output_path: &'static str,
    /// File header (doc comment + imports). The header is verbatim
    /// per tree because the import set varies (`SyntaxTree` pulls in
    /// `AccessReceiver` / `Expression` / etc.; `DataTree` doesn't).
    pub header: &'static str,
    /// Generate the `from_json` deserializer? Only `SyntaxTree`
    /// needs it today; data trees are produced from CST, not from
    /// JSON-as-input.
    pub generate_from_json: bool,
    /// Generate `span_mut_of` + `children_mut_of`? Used by
    /// `assign_ids` and editable-tree mutations.
    pub generate_mut: bool,
}

const SYNTAX_TREE_SPEC: TreeSpec = TreeSpec {
    tree_name: "SyntaxTree",
    input_path: "src/tree/syntax/types.rs",
    output_path: "src/tree/syntax/metadata.generated.rs",
    header: SYNTAX_HEADER,
    generate_from_json: true,
    generate_mut: true,
};

const DATA_TREE_SPEC: TreeSpec = TreeSpec {
    tree_name: "DataTree",
    input_path: "src/tree/data/types.rs",
    output_path: "src/tree/data/metadata.generated.rs",
    header: DATA_HEADER,
    generate_from_json: false,
    generate_mut: true,
};

const SQL_TREE_SPEC: TreeSpec = TreeSpec {
    tree_name: "SqlTree",
    input_path: "src/tree/sql/types.rs",
    output_path: "src/tree/sql/metadata.generated.rs",
    header: SQL_HEADER,
    generate_from_json: false,
    generate_mut: true,
};

/// Run the codegen. Called from `build.rs::main`.
pub fn generate() {
    println!("cargo:rerun-if-changed=build_codegen.rs");
    generate_for_tree(&SYNTAX_TREE_SPEC);
    generate_for_tree(&DATA_TREE_SPEC);
    generate_for_tree(&SQL_TREE_SPEC);
}

fn generate_for_tree(spec: &TreeSpec) {
    println!("cargo:rerun-if-changed={}", spec.input_path);

    let src = fs::read_to_string(spec.input_path)
        .unwrap_or_else(|e| panic!("reading {}: {}", spec.input_path, e));
    let file: syn::File = syn::parse_file(&src)
        .unwrap_or_else(|e| panic!("parsing {}: {}", spec.input_path, e));
    let enum_item = find_enum(&file, spec.tree_name).unwrap_or_else(|| {
        panic!("{} enum not found in {}", spec.tree_name, spec.input_path)
    });

    let tree = spec.tree_name;
    let mut out = String::new();
    out.push_str(spec.header);
    out.push_str(&render_element_name_of(enum_item, tree));
    out.push('\n');
    out.push_str(&render_flags_of(enum_item, tree));
    out.push('\n');
    out.push_str(&render_range_of(enum_item, tree));
    out.push('\n');
    out.push_str(&render_span_of(enum_item, tree));
    out.push('\n');
    out.push_str(&render_scalar_text_of(enum_item, tree));
    out.push('\n');
    out.push_str(&render_children_of(enum_item, tree));
    out.push('\n');
    out.push_str(&render_fields_of(enum_item, tree));
    out.push('\n');
    out.push_str(&render_use_field_projection(enum_item, tree));
    if spec.generate_mut {
        out.push('\n');
        out.push_str(&render_span_mut_of(enum_item, tree));
        out.push('\n');
        out.push_str(&render_children_mut_of(enum_item, tree));
    }
    if spec.generate_from_json {
        out.push('\n');
        out.push_str(&render_from_json(enum_item));
    }

    write_if_changed(spec.output_path, &out);
}

const SYNTAX_HEADER: &str = "\
// DO NOT EDIT — emitted by `tractor/build.rs` on every build.
// Source: SyntaxTree enum in tractor/src/tree/syntax/types.rs.
//
// Variant-blind reflection metadata that drives the XML and JSON
// renderers' mechanical walks (`to_xot.rs`, `to_json.rs`, `from_json.rs`).
// Rules are derived from field types only — no per-variant special
// cases. See `tractor/build_codegen.rs`.

#![cfg(feature = \"native\")]
#![allow(clippy::too_many_lines)]

#[allow(unused_imports)]
use super::types::{
    Access, AccessReceiver, AccessorKind, AccessSegment, ByteRange,
    Expression, Flag, LambdaBody, Marker, Modifiers, OperatorKind, ParamKind,
    QuoteStyle, SlotKind, Span, SyntaxTree,
};

#[allow(unused_imports)]
use crate::tree::walker::TreeField;

#[allow(unused_imports)]
use serde_json::Value;

// Per-variant element-name overrides declared via
// `@element_name = <fn>` on the variant's doc comment in `types.rs`.
#[allow(unused_imports)]
use super::types::{
    element_name_for_accessor, element_name_for_atom,
    element_name_for_field_wrap, element_name_for_generic_type,
    element_name_for_object_access, element_name_for_raw,
    element_name_for_simple_statement, element_name_for_slot,
    element_name_for_type_parameter,
};

";

const DATA_HEADER: &str = "\
// DO NOT EDIT — emitted by `tractor/build.rs` on every build.
// Source: DataTree enum in tractor/src/tree/data/types.rs.
//
// Variant-blind reflection metadata for the data-language tree
// (JSON / YAML / TOML / INI / Markdown / ...). Mirrors the
// `tree/syntax/metadata.generated.rs` shape one-for-one — same
// codegen functions in `tractor/build_codegen.rs`, same accessor
// signatures (modulo `from_json` which is `SyntaxTree`-only).

#![cfg(feature = \"native\")]
#![allow(clippy::too_many_lines)]

#[allow(unused_imports)]
use super::types::{DataTree, element_name_for_data_element};
#[allow(unused_imports)]
use crate::tree::types::{ByteRange, Flag, Marker, Span};
#[allow(unused_imports)]
use crate::tree::walker::TreeField;

";

const SQL_HEADER: &str = "\
// DO NOT EDIT — emitted by `tractor/build.rs` on every build.
// Source: SqlTree enum in tractor/src/tree/sql/types.rs.
//
// Variant-blind reflection metadata for the T-SQL tree. Mirrors the
// `tree/syntax/metadata.generated.rs` shape — same codegen functions
// in `tractor/build_codegen.rs`, same accessor signatures (modulo
// `from_json` which is `SyntaxTree`-only).

#![cfg(feature = \"native\")]
#![allow(clippy::too_many_lines)]

#[allow(unused_imports)]
use super::types::SqlTree;
#[allow(unused_imports)]
use crate::tree::types::{ByteRange, Marker, Span};
#[allow(unused_imports)]
use crate::tree::walker::TreeField;

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

fn render_element_name_of(en: &ItemEnum, tree: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// The XML element name for this tree node, or `None` if the
/// node renders no wrapper (`Inline`, `Skip`).
///
/// Borrowed lifetime: most arms return `&'static str` literals or
/// `&'static str` field values, but `Unknown` / `Raw` carry `String`
/// kinds so the return type ties to the tree.
pub fn element_name_of(tree: &{tree}) -> Option<&str> {{
    match tree {{
",
        tree = tree,
    ));
    for v in &en.variants {
        let name = v.ident.to_string();
        out.push_str(&element_arm(&name, v, tree));
    }
    out.push_str(
        "    }
}
",
    );
    out
}

/// Build one `element_name_of` match arm for `v`.
///
/// Rules (highest priority first):
/// 1. **`Inline` / `Skip`** — return `None` (no wrapper element).
/// 2. **Explicit annotation** — if the variant's doc comment
///    contains `@element_name = <fn_ident>`, emit a call to that
///    hand-written function. Knowledge stays local to the variant.
///    Use this for any variant whose element name isn't a literal.
/// 3. **Default** — `snake_case` of the variant identifier.
///
/// Note: there is **no** magic "field named `kind` is a
/// discriminator" rule. Variants that want to derive the element
/// name from a typed field do so by declaring `@element_name = X`
/// and writing the corresponding `fn X(t: &SyntaxTree) -> &'static str`
/// helper next to the variant in `types.rs`.
fn element_arm(name: &str, v: &Variant, tree: &str) -> String {
    if name == "Inline" || name == "Skip" {
        return format!("        {tree}::{name} {{ .. }} => None,\n");
    }
    if let Some(fn_ident) = element_name_annotation(v) {
        return format!(
            "        {tree}::{name} {{ .. }} => Some({fn_ident}(tree)),\n",
        );
    }
    format!(
        "        {tree}::{name} {{ .. }} => Some({lit:?}),\n",
        lit = snake_case(name),
    )
}

/// Scan a variant's doc comments (the `#[doc = "..."]` attributes
/// that `///` rustdoc syntax expands to) for an `@element_name = X`
/// line. Returns the identifier of the override function when present.
fn element_name_annotation(v: &Variant) -> Option<String> {
    for attr in &v.attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        let syn::Meta::NameValue(nv) = &attr.meta else { continue };
        let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &nv.value else { continue };
        let line = s.value();
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("@element_name") {
            let rest = rest.trim_start_matches(|c: char| c == ' ' || c == '=');
            let ident: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !ident.is_empty() {
                return Some(ident);
            }
        }
    }
    None
}

fn render_flags_of(en: &ItemEnum, tree: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Empty-element marker children for this tree node. Drawn from
/// `Modifiers::markers_with_spans()`, any `Flag` field (named after
/// the field with trailing `_` stripped), `Vec<Marker>` and
/// `Vec<&'static str>` marker-name fields.
///
/// Each entry's `span` lets the XML renderer emit
/// `line` / `column` attributes at the keyword position when the flag
/// is anchored, falling back to a default for implicit flags.
pub fn flags_of(tree: &{tree}) -> Vec<Marker> {{
    let mut out: Vec<Marker> = Vec::new();
    match tree {{
",
        tree = tree,
    ));
    for v in &en.variants {
        out.push_str(&flags_arm(v, tree));
    }
    out.push_str(
        "    }
    out
}
",
    );
    out
}

fn flags_arm(v: &Variant, tree: &str) -> String {
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
                "Vec<&'staticstr>" if fname == "markers" => {
                    // `DataTree::Element.markers: Vec<&'static str>` —
                    // open-set markers stored as bare names. Span
                    // defaults to the host's span; range stays
                    // synthetic since the marker has no source token.
                    bindings.push(fname.clone());
                    needs_span = true;
                    body.push_str(&format!(
                        "            for m in {} {{
                out.push(Marker {{
                    name: m,
                    range: ByteRange::synthetic_empty(),
                    span: *span,
                }});
            }}
",
                        fname
                    ));
                }
                "Option<&'staticstr>" if fname == "marker" => {
                    // `Expression.marker: Option<&'static str>` carries a
                    // single optional marker name on the host element
                    // (`non_null`, `ref`, etc. — Principle #15). The
                    // synthetic span is the host's own span; the
                    // walker's positional sort then keeps the marker
                    // first in the element body (range_start = 0).
                    bindings.push(fname.clone());
                    needs_span = true;
                    body.push_str(&format!(
                        "            if let Some(name) = {} {{
                out.push(Marker {{
                    name,
                    range: ByteRange::synthetic_empty(),
                    span: *span,
                }});
            }}
",
                        fname
                    ));
                }
                _ => {}
            }
        }
    }

    if bindings.is_empty() {
        return format!("        {tree}::{name} {{ .. }} => {{}}\n");
    }

    if needs_span {
        bindings.push("span".to_string());
    }

    let binding_list = bindings.join(", ");
    format!(
        "        {tree}::{name} {{ {binding_list}, .. }} => {{
{body}
        }}
"
    )
}

fn render_range_of(en: &ItemEnum, tree: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Source byte range of this node. Used for verbatim-source
/// recovery (`source[range]`) and for gap-text computation in the
/// renderer. Every variant carries a `range: ByteRange` field.
pub fn range_of(tree: &{tree}) -> ByteRange {{
    match tree {{
",
        tree = tree,
    ));
    for v in &en.variants {
        out.push_str(&format!(
            "        {tree}::{name} {{ range, .. }} => *range,\n",
            name = v.ident,
        ));
    }
    out.push_str(
        "    }
}
",
    );
    out
}

fn render_span_of(en: &ItemEnum, tree: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Source-location span of this node. Used for XML attribute
/// emission (`line` / `column` / `end_line` / `end_column` / `id`).
/// Every variant carries a `span: Span` field.
pub fn span_of(tree: &{tree}) -> Span {{
    match tree {{
",
        tree = tree,
    ));
    for v in &en.variants {
        out.push_str(&format!(
            "        {tree}::{name} {{ span, .. }} => *span,\n",
            name = v.ident,
        ));
    }
    out.push_str(
        "    }
}
",
    );
    out
}

fn render_scalar_text_of(en: &ItemEnum, tree: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Stored text for scalar-leaf variants (`Name`, `Atom`, `Int`,
/// `Float`, `String`, `True`, `False`, `None`, `Null`). Returns
/// `None` for compound variants. The renderer uses this to emit
/// leaf literals without consulting the source string (S13-Z1).
pub fn scalar_text_of(tree: &{tree}) -> Option<&str> {{
    match tree {{
",
        tree = tree,
    ));
    for v in &en.variants {
        // A variant counts as a scalar-text carrier when it has a
        // `text: String` field. Detected mechanically; no whitelist.
        // DataTree::String stores the unquoted content under `value`
        // — handle that synonym too.
        if let Some(text_field) = find_field(v, "text")
            .filter(|f| type_str(&f.ty) == "String")
        {
            let fname = text_field.ident.as_ref().unwrap().to_string();
            out.push_str(&format!(
                "        {tree}::{name} {{ {fname}, .. }} => Some({fname}.as_str()),\n",
                name = v.ident,
            ));
        } else if let Some(value_field) = find_field(v, "value")
            .filter(|f| type_str(&f.ty) == "String")
        {
            let fname = value_field.ident.as_ref().unwrap().to_string();
            out.push_str(&format!(
                "        {tree}::{name} {{ {fname}, .. }} => Some({fname}.as_str()),\n",
                name = v.ident,
            ));
        }
    }
    out.push_str(
        "        _ => None,
    }
}
",
    );
    out
}

/// Generate `children_of`: walks each variant's fields and produces a
/// source-sorted `Vec<&SyntaxTree>` of every reachable sub-tree.
///
/// Field-type rules:
///  - `Box<SyntaxTree>`              → push.
///  - `Option<Box<SyntaxTree>>`      → if let Some, push.
///  - `Vec<SyntaxTree>`              → extend.
///  - `Expression`                   → push the inner SyntaxTree.
///  - `Option<Expression>`           → if let Some, push inner.
///  - `LambdaBody`                   → push `.inner()`.
///  - `AccessReceiver`               → match Instance, push.
///  - `Vec<AccessSegment>`           → iterate, unpack Index/Call.
///
/// Other field types (markers, modifiers, ranges, strings, …) are
/// ignored — they're shape metadata, not tree children. Output is
/// sorted by `range.start` so consumers don't have to repeat it.
fn render_children_of(en: &ItemEnum, tree: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Direct tree children of this node, in source order. Excludes
/// synthetic render-time wrappers, modifier markers, and other shape
/// metadata. Used by every variant-blind walker (`to_xot.rs`,
/// `to_json.rs`, …) as the single source of truth for tree traversal.
pub fn children_of(tree: &{tree}) -> Vec<&{tree}> {{
    let mut v: Vec<&{tree}> = Vec::new();
    match tree {{
",
        tree = tree,
    ));
    for variant in &en.variants {
        out.push_str(&children_arm(variant, tree));
    }
    out.push_str(
        "    }
    v.sort_by_key(|c| range_of(c).start);
    v
}
",
    );
    out
}

fn children_arm(v: &Variant, tree: &str) -> String {
    let name = v.ident.to_string();
    let mut bindings: Vec<String> = Vec::new();
    let mut body = String::new();
    let box_t = format!("Box<{}>", tree);
    let opt_box_t = format!("Option<Box<{}>>", tree);
    let vec_t = format!("Vec<{}>", tree);

    if let Fields::Named(named) = &v.fields {
        for field in &named.named {
            let Some(ident) = &field.ident else { continue };
            let fname = ident.to_string();
            let ty = type_str(&field.ty);
            if ty == box_t {
                bindings.push(fname.clone());
                body.push_str(&format!("            v.push({});\n", fname));
            } else if ty == opt_box_t {
                bindings.push(fname.clone());
                body.push_str(&format!(
                    "            if let Some(__t) = {} {{ v.push(__t); }}\n",
                    fname
                ));
            } else if ty == vec_t {
                bindings.push(fname.clone());
                body.push_str(&format!(
                    "            v.extend({}.iter());\n",
                    fname
                ));
            } else {
                match ty.as_str() {
                    "Expression" => {
                        bindings.push(fname.clone());
                        body.push_str(&format!(
                            "            v.push(&{}.inner);\n",
                            fname
                        ));
                    }
                    "Option<Expression>" => {
                        bindings.push(fname.clone());
                        body.push_str(&format!(
                            "            if let Some(__e) = {} {{ v.push(&__e.inner); }}\n",
                            fname
                        ));
                    }
                    "LambdaBody" => {
                        bindings.push(fname.clone());
                        body.push_str(&format!(
                            "            v.push({}.inner());\n",
                            fname
                        ));
                    }
                    "AccessReceiver" => {
                        bindings.push(fname.clone());
                        body.push_str(&format!(
                            "            if let AccessReceiver::Instance(__t) = {} {{ v.push(__t); }}\n",
                            fname
                        ));
                    }
                    "Vec<AccessSegment>" => {
                        bindings.push(fname.clone());
                        body.push_str(&format!(
                            "            for __s in {} {{
                match __s {{
                    AccessSegment::Member {{ .. }} => {{}}
                    AccessSegment::Index {{ indices, .. }} => v.extend(indices.iter()),
                    AccessSegment::Call {{ arguments, .. }} => v.extend(arguments.iter()),
                }}
            }}
",
                            fname
                        ));
                    }
                    _ => {}
                }
            }
        }
    }

    if bindings.is_empty() {
        return format!("        {tree}::{name} {{ .. }} => {{}}\n");
    }

    let binding_list = bindings.join(", ");
    format!(
        "        {tree}::{name} {{ {binding_list}, .. }} => {{
{body}        }}
"
    )
}

/// Generate `fields_of`: walks each variant's fields and produces a
/// `Vec<TreeField>` projecting the struct fields directly. Consumed
/// by the walker when [`use_field_projection`] returns true.
///
/// Field-type rules (mirror `children_of` + `flags_of`):
///  - `Box<SyntaxTree>`              → `TreeField::Single`
///  - `Option<Box<SyntaxTree>>`      → `Single` if Some, else skipped
///  - `Vec<SyntaxTree>`              → `TreeField::Many`
///  - `Expression`                   → `Single` (`.inner`)
///  - `Option<Expression>`           → `Single` if Some, else skipped
///  - `LambdaBody`                   → `Single` (`.inner()`)
///  - `AccessReceiver`               → `Single` when `Instance`
///  - `Flag` (On)                    → `TreeField::Flag`
///  - `Modifiers`                    → one `Flag` per `markers_with_spans()` entry
///  - `Vec<Marker>`                  → one `Flag` per marker
///  - `Vec<&'static str>` `markers`  → one `Flag` per marker name
///  - `Option<&'static str>` `marker`→ `Flag` if Some
///
/// Other field types (range, span, scalar strings, kind discriminators)
/// are ignored — they're shape metadata, not projected children.
fn render_fields_of(en: &ItemEnum, tree: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Field-projection view of this node — one [`TreeField`] per
/// projected struct field. Only consulted when [`use_field_projection`]
/// returns true. See the walker for projection semantics.
pub fn fields_of(tree: &{tree}) -> Vec<TreeField<'_, {tree}>> {{
    let mut out: Vec<TreeField<'_, {tree}>> = Vec::new();
    match tree {{
",
        tree = tree,
    ));
    for variant in &en.variants {
        out.push_str(&fields_arm(variant, tree));
    }
    out.push_str(
        "    }
    out
}
",
    );
    out
}

fn fields_arm(v: &Variant, tree: &str) -> String {
    let name = v.ident.to_string();
    let mut bindings: Vec<String> = Vec::new();
    let mut body = String::new();
    let mut needs_span = false;
    let box_t = format!("Box<{}>", tree);
    let opt_box_t = format!("Option<Box<{}>>", tree);
    let vec_t = format!("Vec<{}>", tree);

    if let Fields::Named(named) = &v.fields {
        for field in &named.named {
            let Some(ident) = &field.ident else { continue };
            let fname = ident.to_string();
            // Projected field name strips Rust-specific noise
            // (`type_ann` → `type`, `else_` → `else`, etc.). The Rust
            // binding stays the original `fname`; only the user-facing
            // XML element / JSON key uses the projected form.
            let pname = field_name_to_json_key(&fname);
            let ty = type_str(&field.ty);
            if ty == box_t {
                bindings.push(fname.clone());
                body.push_str(&format!(
                    "            out.push(TreeField::Single {{ name: {pname:?}, value: {fname} }});\n",
                ));
            } else if ty == opt_box_t {
                bindings.push(fname.clone());
                body.push_str(&format!(
                    "            if let Some(__t) = {fname} {{ out.push(TreeField::Single {{ name: {pname:?}, value: __t }}); }}\n",
                ));
            } else if ty == vec_t {
                bindings.push(fname.clone());
                body.push_str(&format!(
                    "            out.push(TreeField::Many {{ name: {pname:?}, items: {fname}.iter().collect() }});\n",
                ));
            } else {
                match ty.as_str() {
                    "Expression" => {
                        bindings.push(fname.clone());
                        body.push_str(&format!(
                            "            out.push(TreeField::Single {{ name: {pname:?}, value: &{fname}.inner }});\n",
                        ));
                    }
                    "Option<Expression>" => {
                        bindings.push(fname.clone());
                        body.push_str(&format!(
                            "            if let Some(__e) = {fname} {{ out.push(TreeField::Single {{ name: {pname:?}, value: &__e.inner }}); }}\n",
                        ));
                    }
                    "LambdaBody" => {
                        bindings.push(fname.clone());
                        body.push_str(&format!(
                            "            out.push(TreeField::Single {{ name: {pname:?}, value: {fname}.inner() }});\n",
                        ));
                    }
                    "AccessReceiver" => {
                        bindings.push(fname.clone());
                        body.push_str(&format!(
                            "            if let AccessReceiver::Instance(__t) = {fname} {{ out.push(TreeField::Single {{ name: {pname:?}, value: __t }}); }}\n",
                        ));
                    }
                    "Modifiers" => {
                        bindings.push(fname.clone());
                        needs_span = true;
                        body.push_str(&format!(
                            "            for (mname, mspan) in {fname}.markers_with_spans() {{
                out.push(TreeField::Flag {{
                    name: mname,
                    marker: Marker {{
                        name: mname,
                        range: ByteRange::synthetic_empty(),
                        span: mspan.unwrap_or(*span),
                    }},
                }});
            }}
",
                        ));
                    }
                    "Flag" => {
                        bindings.push(fname.clone());
                        let marker_name = strip_trailing_underscore(&fname);
                        body.push_str(&format!(
                            "            if let Flag::On {{ range: frange, span: fspan }} = {fname} {{
                out.push(TreeField::Flag {{
                    name: {marker_name:?},
                    marker: Marker {{ name: {marker_name:?}, range: *frange, span: *fspan }},
                }});
            }}
",
                        ));
                    }
                    "Vec<Marker>" => {
                        bindings.push(fname.clone());
                        body.push_str(&format!(
                            "            for m in {fname} {{ out.push(TreeField::Flag {{ name: m.name, marker: *m }}); }}\n",
                        ));
                    }
                    "Vec<&'staticstr>" if fname == "markers" => {
                        bindings.push(fname.clone());
                        needs_span = true;
                        body.push_str(&format!(
                            "            for m in {fname} {{
                out.push(TreeField::Flag {{
                    name: m,
                    marker: Marker {{ name: m, range: ByteRange::synthetic_empty(), span: *span }},
                }});
            }}
",
                        ));
                    }
                    "Option<&'staticstr>" if fname == "marker" => {
                        bindings.push(fname.clone());
                        needs_span = true;
                        body.push_str(&format!(
                            "            if let Some(name) = {fname} {{
                out.push(TreeField::Flag {{
                    name,
                    marker: Marker {{ name, range: ByteRange::synthetic_empty(), span: *span }},
                }});
            }}
",
                        ));
                    }
                    "OperatorKind" => {
                        // Closed-enum kind discriminator: project as
                        // a Flag whose name comes from the enum's
                        // marker_name() method. Source range is
                        // synthetic; span derives from the host.
                        bindings.push(fname.clone());
                        needs_span = true;
                        body.push_str(&format!(
                            "            out.push(TreeField::Flag {{
                name: {fname}.marker_name(),
                marker: Marker {{ name: {fname}.marker_name(), range: ByteRange::synthetic_empty(), span: *span }},
            }});
",
                        ));
                    }
                    "String" => {
                        // Inline scalar text — emit as Scalar (JSON
                        // only; XML gap-text covers source-anchored
                        // mode). Limited to variants opting in via
                        // @field_projection; non-opted variants emit
                        // text via the variant-blind scalar_text_of
                        // path instead.
                        bindings.push(fname.clone());
                        body.push_str(&format!(
                            "            out.push(TreeField::Scalar {{ name: {pname:?}, value: {fname}.as_str() }});\n",
                        ));
                    }
                    _ => {}
                }
            }
        }
    }

    if bindings.is_empty() {
        return format!("        {tree}::{name} {{ .. }} => {{}}\n");
    }

    if needs_span && !bindings.iter().any(|b| b == "span") {
        bindings.push("span".to_string());
    }

    let binding_list = bindings.join(", ");
    format!(
        "        {tree}::{name} {{ {binding_list}, .. }} => {{
{body}        }}
"
    )
}

/// Generate `use_field_projection`: per-variant opt-in flag for the
/// field-projection walker path. Variants without the
/// `@field_projection` annotation default to `false` (legacy
/// variant-blind path).
fn render_use_field_projection(en: &ItemEnum, tree: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Per-variant opt-in flag for the field-projection walker
/// path. Returns true for variants whose doc comments carry
/// `@field_projection`; false otherwise.
pub fn use_field_projection(tree: &{tree}) -> bool {{
    match tree {{
",
        tree = tree,
    ));
    let mut any_true = false;
    for v in &en.variants {
        if has_field_projection_annotation(v) {
            any_true = true;
            out.push_str(&format!(
                "        {tree}::{name} {{ .. }} => true,\n",
                name = v.ident,
            ));
        }
    }
    if any_true {
        out.push_str("        _ => false,\n");
    } else {
        out.push_str("        _ => false,\n");
    }
    out.push_str(
        "    }
}
",
    );
    out
}

/// Scan a variant's doc comments for a bare `@field_projection` line.
/// Marks the variant as participating in the field-projection walker
/// path (per-variant opt-in during Phase C migration).
fn has_field_projection_annotation(v: &Variant) -> bool {
    for attr in &v.attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        let syn::Meta::NameValue(nv) = &attr.meta else { continue };
        let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &nv.value else { continue };
        let line = s.value();
        let trimmed = line.trim();
        if trimmed == "@field_projection" || trimmed.starts_with("@field_projection ") {
            return true;
        }
    }
    false
}

fn render_span_mut_of(en: &ItemEnum, tree: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Mutable access to the source-location span. Used by
/// `assign_ids` to stamp `NodeId`s into existing spans without
/// rebuilding nodes. Delegates from `TreeNode::span_mut`.
pub fn span_mut_of(tree: &mut {tree}) -> &mut Span {{
    match tree {{
",
        tree = tree,
    ));
    for v in &en.variants {
        out.push_str(&format!(
            "        {tree}::{name} {{ span, .. }} => span,\n",
            name = v.ident,
        ));
    }
    out.push_str(
        "    }
}
",
    );
    out
}

/// Mutable mirror of `children_of`. Same field-type rules as the
/// immutable version, but produces `Vec<&mut SyntaxTree>` so callers
/// can mutate sub-trees in place (used by editable-trees mutations).
fn render_children_mut_of(en: &ItemEnum, tree: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Mutable mirror of `children_of`. Source-sorted
/// `Vec<&mut {tree}>` covering every reachable sub-tree.
pub fn children_mut_of(tree: &mut {tree}) -> Vec<&mut {tree}> {{
    let mut v: Vec<&mut {tree}> = Vec::new();
    match tree {{
",
        tree = tree,
    ));
    for variant in &en.variants {
        out.push_str(&children_mut_arm(variant, tree));
    }
    out.push_str(
        "    }
    v.sort_by_key(|c| range_of(c).start);
    v
}
",
    );
    out
}

fn children_mut_arm(v: &Variant, tree: &str) -> String {
    let name = v.ident.to_string();
    let mut bindings: Vec<String> = Vec::new();
    let mut body = String::new();
    let box_t = format!("Box<{}>", tree);
    let opt_box_t = format!("Option<Box<{}>>", tree);
    let vec_t = format!("Vec<{}>", tree);

    if let Fields::Named(named) = &v.fields {
        for field in &named.named {
            let Some(ident) = &field.ident else { continue };
            let fname = ident.to_string();
            let ty = type_str(&field.ty);
            if ty == box_t {
                bindings.push(fname.clone());
                body.push_str(&format!(
                    "            v.push({}.as_mut());\n",
                    fname
                ));
            } else if ty == opt_box_t {
                bindings.push(fname.clone());
                body.push_str(&format!(
                    "            if let Some(__t) = {} {{ v.push(__t.as_mut()); }}\n",
                    fname
                ));
            } else if ty == vec_t {
                bindings.push(fname.clone());
                body.push_str(&format!(
                    "            v.extend({}.iter_mut());\n",
                    fname
                ));
            } else { match ty.as_str() {
                "Expression" => {
                    bindings.push(fname.clone());
                    body.push_str(&format!(
                        "            v.push(&mut {}.inner);\n",
                        fname
                    ));
                }
                "Option<Expression>" => {
                    bindings.push(fname.clone());
                    body.push_str(&format!(
                        "            if let Some(__e) = {} {{ v.push(&mut __e.inner); }}\n",
                        fname
                    ));
                }
                "LambdaBody" => {
                    bindings.push(fname.clone());
                    body.push_str(&format!(
                        "            v.push({}.inner_mut());\n",
                        fname
                    ));
                }
                "AccessReceiver" => {
                    bindings.push(fname.clone());
                    body.push_str(&format!(
                        "            if let AccessReceiver::Instance(__t) = {} {{ v.push(__t.as_mut()); }}\n",
                        fname
                    ));
                }
                "Vec<AccessSegment>" => {
                    bindings.push(fname.clone());
                    body.push_str(&format!(
                        "            for __s in {}.iter_mut() {{
                match __s {{
                    AccessSegment::Member {{ .. }} => {{}}
                    AccessSegment::Index {{ indices, .. }} => v.extend(indices.iter_mut()),
                    AccessSegment::Call {{ arguments, .. }} => v.extend(arguments.iter_mut()),
                }}
            }}
",
                        fname
                    ));
                }
                _ => {}
            } }
        }
    }

    if bindings.is_empty() {
        return format!("        {tree}::{name} {{ .. }} => {{}}\n");
    }

    let binding_list = bindings.join(", ");
    format!(
        "        {tree}::{name} {{ {binding_list}, .. }} => {{
{body}        }}
"
    )
}

/// Generate `tree_from_json`: best-effort reverse of [`to_json`].
///
/// Strategy: dispatch on JSON shape, then on `$type` for objects:
/// - **Scalar JSON value** (string / number / bool / null) →
///   reconstruct the matching scalar-leaf variant via [`leaf_from_json`].
/// - **Array** → `SyntaxTree::Inline` carrying each element as a
///   child (mirrors how the forward walker emits inline children).
/// - **Object with `$type`** → look up the variant by tag, populate
///   fields type-by-type from JSON keys.
/// - **Object without `$type`** → `SyntaxTree::Inline` over the
///   non-meta values, in stable key order.
///
/// Field-population rules per variant (no per-variant special cases):
/// - `Box<SyntaxTree>` / `Option<Box<SyntaxTree>>` / `Vec<SyntaxTree>`
///   → drained positionally from the variant's collected tree children
///   (non-meta object values + `$children`).
/// - `Expression` / `Option<Expression>` → wrap drained child.
/// - `LambdaBody` → wrap drained child as `LambdaBody::Expression`
///   (a conservative default; round-trip Lambda may need
///   re-classification at use site).
/// - `AccessReceiver` → drained child as `Instance`; keyword
///   receivers (`this`, `self`, ...) round-trip via the same name
///   leaves.
/// - `Vec<AccessSegment>` → empty (no JSON encoding inverse).
/// - `text: String`, `op_text: String`, etc. → take the matching
///   JSON key as a string, falling back to the empty string.
/// - `Flag` field → JSON bool at `strip_trailing_underscore(field_name)`.
/// - `Modifiers` → reconstructed from all truthy JSON-bool keys via
///   `Modifiers::from_marker_names`.
/// - `Vec<Marker>` → empty.
/// - `Vec<&'static str>` → empty.
/// - `Option<&'static str>` / `Option<ByteRange>` / `Option<Span>` → `None`.
/// - `&'static str` (`element_name`, `kind`, `wrapper`) → derived
///   from the `$type` field via `Box::leak`. Acceptable for a
///   deserializer.
/// - `String` (other than `text`/`op_text`) → empty.
/// - `bool` → JSON bool at the matching field name; default `false`.
/// - Typed enums (`AccessorKind`, `ParamKind`, `QuoteStyle`,
///   `Option<Access>`) → parsed from a matching string field with
///   a sensible default.
/// - `ByteRange` / `Span` → synthetic (no source coordinates after
///   the JSON hop).
fn render_from_json(en: &ItemEnum) -> String {
    let mut out = String::new();
    out.push_str(FROM_JSON_PREAMBLE);

    // Build $type → arm body for each variant. Skip Inline and Skip
    // (they have no $type; reached via the inline fallback).
    let mut variants: Vec<(&Variant, String)> = Vec::new();
    for v in &en.variants {
        let name = v.ident.to_string();
        if name == "Inline" || name == "Skip" {
            continue;
        }
        variants.push((v, snake_case(&name)));
    }

    out.push_str(
        "fn dispatch_from_json_object(map: &serde_json::Map<String, Value>, tag: &str) -> SyntaxTree {
    // Drain non-meta values into a flat list of children, in stable
    // key order. Each child carries the JSON key as a type hint so
    // nested objects with stripped `$type` reconstruct under the
    // correct variant. Booleans become marker names; numbers/null
    // are converted via the value-level dispatcher.
    let mut children: Vec<SyntaxTree> = Vec::new();
    let mut markers: Vec<&'static str> = Vec::new();
    for (key, val) in map.iter() {
        if key == \"$type\" { continue; }
        if key == \"$children\" {
            if let Value::Array(arr) = val {
                for item in arr { children.push(tree_from_json_value(item)); }
            }
            continue;
        }
        match val {
            Value::Bool(true) => markers.push(intern_static(key)),
            Value::Bool(false) => {}
            Value::Array(arr) => {
                for item in arr {
                    children.push(tree_from_json_with_type_hint(item, Some(key.as_str())));
                }
            }
            _ => children.push(tree_from_json_with_type_hint(val, Some(key.as_str()))),
        }
    }
    let marker_strs: Vec<&str> = markers.iter().copied().collect();
    let _ = &marker_strs;
    let leaf_text = map.get(\"text\").and_then(|v| v.as_str()).unwrap_or(\"\").to_string();
    let _ = &leaf_text;
    let static_tag: &'static str = intern_static(tag);
    let _ = static_tag;
    match tag {
",
    );

    for (v, tag) in &variants {
        out.push_str(&from_json_arm(v, tag));
    }

    // Unknown $type fallback:
    //   - If the tag looks like a valid XML/Rust identifier (likely
    //     an open-set SimpleStatement slot name: `type`, `returns`,
    //     `left`, `right`, `where`, ...) → reconstruct as
    //     SimpleStatement with that element_name. Inverts the
    //     stripped-`$type` shape `to_json` produces for slot
    //     wrappers.
    //   - Otherwise (the tag contains hyphens, spaces, or otherwise
    //     can't be a slot name) → SyntaxTree::Unknown carrying the
    //     literal tag in `kind`. Flags typos and genuinely unknown
    //     shapes.
    out.push_str(
        "        _ => {
            if tag_looks_like_slot_name(tag) {
                SyntaxTree::SimpleStatement {
                    element_name: static_tag,
                    modifiers: Modifiers::from_marker_names(&marker_strs),
                    extra_markers: Vec::new(),
                    children,
                    range: ByteRange::synthetic_empty(),
                    span: Span::point(0, 0),
                }
            } else {
                SyntaxTree::Unknown {
                    kind: format!(\"from_json:{}\", tag),
                    range: ByteRange::synthetic_empty(),
                    span: Span::point(0, 0),
                }
            }
        }
    }
}

/// True iff `tag` looks like a valid SimpleStatement-style slot
/// name: starts with `[a-z_]`, continues with `[a-z0-9_]`. Avoids
/// silently rewriting genuine typos (`not-a-real-variant`) into
/// SimpleStatement when they really should surface as Unknown.
fn tag_looks_like_slot_name(tag: &str) -> bool {
    let mut chars = tag.chars();
    let Some(first) = chars.next() else { return false };
    if !(first.is_ascii_lowercase() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}
",
    );

    out
}

const FROM_JSON_PREAMBLE: &str = "\
/// Reconstruct a `SyntaxTree` from its JSON projection.
///
/// Inverse of `tree_to_json` (modulo lossy bits — source positions,
/// the `Vec<AccessSegment>` chain shape, ordering of duplicate-named
/// children). The reconstructed tree is suitable for re-rendering
/// via `render_source` to produce parseable source code; bit-identity
/// is **not** preserved.
///
/// Generated mechanically from `SyntaxTree` field types — no
/// per-variant special cases. See `build_codegen.rs::render_from_json`.
pub fn tree_from_json(value: &Value) -> SyntaxTree {
    tree_from_json_value(value)
}

/// Leak `s` into the static string pool. Used for `&'static str`
/// fields (`element_name`, `kind`, `wrapper`) where the JSON carries
/// a runtime string. One-time leak per distinct tag is acceptable for
/// a deserializer; the alternative would be a static interner map.
fn intern_static(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

/// Strip a common plural suffix so a Vec<SyntaxTree> field named
/// `decorators` falls back to looking up `decorator` (the singular
/// element name). Heuristic — covers `…s`, `…es`, `…_clauses`,
/// `…_branches`. Returns the input unchanged when no rule fires.
fn strip_plural(s: &str) -> &str {
    for suffix in [\"_clauses\", \"_branches\"] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            // `where_clauses` → `where`; matches the typical
            // SimpleStatement element_name pinned at lowering time.
            return stripped;
        }
    }
    for suffix in [\"ies\", \"es\", \"s\"] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            if suffix == \"ies\" {
                // Heuristic — return the stripped form; caller still
                // owns the lookup-fallback chain so a miss here just
                // means we drain from unclaimed_children.
                return stripped;
            }
            return stripped;
        }
    }
    s
}

fn tree_from_json_value(value: &Value) -> SyntaxTree {
    tree_from_json_with_type_hint(value, None)
}

/// Reconstruct with an optional `$type` hint. Used when recursing
/// into a child looked up by JSON key: the key implies the child's
/// type, and `to_json` strips `$type` from such children to avoid
/// duplication. The inverse restores it here.
fn tree_from_json_with_type_hint(value: &Value, type_hint: Option<&str>) -> SyntaxTree {
    match value {
        Value::Null => SyntaxTree::Null {
            text: String::new(),
            range: ByteRange::synthetic_empty(),
            span: Span::point(0, 0),
        },
        Value::Bool(true) => SyntaxTree::True {
            text: \"true\".to_string(),
            range: ByteRange::synthetic_empty(),
            span: Span::point(0, 0),
        },
        Value::Bool(false) => SyntaxTree::False {
            text: \"false\".to_string(),
            range: ByteRange::synthetic_empty(),
            span: Span::point(0, 0),
        },
        Value::Number(n) => SyntaxTree::Int {
            text: n.to_string(),
            range: ByteRange::synthetic_empty(),
            span: Span::point(0, 0),
        },
        Value::String(s) => {
            // A bare string under a typed key (e.g. `\"name\": \"foo\"`)
            // is the scalar form of a leaf variant — pick the variant
            // by the hint so `name`, `int`, `string`, etc. all
            // reconstruct correctly. Without a hint it falls back to
            // `Name` (the most common leaf shape).
            scalar_leaf_from_text(type_hint.unwrap_or(\"name\"), s.clone())
        }
        Value::Array(arr) => {
            // Arrays under a typed key carry siblings of that type
            // (after the Z6 array-grouping fix). Each element inherits
            // the parent key as its type hint.
            let children: Vec<SyntaxTree> = arr
                .iter()
                .map(|v| tree_from_json_with_type_hint(v, type_hint))
                .collect();
            SyntaxTree::Inline {
                children,
                list_name: None,
                range: ByteRange::synthetic_empty(),
                span: Span::point(0, 0),
            }
        }
        Value::Object(map) => from_json_object_with_hint(map, type_hint),
    }
}

/// Construct the right scalar-leaf variant for a JSON string value,
/// keyed by the surrounding `$type` hint. Falls back to `Name` for
/// unknown hints — keeps the lossy inversion robust.
fn scalar_leaf_from_text(tag: &str, text: String) -> SyntaxTree {
    let range = ByteRange::synthetic_empty();
    let span = Span::point(0, 0);
    match tag {
        \"int\" => SyntaxTree::Int { text, range, span },
        \"float\" => SyntaxTree::Float { text, range, span },
        \"string\" => SyntaxTree::String {
            text,
            quote_style: QuoteStyle::Double,
            range,
            span,
        },
        \"true\" => SyntaxTree::True { text, range, span },
        \"false\" => SyntaxTree::False { text, range, span },
        \"none\" => SyntaxTree::None { text, range, span },
        \"null\" => SyntaxTree::Null { text, range, span },
        // Anything else: a bare string under an arbitrary key is most
        // likely an identifier-like leaf. Use Name so renderers see
        // text content without needing source bytes.
        _ => SyntaxTree::Name { text, range, span },
    }
}

fn from_json_object_with_hint(
    map: &serde_json::Map<String, Value>,
    type_hint: Option<&str>,
) -> SyntaxTree {
    // Resolve $type — prefer the explicit field; fall back to the
    // hint passed in by the parent context (key under which this
    // object was nested).
    let tag = map
        .get(\"$type\")
        .and_then(|v| v.as_str())
        .or(type_hint)
        .unwrap_or(\"\");
    dispatch_from_json_object(map, tag)
}

";

/// Build one `$type` arm: pop children positionally into typed slots,
/// fill scalars from JSON keys, default everything else.
///
/// `unclaimed_children` is the **overflow** list — map values whose JSON key
/// doesn't match any of this variant's fields (counting both the
/// raw Rust field name and the canonical JSON-key form from
/// [`field_name_to_json_key`]). Fields that find their value via
/// keyed lookup don't double-consume from `unclaimed_children`; fields that miss
/// fall back to draining `unclaimed_children` positionally.
fn from_json_arm(v: &Variant, tag: &str) -> String {
    let name = v.ident.to_string();
    let mut field_pops = String::new();
    let mut struct_fields: Vec<String> = Vec::new();
    let mut claimed_keys: Vec<String> = Vec::new();

    // Collect the JSON keys each field will claim (raw name + the
    // canonical form). The Vec → array literal is emitted below.
    if let Fields::Named(named) = &v.fields {
        for field in &named.named {
            let Some(ident) = &field.ident else { continue };
            let fname = ident.to_string();
            claimed_keys.push(fname.clone());
            let canonical = field_name_to_json_key(&fname);
            if canonical != fname {
                claimed_keys.push(canonical);
            }
            // Vec<SyntaxTree> fields also try a singular form
            // (parameters → parameter); claim that too.
            let ty_pre = type_str(&field.ty);
            if ty_pre == "Vec<SyntaxTree>" {
                let singular = strip_plural_str(&fname);
                if singular != fname && !claimed_keys.iter().any(|k| k == singular) {
                    claimed_keys.push(singular.to_string());
                }
            }
        }
    }

    // Rebuild unclaimed_children by walking the map once more and including only
    // entries whose key is NOT in the claimed set. The global
    // `children` (built by `dispatch_from_json_object`) is kept for
    // the SimpleStatement fallback path but isn't used per-arm.
    let claimed_lits: Vec<String> = claimed_keys.iter().map(|k| format!("{k:?}")).collect();
    let claimed_arr = claimed_lits.join(", ");
    field_pops.push_str(&format!(
        "            let claimed: &[&str] = &[{claimed_arr}];\n\
         \x20           let mut unclaimed_children: Vec<SyntaxTree> = map.iter()\n\
         \x20               .filter(|(k, _)| k.as_str() != \"$type\" && k.as_str() != \"$children\" && !claimed.contains(&k.as_str()))\n\
         \x20               .flat_map(|(k, v)| match v {{\n\
         \x20                   Value::Bool(_) => Vec::new(),\n\
         \x20                   Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),\n\
         \x20                   _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],\n\
         \x20               }})\n\
         \x20               .collect();\n\
         \x20           let _ = &children;\n",
    ));
    let _ = tag;
    if let Fields::Named(named) = &v.fields {
        for field in &named.named {
            let Some(ident) = &field.ident else { continue };
            let fname = ident.to_string();
            let ty = type_str(&field.ty);
            let (build_expr, _) = from_json_field_expr(&fname, &ty);
            field_pops.push_str(&format!(
                "            let {fname} = {build_expr};\n",
                fname = fname,
                build_expr = build_expr,
            ));
            struct_fields.push(fname);
        }
    }
    let assigns = struct_fields
        .iter()
        .map(|f| format!("                {f}"))
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        "        {tag:?} => {{
{field_pops}            SyntaxTree::{name} {{
{assigns},
            }}
        }}
",
        tag = tag,
        field_pops = field_pops,
        name = name,
        assigns = assigns,
    )
}

/// Per-field-type expression that produces the field value from the
/// in-scope `unclaimed_children: Vec<SyntaxTree>`, `marker_strs: Vec<&str>`,
/// `leaf_text: String`, `map: &serde_json::Map<...>`, `static_tag`.
///
/// For tree-child fields, the lookup is two-tier: first try
/// `map.get(field_name)` (works when the JSON key matches the
/// field's typical inner element name — most slot-wrapper shapes:
/// `Binary.left/right`, `If.condition`, `Class.body`, etc.); fall
/// back to draining from `unclaimed_children` positionally. `unclaimed_children` is the
/// flattened list of all non-meta JSON values in stable key order,
/// so positional drain handles variants whose JSON keys don't match
/// field names (e.g. `Class.bases` lives under `type` in JSON).
///
/// Returns `(expr, consumes_kid)`; second component is informational
/// only.
fn from_json_field_expr(fname: &str, ty: &str) -> (String, bool) {
    match ty {
        // ----- Tree-child fields (lookup → drain fallback) ------------
        // Every keyed lookup passes the field name as the type hint so
        // child objects with stripped `$type` reconstruct as the
        // expected variant.
        "Box<SyntaxTree>" => {
            let json_key = field_name_to_json_key(fname);
            (
                format!(
                    "{{
                let alt_key = {json_key:?};
                let hint = if map.contains_key({fname:?}) {{ {fname:?} }} else {{ alt_key }};
                if let Some(v) = map.get({fname:?}).or_else(|| map.get(alt_key)) {{
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                }} else if !unclaimed_children.is_empty() {{
                    Box::new(unclaimed_children.remove(0))
                }} else {{
                    Box::new(SyntaxTree::Unknown {{
                        kind: \"from_json:missing_child\".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    }})
                }}
            }}",
                    fname = fname,
                    json_key = json_key,
                ),
                true,
            )
        }
        "Option<Box<SyntaxTree>>" => {
            let json_key = field_name_to_json_key(fname);
            (
                format!(
                    "{{
                let alt_key = {json_key:?};
                let hint = if map.contains_key({fname:?}) {{ {fname:?} }} else {{ alt_key }};
                if let Some(v) = map.get({fname:?}).or_else(|| map.get(alt_key)) {{
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                }} else if !unclaimed_children.is_empty() {{
                    Some(Box::new(unclaimed_children.remove(0)))
                }} else {{ None }}
            }}",
                    fname = fname,
                    json_key = json_key,
                ),
                true,
            )
        }
        "Vec<SyntaxTree>" => {
            // Try three lookup forms: the literal field name (`parameters`),
            // its singular (`parameter`), and the canonical form with
            // semantic-suffix/trailing-underscore stripped
            // (`type_anns` → `type`, `where_clauses` → `where`).
            let json_key = field_name_to_json_key(fname);
            (
                format!(
                    "{{
                let singular = strip_plural({fname:?});
                let alt_key = {json_key:?};
                if let Some(v) = map.get({fname:?})
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {{
                    let hint = if map.contains_key(singular) {{ singular }} else {{ alt_key }};
                    match v {{
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }}
                }} else {{
                    std::mem::take(&mut unclaimed_children)
                }}
            }}",
                    fname = fname,
                    json_key = json_key,
                ),
                true,
            )
        }
        "Expression" => (
            format!(
                "{{
                let inner = if let Some(v) = map.get({fname:?}).or_else(|| map.get(\"expression\")) {{
                    tree_from_json_with_type_hint(v, Some(\"expression\"))
                }} else if !unclaimed_children.is_empty() {{
                    unclaimed_children.remove(0)
                }} else {{
                    SyntaxTree::Unknown {{
                        kind: \"from_json:missing_expression\".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    }}
                }};
                Expression::wrap(inner)
            }}",
                fname = fname,
            ),
            true,
        ),
        "Option<Expression>" => (
            format!(
                "{{
                if let Some(v) = map.get({fname:?}).or_else(|| map.get(\"expression\")) {{
                    Some(Expression::wrap(tree_from_json_with_type_hint(v, Some(\"expression\"))))
                }} else if !unclaimed_children.is_empty() {{
                    Some(Expression::wrap(unclaimed_children.remove(0)))
                }} else {{ None }}
            }}",
                fname = fname,
            ),
            true,
        ),
        "LambdaBody" => (
            format!(
                "{{
                let inner = if let Some(v) = map.get({fname:?}).or_else(|| map.get(\"body\")) {{
                    tree_from_json_with_type_hint(v, Some(\"body\"))
                }} else if !unclaimed_children.is_empty() {{
                    unclaimed_children.remove(0)
                }} else {{
                    SyntaxTree::Body {{
                        children: Vec::new(),
                        block_wrap: false,
                        pass_only: false,
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    }}
                }};
                LambdaBody::Expression(Box::new(inner))
            }}",
                fname = fname,
            ),
            true,
        ),
        "AccessReceiver" => (
            format!(
                "{{
                let inner = if let Some(v) = map.get({fname:?}) {{
                    tree_from_json_with_type_hint(v, Some({fname:?}))
                }} else if !unclaimed_children.is_empty() {{
                    unclaimed_children.remove(0)
                }} else {{
                    SyntaxTree::Unknown {{
                        kind: \"from_json:missing_receiver\".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    }}
                }};
                AccessReceiver::from_tree(inner, &[\"this\", \"self\", \"super\", \"base\"])
            }}",
                fname = fname,
            ),
            true,
        ),
        "Vec<AccessSegment>" => (
            // Access segments don't have a clean JSON inverse; any
            // remaining children get dropped here. Round-trip is lossy.
            "{ let _ = &mut unclaimed_children; Vec::new() }".into(),
            false,
        ),

        // ----- Shape metadata --------------------------------------
        "Modifiers" => (
            "Modifiers::from_marker_names(&marker_strs)".into(),
            false,
        ),
        "Vec<Marker>" => ("Vec::new()".into(), false),
        "Vec<&'staticstr>" => ("Vec::new()".into(), false),
        "Option<Access>" => (
            "Access::from_marker_names(&marker_strs)".into(),
            false,
        ),
        "Flag" => {
            let marker = strip_trailing_underscore(fname);
            (
                format!(
                    "{{ if map.get({marker:?}).and_then(|v| v.as_bool()).unwrap_or(false) {{
                    Flag::implicit_at(0, 0)
                }} else {{ Flag::Off }} }}",
                    marker = marker,
                ),
                false,
            )
        }
        "bool" => (
            format!(
                "map.get({fname:?}).and_then(|v| v.as_bool()).unwrap_or(false)",
                fname = fname,
            ),
            false,
        ),
        "String" => (
            // Conventional key names: 'text' for scalar leaves, the
            // field's own name otherwise. Falls back to empty string
            // when the key is missing.
            format!(
                "map.get({fname:?}).and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default()",
                fname = fname,
            ),
            false,
        ),
        "&'staticstr" => (
            // Variant-tag-derived string fields. For element_name /
            // kind / wrapper the natural inverse is the $type tag —
            // we leak it once via intern_static so the resulting
            // reference is genuinely 'static.
            "static_tag".into(),
            false,
        ),
        "Option<&'staticstr>" => ("None".into(), false),
        "Option<ByteRange>" => ("None".into(), false),
        "Option<Span>" => ("None".into(), false),
        "AccessorKind" => (
            "{
                map.get(\"kind\").and_then(|v| v.as_str()).map(|s| match s {
                    \"get\" => AccessorKind::Get,
                    \"set\" => AccessorKind::Set,
                    \"init\" => AccessorKind::Init,
                    _ => AccessorKind::Get,
                }).unwrap_or(AccessorKind::Get)
            }".into(),
            false,
        ),
        "SlotKind" => (
            "{
                // The slot's kind is conveyed by the parent's JSON
                // key ($type after strip), passed in via `tag`.
                match tag {
                    \"left\" => crate::tree::syntax::types::SlotKind::Left,
                    \"right\" => crate::tree::syntax::types::SlotKind::Right,
                    \"condition\" => crate::tree::syntax::types::SlotKind::Condition,
                    \"then\" => crate::tree::syntax::types::SlotKind::Then,
                    \"else\" => crate::tree::syntax::types::SlotKind::Else,
                    \"as\" => crate::tree::syntax::types::SlotKind::As,
                    \"filter\" => crate::tree::syntax::types::SlotKind::Filter,
                    _ => crate::tree::syntax::types::SlotKind::Left,
                }
            }".into(),
            false,
        ),
        "ParamKind" => (
            "{
                if marker_strs.iter().any(|m| *m == \"args\") {
                    ParamKind::Args
                } else if marker_strs.iter().any(|m| *m == \"kwargs\") {
                    ParamKind::Kwargs
                } else {
                    ParamKind::Regular
                }
            }".into(),
            false,
        ),
        "QuoteStyle" => (
            "QuoteStyle::Double".into(),
            false,
        ),
        "ByteRange" => ("ByteRange::synthetic_empty()".into(), false),
        "Span" => ("Span::point(0, 0)".into(), false),
        _ => (
            // Unknown field type — emit a TODO comment in the output so
            // future field-type additions trigger a build-time warning.
            format!(
                "Default::default() /* TODO: from_json for {fname}: {ty} */",
                fname = fname,
                ty = ty,
            ),
            false,
        ),
    }
}

fn strip_trailing_underscore(s: &str) -> String {
    if let Some(stripped) = s.strip_suffix('_') {
        stripped.to_string()
    } else {
        s.to_string()
    }
}

/// Build-time mirror of the generated `strip_plural`. Used when
/// computing the per-arm `claimed_keys` set so a `Vec<SyntaxTree>`
/// field named `parameters` also claims the key `parameter`. Mirrors
/// the same suffix rules — keep in sync with the runtime helper.
fn strip_plural_str(s: &str) -> &str {
    for suffix in ["_clauses", "_branches"] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            return stripped;
        }
    }
    for suffix in ["ies", "es", "s"] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            return stripped;
        }
    }
    s
}

/// Canonical JSON key for a Rust field name.
///
/// `to_json` emits children grouped by their **element name**, but
/// `from_json` looks up by **Rust field name**. When the two differ,
/// the keyed lookup misses and the codegen falls back to positional
/// `unclaimed_children` drain — which is fragile and often wrong.
///
/// This helper returns the form the JSON key would take when the
/// child element name "naturally" matches the field semantically.
/// Currently strips two kinds of noise:
///
/// - **Trailing `_`** — Rust's reserved-word escape. `where_` /
///   `else_` / `type_` lookup as `where` / `else` / `type` in JSON.
/// - **Semantic suffixes** `_ann` / `_target` — developer-readability
///   decorations that don't appear in the JSON projection. The child
///   variant of `Parameter::type_ann` is rendered with element name
///   "type", so the JSON key is `"type"`; same for `Catch::type_target`,
///   `Except::type_target`, etc.
///
/// Returns the canonical key, or the original field name unchanged
/// when no suffix matches.
fn field_name_to_json_key(fname: &str) -> String {
    for suffix in ["_ann", "_target"] {
        if let Some(stripped) = fname.strip_suffix(suffix) {
            return stripped.to_string();
        }
    }
    if let Some(stripped) = fname.strip_suffix('_') {
        return stripped.to_string();
    }
    fname.to_string()
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
