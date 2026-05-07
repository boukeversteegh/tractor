//! `Ir` → [`DataIr`] projection — programming-language IR rendered
//! into the structured-data IR that JSON / YAML / TOML serializers
//! consume.
//!
//! ## Why
//!
//! `tractor/src/ir/to_json.rs` historically owned a ~1000 LOC
//! ad-hoc projection from `Ir` directly to `serde_json::Value`,
//! mixing IR-walking with JSON-shape decisions. Recent iters
//! 29-36 layered on heuristics in `add_children` to fix
//! `$`-prefixed leaks (`$inline`, `$skip`, `$type": "expression"`,
//! plural-of-self collapse, marker-vs-leaf ambiguity) — each one a
//! special case papering over IR/JSON impedance mismatch.
//!
//! See `docs/design-projection-pipeline.md` for the architectural
//! rationale and slice plan.
//!
//! ## Approach
//!
//! Every `Ir` variant projects to a `DataIr` shape via uniform
//! rules. JSON, YAML, etc. then read the projection trivially.
//!
//! - `Ir::Skip` → omitted from the parent's children
//! - `Ir::Inline { list_name: None }` → flattened into parent
//! - `Ir::Inline { list_name: Some(k) }` → `Pair(k, Sequence)` on parent
//! - Scalar leaves (`Atom`, `Name`, `Int`, ...) → matching `DataIr` scalar
//! - `Ir::Class` / `Function` / etc. → `Mapping` with modifier flag
//!   pairs and typed slot pairs
//! - Synthetic markers (zero-width or Skip-only `SimpleStatement`)
//!   → flag pair `(name, Bool(true))` on parent
//! - Repeated same-keyed children → plural-grouped to
//!   `Pair(plural, Sequence)` once at projection time
//!
//! ## Status
//!
//! This module is being built up incrementally per the slice plan.
//! Slice 1 covers `Ir::Class` and reachable scalar leaves —
//! enough to project a class declaration through `data_to_json`
//! and verify the architecture is sound.
//!
//! Until every variant is covered, calls for unhandled variants
//! return [`DataIr::Unknown`] so we can detect coverage gaps in
//! parity tests rather than panic.

#![cfg(feature = "native")]

use super::data::DataIr;
use super::types::{Ir, Modifiers};

/// Project an `Ir` tree into a [`DataIr`] tree.
///
/// `source` is the original parse input — used to slice atom and
/// scalar text from byte ranges.
pub fn lower_to_data_ir(ir: &Ir, source: &str) -> DataIr {
    project(ir, source)
}

fn project(ir: &Ir, source: &str) -> DataIr {
    match ir {
        // ----- Scalar leaves --------------------------------------------
        Ir::Name { range, span } => DataIr::String {
            value: range.slice(source).to_string(),
            range: *range,
            span: *span,
        },
        Ir::Atom { range, span, .. } => DataIr::String {
            value: range.slice(source).to_string(),
            range: *range,
            span: *span,
        },

        // ----- Containers -----------------------------------------------
        Ir::Module { children, range, span, .. } => {
            let pairs = collect_member_pairs(children, source);
            DataIr::Mapping { pairs, range: *range, span: *span }
        }
        Ir::Class {
            kind: _,
            modifiers,
            decorators: _,
            name,
            generics: _,
            bases: _,
            where_clauses: _,
            body,
            range,
            span,
        } => {
            let mut pairs: Vec<DataIr> = Vec::new();
            push_modifier_flags(&mut pairs, modifiers, *range, *span);
            pairs.push(make_pair("name", project(name, source), *range, *span));
            // Body's children populate the class as additional pairs
            // (methods, properties, fields, nested types). The body
            // itself is not a separate slot in JSON — its children
            // are the class's members.
            if let Ir::Body { children, .. } = body.as_ref() {
                let body_pairs = collect_member_pairs(children, source);
                pairs.extend(body_pairs);
            }
            DataIr::Mapping { pairs, range: *range, span: *span }
        }

        // ----- Skip / Inline are transparent ----------------------------
        // (Callers using collect_member_pairs filter these. Reaching
        // them here means a direct project() call on a Skip/Inline,
        // which shouldn't happen in practice — emit Unknown so it's
        // visible in tests.)
        Ir::Skip { range, span } => DataIr::Unknown {
            kind: "skip".into(),
            range: *range,
            span: *span,
        },
        Ir::Inline { range, span, children, list_name } => {
            // Inline at the top level: emit a Mapping (or list) so
            // we don't lose its contents. Mirrors the projection
            // rule for Inline-as-child but at the root.
            if let Some(list) = list_name {
                let items: Vec<DataIr> = children
                    .iter()
                    .filter(|c| !matches!(c, Ir::Skip { .. }))
                    .map(|c| project(c, source))
                    .collect();
                DataIr::Mapping {
                    pairs: vec![make_pair(list, DataIr::Sequence {
                        items,
                        range: *range,
                        span: *span,
                    }, *range, *span)],
                    range: *range,
                    span: *span,
                }
            } else {
                let pairs = collect_member_pairs(children, source);
                DataIr::Mapping { pairs, range: *range, span: *span }
            }
        }

        // ----- Everything else is a coverage gap (slice 1 scope) -------
        other => DataIr::Unknown {
            kind: format!("unhandled:{}", ir_variant_name(other)),
            range: other.range(),
            span: other.span(),
        },
    }
}

/// Collect children into a member-pair list, applying the projection
/// rules:
/// - `Skip` → omitted
/// - `Inline { list_name: None }` → flatten (recurse with its children)
/// - `Inline { list_name: Some(k) }` → `Pair(k, Sequence(items))`
/// - Synthetic-marker SimpleStatement → `Pair(name, Bool(true))`
/// - Otherwise → `Pair(element_name(c), project(c))`
///
/// Repeat-keyed pairs get pluralized in a final pass so that
/// `[Pair("column", v1), Pair("column", v2)]` becomes
/// `[Pair("columns", Sequence([v1, v2]))]`.
fn collect_member_pairs(children: &[Ir], source: &str) -> Vec<DataIr> {
    let mut out: Vec<DataIr> = Vec::new();
    for c in children {
        match c {
            Ir::Skip { .. } => continue,
            Ir::Inline { children: inner, list_name: None, .. } => {
                let nested = collect_member_pairs(inner, source);
                out.extend(nested);
            }
            Ir::Inline { children: inner, list_name: Some(list), range, span } => {
                let items: Vec<DataIr> = inner
                    .iter()
                    .filter(|i| !matches!(i, Ir::Skip { .. }))
                    .map(|i| project(i, source))
                    .collect();
                out.push(make_pair(list, DataIr::Sequence {
                    items,
                    range: *range,
                    span: *span,
                }, *range, *span));
            }
            _ => {
                if let Some(flag_name) = synthetic_marker_name(c) {
                    let r = c.range();
                    let s = c.span();
                    out.push(make_pair(flag_name, DataIr::Bool {
                        value: true,
                        range: r,
                        span: s,
                    }, r, s));
                    continue;
                }
                let key = element_name_for_pair(c);
                let r = c.range();
                let s = c.span();
                out.push(make_pair(key, project(c, source), r, s));
            }
        }
    }
    pluralize_pairs(out)
}

/// Detect synthetic markers — SimpleStatement / Atom shapes that
/// carry no semantic content (no children OR children all Skip,
/// no modifier flags, no extra markers). These collapse to
/// `Bool(true)` flag pairs at the parent level.
fn synthetic_marker_name(ir: &Ir) -> Option<&'static str> {
    if let Ir::SimpleStatement { element_name, children, modifiers, extra_markers, .. } = ir {
        let all_skip_or_empty = children.iter().all(|k| matches!(k, Ir::Skip { .. }));
        if all_skip_or_empty
            && modifiers.marker_names().is_empty()
            && extra_markers.is_empty()
        {
            return Some(element_name);
        }
    }
    None
}

/// Group repeat-keyed pairs into pluralized sequences. Single-keyed
/// slots stay singular.
fn pluralize_pairs(pairs: Vec<DataIr>) -> Vec<DataIr> {
    use std::collections::HashMap;

    // Count occurrences per key.
    let mut counts: HashMap<String, usize> = HashMap::new();
    for p in &pairs {
        if let Some(k) = pair_key_string(p) {
            *counts.entry(k).or_insert(0) += 1;
        }
    }
    // Walk pairs again, grouping repeated keys into Sequence pairs.
    let mut grouped: HashMap<String, Vec<DataIr>> = HashMap::new();
    let mut out: Vec<DataIr> = Vec::new();
    let mut emitted_plural: HashMap<String, usize> = HashMap::new();
    for p in pairs {
        let k = pair_key_string(&p);
        match k {
            Some(key) if counts.get(&key).copied().unwrap_or(0) > 1 => {
                let plural = pluralize(&key);
                let value = pair_take_value(p);
                grouped.entry(plural.clone()).or_default().push(value);
                // Reserve position for the plural entry on first
                // occurrence; replace at end.
                let entry = emitted_plural.entry(plural.clone()).or_insert_with(|| {
                    let placeholder = make_pair(
                        &plural,
                        DataIr::Null { range: super::types::ByteRange::empty_at(0), span: super::types::Span::point(0, 0) },
                        super::types::ByteRange::empty_at(0),
                        super::types::Span::point(0, 0),
                    );
                    out.push(placeholder);
                    out.len() - 1
                });
                let _ = entry;
            }
            _ => out.push(p),
        }
    }
    // Replace placeholders with Sequence pairs.
    for (plural, idx) in emitted_plural {
        let items = grouped.remove(&plural).unwrap_or_default();
        let (first_range, first_span) = items
            .first()
            .map(|i| (i.range(), i.span()))
            .unwrap_or((super::types::ByteRange::empty_at(0), super::types::Span::point(0, 0)));
        out[idx] = make_pair(&plural, DataIr::Sequence {
            items,
            range: first_range,
            span: first_span,
        }, first_range, first_span);
    }
    out
}

fn make_pair(
    key: &str,
    value: DataIr,
    range: super::types::ByteRange,
    span: super::types::Span,
) -> DataIr {
    DataIr::Pair {
        key: Box::new(DataIr::String {
            value: key.to_string(),
            range,
            span,
        }),
        value: Box::new(value),
        range,
        span,
    }
}

fn pair_key_string(ir: &DataIr) -> Option<String> {
    if let DataIr::Pair { key, .. } = ir {
        if let DataIr::String { value, .. } = key.as_ref() {
            return Some(value.clone());
        }
    }
    None
}

fn pair_take_value(ir: DataIr) -> DataIr {
    if let DataIr::Pair { value, .. } = ir {
        *value
    } else {
        ir
    }
}

/// Push modifier flags as `Pair(marker_name, Bool(true))` entries.
fn push_modifier_flags(
    out: &mut Vec<DataIr>,
    modifiers: &Modifiers,
    range: super::types::ByteRange,
    span: super::types::Span,
) {
    for marker in modifiers.marker_names() {
        out.push(make_pair(
            marker,
            DataIr::Bool { value: true, range, span },
            range,
            span,
        ));
    }
}

/// The element name a child should occupy in the parent's mapping.
/// Parallels (and is a subset of) `to_json::element_name` — kept
/// independent so this module doesn't depend on the legacy renderer.
fn element_name_for_pair(ir: &Ir) -> &'static str {
    match ir {
        Ir::Module { element_name, .. } => element_name,
        Ir::SimpleStatement { element_name, .. } => element_name,
        Ir::Atom { element_name, .. } => element_name,
        Ir::Class { kind, .. } => kind,
        Ir::Body { .. } => "body",
        Ir::Function { element_name, .. } => element_name,
        Ir::Property { .. } => "property",
        Ir::Constructor { .. } => "constructor",
        Ir::EnumMember { .. } => "constant",
        Ir::Enum { .. } => "enum",
        Ir::Name { .. } => "name",
        Ir::Comment { .. } => "comment",
        // Catch-all: variants we haven't taught yet. The parent
        // pair will use this generic key; once we cover the
        // variant in `project()` the projection becomes lossless.
        _ => "node",
    }
}

fn ir_variant_name(ir: &Ir) -> &'static str {
    match ir {
        Ir::Module { .. } => "Module",
        Ir::Class { .. } => "Class",
        Ir::Function { .. } => "Function",
        Ir::Property { .. } => "Property",
        Ir::Body { .. } => "Body",
        Ir::Atom { .. } => "Atom",
        Ir::Name { .. } => "Name",
        Ir::Skip { .. } => "Skip",
        Ir::Inline { .. } => "Inline",
        Ir::SimpleStatement { .. } => "SimpleStatement",
        Ir::Binary { .. } => "Binary",
        Ir::Unary { .. } => "Unary",
        Ir::If { .. } => "If",
        Ir::For { .. } => "For",
        Ir::While { .. } => "While",
        Ir::Pair { .. } => "Pair",
        Ir::Tuple { .. } => "Tuple",
        Ir::List { .. } => "List",
        Ir::Set { .. } => "Set",
        Ir::Dictionary { .. } => "Dictionary",
        Ir::Comment { .. } => "Comment",
        _ => "Other",
    }
}

/// Pluralize an element name. Mirrors `transform::helpers::pluralize_list_name`
/// for the projection layer (kept local to avoid coupling to the
/// legacy transform module).
fn pluralize(s: &str) -> String {
    if s.ends_with('s') || s.ends_with("sh") || s.ends_with("ch") || s.ends_with('x') {
        format!("{s}es")
    } else if s.ends_with('y') {
        let mut t = s.to_string();
        t.pop();
        t.push_str("ies");
        t
    } else {
        format!("{s}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{Access, ByteRange, Span};

    fn pos(start: u32, end: u32) -> ByteRange { ByteRange::new(start, end) }
    fn sp() -> Span { Span::point(1, 1) }

    fn name(text: &str) -> Ir {
        Ir::Name { range: pos(0, text.len() as u32), span: sp() }
    }

    #[test]
    fn class_with_public_modifier_projects_to_mapping_with_flag_and_name() {
        let source = "Foo";
        let class = Ir::Class {
            kind: "class",
            modifiers: Modifiers { access: Some(Access::Public), ..Modifiers::default() },
            decorators: vec![],
            name: Box::new(name("Foo")),
            generics: None,
            bases: vec![],
            where_clauses: vec![],
            body: Box::new(Ir::Body {
                children: vec![],
                pass_only: false,
                block_wrap: true,
                range: pos(3, 3),
                span: sp(),
            }),
            range: pos(0, 3),
            span: sp(),
        };
        let proj = lower_to_data_ir(&class, source);
        let DataIr::Mapping { pairs, .. } = proj else {
            panic!("expected Mapping, got {:?}", proj);
        };
        // Expect: [Pair("public", Bool(true)), Pair("name", String("Foo"))]
        assert_eq!(pairs.len(), 2, "pairs: {pairs:?}");
        assert_eq!(pair_key_string(&pairs[0]).as_deref(), Some("public"));
        assert_eq!(pair_key_string(&pairs[1]).as_deref(), Some("name"));
    }

    #[test]
    fn skip_children_are_omitted() {
        let source = "Foo";
        let class = Ir::Class {
            kind: "class",
            modifiers: Modifiers::default(),
            decorators: vec![],
            name: Box::new(name("Foo")),
            generics: None,
            bases: vec![],
            where_clauses: vec![],
            body: Box::new(Ir::Body {
                children: vec![
                    Ir::Skip { range: pos(3, 4), span: sp() },
                    Ir::Skip { range: pos(4, 5), span: sp() },
                ],
                pass_only: false,
                block_wrap: true,
                range: pos(3, 5),
                span: sp(),
            }),
            range: pos(0, 5),
            span: sp(),
        };
        let proj = lower_to_data_ir(&class, source);
        let DataIr::Mapping { pairs, .. } = proj else { panic!(); };
        // Expect just: [Pair("name", "Foo")]
        assert_eq!(pairs.len(), 1);
        assert_eq!(pair_key_string(&pairs[0]).as_deref(), Some("name"));
    }
}
