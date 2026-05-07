//! [`DataIr`] → `serde_json::Value` — direct cross-format render.
//!
//! ## Why
//!
//! Today's tractor JSON output goes `source → CST → DataIr → Xot
//! → xml_to_json → JSON`. Xot serves as a shared container, but
//! that means JSON output depends on XML attributes (`list="X"`)
//! that the renderer-pair (`data_to_xot` + `xml_to_json`) has to
//! coordinate. The IR is supposed to be the source of truth — Xot
//! shouldn't sit in the middle of *every* format conversion.
//!
//! This module provides a direct path:
//!
//! ```text
//!   DataIr ────────► serde_json::Value
//! ```
//!
//! No Xot, no `list=` attrs, no XML-to-JSON projection rules. The
//! IR's structural typing (`Sequence<DataIr>`, `Mapping<Pair>`)
//! gives JSON its array-vs-object decisions for free.
//!
//! Same approach applies to YAML / TOML output (separate
//! `data_to_yaml.rs` / `data_to_toml.rs` modules — DataIr is
//! format-agnostic by design).

#![cfg(feature = "native")]

use serde_json::{Map, Value};

use super::data::DataIr;

/// Render a [`DataIr`] tree to a `serde_json::Value`. The structural
/// IR variants map cleanly onto JSON's universe:
///
///   | DataIr             | JSON                                    |
///   |--------------------|-----------------------------------------|
///   | Document           | object containing top-level pairs       |
///   | Mapping            | object                                  |
///   | Sequence           | array                                   |
///   | Pair               | (key, value) entry of enclosing object  |
///   | Section (TOML/INI) | nested object keyed by section name     |
///   | String             | string                                  |
///   | Number             | number (parsed from `text`)             |
///   | Bool               | boolean                                 |
///   | Null               | null                                    |
///   | Comment            | (skipped — not part of the data shape)  |
///   | Unknown            | object `{ "$unknown": <kind> }`         |
pub fn data_to_json(ir: &DataIr) -> Value {
    match ir {
        DataIr::Document { children, .. } => {
            // A YAML "stream" can have multiple documents — for now,
            // single-document case: collect top-level pairs into one
            // object. Multi-document case wraps in an array.
            let docs: Vec<&DataIr> = children
                .iter()
                .filter(|c| !matches!(c, DataIr::Comment { .. }))
                .collect();
            if docs.len() == 1 {
                data_to_json(docs[0])
            } else if docs.is_empty() {
                Value::Null
            } else {
                Value::Array(docs.iter().map(|c| data_to_json(c)).collect())
            }
        }
        DataIr::Mapping { pairs, .. } => {
            let mut obj = Map::new();
            collect_pairs(&mut obj, pairs);
            Value::Object(obj)
        }
        DataIr::Sequence { items, .. } => {
            let arr: Vec<Value> = items
                .iter()
                .filter(|c| !matches!(c, DataIr::Comment { .. }))
                .map(data_to_json)
                .collect();
            Value::Array(arr)
        }
        DataIr::Pair { .. } => {
            // A bare Pair shouldn't be rendered standalone — it's
            // always a child of a Mapping/Section. Falling here
            // means a misuse: emit a single-pair object.
            let mut obj = Map::new();
            collect_pairs(&mut obj, std::slice::from_ref(ir));
            Value::Object(obj)
        }
        DataIr::Section { name, children, .. } => {
            // Section becomes a single-key object: `{ name: { ...children... } }`.
            // The TOML/INI imperative pipelines collapse the section
            // into the top-level via key-as-element-name, but the
            // typed JSON projection nests naturally.
            let key = scalar_str(name).unwrap_or_else(|| "section".to_string());
            let mut inner = Map::new();
            collect_pairs(&mut inner, children);
            let mut outer = Map::new();
            outer.insert(key, Value::Object(inner));
            Value::Object(outer)
        }
        DataIr::String { value, .. } => Value::String(value.clone()),
        DataIr::Number { text, .. } => parse_number(text),
        DataIr::Bool { value, .. } => Value::Bool(*value),
        DataIr::Null { .. } => Value::Null,
        DataIr::Comment { .. } => Value::Null, // dropped — not data
        DataIr::Unknown { kind, .. } => {
            let mut o = Map::new();
            o.insert("$unknown".to_string(), Value::String(kind.clone()));
            Value::Object(o)
        }
    }
}

/// Add every `Pair` from `children` into `obj`. Sections are
/// nested by their name. Comments are dropped. Repeated keys
/// promote earlier value to a 1-element array, then append (rare in
/// JSON, common in TOML's `[[x]]` array-of-tables).
fn collect_pairs(obj: &mut Map<String, Value>, children: &[DataIr]) {
    for c in children {
        match c {
            DataIr::Pair { key, value, .. } => {
                let k = match scalar_str(key) {
                    Some(s) => s,
                    None => continue,
                };
                insert_or_append(obj, k, data_to_json(value));
            }
            DataIr::Section { name, children: sec_children, .. } => {
                let k = match scalar_str(name) {
                    Some(s) => s,
                    None => continue,
                };
                let mut inner = Map::new();
                collect_pairs(&mut inner, sec_children);
                insert_or_append(obj, k, Value::Object(inner));
            }
            DataIr::Comment { .. } => { /* drop */ }
            // A loose scalar / sequence inside a mapping body is
            // unusual but recoverable as numbered keys.
            other => {
                let idx = obj.len();
                obj.insert(format!("_{idx}"), data_to_json(other));
            }
        }
    }
}

/// Insert into `obj`, promoting to an array on duplicate key (so
/// repeated `[[x]]` TOML sections accumulate naturally).
fn insert_or_append(obj: &mut Map<String, Value>, key: String, value: Value) {
    match obj.remove(&key) {
        None => {
            obj.insert(key, value);
        }
        Some(Value::Array(mut arr)) => {
            arr.push(value);
            obj.insert(key, Value::Array(arr));
        }
        Some(existing) => {
            obj.insert(key, Value::Array(vec![existing, value]));
        }
    }
}

/// Pull a string-shaped value out of a scalar IR for use as a JSON
/// object key.
fn scalar_str(ir: &DataIr) -> Option<String> {
    match ir {
        DataIr::String { value, .. } => Some(value.clone()),
        DataIr::Number { text, .. } => Some(text.clone()),
        DataIr::Bool { value, .. } => Some(value.to_string()),
        DataIr::Null { .. } => Some("null".to_string()),
        _ => None,
    }
}

/// Parse a numeric literal text into a JSON number. Preserves
/// integer shape when the literal is integral (`1` not `1.0`),
/// otherwise renders as float. Falls back to a string on parse
/// failure (e.g. TOML's hex / binary / underscored numbers).
fn parse_number(text: &str) -> Value {
    let trimmed = text.trim();
    if let Ok(i) = trimmed.parse::<i64>() {
        return Value::Number(i.into());
    }
    if let Ok(f) = trimmed.parse::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(f) {
            return Value::Number(n);
        }
    }
    Value::String(text.to_string())
}
