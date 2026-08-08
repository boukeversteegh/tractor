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
//! every variable source — inline config values, `$file` documents, future
//! source kinds — deserializes into the exact same type, so downstream code
//! never knows or cares where a value came from.
//!
//! ## Sources (`$`-directives)
//!
//! Any value in a `variables:` tree may be a *source directive* instead of a
//! literal: a map with exactly one `$`-prefixed key saying where the value
//! comes from. Each source binds under its own name — there are no merge or
//! shadowing semantics; two sources cannot collide because they are distinct
//! keys in one map.
//!
//! ```yaml
//! variables:
//!   env: production              # inline literal
//!   settings:
//!     $file: "config/vars.yml"   # whole file bound under this name
//!   team:
//!     prefixes:
//!       $file: "prefixes.yml"    # directives may appear at any depth
//! ```
//!
//! - **`$file`** — load a JSON/YAML/TOML document (path relative to the
//!   config file's directory). File content is pure data: directives inside
//!   loaded files are *not* processed.
//! - **`$literal`** — the inner value verbatim; the escape hatch for literal
//!   data whose keys start with `$`.
//! - **`$query`** — reserved for binding the result of a tractor query;
//!   errors as "not implemented yet" so it cannot silently become data.
//!
//! `$`-prefixed keys are reserved everywhere outside `$literal`: an unknown
//! directive or a `$`-key mixed into an ordinary map is a load error, so a
//! typo'd directive can never silently pass through as literal data.
//!
//! ### When a source is read
//!
//! Directives are checked in two stages, because a source may name a file
//! that an *earlier operation in the same run* has yet to write:
//!
//! 1. **At config load**, every directive is validated structurally
//!    ([`QueryVariables::validate_sources`]) — unknown directives, reserved
//!    `$query`, `$`-keys in ordinary maps, and non-string `$file` paths all
//!    fail before any operation runs. No filesystem access happens here.
//! 2. **At the start of each operation**, the sources that operation reads
//!    are loaded ([`QueryVariables::resolve_sources`]). Operations run in
//!    order, so a file written by an earlier operation (e.g. a query op's
//!    `output:`) is read fresh by the next one, and every entry within one
//!    operation sees a single consistent snapshot.
//!
//! What counts as "reads" is recorded per entry at load time by
//! [`EntryVariables`] — not re-derived from XPath strings during execution.
//! Resolution is therefore per entry, not per operation: one rule reading
//! `$rule.variables` never forces resolution for a sibling rule that
//! doesn't. A declared-but-unread source is neither resolved (so it cannot
//! fail the run over a file that does not exist yet) nor bound raw: it
//! binds an empty map. Downstream code therefore only ever sees plain
//! resolved data — never an unresolved directive.
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
use std::path::Path;
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

// ---------------------------------------------------------------------------
// Source directives ($file, $literal, reserved $query)
// ---------------------------------------------------------------------------

/// Error while resolving `$`-directive variable sources. Always fatal: a
/// declared source is a promise, unlike an optional key lookup.
#[derive(Debug)]
pub struct VariableSourceError {
    /// Which `variables:` block the source is in — `None` for the config
    /// root, otherwise the entry that owns it (e.g. `rule 'no-console'`).
    /// The value knows its lookup path but not its owner, so callers
    /// attach this via [`VariableSourceError::in_scope`].
    scope: Option<String>,
    /// Lookup path to the offending value, e.g. `?settings?db`.
    path: String,
    message: String,
}

impl VariableSourceError {
    fn new(path: &[String], message: impl Into<String>) -> Self {
        Self {
            scope: None,
            path: path.iter().map(|p| format!("?{}", p)).collect(),
            message: message.into(),
        }
    }

    /// Name the `variables:` block this error came from, so a message
    /// points at one entry instead of at "some variables somewhere".
    /// Only the outermost scope sticks — resolution never nests.
    pub fn in_scope(mut self, scope: impl Into<String>) -> Self {
        self.scope.get_or_insert_with(|| scope.into());
        self
    }
}

impl std::fmt::Display for VariableSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.scope {
            Some(scope) => write!(f, "in {} variables{}: {}", scope, self.path, self.message),
            None => write!(f, "in variables{}: {}", self.path, self.message),
        }
    }
}

impl std::error::Error for VariableSourceError {}

impl QueryVariables {
    /// Resolve every `$`-directive source in the tree into plain data.
    /// `base_dir` anchors relative `$file` paths (the config file's
    /// directory). Called at *operation start* — not config load — so a
    /// file written by an earlier operation in the run is read fresh.
    /// Each operation sees one consistent snapshot.
    pub fn resolve_sources(self, base_dir: &Path) -> Result<QueryVariables, VariableSourceError> {
        self.walk(&mut |file, path| load_variables_file(&base_dir.join(file), path))
    }

    /// Statically validate every `$`-directive in the tree without touching
    /// the filesystem: unknown directives, reserved `$query`, `$`-keys mixed
    /// into ordinary maps, and non-string `$file` paths all fail here.
    /// Called at config load, so a typo'd directive fails before any
    /// operation runs — only the file *read* is deferred to execution.
    pub fn validate_sources(&self) -> Result<(), VariableSourceError> {
        self.clone().walk(&mut |_, _| Ok(VariableValue::Null)).map(|_| ())
    }

    /// Whether the tree contains any `$`-directive (a resolution would
    /// change it). Used to skip cloning when there is nothing to resolve.
    pub fn has_sources(&self) -> bool {
        fn scan(v: &VariableValue) -> bool {
            match v {
                VariableValue::Map(m) => {
                    (m.len() == 1 && m.keys().next().unwrap().starts_with('$'))
                        || m.values().any(scan)
                }
                VariableValue::Array(items) => items.iter().any(scan),
                _ => false,
            }
        }
        self.0.values().any(scan)
    }

    /// Shared walk for resolve/validate: `load` supplies a `$file`'s content
    /// (or a placeholder during validation).
    fn walk(
        self,
        load: &mut dyn FnMut(&str, &[String]) -> Result<VariableValue, VariableSourceError>,
    ) -> Result<QueryVariables, VariableSourceError> {
        let mut path = Vec::new();
        self.0
            .into_iter()
            .map(|(name, value)| {
                path.push(name.clone());
                let resolved = resolve_value(value, load, &mut path)?;
                path.pop();
                Ok((name, resolved))
            })
            .collect()
    }
}

/// Recursively resolve one value. `path` carries the lookup trail for error
/// messages and is restored before returning.
fn resolve_value(
    value: VariableValue,
    load: &mut dyn FnMut(&str, &[String]) -> Result<VariableValue, VariableSourceError>,
    path: &mut Vec<String>,
) -> Result<VariableValue, VariableSourceError> {
    let VariableValue::Map(map) = value else {
        return match value {
            VariableValue::Array(items) => {
                let resolved = items
                    .into_iter()
                    .enumerate()
                    .map(|(i, item)| {
                        path.push(i.to_string());
                        let resolved = resolve_value(item, load, path)?;
                        path.pop();
                        Ok(resolved)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(VariableValue::Array(resolved))
            }
            scalar => Ok(scalar),
        };
    };

    // A single-`$`-key map is a source directive.
    if map.len() == 1 {
        let directive = map.keys().next().unwrap().strip_prefix('$').map(str::to_owned);
        if let Some(directive) = directive {
            let inner = map.into_values().next().unwrap();
            return match directive.as_str() {
                // Verbatim escape hatch — no directive processing inside.
                "literal" => Ok(inner),
                "file" => {
                    let VariableValue::String(rel) = inner else {
                        return Err(VariableSourceError::new(
                            path,
                            "`$file` expects a path string",
                        ));
                    };
                    load(&rel, path)
                }
                "query" => Err(VariableSourceError::new(
                    path,
                    "`$query` variable sources are not implemented yet",
                )),
                other => Err(VariableSourceError::new(
                    path,
                    format!(
                        "unknown variable source directive `${}` (known: $file, $literal; \
                         wrap literal data in `$literal:` if the `$` key is intentional)",
                        other
                    ),
                )),
            };
        }
    }

    // Ordinary map: `$`-keys are reserved here, so a typo'd or misplaced
    // directive fails loudly instead of passing through as data.
    if let Some(stray) = map.keys().find(|k| k.starts_with('$')) {
        return Err(VariableSourceError::new(
            path,
            format!(
                "`{}` is a reserved directive key inside a map with other keys; \
                 wrap the map in `$literal:` if it is literal data",
                stray
            ),
        ));
    }

    let resolved = map
        .into_iter()
        .map(|(k, v)| {
            path.push(k.clone());
            let resolved = resolve_value(v, load, path)?;
            path.pop();
            Ok((k, resolved))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    Ok(VariableValue::Map(resolved))
}

/// A path for messages: forward slashes on every platform. A Windows join
/// puts a backslash before the file name, which in a message like
/// "cannot read 'dir\nope.json'" reads as an escape sequence.
fn display_path(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

/// Load a JSON/YAML/TOML document as a single [`VariableValue`]. The content
/// is pure data — directives inside loaded files are not processed.
///
/// Native-only: reading a file needs a filesystem, and the YAML/TOML parsers
/// are native-only dependencies. The wasm build has neither, so it reports
/// the directive as unsupported instead (see the `not(native)` variant).
#[cfg(feature = "native")]
fn load_variables_file(
    file: &Path,
    path: &[String],
) -> Result<VariableValue, VariableSourceError> {
    let content = std::fs::read_to_string(file).map_err(|e| {
        VariableSourceError::new(path, format!("cannot read '{}': {}", display_path(file), e))
    })?;
    let parse_err = |e: String| {
        VariableSourceError::new(path, format!("invalid variables in '{}': {}", display_path(file), e))
    };
    match file.extension().and_then(|e| e.to_str()) {
        Some("json") => serde_json::from_str(&content).map_err(|e| parse_err(e.to_string())),
        Some("yml") | Some("yaml") => {
            serde_yaml::from_str(&content).map_err(|e| parse_err(e.to_string()))
        }
        Some("toml") => toml::from_str(&content).map_err(|e| parse_err(e.to_string())),
        _ => Err(VariableSourceError::new(
            path,
            format!(
                "unsupported variables file '{}': use .json, .yml/.yaml, or .toml",
                display_path(file)
            ),
        )),
    }
}

/// Builds without the `native` feature (wasm) have no filesystem, so a
/// `$file` source cannot be loaded. Directive *validation* still works —
/// only the read is unavailable.
#[cfg(not(feature = "native"))]
fn load_variables_file(
    file: &Path,
    path: &[String],
) -> Result<VariableValue, VariableSourceError> {
    Err(VariableSourceError::new(
        path,
        format!(
            "`$file` variable sources need a filesystem and are unavailable in this build \
             (tried to load '{}')",
            display_path(file)
        ),
    ))
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

/// An operation entry's `variables:` together with what its expression
/// actually reads — decided once, where both are known (config load), so
/// execution never re-derives resolution policy from XPath strings.
///
/// Resolving a `$`-directive source has an observable cost: it reads a file
/// that a *later* operation in the run may not have written yet. Only an
/// expression that reads a namespace may depend on (and fail on) its
/// sources, and that question is per entry — one rule referencing
/// `$rule.variables` must not force resolution for a sibling rule that
/// doesn't.
///
/// The reference test is a literal substring scan, which is sound because
/// XPath 3.1 has no dynamic variable references (no `eval`, no computed
/// variable names): a namespace can only be read by writing its name. It
/// over-approximates — a name inside a string literal counts as a read —
/// which is the safe direction: an unread source may be resolved need-
/// lessly, but a read source is never left unresolved.
///
/// Deliberately **not** `Default`: a default value would claim "reads
/// nothing" without having seen an expression, and a construction site that
/// forgot to pass its xpath would silently disable resolution for the whole
/// operation. Every value must be built from the expression it describes.
/// Prefer the entry constructors (`SetMapping::new`, `QueryExpr::new`,
/// `TestAssertion::new`, `Rule::with_variables`), which pass their own
/// xpath so the two cannot disagree.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryVariables {
    variables: std::sync::Arc<QueryVariables>,
    /// The expression reads this entry's own `$<entry>.variables`.
    reads_own: bool,
    /// The expression reads the run-level `$variables`.
    reads_run: bool,
}

impl EntryVariables {
    /// Bind `variables` to an entry of `kind` whose expression is `xpath`,
    /// recording what that expression reads.
    pub fn new(variables: QueryVariables, kind: EntryKind, xpath: &str) -> Self {
        Self {
            variables: std::sync::Arc::new(variables),
            reads_own: xpath.contains(&format!("${}", kind.variables_name())),
            reads_run: reads_run_variables(xpath),
        }
    }

    /// The entry's declared variables, directives unresolved.
    pub fn declared(&self) -> &std::sync::Arc<QueryVariables> {
        &self.variables
    }

    /// Whether the entry's expression reads the run-level `$variables`.
    pub fn reads_run_variables(&self) -> bool {
        self.reads_run
    }

    /// The map to bind as this entry's namespace, with `$`-directive
    /// sources resolved against `base_dir`.
    ///
    /// An entry that never reads its own namespace binds an empty map: its
    /// sources are neither resolved (so a not-yet-written file cannot fail
    /// the run) nor bound raw (so an unresolved directive never reaches the
    /// XPath context as data).
    pub fn bind(&self, base_dir: &Path) -> Result<std::sync::Arc<QueryVariables>, VariableSourceError> {
        bind_variables(&self.variables, self.reads_own, base_dir)
    }

    /// The same entry with its declared variables replaced by an already
    /// bound (resolved) map — what the executor stores back into the plan
    /// before running the entry.
    pub fn with_bound(mut self, bound: std::sync::Arc<QueryVariables>) -> Self {
        self.variables = bound;
        self
    }
}

impl std::ops::Deref for EntryVariables {
    type Target = QueryVariables;
    fn deref(&self) -> &QueryVariables {
        &self.variables
    }
}

/// Whether an XPath expression reads the run-level `$variables`.
///
/// `$rule.variables` and friends don't count: the character before
/// `variables` there is `.`, not `$`.
pub fn reads_run_variables(xpath: &str) -> bool {
    xpath.match_indices("$variables").any(|(i, _)| {
        // Reject `$variables` as a prefix of a longer name (NCNames may
        // contain `-`, `_`, `.`, and digits).
        !xpath[i + "$variables".len()..]
            .starts_with(|c: char| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
    })
}

/// Shared binding policy: resolve when read, empty when not, and never
/// bind an unresolved directive map.
fn bind_variables(
    variables: &std::sync::Arc<QueryVariables>,
    is_read: bool,
    base_dir: &Path,
) -> Result<std::sync::Arc<QueryVariables>, VariableSourceError> {
    if !variables.has_sources() {
        return Ok(std::sync::Arc::clone(variables));
    }
    if !is_read {
        return Ok(std::sync::Arc::default());
    }
    Ok(std::sync::Arc::new(
        (**variables).clone().resolve_sources(base_dir)?,
    ))
}

/// Resolve the run-level `$variables` for an operation: same policy as
/// [`EntryVariables::bind`], driven by whether any of the operation's
/// expressions read it.
pub fn bind_run_variables(
    variables: &std::sync::Arc<QueryVariables>,
    is_read: bool,
    base_dir: &Path,
) -> Result<std::sync::Arc<QueryVariables>, VariableSourceError> {
    bind_variables(variables, is_read, base_dir)
}

/// Everything user-defined that binds into a query's dynamic context: the
/// run-level `$variables` map and the current operation entry (which
/// supplies `$<entry>.variables` and `$rule.id`).
///
/// One value instead of two loose parameters, so adding a future binding
/// changes this struct rather than every signature along the way. `Default`
/// is "nothing bound" — every namespace an empty map.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QueryBindings {
    /// Run-level variables, bound as `$variables`.
    pub variables: std::sync::Arc<QueryVariables>,
    /// The current operation entry, or `None` outside any entry.
    pub entry: Option<EntryContext>,
}

impl QueryBindings {
    /// Bindings with only the run-level variables set.
    pub fn run(variables: std::sync::Arc<QueryVariables>) -> Self {
        Self { variables, entry: None }
    }

    /// The same bindings with `entry` as the current operation entry.
    pub fn with_entry(mut self, entry: EntryContext) -> Self {
        self.entry = Some(entry);
        self
    }
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

    fn resolve(yaml: &str, base_dir: &Path) -> Result<QueryVariables, VariableSourceError> {
        let vars: QueryVariables = serde_yaml::from_str(yaml).unwrap();
        vars.resolve_sources(base_dir)
    }

    #[test]
    fn resolve_sources_passes_literals_through() {
        let vars = resolve("env: prod\nlimits:\n  max: 4\n", Path::new(".")).unwrap();
        assert_eq!(vars.get("env"), Some(&VariableValue::String("prod".into())));
        match vars.get("limits") {
            Some(VariableValue::Map(m)) => assert_eq!(m.get("max"), Some(&VariableValue::Int(4))),
            other => panic!("expected map, got {:?}", other),
        }
    }

    #[test]
    fn resolve_sources_loads_files_at_any_depth() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("vars.yml"), "db:\n  host: localhost\n").unwrap();
        std::fs::write(dir.path().join("list.json"), r#"["a", "b"]"#).unwrap();
        std::fs::write(dir.path().join("vars.toml"), "port = 8080\n").unwrap();

        let vars = resolve(
            "settings:\n  $file: vars.yml\nteam:\n  prefixes:\n    $file: list.json\nnet:\n  $file: vars.toml\n",
            dir.path(),
        )
        .unwrap();

        // Whole YAML document bound under its name
        match vars.get("settings") {
            Some(VariableValue::Map(m)) => match m.get("db") {
                Some(VariableValue::Map(db)) => {
                    assert_eq!(db.get("host"), Some(&VariableValue::String("localhost".into())));
                }
                other => panic!("expected db map, got {:?}", other),
            },
            other => panic!("expected settings map, got {:?}", other),
        }
        // Directive nested below the top level
        match vars.get("team") {
            Some(VariableValue::Map(m)) => assert_eq!(
                m.get("prefixes"),
                Some(&VariableValue::Array(vec![
                    VariableValue::String("a".into()),
                    VariableValue::String("b".into()),
                ]))
            ),
            other => panic!("expected team map, got {:?}", other),
        }
        // TOML file
        match vars.get("net") {
            Some(VariableValue::Map(m)) => assert_eq!(m.get("port"), Some(&VariableValue::Int(8080))),
            other => panic!("expected net map, got {:?}", other),
        }
    }

    #[test]
    fn resolve_sources_literal_escape_hatch() {
        // $literal keeps its content verbatim — including `$` keys inside.
        let vars = resolve(
            "schema:\n  $literal:\n    $file: not-a-directive\n    $ref: kept\n",
            Path::new("."),
        )
        .unwrap();
        match vars.get("schema") {
            Some(VariableValue::Map(m)) => {
                assert_eq!(m.get("$file"), Some(&VariableValue::String("not-a-directive".into())));
                assert_eq!(m.get("$ref"), Some(&VariableValue::String("kept".into())));
            }
            other => panic!("expected map, got {:?}", other),
        }
    }

    #[test]
    fn resolve_sources_fails_loudly() {
        // Unknown directive (e.g. a typo) never passes through as data.
        let err = resolve("x:\n  $fiel: vars.yml\n", Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("unknown variable source directive `$fiel`"), "{}", err);
        assert!(err.to_string().contains("variables?x"), "{}", err);

        // $query is reserved, not silently data.
        let err = resolve("x:\n  $query:\n    xpath: //a\n", Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("not implemented"), "{}", err);

        // A `$` key mixed into an ordinary map is reserved.
        let err = resolve("x:\n  $file: vars.yml\n  other: 1\n", Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("reserved directive key"), "{}", err);

        // Missing file is fatal, with the lookup path in the message.
        let err = resolve("deep:\n  nested:\n    $file: nope.yml\n", Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("variables?deep?nested"), "{}", err);
        assert!(err.to_string().contains("cannot read"), "{}", err);

        // $file expects a string path.
        let err = resolve("x:\n  $file: [a, b]\n", Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("expects a path string"), "{}", err);
    }

    /// The reference scan decides resolution policy, so its edges are
    /// pinned here: own vs run namespace, and the deliberate
    /// over-approximation on string literals.
    #[test]
    fn entry_variables_records_what_the_expression_reads() {
        let vars: QueryVariables = serde_yaml::from_str("max: 4\n").unwrap();

        let reads_own = EntryVariables::new(
            vars.clone(),
            EntryKind::Rule,
            "//f[count(p) > $rule.variables?max]",
        );
        assert!(reads_own.bind(Path::new(".")).is_ok());
        assert!(!reads_own.reads_run_variables());

        let reads_run = EntryVariables::new(vars.clone(), EntryKind::Rule, "//a[$variables?env]");
        assert!(reads_run.reads_run_variables());

        // Another entry kind's namespace is not this entry's own.
        let other_kind = EntryVariables::new(
            vars.clone(),
            EntryKind::Rule,
            "//a[$mapping.variables?max]",
        );
        assert!(!other_kind.reads_run_variables());

        // `$variables` as a prefix of a longer name is not a read of it.
        assert!(!EntryVariables::new(vars.clone(), EntryKind::Rule, "//a[$variables-x]")
            .reads_run_variables());
        // ...but `$rule.variables` never counts as `$variables` either.
        assert!(!EntryVariables::new(vars.clone(), EntryKind::Rule, "//a[$rule.variables?max]")
            .reads_run_variables());

        // Over-approximation: a namespace name inside a string literal
        // counts as a read. XPath has no dynamic variable references, so
        // the scan can never MISS a real read; treating a literal as a
        // read only resolves a source needlessly, which is the safe
        // direction. This test pins that direction deliberately.
        assert!(
            EntryVariables::new(vars, EntryKind::Rule, "//a[. = '$variables?x']")
                .reads_run_variables(),
            "a literal occurrence must be treated as a read (conservative)"
        );
    }

    /// An entry that never reads its own namespace neither resolves its
    /// sources nor binds them raw — the invariant that downstream only
    /// ever sees plain resolved data.
    #[test]
    fn unread_sources_bind_empty_and_never_resolve() {
        let declared: QueryVariables =
            serde_yaml::from_str("data:\n  $file: does-not-exist.json\n").unwrap();

        // Reads its own namespace: resolution runs, and the missing file
        // is fatal.
        let read = EntryVariables::new(declared.clone(), EntryKind::Rule, "//a[$rule.variables?data]");
        assert!(read.bind(Path::new(".")).is_err(), "a read source must resolve (and fail loudly)");

        // Does not read it: no resolution (so a file a later operation
        // will write cannot fail this entry) and nothing raw is bound.
        let unread = EntryVariables::new(declared, EntryKind::Rule, "//a[@x = 1]");
        let bound = unread.bind(Path::new(".")).expect("unread sources must not resolve");
        assert!(bound.is_empty(), "unread sources must bind empty, not raw directives: {:?}", bound);
        assert!(!bound.has_sources(), "a raw `$file` directive must never reach the query context");

        // Plain values are bound as-is whether read or not.
        let plain: QueryVariables = serde_yaml::from_str("env: prod\n").unwrap();
        let unread_plain = EntryVariables::new(plain, EntryKind::Rule, "//a[@x = 1]");
        let bound = unread_plain.bind(Path::new(".")).unwrap();
        assert_eq!(bound.get("env"), Some(&VariableValue::String("prod".into())));
    }

    /// A source failure names the `variables:` block it came from — a
    /// rule's sources fail mid-run, so "somewhere in variables" is not
    /// enough to act on.
    #[test]
    fn source_errors_name_their_scope() {
        let vars: QueryVariables =
            serde_yaml::from_str("data:\n  $file: nope.json\n").unwrap();

        // Unscoped (config root).
        let err = vars.clone().resolve_sources(Path::new(".")).unwrap_err();
        assert!(err.to_string().starts_with("in variables?data:"), "{}", err);

        // Scoped by the owning entry.
        let err = vars
            .clone()
            .resolve_sources(Path::new("."))
            .unwrap_err()
            .in_scope("rule 'needs-baseline'");
        assert!(
            err.to_string().starts_with("in rule 'needs-baseline' variables?data:"),
            "{}", err,
        );

        // The outermost scope wins — an entry label is not overwritten.
        let err = vars
            .resolve_sources(Path::new("."))
            .unwrap_err()
            .in_scope("set mapping 2")
            .in_scope("something later");
        assert!(err.to_string().contains("set mapping 2"), "{}", err);

        // Paths render with forward slashes so they don't read as escapes.
        assert!(!err.to_string().contains('\\'), "{}", err);
    }

    #[test]
    fn to_json_roundtrip() {
        let vars: QueryVariables =
            serde_yaml::from_str("cfg:\n  hosts: [a, b]\n  port: 1\n").unwrap();
        let json = vars.get("cfg").unwrap().to_json();
        assert_eq!(json, serde_json::json!({"hosts": ["a", "b"], "port": 1}));
    }
}
