//! User-defined variables for the XPath dynamic context.
//!
//! Config files can declare named values that are bound into every query of
//! the run as two well-known map-valued XPath variables, alongside the
//! built-in `$file`:
//!
//! - `$variables` — the config root's `variables:` mapping
//! - `$rule.variables` — the current check rule's `variables:` mapping
//!   (an empty map outside a rule context)
//!
//! ```yaml
//! variables:
//!   env: production
//! check:
//!   rules:
//!     - id: too-long
//!       variables:
//!         max-params: 4
//!       xpath: "//function[count(parameters/parameter) > $rule.variables?max-params][$variables?env = 'production']"
//! ```
//!
//! Because user values are map *keys* (not XPath variable names), any string
//! is a legal name; keys that aren't valid NCNames are reachable via
//! `$variables?("weird key")`.
//!
//! ## Data model
//!
//! [`VariableValue`] mirrors the JSON data model (null, bool, number, string,
//! array, object) rather than any single config format. This is deliberate:
//! a future `variables-file:` feature that loads a JSON or YAML document as
//! variable context deserializes into the exact same type — merging file-based
//! and inline variables is then a plain [`QueryVariables::merge`], with no new
//! conversion machinery.
//!
//! ## Conversion to XPath values
//!
//! At execution time the whole map becomes a single XPath `map(*)` item
//! (an `xee_xpath::Sequence` — the datatype the dynamic context accepts for
//! variable bindings) by serializing to JSON and evaluating `parse-json()`.
//! JSON semantics apply inside the map: numbers become `xs:double`, `null`
//! becomes the empty sequence, nested arrays/objects become `array(*)` /
//! `map(*)`.
//!
//! The conversion itself lives in `xpath::engine` (it needs xee internals);
//! this module is plain `Send + Sync` data so it can cross rayon threads —
//! xee sequences are `Rc`-based and must be built per thread.

use std::collections::BTreeMap;
use serde::Deserialize;

/// A single variable value, mirroring the JSON data model.
///
/// Deserializes untagged, so plain YAML/TOML/JSON scalars, lists, and tables
/// map directly: `env: production` → `String`, `port: 8080` → `Int`,
/// `tags: [a, b]` → `Array`, nested mappings → `Map`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum VariableValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<VariableValue>),
    Map(BTreeMap<String, VariableValue>),
}

impl VariableValue {
    /// Whether the value is a scalar (bindable as a single atomic item)
    /// as opposed to a structured array/map.
    pub fn is_scalar(&self) -> bool {
        !matches!(self, VariableValue::Array(_) | VariableValue::Map(_))
    }

    /// Convert to a `serde_json::Value`. Structured values are bound into
    /// the XPath context by serializing through JSON and parsing with the
    /// engine's `parse-json` — one canonical bridge for every config format.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            VariableValue::Null => serde_json::Value::Null,
            VariableValue::Bool(b) => serde_json::Value::Bool(*b),
            VariableValue::Int(i) => serde_json::Value::from(*i),
            VariableValue::Float(f) => serde_json::Value::from(*f),
            VariableValue::String(s) => serde_json::Value::String(s.clone()),
            VariableValue::Array(items) => {
                serde_json::Value::Array(items.iter().map(|v| v.to_json()).collect())
            }
            VariableValue::Map(entries) => serde_json::Value::Object(
                entries.iter().map(|(k, v)| (k.clone(), v.to_json())).collect(),
            ),
        }
    }
}

/// Named variables bound into every XPath query of a run.
///
/// Plain data (no xee types), so it is cheap to share across threads via
/// `Arc` and convert per thread at query time. Backed by a `BTreeMap` for
/// deterministic iteration order.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct QueryVariables(BTreeMap<String, VariableValue>);

impl QueryVariables {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Insert a variable, replacing any existing value under the same name.
    pub fn insert(&mut self, name: impl Into<String>, value: VariableValue) {
        self.0.insert(name.into(), value);
    }

    pub fn get(&self, name: &str) -> Option<&VariableValue> {
        self.0.get(name)
    }

    /// Iterate over (name, value) pairs in deterministic (sorted) order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &VariableValue)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Variable names in deterministic (sorted) order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(|k| k.as_str())
    }

    /// Overlay `other` on top of `self` — `other` wins on name clashes.
    /// This is the extension seam for `variables-file:`: load the file into
    /// a `QueryVariables`, then merge the inline `variables:` over it.
    pub fn merge(&mut self, other: QueryVariables) {
        self.0.extend(other.0);
    }

    /// The whole variable map as a JSON object — the bridge format used to
    /// bind it into the XPath context as a `map(*)` item via `parse-json()`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::Value::Object(
            self.0.iter().map(|(k, v)| (k.clone(), v.to_json())).collect(),
        )
    }
}

impl FromIterator<(String, VariableValue)> for QueryVariables {
    fn from_iter<I: IntoIterator<Item = (String, VariableValue)>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_yaml_scalars() {
        let vars: QueryVariables = serde_yaml::from_str(
            "env: production\nport: 8080\nratio: 1.5\nstrict: true\nnothing: null\n",
        )
        .unwrap();
        assert_eq!(vars.get("env"), Some(&VariableValue::String("production".into())));
        assert_eq!(vars.get("port"), Some(&VariableValue::Int(8080)));
        assert_eq!(vars.get("ratio"), Some(&VariableValue::Float(1.5)));
        assert_eq!(vars.get("strict"), Some(&VariableValue::Bool(true)));
        assert_eq!(vars.get("nothing"), Some(&VariableValue::Null));
    }

    #[test]
    fn deserialize_yaml_structured() {
        let vars: QueryVariables = serde_yaml::from_str(
            "tags: [a, b]\nlimits:\n  max: 10\n",
        )
        .unwrap();
        assert_eq!(
            vars.get("tags"),
            Some(&VariableValue::Array(vec![
                VariableValue::String("a".into()),
                VariableValue::String("b".into()),
            ]))
        );
        match vars.get("limits") {
            Some(VariableValue::Map(m)) => assert_eq!(m.get("max"), Some(&VariableValue::Int(10))),
            other => panic!("expected map, got {:?}", other),
        }
    }

    #[test]
    fn whole_map_to_json() {
        let vars: QueryVariables =
            serde_yaml::from_str("env: prod\nmax: 4\n").unwrap();
        assert_eq!(vars.to_json(), serde_json::json!({"env": "prod", "max": 4}));
        assert_eq!(QueryVariables::new().to_json(), serde_json::json!({}));
    }

    #[test]
    fn merge_overlays() {
        let mut base = QueryVariables::new();
        base.insert("a", VariableValue::Int(1));
        base.insert("b", VariableValue::Int(2));
        let mut over = QueryVariables::new();
        over.insert("b", VariableValue::Int(20));
        over.insert("c", VariableValue::Int(3));
        base.merge(over);
        assert_eq!(base.get("a"), Some(&VariableValue::Int(1)));
        assert_eq!(base.get("b"), Some(&VariableValue::Int(20)));
        assert_eq!(base.get("c"), Some(&VariableValue::Int(3)));
    }

    #[test]
    fn to_json_roundtrip() {
        let vars: QueryVariables =
            serde_yaml::from_str("cfg:\n  hosts: [a, b]\n  port: 1\n").unwrap();
        let json = vars.get("cfg").unwrap().to_json();
        assert_eq!(json, serde_json::json!({"hosts": ["a", "b"], "port": 1}));
    }
}
