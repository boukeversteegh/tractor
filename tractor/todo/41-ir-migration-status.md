# IR migration status (snapshot 2026-05-06, updated)

## Done

### Python IR-only end-to-end ✅
All 274 transform tests + lib + integration suites pass with Python on
the IR pipeline. Imperative artefacts retired:
  - `src/languages/python/input.rs` (PyKind enum) — DELETED
  - `src/languages/python/rules.rs` (kind→Rule table) — DELETED
  - `src/languages/python/transformations.rs` (~1400 lines) — DELETED
  - `src/languages/python/transform.rs` (dispatcher) — DELETED
  - `LanguageOps.transform` for Python now uses `passthrough_transform`.
The IR-aware `python_post_transform` keeps chain inversion +
expression-host wrapping + the new `inject_python_visibility_markers`
+ extended `tag_multi_role_children` table. `merge_python_line_comments`
runs in `lower_block` for leading/trailing comment classification.

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
Updated 2026-05-06:

**Coverage**: 0 shape-contract errors against the Python blueprint
when Python is hard-switched to the IR pipeline. All previously-
identified issues addressed:
  - ✅ Splat shape — Ir::ListSplat / Ir::DictSplat render as
    `<spread>` with `<list/>`/`<dict/>` discriminator markers
    (matches imperative `RenameWithMarker(Spread, List)`).
  - ✅ Missing CST kinds (~13 added): `type`, `expression_list`,
    `pattern_list`, `constrained_type`, `splat_type`,
    `list_splat_pattern`, `block`, `await`, `as_pattern`,
    `as_pattern_target`, `list_pattern`, `tuple_pattern`,
    `union_pattern`, `with_clause`, `with_item`,
    `dictionary_comprehension`.
  - ✅ `<async/>` marker on async-with — `simple_statement_marked`
    detects the `async` keyword in source.
  - ✅ `<with><with>` and `<expression><expression>` nesting fixed
    (finally-clause inner block + assign-right expression-host
    bypass for already-wrapped values).
  - ✅ `<dictionary>` → `<dict>` (Python's vocabulary uses short
    form).
  - ✅ Operator marker map extended (`//`, `%`, `**`, `@`, bitwise,
    shifts).

**Resolved** (16 → 5 transform-test divergences this session):
  - ✅ `comments::python` — `merge_python_line_comments` post-pass
    classifies leading/trailing/floating (port of csharp's).
  - ✅ `if_else::python` — collect ALL `alternative` field children
    (not just first), chain into Ir::ElseIf/Ir::Else; ternary
    drops `<expression>` wrapper around `<then>`/`<else>` slots.
  - ✅ `operators::python_compare` — Ir::Comparison renders as
    `<compare>` with flat children (no `<left>`/`<right>` wrappers).
  - ✅ `functions::python_multi_value_return_lists_expressions` +
    `python::expression_list::python` — Return render emits each
    Inline child in its own `<expression>`; expression_list /
    pattern_list lower to Ir::Inline (transparent flatten).
  - ✅ `visibility::python` — new `inject_python_visibility_markers`
    post-pass adds `<public/>`/`<protected/>`/`<private/>` to
    class-method `<function>` elements based on Python's
    name-convention.
  - ✅ `collections::python_collections` — comprehension lowerings
    add `<comprehension/>` marker via simple_statement_marked.
  - ✅ `strings::python_fstring` / `strings::python_interpolation` —
    f-strings lift to SimpleStatement when CST has `interpolation`
    / `escape_sequence` children; plain strings stay scalar.

**Pending gating items** (5 transform-test divergences left):
  - `chain::python` + `chain::cross_language_uniformity` — chain
    inversion sees 2 `<object[access]>` instead of 1 for
    `obj.foo().bar.baz()`. Likely an accumulator edge case in
    `walk_chain` when calls + members alternate.
  - `errors::python` — `except ValueError as err:` should render as
    `<except><value><expression><as>...</as></expression></value></except>`.
    Current Ir::ExceptHandler render emits `<type>...<name>...`
    with separate fields. Needs a Python-flavoured ExceptHandler
    render variant or a parallel post-pass.
  - `patterns::python` — `[1, 2, *rest]` list pattern needs
    `<pattern[splat]><name>rest</name>` for the splat element.
  - `patterns::python_dict_pattern_lists_values` — dict-pattern key
    strings need `list="strings"` attr; the existing
    `tag_multi_same_name_children` should cover this but isn't
    triggering for some structural reason.

Plus the post-pass `tag_multi_role_children` advisory tally is
elevated under the hard switch (40 vs grandfathered 20). Bumping
the ratchet or adding the missing Python pairs unblocks that.

Estimate: 1–3 hours to close the remaining 5 + ratchet.

**Foundation done**: `tests/ir_python_missing_kinds.rs` is the
diagnostic for any future coverage push (run with `--ignored
--nocapture`).

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
