//! Query operation: run XPath expressions against sources, return matches.

use tractor::normalized_xpath::NormalizedXpath;
use tractor::report::ReportBuilder;
use tractor::tree_mode::TreeMode;

use crate::matcher::validate_xpath_diagnostic;
use crate::input::filter::Filters;
use crate::input::Source;

use crate::cli::context::ExecCtx;

use super::{match_to_report_match, query_files_multi};

// ---------------------------------------------------------------------------
// Operation type
// ---------------------------------------------------------------------------

/// A query operation plan: run XPath expressions against sources, return matches.
///
/// Disk and inline inputs are already unified into `sources` at construction
/// time — the executor does not branch on input kind.
///
/// Multiple queries can target the same set of sources — each source is parsed
/// once and all XPath expressions are evaluated against it.
#[derive(Debug, Clone)]
pub struct QueryOperationPlan {
    /// Pre-resolved unified input list.
    pub sources: Vec<Source>,
    /// Pre-built result filters (diff-lines, etc.).
    pub filters: Filters,
    /// XPath queries to evaluate.
    pub queries: Vec<QueryExpr>,
    /// Tree mode override for parsing.
    pub tree_mode: Option<TreeMode>,
    /// Language override applied during parsing (per-op, overrides source's own).
    pub language: Option<String>,
    /// Maximum number of matches to return (across all queries).
    pub limit: Option<usize>,
    /// Ignore whitespace-only text nodes during parsing.
    pub ignore_whitespace: bool,
    /// Maximum parse depth.
    pub parse_depth: Option<usize>,
    /// Materialize results to a JSON file, keyed by source file — the
    /// artifact later operations consume via a `$file` variable source.
    pub output: Option<QueryOutput>,
}

/// Pre-resolution shape for a query operation. Mirrors [`QueryOperationPlan`]
/// but omits the input-resolution-derived fields (`sources`, `filters`).
/// Produced by the config parser and CLI layer, then turned into a
/// fully-resolved `QueryOperationPlan` by the planner via
/// [`QueryOperation::into_plan`].
#[derive(Debug, Clone)]
pub struct QueryOperation {
    /// XPath queries to evaluate.
    pub queries: Vec<QueryExpr>,
    /// Tree mode override for parsing.
    pub tree_mode: Option<TreeMode>,
    /// Language override applied during parsing (per-op, overrides source's own).
    pub language: Option<String>,
    /// Maximum number of matches to return (across all queries).
    pub limit: Option<usize>,
    /// Ignore whitespace-only text nodes during parsing.
    pub ignore_whitespace: bool,
    /// Maximum parse depth.
    pub parse_depth: Option<usize>,
    /// Materialize results to a JSON file (see [`QueryOutput`]).
    pub output: Option<QueryOutput>,
}

impl QueryOperation {
    /// Attach resolved inputs and produce the final executor-ready plan.
    pub fn into_plan(self, sources: Vec<Source>, filters: Filters) -> QueryOperationPlan {
        QueryOperationPlan {
            sources,
            filters,
            queries: self.queries,
            tree_mode: self.tree_mode,
            language: self.language,
            limit: self.limit,
            ignore_whitespace: self.ignore_whitespace,
            parse_depth: self.parse_depth,
            output: self.output,
        }
    }
}

/// Materialized-output settings for a query operation.
///
/// # Writing policy
///
/// Two operations write to disk, and they answer "may I write?" differently
/// on purpose, because they write different things:
///
/// - `set` and `update` mutate **their own inputs**, so whether they may
///   write depends on what those inputs are — an inline or stdin source has
///   no file to write back to. [`SetWriteMode`](super::SetWriteMode) owns
///   that decision and routes such a run to `Capture` instead.
/// - `output:` writes an **artifact the user named**, which exists only
///   because it was declared. It is the point of the option, not a side
///   effect of reading, so it is written unconditionally and its path never
///   depends on the input mode.
///
/// The input mode still matters for the *contents*: the index is keyed by
/// source path, so matches from virtual sources are not indexable and are
/// left out (see `write_query_output`).
///
/// A future `--dry-run` would gate both, and that is where the two meet —
/// not here.
///
/// The written file is a JSON index keyed by source file:
/// `{ "files": { "<path>": [ <entry>, ... ] } }`. File keys are relative to
/// the run's base dir when possible, so the artifact is stable and
/// committable. The per-file grouping is not optional — it is what makes
/// incremental merges sound: on each run, entries for every *queried* file
/// are replaced wholesale (an empty result removes the key), entries for
/// files outside the queried set (e.g. excluded by `--diff-files`) are
/// kept, and entries whose file no longer exists are pruned.
#[derive(Debug, Clone)]
pub struct QueryOutput {
    /// Output path, relative to the run's base dir.
    pub file: String,
    /// Fields stored per entry. `[Value]` (the default) writes bare value
    /// strings; anything else writes one JSON object per match.
    pub view: Vec<QueryOutputField>,
}

impl QueryOutput {
    /// Check that a configured output path stays inside the config
    /// directory: relative, with no `..`.
    ///
    /// The check is **lexical, on the raw string**, deliberately not via
    /// `Path`. A `tractor.yml` is committed and read on every platform, so
    /// `output: "C:\gathered.json"` has to mean the same thing everywhere —
    /// but `Path` is platform-dependent: on Unix that string is a single
    /// ordinary filename, while on Windows it is a drive-qualified path
    /// that escapes the config directory. A config's meaning must not
    /// depend on which machine loaded it.
    ///
    /// Rejected: a leading `/` or `\` (rooted — on Windows `join` keeps
    /// only the drive and drops the base directory, and `is_absolute` does
    /// not catch it), a `<letter>:` prefix (absolute or drive-relative),
    /// any `..` segment, and the empty path.
    ///
    /// One owner, called from config load (so a bad path fails before any
    /// work) and again at the write (which programmatic plans reach
    /// without passing through config loading).
    pub fn validate_path(file: &str) -> Result<(), String> {
        let reject = |why: &str| {
            Err(format!(
                "query output '{}' must be a relative path inside the config directory ({})",
                file, why
            ))
        };
        if file.is_empty() {
            return reject("it is empty");
        }
        if file.starts_with('/') || file.starts_with('\\') {
            return reject("it starts at a filesystem root");
        }
        let mut chars = file.chars();
        if matches!(
            (chars.next(), chars.next()),
            (Some(letter), Some(':')) if letter.is_ascii_alphabetic()
        ) {
            return reject("it names a drive");
        }
        if file.split(['/', '\\']).any(|segment| segment == "..") {
            return reject("it walks out with `..`");
        }
        Ok(())
    }
}

/// A field of a materialized query-output entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryOutputField {
    Value,
    Line,
    Column,
    Tree,
}

impl QueryOutputField {
    /// The field's name as written in config and in multi-field entries.
    pub fn name(self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::Line => "line",
            Self::Column => "column",
            Self::Tree => "tree",
        }
    }

}

impl std::str::FromStr for QueryOutputField {
    type Err = String;

    /// Parse a config `view:` field name, so the config parser can use
    /// `parse()` like every other field.
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "value" => Ok(Self::Value),
            "line" => Ok(Self::Line),
            "column" => Ok(Self::Column),
            "tree" => Ok(Self::Tree),
            other => Err(format!(
                "invalid query output view field '{}': use value, line, column, or tree",
                other
            )),
        }
    }
}

/// A single XPath query expression.
#[derive(Debug, Clone)]
pub struct QueryExpr {
    /// XPath expression to evaluate.
    pub xpath: NormalizedXpath,
    /// Query-level variables, bound as the `$query.variables` map in this
    /// expression (e.g. `//user[@role = $query.variables?role]`).
    /// Build via [`QueryExpr::new`] so the variables' resolution flags are
    /// always derived from the xpath that actually runs.
    pub variables: tractor::EntryVariables,
}

impl QueryExpr {
    /// Build a query expression, deriving from `xpath` what its variables read.
    pub fn new(xpath: impl Into<NormalizedXpath>, variables: tractor::QueryVariables) -> Self {
        let xpath = xpath.into();
        let variables =
            tractor::EntryVariables::new(variables, tractor::EntryKind::Query, xpath.as_str());
        QueryExpr { xpath, variables }
    }
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

pub(crate) fn execute_query(
    op: &QueryOperationPlan,
    ctx: &ExecCtx<'_>,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    // Validate all XPath expressions upfront — add fatal diagnostics on failure
    let diagnostics: Vec<_> = op.queries.iter()
        .filter_map(|q| validate_xpath_diagnostic(&q.xpath, "query"))
        .collect();
    if !diagnostics.is_empty() {
        report.add_all(diagnostics);
        return Ok(());
    }

    // The bindings each query runs with, built once and used for both the
    // advisory pass and execution — what is diagnosed is what is bound.
    let variables = ctx.query_variables();
    let queries: Vec<(&str, tractor::QueryBindings)> = op.queries.iter()
        .map(|q| (
            q.xpath.as_str(),
            tractor::QueryBindings::run(std::sync::Arc::clone(&variables))
                .with_entry(tractor::EntryContext::query(
                    std::sync::Arc::clone(q.variables.declared()),
                )),
        ))
        .collect();

    // Advisory: warn about variable lookups that silently yield the empty
    // sequence (unknown keys, namespaces of other entry kinds).
    for (i, (xpath, bindings)) in queries.iter().enumerate() {
        report.add_all(crate::matcher::variable_diagnostics("query", bindings, i, xpath));
    }

    if op.sources.is_empty() {
        return Ok(());
    }

    let matches = query_files_multi(
        &op.sources, &queries, op.language.as_deref(),
        op.tree_mode, op.ignore_whitespace, op.parse_depth,
        op.limit, ctx.verbose, &op.filters,
    )?;

    if let Some(output) = &op.output {
        let base_dir = ctx.base_dir.unwrap_or_else(|| std::path::Path::new("."));
        write_query_output(output, &matches, &op.sources, base_dir)?;
    }

    report.add_all(matches.into_iter().map(|m| match_to_report_match(m, "query")));

    Ok(())
}

// ---------------------------------------------------------------------------
// Materialized output
// ---------------------------------------------------------------------------

/// Write (or incrementally merge) query results into the output file.
///
/// Merge semantics: entries for every queried source file are replaced
/// wholesale by this run's results (zero matches removes the key); entries
/// for files outside the queried set are kept; entries whose file no longer
/// exists on disk are pruned. Keys sort deterministically for stable diffs.
fn write_query_output(
    output: &QueryOutput,
    matches: &[tractor::Match],
    sources: &[Source],
    base_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::BTreeMap;

    let base_key = normalize_key(&base_dir.to_string_lossy());
    let relativize = |path: &str| -> String {
        let p = normalize_key(path);
        match p.strip_prefix(&format!("{}/", base_key)) {
            Some(rel) if base_key != "." => rel.to_string(),
            _ => p,
        }
    };

    // Disk sources only, on both sides of the merge. The index is keyed by
    // path, and a virtual (inline / stdin) source has no path that means
    // anything to a later run — its matches are not indexable, so they are
    // left out rather than written and pruned again.
    //
    // This compares a match's file against the source paths, which works
    // because `query_files_multi` parses with `source.path.as_str()` and
    // hands that same string to every match: a match's file *is* its
    // source's path, not an independently derived spelling of it. Anything
    // constructing matches by another route (a test fabricating a path from
    // `tempdir()`, say) can produce a string that normalizes differently
    // and will silently match nothing.
    let indexable = |path: &str| -> bool {
        sources
            .iter()
            .any(|s| !s.is_virtual() && relativize(s.path.as_str()) == relativize(path))
    };

    // This run's results, grouped per file.
    let mut fresh: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();
    for m in matches.iter().filter(|m| indexable(&m.file)) {
        fresh.entry(relativize(&m.file)).or_default().push(output_entry(output, m));
    }

    // The queried set: files whose entries this run owns.
    let queried: std::collections::BTreeSet<String> = sources
        .iter()
        .filter(|s| !s.is_virtual())
        .map(|s| relativize(s.path.as_str()))
        .collect();

    // Config-loaded plans are checked at load (`convert_query`), where the
    // path is knowable and an error costs nothing. This guard covers plans
    // built programmatically, which bypass that path entirely — the write
    // itself is where the promise has to hold.
    QueryOutput::validate_path(&output.file)?;
    let out_path = base_dir.join(&output.file);

    // Start from the existing index when merging is possible.
    let mut files: BTreeMap<String, serde_json::Value> = match std::fs::read_to_string(&out_path) {
        Ok(existing) => {
            let parsed: serde_json::Value = serde_json::from_str(&existing).map_err(|e| {
                format!(
                    "existing query output '{}' is not valid JSON ({}); delete it to regenerate",
                    out_path.display(), e
                )
            })?;
            match parsed.get("files").and_then(|f| f.as_object()) {
                Some(obj) => obj.clone().into_iter().collect(),
                None => {
                    return Err(format!(
                        "existing file '{}' is not a tractor query output (expected a top-level \
                         \"files\" object); delete it or choose another output path",
                        out_path.display()
                    ).into());
                }
            }
        }
        Err(_) => BTreeMap::new(),
    };

    // Replace everything this run queried, then overlay fresh results.
    files.retain(|key, _| !queried.contains(key));
    for (key, entries) in fresh {
        files.insert(key, serde_json::Value::Array(entries));
    }

    // Prune entries whose file no longer exists (deletes / renames).
    files.retain(|key, _| {
        let p = std::path::Path::new(key);
        if p.is_absolute() { p.exists() } else { base_dir.join(p).exists() }
    });

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let doc = serde_json::json!({ "files": files });
    std::fs::write(&out_path, format!("{}\n", serde_json::to_string_pretty(&doc)?))?;
    Ok(())
}

/// Normalize a path string for use as an index key (forward slashes, no
/// trailing slash) so keys compare consistently across platforms and runs.
fn normalize_key(path: &str) -> String {
    let p = path.replace('\\', "/");
    p.trim_end_matches('/').to_string()
}

/// One field of a match as JSON. `Tree` is `None` for atomic matches, which
/// carry no node.
fn field_value(field: QueryOutputField, m: &tractor::Match) -> Option<serde_json::Value> {
    match field {
        QueryOutputField::Value => Some(serde_json::Value::String(m.value.clone())),
        QueryOutputField::Line => Some(serde_json::Value::from(m.line)),
        QueryOutputField::Column => Some(serde_json::Value::from(m.column)),
        QueryOutputField::Tree => m.xml_node.as_ref().map(|n| tractor::xml_node_to_json(n, None)),
    }
}

/// Build one output entry for a match: always an object whose keys are the
/// configured `view:` fields.
///
/// The shape never depends on how many fields were selected, or on what a
/// match happens to contain — `view:` chooses which keys appear, and
/// nothing else. Unwrapping a lone field would make a config's structure
/// change silently when a second field is added, and would make the
/// artifact's shape a function of its content rather than of its
/// parameters. It also keeps the file consistent with the JSON report,
/// where a match is always a labelled object.
fn output_entry(output: &QueryOutput, m: &tractor::Match) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    for field in &output.view {
        if let Some(value) = field_value(*field, m) {
            obj.insert(field.name().to_string(), value);
        }
    }
    serde_json::Value::Object(obj)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tractor::report::ReportBuilder;
    use tractor::NormalizedPath;
    use crate::cli::context::ExecCtx;
    use crate::executor::{OperationPlan, execute};

    fn temp_json_file(content: &str) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        std::fs::write(&path, content).unwrap();
        (dir, path.to_str().unwrap().to_string())
    }

    fn disk_source(path: &str) -> Source {
        let np = NormalizedPath::absolute(path);
        let lang = tractor::detect_language(np.as_str()).to_string();
        Source::disk(np, lang)
    }

    fn run_query_ops(ops: &[OperationPlan]) -> tractor::report::Report {
        let mut builder = ReportBuilder::new();
        builder.set_no_verdict();
        execute(ops, &ExecCtx::default(), &mut builder).unwrap();
        builder.build()
    }

    #[test]
    fn query_returns_matches() {
        let (_dir, path) = temp_json_file(r#"{"name": "alice", "age": 30}"#);
        let ops = vec![OperationPlan::Query(QueryOperationPlan {
            sources: vec![disk_source(&path)],
            filters: Filters::default(),
            queries: vec![QueryExpr::new("//name", tractor::QueryVariables::new())],
            tree_mode: None,
            language: None,
            limit: None,
            ignore_whitespace: false,
            parse_depth: None,
            output: None,
        })];
        let report = run_query_ops(&ops);
        assert!(report.success.is_none());
        assert_eq!(report.all_matches().len(), 1);
        assert_eq!(report.all_matches()[0].value.as_deref(), Some("alice"));
    }

    #[test]
    fn query_with_limit() {
        let (_dir, path) = temp_json_file(r#"{"a": 1, "b": 2, "c": 3}"#);
        let ops = vec![OperationPlan::Query(QueryOperationPlan {
            sources: vec![disk_source(&path)],
            filters: Filters::default(),
            queries: vec![QueryExpr::new("//*[number(.) > 0]", tractor::QueryVariables::new())],
            tree_mode: None,
            language: None,
            limit: Some(2),
            ignore_whitespace: false,
            parse_depth: None,
            output: None,
        })];
        let report = run_query_ops(&ops);
        assert!(report.all_matches().len() <= 2);
    }

    /// Referencing another entry kind's namespace (or $rule.id) in a query
    /// produces an advisory warning — it silently evaluates against an
    /// empty map there.
    #[test]
    fn query_warns_on_cross_namespace_reference() {
        use tractor::report::Severity;

        let (_dir, path) = temp_json_file(r#"{"debug": true}"#);
        let ops = vec![OperationPlan::Query(QueryOperationPlan {
            sources: vec![disk_source(&path)],
            filters: Filters::default(),
            queries: vec![QueryExpr::new(
                "//debug[$rule.variables?max > 1]",
                tractor::QueryVariables::new(),
            )],
            tree_mode: None,
            language: None,
            limit: None,
            ignore_whitespace: false,
            parse_depth: None,
            output: None,
        })];
        let report = run_query_ops(&ops);
        let warnings: Vec<_> = report.all_matches().into_iter()
            .filter(|m| m.severity == Some(Severity::Warning))
            .collect();
        assert_eq!(warnings.len(), 1, "{:?}", report.all_matches());
        assert!(warnings[0].reason.as_deref().unwrap().contains("only bound in check rules"));
        // The query itself still runs and matches nothing (empty lookup).
        assert!(report.all_matches().iter().all(|m| m.severity == Some(Severity::Warning)));
    }

    /// The output path must stay inside the config directory. `is_absolute`
    /// alone misses the Windows forms that still escape, so every rooted or
    /// drive-qualified shape is rejected explicitly.
    #[test]
    fn query_output_path_must_stay_inside_the_config_directory() {
        for ok in ["out.json", "gathered/repos.json", "./a/b.json"] {
            assert!(QueryOutput::validate_path(ok).is_ok(), "{} should be allowed", ok);
        }
        for escaping in [
            "../outside.json",       // parent traversal
            "a/../../outside.json",  // traversal after a descent
            "/rooted.json",          // rooted, no drive: is_absolute() is false on Windows
            "\\rooted.json",         // the same with a backslash
            "C:\\abs.json",          // fully absolute
            "C:rel.json",            // drive-relative
        ] {
            let err = match QueryOutput::validate_path(escaping) {
                Err(e) => e,
                Ok(()) => panic!("{} should be rejected", escaping),
            };
            assert!(err.contains("must be a relative path"), "{}: {}", escaping, err);
        }
    }

    /// Entries are always labelled objects: `view:` selects which keys
    /// appear and nothing else, so adding a field never changes the shape
    /// of the ones already there.
    #[test]
    fn query_output_entries_are_always_labelled() {
        use tractor::XmlNode;

        let mut m = tractor::Match::new("a.json".to_string(), "Name".to_string());
        m.line = 7;
        m.xml_node = Some(XmlNode::Map {
            entries: vec![
                ("name".to_string(), XmlNode::Text("Name".into())),
                ("max".to_string(), XmlNode::Text("256".into())),
            ],
        });

        // The default view — still an object, not a bare string.
        let default_view = QueryOutput { file: "o.json".into(), view: vec![QueryOutputField::Value] };
        assert_eq!(output_entry(&default_view, &m), serde_json::json!({"value": "Name"}));

        // A structured match keeps its map under the `tree` key.
        let tree = QueryOutput { file: "o.json".into(), view: vec![QueryOutputField::Tree] };
        assert_eq!(
            output_entry(&tree, &m),
            serde_json::json!({"tree": {"name": "Name", "max": "256"}}),
        );

        // Adding a field only adds a key; `value` is unchanged.
        let two = QueryOutput {
            file: "o.json".into(),
            view: vec![QueryOutputField::Value, QueryOutputField::Line],
        };
        assert_eq!(output_entry(&two, &m), serde_json::json!({"value": "Name", "line": 7}));
    }

    /// Merge semantics of the materialized output: queried files are
    /// replaced wholesale, unqueried files keep their entries, and entries
    /// whose file no longer exists are pruned.
    #[test]
    fn query_output_incremental_merge() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.json"), "{}").unwrap();
        std::fs::write(dir.path().join("b.json"), "{}").unwrap();
        // "gone.json" is referenced by the existing index but no longer exists.
        std::fs::write(
            dir.path().join("out.json"),
            r#"{ "files": { "a.json": ["old-a"], "b.json": ["old-b"], "gone.json": ["x"] } }"#,
        ).unwrap();

        let output = QueryOutput {
            file: "out.json".into(),
            view: vec![QueryOutputField::Value],
        };
        // This run queried only b.json and found one match. The match path
        // comes from the source, exactly as `query_files_multi` produces it
        // — fabricating it from `tempdir()` instead would diverge wherever
        // the two normalize differently (on a Windows CI runner the source
        // resolves the 8.3 alias `RUNNER~1` to `runneradmin`, the raw temp
        // path does not, and nothing matches).
        let b_path = dir.path().join("b.json");
        let sources = vec![disk_source(b_path.to_str().unwrap())];
        let matches = vec![tractor::Match::new(
            sources[0].path.as_str().to_string(),
            "new-b".to_string(),
        )];

        write_query_output(&output, &matches, &sources, dir.path()).unwrap();

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.path().join("out.json")).unwrap()).unwrap();
        let files = written.get("files").unwrap().as_object().unwrap();
        assert_eq!(files.get("a.json").unwrap(), &serde_json::json!(["old-a"]), "unqueried file kept");
        assert_eq!(
            files.get("b.json").unwrap(),
            &serde_json::json!([{"value": "new-b"}]),
            "queried file replaced",
        );
        assert!(files.get("gone.json").is_none(), "missing file pruned: {:?}", files);
    }

    #[test]
    fn query_empty_sources() {
        let ops = vec![OperationPlan::Query(QueryOperationPlan {
            sources: vec![],
            filters: Filters::default(),
            queries: vec![QueryExpr::new("//x", tractor::QueryVariables::new())],
            tree_mode: None,
            language: None,
            limit: None,
            ignore_whitespace: false,
            parse_depth: None,
            output: None,
        })];
        let report = run_query_ops(&ops);
        assert_eq!(report.all_matches().len(), 0);
        assert!(report.success.is_none());
    }
}
