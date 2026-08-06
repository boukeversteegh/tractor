//! User-defined variables for the XPath dynamic context.
//!
//! Config files can declare named values that are bound into every query of
//! the run as well-known map-valued XPath variables, alongside the built-in
//! `$file`:
//!
//! - `$variables` — the config root's `variables:` mapping
//! - `$rule.variables` / `$mapping.variables` / `$query.variables` /
//!   `$assertion.variables` — the current operation entry's `variables:`
//!   mapping, named after the config key the entry lives under (`rules:`,
//!   `mappings:`, `queries:`, `assertions:`). Only the namespace matching
//!   the current entry is populated; the others are empty maps.
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

/// The kind of operation entry a query runs under. Each kind owns one XPath
/// namespace for its `variables:` — named after the config key the entry
/// lives under, so the XPath name mirrors the YAML right above it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A check rule (`check: rules:`) — `$rule.variables`, `$rule.id`.
    Rule,
    /// A set mapping (`set: mappings:`) — `$mapping.variables`.
    Mapping,
    /// A query expression (`query: queries:`) — `$query.variables`.
    Query,
    /// A test assertion (`test: assertions:`) — `$assertion.variables`.
    Assertion,
}

impl EntryKind {
    /// Every kind, in declaration order. The engine binds one map per kind
    /// on every query so the static context can stay constant.
    pub const ALL: [EntryKind; 4] = [
        EntryKind::Rule,
        EntryKind::Mapping,
        EntryKind::Query,
        EntryKind::Assertion,
    ];

    /// The XPath variable name this kind's `variables:` bind to.
    pub fn variables_name(self) -> &'static str {
        match self {
            EntryKind::Rule => "rule.variables",
            EntryKind::Mapping => "mapping.variables",
            EntryKind::Query => "query.variables",
            EntryKind::Assertion => "assertion.variables",
        }
    }

    /// The entry noun as it appears in config and diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            EntryKind::Rule => "rule",
            EntryKind::Mapping => "mapping",
            EntryKind::Query => "query",
            EntryKind::Assertion => "assertion",
        }
    }

    /// Where entries of this kind live, for "only bound in …" diagnostics.
    pub fn config_home(self) -> &'static str {
        match self {
            EntryKind::Rule => "check rules",
            EntryKind::Mapping => "set mappings",
            EntryKind::Query => "query entries",
            EntryKind::Assertion => "test assertions",
        }
    }
}

/// The operation entry a query is executing for: which kind it is, its
/// `variables:`, and (for rules) its id. Binding is all-or-nothing — the
/// pairing of kind, variables, and id is structural, not a set of loose
/// fields kept in sync by convention.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryContext {
    pub kind: EntryKind,
    /// The entry's own `variables:`, bound under `kind.variables_name()`.
    pub variables: std::sync::Arc<QueryVariables>,
    /// The entry's id, bound as `$rule.id`. Only check rules have ids.
    pub id: Option<String>,
}

impl EntryContext {
    pub fn rule(variables: std::sync::Arc<QueryVariables>, id: impl Into<String>) -> Self {
        Self { kind: EntryKind::Rule, variables, id: Some(id.into()) }
    }

    pub fn mapping(variables: std::sync::Arc<QueryVariables>) -> Self {
        Self { kind: EntryKind::Mapping, variables, id: None }
    }

    pub fn query(variables: std::sync::Arc<QueryVariables>) -> Self {
        Self { kind: EntryKind::Query, variables, id: None }
    }

    pub fn assertion(variables: std::sync::Arc<QueryVariables>) -> Self {
        Self { kind: EntryKind::Assertion, variables, id: None }
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
