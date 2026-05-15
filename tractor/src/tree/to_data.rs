//! `SyntaxTree` → [`DataTree`] projection — programming-language tree rendered
//! into the structured-data tree that JSON / YAML / TOML serializers
//! consume.
//!
//! ## Why
//!
//! `tractor/src/tree/to_json.rs` historically owned a ~1000 LOC
//! ad-hoc projection from `SyntaxTree` directly to `serde_json::Value`,
//! mixing tree-walking with JSON-shape decisions. Recent iters
//! 29-36 layered on heuristics in `add_children` to fix
//! `$`-prefixed leaks (`$inline`, `$skip`, `$type": "expression"`,
//! plural-of-self collapse, marker-vs-leaf ambiguity) — each one a
//! special case papering over tree/JSON impedance mismatch.
//!
//! See `docs/design-projection-pipeline.md` for the architectural
//! rationale and slice plan.
//!
//! ## Approach
//!
//! Every `SyntaxTree` variant projects to a `DataTree` shape via uniform
//! rules. JSON, YAML, etc. then read the projection trivially.
//!
//! - `SyntaxTree::Skip` → omitted from the parent's children
//! - `SyntaxTree::Inline { list_name: None }` → flattened into parent
//! - `SyntaxTree::Inline { list_name: Some(k) }` → `Pair(k, Sequence)` on parent
//! - Scalar leaves (`Atom`, `Name`, `Int`, ...) → matching `DataTree` scalar
//! - `SyntaxTree::Class` / `Function` / etc. → `Mapping` with modifier flag
//!   pairs and typed slot pairs
//! - Synthetic markers (zero-width or Skip-only `SimpleStatement`)
//!   → flag pair `(name, Bool(true))` on parent
//! - Repeated same-keyed children → plural-grouped to
//!   `Pair(plural, Sequence)` once at projection time
//!
//! ## Status
//!
//! This module is being built up incrementally per the slice plan.
//! Slice 1 covers `SyntaxTree::Class` and reachable scalar leaves —
//! enough to project a class declaration through `data_to_json`
//! and verify the architecture is sound.
//!
//! Until every variant is covered, calls for unhandled variants
//! return [`DataTree::Unknown`] so we can detect coverage gaps in
//! parity tests rather than panic.

#![cfg(feature = "native")]

use super::data::DataTree;
use super::types::{AccessReceiver, QuoteStyle, SyntaxTree, Modifiers};

/// Project an `SyntaxTree` tree into a [`DataTree`] tree.
///
/// `source` is the original parse input — used to slice atom and
/// scalar text from byte ranges.
pub fn lower_to_data_ir(tree: &SyntaxTree, source: &str) -> DataTree {
    project(tree, source)
}

/// Walk a projected [`DataTree`] tree and return `true` if any node is
/// the `unhandled:<variant>` coverage-gap marker (see the catch-all
/// arm in `project`). Used by the JSON dispatch path to decide
/// whether to flow through the new `to_data` → `data_to_json` path
/// or fall back to the legacy heuristic `tree_to_json` for documents
/// the projection doesn't yet cover end-to-end.
pub fn has_unhandled(tree: &DataTree) -> bool {
    if let DataTree::Unknown { kind, .. } = tree {
        if kind.starts_with("unhandled:") {
            return true;
        }
    }
    match tree {
        DataTree::Document { children, .. }
        | DataTree::Sequence { items: children, .. }
        | DataTree::Section { children, .. }
        | DataTree::Element { children, .. }
        | DataTree::Directive { children, .. } => children.iter().any(has_unhandled),
        DataTree::Mapping { pairs, .. } => pairs.iter().any(has_unhandled),
        DataTree::Pair { key, value, .. } => has_unhandled(key) || has_unhandled(value),
        _ => false,
    }
}

fn project(tree: &SyntaxTree, source: &str) -> DataTree {
    match tree {
        // ----- Scalar leaves --------------------------------------------
        SyntaxTree::Name { text, range, span } => DataTree::String {
            value: text.clone(),
            quote_style: QuoteStyle::default_double(),
            range: *range,
            span: *span,
        },
        SyntaxTree::Atom { text, range, span, .. } => DataTree::String {
            value: text.clone(),
            quote_style: QuoteStyle::default_double(),
            range: *range,
            span: *span,
        },

        // ----- Containers -----------------------------------------------
        SyntaxTree::Module { children, range, span, .. } => {
            let pairs = collect_member_pairs(children, source);
            DataTree::Mapping { pairs, range: *range, span: *span }
        }
        SyntaxTree::Class {
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
            let mut pairs: Vec<DataTree> = Vec::new();
            push_modifier_flags(&mut pairs, modifiers, *range, *span);
            pairs.push(make_pair("name", project(name, source), *range, *span));
            // Body's children populate the class as additional pairs
            // (methods, properties, fields, nested types). The body
            // itself is not a separate slot in JSON — its children
            // are the class's members.
            if let SyntaxTree::Body { children, .. } = body.as_ref() {
                let body_pairs = collect_member_pairs(children, source);
                pairs.extend(body_pairs);
            }
            DataTree::Mapping { pairs, range: *range, span: *span }
        }

        // ----- Skip / Inline are transparent ----------------------------
        // (Callers using collect_member_pairs filter these. Reaching
        // them here means a direct project() call on a Skip/Inline,
        // which shouldn't happen in practice — emit Unknown so it's
        // visible in tests.)
        SyntaxTree::Skip { range, span } => DataTree::Unknown {
            kind: "skip".into(),
            range: *range,
            span: *span,
        },
        SyntaxTree::Inline { range, span, children, list_name } => {
            // Inline at the top level: emit a Mapping (or list) so
            // we don't lose its contents. Mirrors the projection
            // rule for Inline-as-child but at the root.
            if let Some(list) = list_name {
                let items: Vec<DataTree> = children
                    .iter()
                    .filter(|c| !matches!(c, SyntaxTree::Skip { .. }))
                    .map(|c| project(c, source))
                    .collect();
                DataTree::Mapping {
                    pairs: vec![make_pair(list, DataTree::Sequence {
                        items,
                        range: *range,
                        span: *span,
                    }, *range, *span)],
                    range: *range,
                    span: *span,
                }
            } else {
                let pairs = collect_member_pairs(children, source);
                DataTree::Mapping { pairs, range: *range, span: *span }
            }
        }

        // ----- Body — transparent: its children are the parent's pairs.
        // When a caller projects a Body directly (not as a Class /
        // Function slot), emit a Mapping containing those pairs. The
        // typical use is via `collect_member_pairs(body.children)` at
        // the parent level, but standalone projection still works.
        SyntaxTree::Body { children, range, span, .. } => {
            let pairs = collect_member_pairs(children, source);
            DataTree::Mapping { pairs, range: *range, span: *span }
        }

        // ----- Function — modifier flags + name + parameters list +
        // returns + body member pairs. Mirrors the legacy projection
        // shape but without `$type`.
        SyntaxTree::Function {
            element_name: _,
            modifiers,
            decorators: _,
            name,
            generics: _,
            parameters,
            returns,
            throws: _,
            body,
            range,
            span,
        } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            push_modifier_flags(&mut pairs, modifiers, *range, *span);
            pairs.push(make_pair("name", project(name, source), *range, *span));
            for p in parameters {
                pairs.push(make_pair("parameter", project(p, source), p.range(), p.span()));
            }
            if let Some(r) = returns {
                pairs.push(make_pair("returns", project(r, source), r.range(), r.span()));
            }
            if let Some(b) = body {
                if let SyntaxTree::Body { children, .. } = b.as_ref() {
                    pairs.extend(collect_member_pairs(children, source));
                }
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // ----- Variable — modifier flags + type? + name + value?.
        SyntaxTree::Variable {
            element_name: _,
            modifiers,
            decorators: _,
            type_ann,
            name,
            value,
            range,
            span,
        } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            push_modifier_flags(&mut pairs, modifiers, *range, *span);
            if let Some(t) = type_ann {
                pairs.push(make_pair("type", project(t, source), t.range(), t.span()));
            }
            pairs.push(make_pair("name", project(name, source), *range, *span));
            if let Some(v) = value {
                let inner = &v.inner;
                pairs.push(make_pair("value", project(inner, source), inner.range(), inner.span()));
            }
            DataTree::Mapping { pairs, range: *range, span: *span }
        }

        // ----- Property — modifier flags + type? + name + accessors +
        // value?. Each accessor renders by its kind ("get" / "set" /
        // "init") as a pair under that name; multiple of the same kind
        // are pluralized by `pluralize_pairs`.
        SyntaxTree::Property {
            modifiers,
            decorators: _,
            type_ann,
            name,
            accessors,
            value,
            range,
            span,
        } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            push_modifier_flags(&mut pairs, modifiers, *range, *span);
            if let Some(t) = type_ann {
                pairs.push(make_pair("type", project(t, source), t.range(), t.span()));
            }
            pairs.push(make_pair("name", project(name, source), *range, *span));
            for a in accessors {
                let key = element_name_for_pair(a);
                pairs.push(make_pair(key, project(a, source), a.range(), a.span()));
            }
            if let Some(v) = value {
                pairs.push(make_pair("value", project(v, source), v.range(), v.span()));
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // ----- Returns — wrapper around a type-annotation slot.
        SyntaxTree::Returns { type_ann, range, span } => {
            DataTree::Mapping {
                pairs: vec![make_pair(
                    "type",
                    project(type_ann, source),
                    type_ann.range(),
                    type_ann.span(),
                )],
                range: *range,
                span: *span,
            }
        }

        // ----- Parameter — kind/extra/modifier flags, type?, name,
        // default?. Mirrors legacy modulo `$type`.
        SyntaxTree::Parameter {
            kind,
            extra_markers,
            modifiers,
            name,
            type_ann,
            default,
            range,
            span,
        } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            match kind {
                super::types::ParamKind::Args => pairs.push(make_flag("args", *range, *span)),
                super::types::ParamKind::Kwargs => pairs.push(make_flag("kwargs", *range, *span)),
                _ => {}
            }
            push_modifier_flags(&mut pairs, modifiers, *range, *span);
            for m in extra_markers.iter() {
                pairs.push(make_flag(m.name, m.range, m.span));
            }
            if let Some(t) = type_ann {
                pairs.push(make_pair("type", project(t, source), t.range(), t.span()));
            }
            pairs.push(make_pair("name", project(name, source), *range, *span));
            if let Some(d) = default {
                pairs.push(make_pair("value", project(d, source), d.range(), d.span()));
            }
            DataTree::Mapping { pairs, range: *range, span: *span }
        }

        // ----- SimpleStatement (non-marker case) — modifier flags +
        // extra markers + children as keyed pairs. Marker-only cases
        // are folded by `synthetic_marker_name` at the parent level
        // and never reach here as a project() call.
        SyntaxTree::SimpleStatement {
            element_name: _,
            modifiers,
            extra_markers,
            children,
            range,
            span,
        } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            push_modifier_flags(&mut pairs, modifiers, *range, *span);
            for m in extra_markers.iter() {
                pairs.push(make_flag(m.name, m.range, m.span));
            }
            pairs.extend(collect_member_pairs(children, source));
            DataTree::Mapping { pairs, range: *range, span: *span }
        }

        // ----- Scalar literals — leaf text from `source[range]`. Each
        // maps to its natural DataTree scalar variant; the JSON output
        // becomes a bare value (number / string / bool / null) at
        // that position rather than a wrapper object.
        SyntaxTree::Int { text, range, span } => DataTree::Number {
            text: text.clone(),
            range: *range,
            span: *span,
        },
        SyntaxTree::Float { text, range, span } => DataTree::Number {
            text: text.clone(),
            range: *range,
            span: *span,
        },
        SyntaxTree::String { text, range, span, .. } => DataTree::String {
            value: text.clone(),
            quote_style: QuoteStyle::default_double(),
            range: *range,
            span: *span,
        },
        SyntaxTree::True { text, range, span, .. } => DataTree::Bool {
            value: true,
            text: text.clone(),
            range: *range,
            span: *span,
        },
        SyntaxTree::False { text, range, span, .. } => DataTree::Bool {
            value: false,
            text: text.clone(),
            range: *range,
            span: *span,
        },
        SyntaxTree::None { text, range, span, .. } => DataTree::Null {
            text: text.clone(),
            range: *range,
            span: *span,
        },
        SyntaxTree::Null { text, range, span, .. } => DataTree::Null {
            text: text.clone(),
            range: *range,
            span: *span,
        },

        // ----- Tuple / List / Set — anonymous-ordered collections.
        // JSON renders as an array of projected children.
        SyntaxTree::Tuple { children, range, span }
        | SyntaxTree::List { children, range, span }
        | SyntaxTree::Set { children, range, span } => DataTree::Sequence {
            items: children
                .iter()
                .filter(|c| !matches!(c, SyntaxTree::Skip { .. }))
                .map(|c| project(c, source))
                .collect(),
            range: *range,
            span: *span,
        },

        // ----- Dictionary — keyed pairs. Each `SyntaxTree::Pair { key, value }`
        // projects to `DataTree::Pair`; the dictionary itself becomes a
        // Mapping over those pairs.
        SyntaxTree::Dictionary { pairs, range, span } => DataTree::Mapping {
            pairs: pairs
                .iter()
                .filter(|p| !matches!(p, SyntaxTree::Skip { .. }))
                .map(|p| project(p, source))
                .collect(),
            range: *range,
            span: *span,
        },

        // ----- Pair — a dictionary entry. Becomes a `DataTree::Pair`
        // so that an enclosing `Dictionary` projects cleanly to a
        // JSON object. (Distinct from the structural `make_pair`
        // helper used at the parent-of-class level.)
        SyntaxTree::Pair { key, value, range, span } => DataTree::Pair {
            key: Box::new(project(key, source)),
            value: Box::new(project(value, source)),
            range: *range,
            span: *span,
        },

        // ----- Expression — Principle #15 stable host wrapper.
        // No marker: transparent projection to inner (the host has
        // no JSON-visible content of its own). Marker case
        // (`non_null` / `await`): emit a Mapping with the marker
        // as a flag, then merge the inner's projected pairs (if it
        // projects to a Mapping) or place the inner under its
        // element-name slot.
        SyntaxTree::Expression { inner, marker, range, span } => match marker {
            None => project(inner, source),
            Some(m) => {
                let mut pairs: Vec<DataTree> = vec![make_flag(m, *range, *span)];
                match project(inner, source) {
                    DataTree::Mapping { pairs: inner_pairs, .. } => pairs.extend(inner_pairs),
                    other => {
                        let key = element_name_for_pair(inner);
                        pairs.push(make_pair(key, other, inner.range(), inner.span()));
                    }
                }
                DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
            }
        },

        // ----- Binary / Comparison — symmetric `<left><op><right>`
        // shape. Lowering pre-wraps left/right in
        // `SimpleStatement("left"/"right", [Expression(operand)])`;
        // the data view reflects that shape faithfully (no unwrap).
        SyntaxTree::Binary { left, op_text, op_marker, right, range, span, .. }
        | SyntaxTree::Comparison { left, op_text, op_marker, right, range, span, .. } => {
            DataTree::Mapping {
                pairs: vec![
                    make_pair("left", project(left, source), left.range(), left.span()),
                    make_pair("op", make_op_mapping(op_text, op_marker, *range, *span), *range, *span),
                    make_pair("right", project(right, source), right.range(), right.span()),
                ],
                range: *range,
                span: *span,
            }
        }

        // ----- Unary — op + extra-marker flags + operand.
        SyntaxTree::Unary { op_text, op_marker, operand, extra_markers, range, span, .. } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            for m in extra_markers.iter() {
                pairs.push(make_flag(m.name, m.range, m.span));
            }
            pairs.push(make_pair("op", make_op_mapping(op_text, op_marker, *range, *span), *range, *span));
            pairs.push(make_pair("operand", project(operand, source), operand.range(), operand.span()));
            DataTree::Mapping { pairs, range: *range, span: *span }
        }

        // ----- Ternary — `<condition><then><else>` shape. Lowering
        // pre-wraps each slot; the data view reflects that shape.
        SyntaxTree::Ternary { condition, if_true, if_false, range, span } => DataTree::Mapping {
            pairs: vec![
                make_pair("condition", project(condition, source), condition.range(), condition.span()),
                make_pair("then", project(if_true, source), if_true.range(), if_true.span()),
                make_pair("else", project(if_false, source), if_false.range(), if_false.span()),
            ],
            range: *range,
            span: *span,
        },

        // ----- Is — `value is type_target`. Projects to a Mapping
        // with `left` (value) and `right` (type) for symmetry with
        // Binary / Comparison.
        SyntaxTree::Is { value, type_target, range, span } => DataTree::Mapping {
            pairs: vec![
                make_pair("left", project(value, source), value.range(), value.span()),
                make_pair("right", project(type_target, source), type_target.range(), type_target.span()),
            ],
            range: *range,
            span: *span,
        },

        // ----- Cast — `(Type)expr`. Mapping with `type` and `value`.
        SyntaxTree::Cast { type_ann, value, range, span } => DataTree::Mapping {
            pairs: vec![
                make_pair("type", project(type_ann, source), type_ann.range(), type_ann.span()),
                make_pair("value", project(value, source), value.range(), value.span()),
            ],
            range: *range,
            span: *span,
        },

        // ----- Call — callee + arguments. Each argument projects
        // under its own element-name; pluralization groups multiple
        // same-keyed siblings (e.g. `name: [...]`).
        SyntaxTree::Call { callee, arguments, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            pairs.push(make_pair("callee", project(callee, source), callee.range(), callee.span()));
            for a in arguments {
                let key = element_name_for_pair(a);
                pairs.push(make_pair(key, project(a, source), a.range(), a.span()));
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // ----- KeywordArgument — `name=value` named argument.
        // Mapping with `name` + `value` slots.
        SyntaxTree::KeywordArgument { name, value, range, span } => DataTree::Mapping {
            pairs: vec![
                make_pair("name", project(name, source), name.range(), name.span()),
                make_pair("value", project(value, source), value.range(), value.span()),
            ],
            range: *range,
            span: *span,
        },

        // ----- ListSplat / DictSplat — `*x` / `**x` spread. Project
        // the inner expression directly; the splat marker is dropped
        // from the JSON view. (The `<spread[list]/>` / `<spread[dict]/>`
        // distinction is XML-only structure for queryability.)
        SyntaxTree::ListSplat { inner, .. } | SyntaxTree::DictSplat { inner, .. } => project(inner, source),

        // ----- Control flow ----------------------------------------------

        // If / ElseIf — `condition`, `body` (member pairs flattened),
        // optional `else_branch`. ElseIf chains stay flat (each one
        // is its own pair under the parent if's `else_if` slot).
        SyntaxTree::If { condition, body, else_branch, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            pairs.push(make_pair("condition", project(condition, source), condition.range(), condition.span()));
            if let SyntaxTree::Body { children, .. } = body.as_ref() {
                pairs.extend(collect_member_pairs(children, source));
            } else {
                pairs.push(make_pair("body", project(body, source), body.range(), body.span()));
            }
            if let Some(e) = else_branch {
                let key = element_name_for_pair(e);
                pairs.push(make_pair(key, project(e, source), e.range(), e.span()));
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }
        SyntaxTree::ElseIf { condition, body, else_branch, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            pairs.push(make_pair("condition", project(condition, source), condition.range(), condition.span()));
            if let SyntaxTree::Body { children, .. } = body.as_ref() {
                pairs.extend(collect_member_pairs(children, source));
            } else {
                pairs.push(make_pair("body", project(body, source), body.range(), body.span()));
            }
            if let Some(e) = else_branch {
                let key = element_name_for_pair(e);
                pairs.push(make_pair(key, project(e, source), e.range(), e.span()));
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }
        SyntaxTree::Else { body, range, span } => {
            let pairs = if let SyntaxTree::Body { children, .. } = body.as_ref() {
                collect_member_pairs(children, source)
            } else {
                vec![make_pair("body", project(body, source), body.range(), body.span())]
            };
            DataTree::Mapping { pairs, range: *range, span: *span }
        }

        // For — Python `for target in iter: body [else]`. `is_async`
        // becomes a flag; multiple targets / iterables stay as
        // sibling pairs (pluralized).
        SyntaxTree::For { is_async, targets, iterables, body, else_body, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            if *is_async {
                pairs.push(make_flag("async", *range, *span));
            }
            for t in targets {
                pairs.push(make_pair("left", project(t, source), t.range(), t.span()));
            }
            for i in iterables {
                pairs.push(make_pair("right", project(i, source), i.range(), i.span()));
            }
            if let SyntaxTree::Body { children, .. } = body.as_ref() {
                pairs.extend(collect_member_pairs(children, source));
            } else {
                pairs.push(make_pair("body", project(body, source), body.range(), body.span()));
            }
            if let Some(e) = else_body {
                pairs.push(make_pair("else", project(e, source), e.range(), e.span()));
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // While — `condition`, body members flat, optional `else`.
        SyntaxTree::While { condition, body, else_body, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            pairs.push(make_pair("condition", project(condition, source), condition.range(), condition.span()));
            if let SyntaxTree::Body { children, .. } = body.as_ref() {
                pairs.extend(collect_member_pairs(children, source));
            } else {
                pairs.push(make_pair("body", project(body, source), body.range(), body.span()));
            }
            if let Some(e) = else_body {
                pairs.push(make_pair("else", project(e, source), e.range(), e.span()));
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // Foreach — C#/Java `foreach`. Optional type, single target,
        // single iterable, body. `in` flag for parity with legacy.
        SyntaxTree::Foreach { type_ann, target, iterable, body, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            pairs.push(make_flag("in", *range, *span));
            if let Some(t) = type_ann {
                pairs.push(make_pair("type", project(t, source), t.range(), t.span()));
            }
            pairs.push(make_pair("left", project(target, source), target.range(), target.span()));
            pairs.push(make_pair("right", project(iterable, source), iterable.range(), iterable.span()));
            if let SyntaxTree::Body { children, .. } = body.as_ref() {
                pairs.extend(collect_member_pairs(children, source));
            } else {
                pairs.push(make_pair("body", project(body, source), body.range(), body.span()));
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // CFor — C-style `for(init; cond; update) body`.
        SyntaxTree::CFor { initializer, condition, updates, body, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            if let Some(i) = initializer {
                let key = element_name_for_pair(i);
                pairs.push(make_pair(key, project(i, source), i.range(), i.span()));
            }
            if let Some(c) = condition {
                pairs.push(make_pair("condition", project(c, source), c.range(), c.span()));
            }
            for u in updates {
                let key = element_name_for_pair(u);
                pairs.push(make_pair(key, project(u, source), u.range(), u.span()));
            }
            if let SyntaxTree::Body { children, .. } = body.as_ref() {
                pairs.extend(collect_member_pairs(children, source));
            } else {
                pairs.push(make_pair("body", project(body, source), body.range(), body.span()));
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // DoWhile — body + condition (legacy renders body first).
        SyntaxTree::DoWhile { body, condition, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            if let SyntaxTree::Body { children, .. } = body.as_ref() {
                pairs.extend(collect_member_pairs(children, source));
            } else {
                pairs.push(make_pair("body", project(body, source), body.range(), body.span()));
            }
            pairs.push(make_pair("condition", project(condition, source), condition.range(), condition.span()));
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // Break / Continue — bare keyword statements; empty Mapping.
        SyntaxTree::Break { range, span } | SyntaxTree::Continue { range, span } => {
            DataTree::Mapping { pairs: vec![], range: *range, span: *span }
        }

        // Try — protected body + handlers + optional else / finally.
        SyntaxTree::Try { try_body, handlers, else_body, finally_body, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            if let SyntaxTree::Body { children, .. } = try_body.as_ref() {
                pairs.extend(collect_member_pairs(children, source));
            } else {
                pairs.push(make_pair("body", project(try_body, source), try_body.range(), try_body.span()));
            }
            for h in handlers {
                let key = element_name_for_pair(h); // "catch" or "except"
                pairs.push(make_pair(key, project(h, source), h.range(), h.span()));
            }
            if let Some(e) = else_body {
                pairs.push(make_pair("else", project(e, source), e.range(), e.span()));
            }
            if let Some(f) = finally_body {
                pairs.push(make_pair("finally", project(f, source), f.range(), f.span()));
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // ExceptHandler / catch.
        SyntaxTree::ExceptHandler { kind: _, type_target, binding, filter, body, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            if let Some(t) = type_target {
                pairs.push(make_pair("type", project(t, source), t.range(), t.span()));
            }
            if let Some(b) = binding {
                pairs.push(make_pair("as", project(b, source), b.range(), b.span()));
            }
            if let Some(f) = filter {
                pairs.push(make_pair("filter", project(f, source), f.range(), f.span()));
            }
            if let SyntaxTree::Body { children, .. } = body.as_ref() {
                pairs.extend(collect_member_pairs(children, source));
            } else {
                pairs.push(make_pair("body", project(body, source), body.range(), body.span()));
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // ----- Misc tail --------------------------------------------------

        // Lambda — modifier flags + parameters + body.
        SyntaxTree::Lambda { modifiers, parameters, body, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            push_modifier_flags(&mut pairs, modifiers, *range, *span);
            for p in parameters {
                pairs.push(make_pair("parameter", project(p, source), p.range(), p.span()));
            }
            match body {
                crate::tree::types::LambdaBody::Block(b) => {
                    if let SyntaxTree::Body { children, .. } = b.as_ref() {
                        pairs.extend(collect_member_pairs(children, source));
                    } else {
                        pairs.push(make_pair("body", project(b, source), b.range(), b.span()));
                    }
                }
                crate::tree::types::LambdaBody::Expression(e) => {
                    pairs.push(make_pair("body", project(e, source), e.range(), e.span()));
                }
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // ObjectCreation — `new Type(args) { Init }`.
        SyntaxTree::ObjectCreation { type_target, arguments, initializer, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            if let Some(t) = type_target {
                pairs.push(make_pair("type", project(t, source), t.range(), t.span()));
            }
            for a in arguments {
                let key = element_name_for_pair(a);
                pairs.push(make_pair(key, project(a, source), a.range(), a.span()));
            }
            if let Some(init) = initializer {
                pairs.push(make_pair("literal", project(init, source), init.range(), init.span()));
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // Constructor — modifier flags + name + parameters + body.
        SyntaxTree::Constructor { modifiers, decorators: _, name, parameters, body, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            push_modifier_flags(&mut pairs, modifiers, *range, *span);
            pairs.push(make_pair("name", project(name, source), name.range(), name.span()));
            for p in parameters {
                pairs.push(make_pair("parameter", project(p, source), p.range(), p.span()));
            }
            if let SyntaxTree::Body { children, .. } = body.as_ref() {
                pairs.extend(collect_member_pairs(children, source));
            } else {
                pairs.push(make_pair("body", project(body, source), body.range(), body.span()));
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // Generic — flat list of TypeParameter items. Each renders
        // under its element name (`type`); pluralization groups them.
        SyntaxTree::Generic { items, range, span } => {
            let pairs: Vec<DataTree> = items
                .iter()
                .map(|it| {
                    let key = element_name_for_pair(it);
                    make_pair(key, project(it, source), it.range(), it.span())
                })
                .collect();
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // TypeParameter — name + optional constraint.
        SyntaxTree::TypeParameter { name, constraint, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            pairs.push(make_pair("name", project(name, source), name.range(), name.span()));
            if let Some(c) = constraint {
                let key = element_name_for_pair(c);
                pairs.push(make_pair(key, project(c, source), c.range(), c.span()));
            }
            DataTree::Mapping { pairs, range: *range, span: *span }
        }

        // GenericType — `Name[T, U]` instantiation. `generic` flag +
        // base name + each param under its element name (typically
        // "type"; pluralized).
        SyntaxTree::GenericType { name, params, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            pairs.push(make_flag("generic", *range, *span));
            pairs.push(make_pair("name", project(name, source), name.range(), name.span()));
            for p in params {
                let key = element_name_for_pair(p);
                pairs.push(make_pair(key, project(p, source), p.range(), p.span()));
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // TypeAlias — `type Foo[T] = Bar`. name + type_params? + value.
        SyntaxTree::TypeAlias { name, type_params, value, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            pairs.push(make_pair("name", project(name, source), name.range(), name.span()));
            if let Some(t) = type_params {
                pairs.push(make_pair("generic", project(t, source), t.range(), t.span()));
            }
            pairs.push(make_pair("value", project(value, source), value.range(), value.span()));
            DataTree::Mapping { pairs, range: *range, span: *span }
        }

        // Enum — modifier flags + name + optional underlying type +
        // members (each EnumMember).
        SyntaxTree::Enum { modifiers, decorators: _, name, underlying_type, members, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            push_modifier_flags(&mut pairs, modifiers, *range, *span);
            pairs.push(make_pair("name", project(name, source), name.range(), name.span()));
            if let Some(u) = underlying_type {
                pairs.push(make_pair("type", project(u, source), u.range(), u.span()));
            }
            for m in members {
                pairs.push(make_pair("constant", project(m, source), m.range(), m.span()));
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // EnumMember — name + optional value.
        SyntaxTree::EnumMember { decorators: _, name, value, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            pairs.push(make_pair("name", project(name, source), name.range(), name.span()));
            if let Some(v) = value {
                pairs.push(make_pair("value", project(v, source), v.range(), v.span()));
            }
            DataTree::Mapping { pairs, range: *range, span: *span }
        }

        // Accessor — modifier flags + optional body.
        SyntaxTree::Accessor { modifiers, kind: _, body, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            push_modifier_flags(&mut pairs, modifiers, *range, *span);
            if let Some(b) = body {
                if let SyntaxTree::Body { children, .. } = b.as_ref() {
                    pairs.extend(collect_member_pairs(children, source));
                } else {
                    pairs.push(make_pair("body", project(b, source), b.range(), b.span()));
                }
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // Using — C# `using System;`. is_static flag + path + alias?.
        SyntaxTree::Using { is_static, alias, path, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            if *is_static {
                pairs.push(make_flag("static", *range, *span));
            }
            pairs.push(make_pair("path", project(path, source), path.range(), path.span()));
            if let Some(a) = alias {
                pairs.push(make_pair("alias", project(a, source), a.range(), a.span()));
            }
            DataTree::Mapping { pairs, range: *range, span: *span }
        }

        // Namespace — name + member children + file_scoped flag.
        SyntaxTree::Namespace { name, children, file_scoped, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            if *file_scoped {
                pairs.push(make_flag("file", *range, *span));
            }
            pairs.push(make_pair("name", project(name, source), name.range(), name.span()));
            pairs.extend(collect_member_pairs(children, source));
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // From — Python `from x import y`. relative flag + path? +
        // imports list (each FromImport).
        SyntaxTree::From { relative, path, imports, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            if *relative {
                pairs.push(make_flag("relative", *range, *span));
            }
            if let Some(p) = path {
                pairs.push(make_pair("path", project(p, source), p.range(), p.span()));
            }
            for i in imports {
                let key = element_name_for_pair(i);
                pairs.push(make_pair(key, project(i, source), i.range(), i.span()));
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // FromImport — has_alias flag + name + alias?.
        SyntaxTree::FromImport { has_alias, name, alias, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            if *has_alias {
                pairs.push(make_flag("alias", *range, *span));
            }
            pairs.push(make_pair("name", project(name, source), name.range(), name.span()));
            if let Some(a) = alias {
                pairs.push(make_pair("alias", project(a, source), a.range(), a.span()));
            }
            DataTree::Mapping { pairs, range: *range, span: *span }
        }

        // Path — flat segment list. Renders each segment as a "name"
        // pair; pluralization → `names: [...]`.
        SyntaxTree::Path { segments, range, span } => {
            let pairs: Vec<DataTree> = segments
                .iter()
                .map(|s| {
                    let key = element_name_for_pair(s);
                    make_pair(key, project(s, source), s.range(), s.span())
                })
                .collect();
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // Aliased — wrapper around the renamed-target identifier.
        // Project transparently to the inner.
        SyntaxTree::Aliased { inner, .. } => project(inner, source),

        // Assign — `target = value` / `t1, t2 = v1, v2` / `t: T = v`.
        // op_markers as flags + optional type + left(s) + right(s).
        SyntaxTree::Assign { targets, type_annotation, op_markers, values, range, span, .. } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            for m in op_markers {
                pairs.push(make_flag(m, *range, *span));
            }
            if let Some(t) = type_annotation {
                pairs.push(make_pair("type", project(t, source), t.range(), t.span()));
            }
            for t in targets {
                pairs.push(make_pair("left", project(t, source), t.range(), t.span()));
            }
            for v in values {
                pairs.push(make_pair("right", project(v, source), v.range(), v.span()));
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // FieldWrap — `<type>{inner}</type>` or `<value>{inner}</value>`.
        // Project transparently to the inner; the wrapper is XML-only
        // for queryability (the parent's slot key already conveys the
        // role at JSON level).
        SyntaxTree::FieldWrap { inner, .. } => project(inner, source),

        // PositionalSeparator / KeywordSeparator — pure-syntax markers
        // (`/` and `*` in Python parameter lists). No JSON content.
        SyntaxTree::PositionalSeparator { range, span } | SyntaxTree::KeywordSeparator { range, span } => {
            DataTree::Null {
                text: String::new(),
                range: *range,
                span: *span,
            }
        }

        // Unknown — keep the kind visible so coverage gaps surface.
        SyntaxTree::Unknown { kind, range, span } => DataTree::Unknown {
            kind: format!("tree-unknown:{}", kind),
            range: *range,
            span: *span,
        },

        // Raw passthrough — project as Unknown carrying the kind.
        // Languages on the typed passthrough (HTML / CSS / C / C++ /
        // bash / scala / lua / haskell / ocaml / r / julia) have no
        // semantic data tree; surface the structural kind so a
        // downstream consumer can still see what was there.
        SyntaxTree::Raw { kind, range, span, .. } => DataTree::Unknown {
            kind: format!("tree-raw:{}", kind),
            range: *range,
            span: *span,
        },

        // ----- Access chain (deferred from Z3) ---------------------------
        // Receiver + segment list (Member / Index / Call). The legacy
        // path emits `access` flag, then unfolds receiver into the
        // outer object's properties, then each segment as a
        // pluralized pair under `member` / `index` / `call`.
        SyntaxTree::Access { receiver, segments, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            pairs.push(make_flag("access", *range, *span));
            pairs.push(make_pair(
                "receiver",
                project_access_receiver(receiver, source),
                receiver.range(),
                receiver.span(),
            ));
            for seg in segments {
                let (key, mapping) = project_access_segment(seg, source);
                let r = seg.range();
                let s = seg.span();
                pairs.push(make_pair(key, mapping, r, s));
            }
            DataTree::Mapping { pairs: pluralize_pairs(pairs), range: *range, span: *span }
        }

        // ----- Comment — emit the source text as a String scalar.
        // Comments appear as `Pair("comment", String("// ..."))` under
        // their parent. Leading/trailing positional info is dropped
        // from the JSON shape (it lives on the tree for tree-text
        // rendering, but isn't part of the data view).
        SyntaxTree::Comment { range, span, .. } => DataTree::String {
            value: range.slice(source).to_string(),
            quote_style: QuoteStyle::default_double(),
            range: *range,
            span: *span,
        },

        // ----- Return — `<return><value>...</value></return>` → a
        // Mapping with one `expression` slot pair when a value is
        // present; an empty Mapping for bare `return;`.
        SyntaxTree::Return { value, range, span } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            if let Some(v) = value {
                pairs.push(make_pair(
                    "expression",
                    project(v, source),
                    v.range(),
                    v.span(),
                ));
            }
            DataTree::Mapping { pairs, range: *range, span: *span }
        }

        // ----- Decorator — wraps an inner expression. Project the
        // inner directly under the parent's `decorator` slot; the
        // decorator wrapper itself adds nothing the JSON view needs.
        SyntaxTree::Decorator { inner, range: _, span: _ } => project(inner, source),

        // ----- Import — its children are key/value pairs (path,
        // alias, etc.) that flatten directly under the parent. Same
        // shape as Module / Body.
        SyntaxTree::Import { children, range, span, .. } => {
            let pairs = collect_member_pairs(children, source);
            DataTree::Mapping { pairs, range: *range, span: *span }
        }

        // ----- Exhaustiveness checkpoint (S5A-Z7) ---------------------
        // No catch-all arm: every `SyntaxTree` variant has its own
        // projection rule above. Adding a new variant to `SyntaxTree`
        // forces the compiler to surface it here, which prevents
        // silent miscompiles back to "all-unknown" output. The
        // `has_unhandled` runtime predicate stays for the
        // marker-carrying `Unknown` projection (a real coverage
        // gap, not just an enum mismatch).
    }
}

fn make_flag(name: &str, range: super::types::ByteRange, span: super::types::Span) -> DataTree {
    make_pair(
        name,
        DataTree::Bool {
            value: true,
            text: "true".to_string(),
            range,
            span,
        },
        range,
        span,
    )
}

/// Project an [`AccessSegment`] into `(slot_name, mapping)` for use
/// inside the enclosing `Access` Mapping. Mirrors the legacy
/// `to_json::Renderer::add_access_chain` shape, modulo the `$type`
/// key (each segment knows its kind via the slot name).
/// Project an [`AccessReceiver`] to a [`DataTree`]. Keyword receivers
/// emit a flag mapping (`{base: true}` / `{this: true}` / ...) so the
/// JSON shape carries the keyword identity as a structured marker;
/// expression receivers project their inner tree directly.
fn project_access_receiver(receiver: &AccessReceiver, source: &str) -> DataTree {
    match receiver {
        AccessReceiver::Base { range, span }
        | AccessReceiver::This { range, span }
        | AccessReceiver::Super { range, span }
        | AccessReceiver::Self_ { range, span } => {
            let kw = receiver.keyword_element().expect("keyword variant");
            DataTree::Mapping {
                pairs: vec![make_flag(kw, *range, *span)],
                range: *range,
                span: *span,
            }
        }
        AccessReceiver::Instance(t) => project(t, source),
    }
}

fn project_access_segment(
    seg: &super::types::AccessSegment,
    source: &str,
) -> (&'static str, DataTree) {
    use super::types::AccessSegment;
    let r = seg.range();
    let s = seg.span();
    match seg {
        AccessSegment::Member { property_range, optional, .. } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            if *optional {
                pairs.push(make_flag("optional", r, s));
            }
            pairs.push(make_pair(
                "name",
                DataTree::String {
                    value: property_range.slice(source).to_string(),
                    quote_style: QuoteStyle::default_double(),
                    range: *property_range,
                    span: s,
                },
                *property_range,
                s,
            ));
            ("member", DataTree::Mapping { pairs, range: r, span: s })
        }
        AccessSegment::Index { indices, .. } => {
            let pairs: Vec<DataTree> = indices
                .iter()
                .map(|i| {
                    let key = element_name_for_pair(i);
                    make_pair(key, project(i, source), i.range(), i.span())
                })
                .collect();
            ("index", DataTree::Mapping { pairs: pluralize_pairs(pairs), range: r, span: s })
        }
        AccessSegment::Call { name, name_span, arguments, .. } => {
            let mut pairs: Vec<DataTree> = Vec::new();
            if let (Some(nr), Some(ns)) = (name, name_span) {
                pairs.push(make_pair(
                    "name",
                    DataTree::String {
                        value: nr.slice(source).to_string(),
                        quote_style: QuoteStyle::default_double(),
                        range: *nr,
                        span: *ns,
                    },
                    *nr,
                    *ns,
                ));
            }
            for a in arguments {
                let key = element_name_for_pair(a);
                pairs.push(make_pair(key, project(a, source), a.range(), a.span()));
            }
            ("call", DataTree::Mapping { pairs: pluralize_pairs(pairs), range: r, span: s })
        }
    }
}

/// Build the `<op>` Mapping for Binary / Unary / Comparison: a
/// `text` pair (the literal operator) plus a boolean flag named for
/// the tree's `op_marker` (e.g. `plus`, `minus`, `lt`). Mirrors
/// `to_json::Renderer::op_value`.
fn make_op_mapping(
    op_text: &str,
    op_marker: &'static str,
    range: super::types::ByteRange,
    span: super::types::Span,
) -> DataTree {
    DataTree::Mapping {
        pairs: vec![
            make_pair(
                "text",
                DataTree::String {
                    value: op_text.to_string(),
                    quote_style: QuoteStyle::default_double(),
                    range,
                    span,
                },
                range,
                span,
            ),
            make_flag(op_marker, range, span),
        ],
        range,
        span,
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
fn collect_member_pairs(children: &[SyntaxTree], source: &str) -> Vec<DataTree> {
    let mut out: Vec<DataTree> = Vec::new();
    for c in children {
        match c {
            SyntaxTree::Skip { .. } => continue,
            SyntaxTree::Inline { children: inner, list_name: None, .. } => {
                let nested = collect_member_pairs(inner, source);
                out.extend(nested);
            }
            SyntaxTree::Inline { children: inner, list_name: Some(list), range, span } => {
                let items: Vec<DataTree> = inner
                    .iter()
                    .filter(|i| !matches!(i, SyntaxTree::Skip { .. }))
                    .map(|i| project(i, source))
                    .collect();
                out.push(make_pair(list, DataTree::Sequence {
                    items,
                    range: *range,
                    span: *span,
                }, *range, *span));
            }
            _ => {
                if let Some(flag_name) = synthetic_marker_name(c) {
                    let r = c.range();
                    let s = c.span();
                    out.push(make_pair(flag_name, DataTree::Bool {
                        value: true,
                        text: "true".to_string(),
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
fn synthetic_marker_name(tree: &SyntaxTree) -> Option<&'static str> {
    if let SyntaxTree::SimpleStatement { element_name, children, modifiers, extra_markers, .. } = tree {
        let all_skip_or_empty = children.iter().all(|k| matches!(k, SyntaxTree::Skip { .. }));
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
fn pluralize_pairs(pairs: Vec<DataTree>) -> Vec<DataTree> {
    use std::collections::HashMap;

    // Count occurrences per key.
    let mut counts: HashMap<String, usize> = HashMap::new();
    for p in &pairs {
        if let Some(k) = pair_key_string(p) {
            *counts.entry(k).or_insert(0) += 1;
        }
    }
    // Walk pairs again, grouping repeated keys into Sequence pairs.
    let mut grouped: HashMap<String, Vec<DataTree>> = HashMap::new();
    let mut out: Vec<DataTree> = Vec::new();
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
                        DataTree::Null {
                            text: String::new(),
                            range: super::types::ByteRange::synthetic_empty(),
                            span: super::types::Span::point(0, 0),
                        },
                        super::types::ByteRange::synthetic_empty(),
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
        out[idx] = make_pair(&plural, DataTree::Sequence {
            items,
            range: first_range,
            span: first_span,
        }, first_range, first_span);
    }
    out
}

fn make_pair(
    key: &str,
    value: DataTree,
    range: super::types::ByteRange,
    span: super::types::Span,
) -> DataTree {
    DataTree::Pair {
        key: Box::new(DataTree::String {
            value: key.to_string(),
            quote_style: QuoteStyle::default_double(),
            range,
            span,
        }),
        value: Box::new(value),
        range,
        span,
    }
}

fn pair_key_string(tree: &DataTree) -> Option<String> {
    if let DataTree::Pair { key, .. } = tree {
        if let DataTree::String { value, .. } = key.as_ref() {
            return Some(value.clone());
        }
    }
    None
}

fn pair_take_value(tree: DataTree) -> DataTree {
    if let DataTree::Pair { value, .. } = tree {
        *value
    } else {
        tree
    }
}

/// Push modifier flags as `Pair(marker_name, Bool(true))` entries.
fn push_modifier_flags(
    out: &mut Vec<DataTree>,
    modifiers: &Modifiers,
    range: super::types::ByteRange,
    span: super::types::Span,
) {
    for marker in modifiers.marker_names() {
        out.push(make_pair(
            marker,
            DataTree::Bool {
                value: true,
                text: "true".to_string(),
                range,
                span,
            },
            range,
            span,
        ));
    }
}

/// The element name a child should occupy in the parent's mapping.
/// Parallels (and is a subset of) `to_json::element_name` — kept
/// independent so this module doesn't depend on the legacy renderer.
fn element_name_for_pair(tree: &SyntaxTree) -> &'static str {
    match tree {
        SyntaxTree::Module { element_name, .. } => element_name,
        SyntaxTree::SimpleStatement { element_name, .. } => element_name,
        SyntaxTree::Atom { element_name, .. } => element_name,
        SyntaxTree::Class { kind, .. } => kind,
        SyntaxTree::Body { .. } => "body",
        SyntaxTree::Function { element_name, .. } => element_name,
        SyntaxTree::Property { .. } => "property",
        SyntaxTree::Constructor { .. } => "constructor",
        SyntaxTree::EnumMember { .. } => "constant",
        SyntaxTree::Enum { .. } => "enum",
        SyntaxTree::Name { .. } => "name",
        SyntaxTree::Comment { .. } => "comment",
        // Catch-all: variants we haven't taught yet. The parent
        // pair will use this generic key; once we cover the
        // variant in `project()` the projection becomes lossless.
        _ => "node",
    }
}

fn tree_node_name(tree: &SyntaxTree) -> &'static str {
    match tree {
        SyntaxTree::Module { .. } => "Module",
        SyntaxTree::Class { .. } => "Class",
        SyntaxTree::Function { .. } => "Function",
        SyntaxTree::Property { .. } => "Property",
        SyntaxTree::Body { .. } => "Body",
        SyntaxTree::Atom { .. } => "Atom",
        SyntaxTree::Name { .. } => "Name",
        SyntaxTree::Skip { .. } => "Skip",
        SyntaxTree::Inline { .. } => "Inline",
        SyntaxTree::SimpleStatement { .. } => "SimpleStatement",
        SyntaxTree::Binary { .. } => "Binary",
        SyntaxTree::Unary { .. } => "Unary",
        SyntaxTree::If { .. } => "If",
        SyntaxTree::For { .. } => "For",
        SyntaxTree::While { .. } => "While",
        SyntaxTree::Pair { .. } => "Pair",
        SyntaxTree::Tuple { .. } => "Tuple",
        SyntaxTree::List { .. } => "List",
        SyntaxTree::Set { .. } => "Set",
        SyntaxTree::Dictionary { .. } => "Dictionary",
        SyntaxTree::Comment { .. } => "Comment",
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

    fn name(text: &str) -> SyntaxTree {
        SyntaxTree::Name {
            text: text.to_string(),
            range: pos(0, text.len() as u32),
            span: sp(),
        }
    }

    #[test]
    fn class_with_public_modifier_projects_to_mapping_with_flag_and_name() {
        let source = "Foo";
        let class = SyntaxTree::Class {
            kind: "class",
            modifiers: Modifiers { access: Some(Access::Public), ..Modifiers::default() },
            decorators: vec![],
            name: Box::new(name("Foo")),
            generics: vec![],
            bases: vec![],
            where_clauses: vec![],
            body: Box::new(SyntaxTree::Body {
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
        let DataTree::Mapping { pairs, .. } = proj else {
            panic!("expected Mapping, got {:?}", proj);
        };
        // Expect: [Pair("public", Bool(true)), Pair("name", String("Foo"))]
        assert_eq!(pairs.len(), 2, "pairs: {pairs:?}");
        assert_eq!(pair_key_string(&pairs[0]).as_deref(), Some("public"));
        assert_eq!(pair_key_string(&pairs[1]).as_deref(), Some("name"));
    }

    #[test]
    fn has_unhandled_predicate_trips_on_unhandled_marker() {
        // `has_unhandled` walks the DataTree looking for the
        // `unhandled:<variant>` coverage-gap marker. After S5A-Z7
        // the projection has zero arms that emit such markers (the
        // `SyntaxTree::project` match is exhaustive — compiler-checked), so
        // any test must construct the DataTree by hand.
        use super::super::types::{ByteRange, Span};
        let r = ByteRange::new(0, 1);
        let s = Span::point(1, 1);
        let bare = DataTree::Unknown { kind: "unhandled:fake".into(), range: r, span: s };
        assert!(has_unhandled(&bare));

        let nested = DataTree::Mapping {
            pairs: vec![DataTree::Pair {
                key: Box::new(DataTree::String {
                    value: "x".into(),
                    quote_style: QuoteStyle::default_double(),
                    range: r,
                    span: s,
                }),
                value: Box::new(DataTree::Unknown { kind: "unhandled:nested".into(), range: r, span: s }),
                range: r,
                span: s,
            }],
            range: r,
            span: s,
        };
        assert!(has_unhandled(&nested), "should walk into pair values");

        // Non-`unhandled:` Unknown (e.g. `tree-unknown:` from the
        // typed `SyntaxTree::Unknown` projection) is NOT a coverage gap.
        let benign = DataTree::Unknown { kind: "tree-unknown:foo_kind".into(), range: r, span: s };
        assert!(!has_unhandled(&benign));
    }

    #[test]
    fn projection_is_exhaustive_for_all_ir_variants() {
        // Compile-enforced via the absence of a catch-all in
        // `project`, but assert at runtime too so that any future
        // accidental re-introduction of an `unhandled:` arm
        // surfaces as a test failure instead of a silent regression.
        let source = "Foo";
        let class = SyntaxTree::Class {
            kind: "class",
            modifiers: Modifiers { access: Some(Access::Public), ..Modifiers::default() },
            decorators: vec![],
            name: Box::new(name("Foo")),
            generics: vec![],
            bases: vec![],
            where_clauses: vec![],
            body: Box::new(SyntaxTree::Body {
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
        assert!(!has_unhandled(&proj));
    }

    #[test]
    fn skip_children_are_omitted() {
        let source = "Foo";
        let class = SyntaxTree::Class {
            kind: "class",
            modifiers: Modifiers::default(),
            decorators: vec![],
            name: Box::new(name("Foo")),
            generics: vec![],
            bases: vec![],
            where_clauses: vec![],
            body: Box::new(SyntaxTree::Body {
                children: vec![
                    SyntaxTree::Skip { range: pos(3, 4), span: sp() },
                    SyntaxTree::Skip { range: pos(4, 5), span: sp() },
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
        let DataTree::Mapping { pairs, .. } = proj else { panic!(); };
        // Expect just: [Pair("name", "Foo")]
        assert_eq!(pairs.len(), 1);
        assert_eq!(pair_key_string(&pairs[0]).as_deref(), Some("name"));
    }
}
