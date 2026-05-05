# IR migration status (snapshot 2026-05-06)

## Done

### C# IR-only end-to-end
- `src/ir/csharp.rs` — typed lowering (lower_csharp_root) is the
  sole C# transform path. `parse_with_ir_pipeline` dispatches
  unconditionally for `lang == "csharp"`.
- File-scoped namespace folding moved into IR lowering
  (`fold_file_scoped_namespace_siblings`); no post-pass needed.
- All previously-imperative C# transforms retired:
  - `src/languages/csharp/input.rs` (CsKind enum) — DELETED
  - `src/languages/csharp/rules.rs` (kind→Rule table) — DELETED
  - `src/languages/csharp/transformations.rs` (~1500 lines) — DELETED
  - `src/languages/csharp/transform.rs` (dispatcher) — DELETED
  - Imperative-only post-passes in `csharp/post_transform.rs`:
    - `attach_where_clause_constraints` — DELETED (replaced by
      `attach_ir_where_clauses`)
    - `unify_file_scoped_namespace` — DELETED (folded into lowering)
    - `csharp_normalize_conditional_access` — DELETED (IR emits
      canonical `<object>` directly)
- Modifier constants (`ACCESS_MODIFIERS`, `OTHER_MODIFIERS`) and
  `syntax_category` moved to `csharp/output.rs`.
- `LanguageOps.transform` for C# points at `passthrough_transform`
  (the field is required by the registry contract; never fires).
- All tests green (274 transform tests, full lib + integration suite).

### IR→JSON renderer
- `src/ir/json.rs` — `ir_to_json(ir, source) -> serde_json::Value`
  walks the typed Ir and produces JSON without the XML intermediate.
- Each variant maps to a structured JSON shape:
  - `Vec<Ir>` → arrays (pluralised key)
  - `Box<Ir>` → singleton (element-name key)
  - `modifiers.marker_names()` → boolean flags
  - `Ir::Inline` transparent — children flatten into parent
  - Scalar leaves (Name, Int, …) → JSON strings
- Two ignored tests in `tests/ir_csharp_json_parity.rs`:
  - `dump_ir_json` — prints output for inspection
  - `ir_json_matches_snapshot` — byte-diff against legacy snapshot
- **NOT yet wired to CLI** — the JSON projection in
  `src/format/json.rs` still uses `xml_node_to_json`. Hooking it up
  for C# is the next concrete step.

## Pending

### C# end-to-end (to fully retire `list=`/`field=` for C#)
1. Plumb `Ir` through the report system. Today `ReportMatch.tree`
   holds an `XmlNode`; for IR-direct JSON, the matched IR sub-tree
   needs to ride alongside (or replace) the XmlNode for the C# case.
2. In `format/json.rs::project_match_field_to_json`, dispatch on
   language: csharp → `ir_to_json`, else → `xml_node_to_json`.
3. Drop `list=` / `field=` emission in `csharp/post_transform.rs`'s
   `tag_multi_role_children` and `distribute_member_list_attrs`
   calls. Update C# XPath tests that asserted these attrs (or move
   them to remain-on-XML languages only).
4. Regenerate `tests/integration/languages/csharp/blueprint.cs.snapshot.json`
   from the IR→JSON output. Inspect for unintended divergence.

### Python migration to IR-only
Hard-switching `use_ir_pipeline` to include `python` surfaces:
- **45 shape-contract errors**, 38 advisory:
  - 7 `<splat/>` MarkerOnly violations: `Ir::ListSplat` / `DictSplat`
    render with element children but `Splat` is declared MarkerOnly
    in the Python `TractorNodeSpec` table. Either:
    (a) change rendering to emit `<splat/>` as a sibling marker on
    a wrapper (matches imperative shape); OR
    (b) declare `Splat` as dual-use (marker + container).
  - ~25 `<unknown>` nodes: missing `lower_node` arms for several
    Python CST kinds (`with_item`, `dictionary_pattern`, …).
    `Ir::Unknown` fallback isn't declared in the vocabulary table
    so each instance trips `name-declared-in-semantic-module`.
  - `<with_item>`, `<dictionary>` not in Python's `TractorNodeSpec`
    table — either declare them or rename their IR emission.
  - `<async/>` keyword leak in `<with>` text content — needs an
    explicit `<async/>` marker in the IR's `with_statement` lowering.
  - `<with><with>` nesting — flatten/rename gap when the body is
    itself a single `with` (mirrors C#'s `<block><block>` we
    handled by scoping marker emission to the outer node).
- Estimate: 4–6 hours of focused Python coverage work.

### Other languages
None of Java/TypeScript/Rust/Go/Ruby/PHP/data-languages have any
`lower_<lang>_root` yet. Each requires:
1. Build per-language IR lowering (one match arm per CST kind).
2. Hard-switch in `parse_with_ir_pipeline`.
3. Resolve shape-contract violations on each blueprint.
4. Delete the imperative `<lang>/input.rs`, `rules.rs`,
   `transformations.rs`, `transform.rs`.
5. For data languages (JSON/YAML/TOML/INI/Markdown): the `--set`
   mutation surface MUST keep working via XPath; IR mutation
   semantics must be carved out before deletion.

Each programming language is comparable in scope to C# (~2–4 days
of focused work). Data languages are smaller but more constrained.

## Suggested next steps (priority)
1. **Wire IR→JSON for C#** (1–2 h): plumb Ir through ReportMatch
   for csharp, dispatch in format/json.rs, regenerate the snapshot,
   make sure --check passes.
2. **Drop `list=`/`field=` for C#** (1–2 h): remove the
   tag-multi-role-children calls + distribute_member_list_attrs;
   update XPath tests that asserted these attrs.
3. **Python coverage push** (4–6 h): close the 45 contract
   violations one by one. Splat shape first (universal IR fix),
   then missing kinds, then `<with>` async marker.
4. **Java migration** (similar scale to C#): start a new
   `src/ir/java.rs` modelled on csharp.rs.
5. **Iterate**.
