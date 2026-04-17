# `-p` / `--project` Projection Flag

**Date:** 2026-04-17
**Status:** Design proposal
**Related:** [#120](https://github.com/boukeversteegh/tractor/issues/120), `specs/report-model.md`, `todo/7-count-schema-short-circuit.md`

## Goals

- Let users emit a projection of the report (e.g. just the tree, just the summary) without the `<report>/<results>/<match>` envelope.
- Unify the `-v count` / `-v schema` short-circuit with the normal render pipeline.
- Preserve two contracts: (a) structured-format output is always valid (XML parses, JSON parses); (b) output structure is determined by input flags, never by result cardinality.
- Keep the flag surface small — one new flag with a closed set of values.
- Stay compatible with existing `-v` behavior. No breaking changes to existing `-v` flags.

## Non-goals

- Do **not** support arbitrary XPath in `-p` yet. Closed enum of shorthands only.
- Do **not** resolve the larger `-p` design questions in `specs/report-model.md` (multi-`-p` chaining, map-vs-reduce semantics, AST boundary, full XPath). Those remain open.
- Do **not** replace or deprecate `-v`. `-v` keeps its current role: determining which fields exist in each match.
- Do **not** auto-switch output shape based on result count. Same flags → same shape, always.

## Design principle: `-p` values are report element names

Every `-p` value refers to a concrete element in the report XML. When the user writes `-p tree`, they are selecting `<tree>` elements from the report. When they write `-p summary`, they are selecting the `<summary>` element. This keeps the interface learnable — the report is the API, and `-p` is navigation over it.

This principle drives two report-structure changes below.

## Report structure changes (prerequisite)

The current report has two structural issues that `-p` exposes. Fixing them is part of this refactor.

### Issue 1: `<results>` naming collision

Today `<results>` is used twice:
- Inside `<totals>` as a scalar count (number of matches).
- At the top level as the list wrapper containing matches / groups.

With `-p results` meaning "the list wrapper", the scalar inside totals needs a new name.

**Change:** rename `<totals>/<results>` → `<totals>/<matches>`. Reads naturally — `totals.matches` is the number of matches, consistent with `totals.files`, `totals.errors`, etc.

### Issue 2: Summary fields are loose at the top level

Today `<success>`, `<totals>`, `<expected>`, `<query>` are all direct children of `<report>`, alongside `<results>`. There's no container that groups them, so `-p summary` can't map to a real element.

**Change:** introduce a `<summary>` container that wraps the verdict/metadata fields.

### Issue 3: `<schema>` has no XML representation

Today `-v schema` bypasses the XML serializer entirely — it prints the text-mode schema rendering even when `-f xml` is requested. `-p schema` needs a real `<schema>` element in the report.

**Change:** when schema is computed, emit it as a `<schema>` element inside the report, serialized per format like any other element.

### Revised report structure

```xml
<report>
  <summary>
    <success>true</success>
    <totals>
      <matches>2</matches>
      <files>1</files>
      <errors>0</errors>
    </totals>
    <expected>2</expected>
    <query>//a</query>
  </summary>
  <schema>
    <!-- structural overview when -v schema or -p schema -->
  </schema>
  <results>
    <match file="..." line="1" column="1">
      <tree>...</tree>
      <value>...</value>
    </match>
    <match>...</match>
  </results>
</report>
```

Every element here is addressable via `-p`.

## Problem

Today, `-v tree` narrows *which fields* render inside each match, but structured formats still wrap output in the full report envelope:

```bash
$ echo '<root><a>1</a></root>' | tractor -l xml -x '//a' -v tree -f xml
<?xml version="1.0" encoding="UTF-8"?>
<report>
  <results>
    <match file="<stdin>" line="1" column="1">
      <tree>
        <a>1</a>
      </tree>
    </match>
  </results>
</report>
```

The user wanted just `<a>1</a>`. Text mode already does this by accident. `-v count` and `-v schema` bypass the envelope today, but via an ad-hoc short-circuit in `tractor/src/cli/query.rs:133-142`.

## Core insight

The envelope question dissolves if we separate two concerns:

- **`-v` — report construction.** Which fields are computed and included in each `<match>`.
- **`-p` — report projection.** Which element from the built report is emitted.

Both flags coexist. Neither replaces the other.

```
source → parse → -x (query) → build report (using -v) → project (-p) → serialize (-f)
                                 ↑                        ↑
                                 what's in the report     what comes out
```

## The `-p` / `--project` flag

### Values

All values correspond to elements in the revised report structure:

| `-p` value | Projects to | Effect on `-v` |
|---|---|---|
| `tree` | `<tree>` elements, one per match | **Replace** with `[tree]` |
| `value` | `<value>` elements, one per match | **Replace** with `[value]` |
| `source` | `<source>` elements, one per match | **Replace** with `[source]` |
| `lines` | `<lines>` elements, one per match | **Replace** with `[lines]` |
| `schema` | The `<schema>` element | **Replace** with `[schema]` (expensive — only compute when requested) |
| `summary` | The `<summary>` container | None — `-v` doesn't apply |
| `totals` | The `<totals>` element inside summary | None — `-v` doesn't apply |
| `results` | The `<results>` list wrapper (list of matches) | None — respects user's `-v` |
| `report` | The whole `<report>` (default when `-p` omitted) | None — respects user's `-v` |

### The `-v` replacement rule

When `-p` is a **view-level field** (`tree`, `value`, `source`, `lines`, `schema`), it **replaces** the view set with exactly `[that field]`. Reason: `-p tree` means "I just want trees" — keeping `-v file,source` alongside would compute data that's thrown away. If the user wanted multiple fields per match, they'd use `-p results` (which respects `-v`) instead.

When `-p` is **structural** (`results`, `report`), `-v` is respected — those projections emit per-match content, so `-v` still drives what each match contains.

When `-p` is a **metadata container** (`summary`, `totals`), `-v` is untouched but irrelevant — these elements don't contain per-match fields, so the view set has nothing to influence.

Schema is notable: it's the only view-level field with real computation cost. The replacement rule ensures `-p schema` always triggers schema computation, and `-p tree` (or any other projection) never does.

### Warning on discarded `-v` fields

When `-p` replaces the view set, any fields the user explicitly passed to `-v` that aren't the new single field are silently discarded by the replacement rule. That would be confusing — the user wrote them, expected them, and won't see them.

**Rule:** warn on stderr when all of the following hold:

1. The user explicitly passed `-v` (not the default view).
2. `-p` is a view-level field (`tree`/`value`/`source`/`lines`/`schema`).
3. The user's `-v` contains fields other than the one `-p` resolved to.

Warning text should name the dropped fields and point to the fix:

```
warning: -v fields {file, source} were discarded because -p tree replaces the view set.
  To keep -v intact, use `-p results` (respects -v) instead of `-p tree`.
```

No warning for:
- Redundant overlap: `-v tree -p tree` — user's intent is preserved.
- Structural / metadata `-p`: `-p results`, `-p report`, `-p summary`, `-p totals` don't replace `-v`.
- Default `-v` (no explicit flag): the replacement can't surprise a user who didn't specify anything.

This is a warning, not an error — combinations like `-v tree,file -p tree` are malformed but not fatal, and tractor should produce the output anyway.

### Pipeline ordering: early normalization

The `-p` replacement of `-v` is an **early-stage normalization**, applied right after flag parsing. It is not the final view set the renderer sees.

Downstream stages still run as today. In particular, each output format may adjust the view set to include fields it structurally requires — e.g. formats that always emit location may add `file`/`line`/`column`, and the per-match renderer injects diagnostic extras (`severity`, `reason`, `origin`, `lines`) when a match has them, via `render_fields_for_match` in `tractor/src/format/shared.rs:64`. That behavior remains intact.

So the order is:

```
user flags → -p replaces -v (early)  →  format-layer adjustments  →  renderer
```

The replacement rule sets the *user-intended* view. Formats retain the freedom to add what they need on top.

### When `-p` is omitted

Default: `-p report` (the full tree). Preserves today's behavior exactly — `-v` drives per-match fields, the whole report is emitted.

### When `-p` is omitted

Default: `-p report` (the full tree). Preserves today's behavior exactly.

## The `--single` flag

`-p` projects to element types. A projection to `<tree>` generally returns a *sequence* of tree elements. Users often want exactly one, bare, no list wrapper. `--single` expresses that.

### Rule

`--single` is a modifier on projection. It:

1. Limits match processing to the first match (implicit `-n 1`).
2. Strips list wrappers from the output — the projected element is emitted bare, no `<results>` root, no JSON array, no YAML sequence.

When `-p` is omitted, `--single` implies `-p results` (not `-p report`) — asking for one thing while emitting the whole singular report is a no-op, which is never what the user meant.

### Composition ladder

The cleanest way to see how `-v`, `-p`, and `--single` compose:

| Command | Shape |
|---|---|
| `-v tree` (no `-p`) | `<report>…<results><match><tree/></match>…</results></report>` |
| `-v tree --single` | `<match line="1" column="1"><tree/></match>` (one match bare) |
| `-p tree` | `<results><tree/><tree/>…</results>` (list of trees) |
| `-p tree --single` | `<tree/>` (one tree bare — the #120 snapshot case) |
| `-p summary` | `<summary>…</summary>` (already singular) |
| `-p summary --single` | `<summary>…</summary>` (no-op — already one element) |
| `-p schema --single` | `<schema>…</schema>` (no-op) |

Each row is one step toward "less wrapping". Every step is input-driven. `--single` always means "first, bare". `-p` always means "which element type".

### Content-independence holds

`--single` is an *input*. Flags determine shape regardless of how many matches the query hits:

- `--single` + 0 matches → empty stdout, non-zero exit.
- `--single` + N≥1 matches → take the first. Never a list.

No inspection of result cardinality; the shape is decided before the query runs.

## Interaction with `-v`

### For per-match field values of `-p`

When `-p` is a per-match field (`tree`, `value`, `source`, `lines`), `-p` **replaces** the view set with just that field. Reason: `-p tree` means "I just want trees" — keeping `-v file,source` alongside would compute data that's thrown away.

### For structural values of `-p`

When `-p` is `results` or `report`, `-v` is **respected** — these project to structures that contain per-match fields, so `-v` still drives what those matches look like.

### For aggregate values of `-p`

When `-p` is `summary` or `totals`, `-v` is **irrelevant** — these don't render per-match fields.

`schema` is a view-level field, not a metadata container — `-p schema` replaces `-v` with `[schema]` (same rule as `tree`/`value`/etc.). The distinction matters because schema has real computation cost and must be triggered by an explicit request.

### Summary table

| `-p` kind | Values | `-v` interaction |
|---|---|---|
| Per-match field | `tree`, `value`, `source`, `lines` | Replace — view becomes just that field |
| Structural | `results`, `report` | Respect — `-v` drives per-match field set |
| Aggregate | `summary`, `totals`, `schema` | Irrelevant |

## Interaction with `-f` (format)

Each format serializes the projection:

| Format | `-p` returns sequence (N nodes) | `-p` returns single node | With `--single` |
|---|---|---|---|
| `text` | Newline-separated | Rendered inline | Same, one element |
| `json` | Top-level array | Top-level object/value | Bare value / object |
| `yaml` | Top-level sequence | Top-level mapping | Bare mapping |
| `xml` | `<results>…</results>` root ¹ | Single root element | Bare root element |

¹ XML requires a single root for validity. The stable wrapper is `<results>` for any multi-node per-match projection (e.g. `-p tree`). For singular projections (`-p summary`, `-p schema`, `-p totals`), the element itself is the root.

## Interaction with `-n`

Unchanged. `-n` limits the number of matches processed. `--single` implies `-n 1`; explicit `-n 1 --single` is redundant but fine. `-p tree -n 2` (no `--single`) emits a list of up to two trees with the `<results>` root.

## Behavior examples

### Issue #120 — snapshot use case

```bash
$ tractor file.cs -p tree --single -f xml
<?xml version="1.0" encoding="UTF-8"?>
<unit>
  <class>...</class>
</unit>

$ tractor file.cs -p tree --single -f json
{"unit": {"class": ...}}
```

### Count / schema — replacing the short-circuit

```bash
$ tractor src/**/*.cs -x '//method' -p totals -f json
{"matches": 42, "files": 7}

$ tractor src/**/*.cs -x '//class' -p schema -f text
<class>
  <name/>
  <body>
    <method/>
  </body>
</class>

$ tractor src/**/*.cs -x '//class' -p schema -f xml
<?xml version="1.0" encoding="UTF-8"?>
<schema>
  ...
</schema>
```

Today `-v count` / `-v schema` bypass the renderer via `cli/query.rs:133-142`. With `-p`, they go through the normal pipeline: build report → project → serialize. The short-circuit can be removed. Closes `todo/7-count-schema-short-circuit.md`.

### Summary-only projection

```bash
$ tractor check src/**/*.cs -x '//comment[contains(.,"TODO")]' --reason TODO -p summary -f json
{
  "success": false,
  "totals": {"matches": 3, "files": 2, "errors": 3},
  "query": "//comment[contains(.,\"TODO\")]"
}
```

### Results list without report envelope

```bash
$ tractor src/**/*.cs -x '//function' -p results -f json
[
  {"file": "src/a.cs", "line": 5, "column": 1, "tree": {...}},
  {"file": "src/b.cs", "line": 12, "column": 1, "tree": {...}}
]
```

## Migration of existing behavior

### `-v count` / `-v schema` short-circuit (cleanup)

`tractor/src/cli/query.rs:133-142` short-circuits before the renderer when `-v count` or `-v schema` is requested. With this design:

1. Schema is emitted as a real `<schema>` element in the report.
2. `-v count` stays as a view-level field but now routes through the renderer. For XML/JSON/YAML, the output is wrapped in the envelope today (breaking change) unless `--single` or `-p` is used. For text, today's bare scalar is preserved (text has no envelope).
3. Users wanting bare count migrate to `-p totals --single` or `-p totals`.

If backwards-compat is a concern, `-v count` without `-p`/`--single` can keep the bare-scalar behavior for one version and emit a deprecation note pointing to `-p totals`.

### Report shape changes

Introducing `<summary>` and `<schema>`, and renaming `<totals>/<results>` → `<totals>/<matches>`, changes the XML/JSON/YAML shape of reports. Snapshot tests under `tests/integration/languages/*/.xml` and `tests/integration/formats/snapshots/` regenerate. No code-level compatibility layer needed — the report model is an output contract, not a stable API surface.

### Spec update

`specs/cli-output-design.md:305-355` declares "the report envelope is always present, in every format, for every command." Update to state:

- The envelope is always present when `-p` is omitted (default `-p report`).
- `-p` projects the report; the output is whatever nodes the projection returns.
- For XML, a `<results>` root wrapper is used for multi-node per-match projections to preserve XML validity.
- `--single` drops list wrappers and emits one element bare.

## Open questions

1. **Field naming: `tree` vs `ast`.** `specs/report-model.md` uses `<ast>` in several places; the current `-v` uses `tree`. This doc keeps `tree` for consistency with the existing flag. If the report-model doc ever migrates to `<ast>`, `-p` follows.
2. **`-p count` shorthand.** Dropped from the initial enum — `<count>` isn't a single element. If the ergonomic shorthand is missed, reintroduce later as sugar (e.g. `-p count` = `-p totals --single` + value extraction). For now users can do `-p totals` and pick out `matches`.
3. **Grouped reports.** When `--group` is active, the `<results>` wrapper contains `<group>` elements that contain `<match>` elements. Does `-p tree` descend into groups? Proposal: yes — it flattens trees across all groups. If the user wants group structure, they keep the default report.
4. **`-p summary` in query mode.** Query reports have no verdict (no `<success>`), so `<summary>` may be empty or absent. Proposal: emit `<summary>` with whatever fields are present (e.g. just `<query>` if `-v query` was set). If nothing is present, `-p summary` emits an empty element.
5. **Interaction with `-m` (message template).** Message templates replace tree/value rendering in text. Does `-p tree` with `-m` emit the template output instead of the tree? Proposal: `-p` takes precedence — `-p tree` means trees, message template is ignored (or errors).

## Implementation checklist (test cases)

Each item below is a testable behavior derived from the design rules above. Grouped by theme; within each group, items are independent. Format: `command` → `expected`.

Use the following fixture conventions for brevity:
- `one.xml` contains one `<a>` element. Query `//a` → 1 match.
- `multi.xml` contains three `<a>` elements. Query `//a` → 3 matches.
- `empty.xml` contains no `<a>` elements. Query `//a` → 0 matches.
- `dir/` contains both files. Globbed query → 4 matches across 2 files.

### 1. `-p` value × format shape matrix

The core table. For each `-p` value and each format, verify the emitted document has the expected root/shape.

#### 1.1 Per-match projections (sequence-valued)

- [ ] `-p tree -f text multi.xml` → 3 bare trees, newline-separated, no envelope.
- [ ] `-p tree -f xml multi.xml` → `<results>` root containing 3 `<tree>` children. Parses as XML.
- [ ] `-p tree -f json multi.xml` → top-level JSON array of 3 tree objects. Parses as JSON.
- [ ] `-p tree -f yaml multi.xml` → top-level YAML sequence of 3 tree mappings. Parses as YAML.
- [ ] `-p tree -f xml one.xml` → `<results>` root with 1 `<tree>` child (not a bare `<tree>` — content-independence).
- [ ] `-p tree -f json one.xml` → top-level array of 1 tree (not a bare object).
- [ ] `-p tree -f xml empty.xml` → `<results/>` or empty `<results></results>`. Parses.
- [ ] `-p tree -f json empty.xml` → `[]`.
- [ ] Same eight cases with `-p value`.
- [ ] Same eight cases with `-p source`.
- [ ] Same eight cases with `-p lines`.

#### 1.2 Singular projections

- [ ] `-p schema -f text` → bare text schema rendering.
- [ ] `-p schema -f xml` → `<schema>` root element. Parses.
- [ ] `-p schema -f json` → top-level JSON object.
- [ ] `-p schema -f yaml` → top-level YAML mapping.
- [ ] `-p summary -f xml` → `<summary>` root element.
- [ ] `-p summary -f json` → top-level JSON object with summary fields.
- [ ] `-p summary -f yaml` → top-level YAML mapping.
- [ ] `-p totals -f xml` → `<totals>` root element containing `<matches>`, `<files>`, `<errors>`.
- [ ] `-p totals -f json` → top-level JSON object, e.g. `{"matches": 3, "files": 1, "errors": 0}`.
- [ ] `-p totals -f yaml` → top-level YAML mapping.

#### 1.3 Structural projections

- [ ] `-p results -f xml` → `<results>` root containing `<match>` children with `-v`-driven fields.
- [ ] `-p results -f json` → top-level array of match objects.
- [ ] `-p results -f yaml` → top-level sequence of match mappings.
- [ ] `-p report -f xml` → full `<report>` envelope (same as no `-p`).
- [ ] `-p report -f json` → full report object.
- [ ] `-p report` omitted ≡ `-p report` explicit — output byte-identical.

### 2. `--single` flag

- [ ] `-p tree --single -f xml one.xml` → bare `<a/>` (no `<results>` wrapper).
- [ ] `-p tree --single -f xml multi.xml` → bare first `<a/>`.
- [ ] `-p tree --single -f xml empty.xml` → empty stdout, non-zero exit.
- [ ] `-p tree --single -f json` → bare tree object (not array).
- [ ] `-p tree --single -f yaml` → bare mapping.
- [ ] `-p value --single -f text` → single value, no newline list.
- [ ] `-p results --single` → single `<match>` bare, no `<results>` wrapper.
- [ ] `--single` with `-p` omitted → treated as `-p results --single`.
- [ ] `-p summary --single` → `<summary>…</summary>` (no-op; already singular).
- [ ] `-p schema --single` → `<schema>…</schema>` (no-op).
- [ ] `-p totals --single` → `<totals>…</totals>` (no-op).
- [ ] `-p report --single` → first `<match>` bare? Or error? (document explicit choice; proposal: same as `-p results --single`.)
- [ ] `-n 1 --single` → same as `--single` (redundant but accepted).
- [ ] `--single -n 2` → treated as `-n 1`; `--single` takes precedence (or error — pick one).
- [ ] `--single` never emits a list wrapper, in any format.

### 3. Report-structure refactor

- [ ] Default report XML contains `<summary>` child with `<success>`, `<totals>`, `<expected>`, `<query>` inside it (when applicable).
- [ ] `<success>` is no longer a direct child of `<report>`.
- [ ] `<totals>` appears **inside** `<summary>` — not at the top level.
- [ ] `<totals>/<matches>` exists (renamed from `<totals>/<results>`).
- [ ] `<totals>/<results>` no longer exists.
- [ ] `<totals>/<files>` and `<totals>/<errors>` preserved.
- [ ] Top-level `<results>` still exists as the list wrapper for matches.
- [ ] `<schema>` is a direct child of `<report>` (not inside `<summary>`) when `-v schema` or `-p schema` is set.
- [ ] JSON shape mirrors XML: `report.summary.totals.matches` (not `report.summary.totals.results`).
- [ ] YAML shape mirrors JSON.
- [ ] Existing snapshot fixtures regenerate cleanly — no ad-hoc transforms needed.

### 4. `-v` replacement rule

#### 4.1 Replace cases (view-level `-p`)

- [ ] `-v tree,file -p tree` → match contains only `<tree>`, no `<file>`.
- [ ] `-v tree,file -p value` → match contains only `<value>`, no `<tree>` or `<file>`.
- [ ] `-v tree,file -p source` → match contains only `<source>`.
- [ ] `-v tree,file -p lines` → match contains only `<lines>`.
- [ ] `-v tree,file -p schema` → `<schema>` emitted; `<tree>`/`<file>` not.
- [ ] Default `-v` with `-p tree` → only `<tree>` per match.
- [ ] `-p schema` always computes schema regardless of `-v`.
- [ ] `-p tree` never computes schema even if default `-v` would.

#### 4.2 Respect cases (structural `-p`)

- [ ] `-v tree,file -p results` → each match contains `<tree>` and `<file>`.
- [ ] `-v tree,file -p report` → same.
- [ ] Default `-v` with `-p results` → match contains default view fields.

#### 4.3 Irrelevant cases (metadata `-p`)

- [ ] `-v tree,file -p summary` → output is `<summary>`; `-v` has no effect.
- [ ] `-v tree,file -p totals` → output is `<totals>`; `-v` has no effect.

### 5. Warning on discarded `-v` fields

Warning fires on stderr; output still produced on stdout.

#### 5.1 Warning fires

- [ ] `-v tree,file -p tree` → warning naming `file` as dropped.
- [ ] `-v tree,file,source -p tree` → warning naming `file, source`.
- [ ] `-v file -p tree` → warning naming `file`.
- [ ] `-v tree,file -p schema` → warning naming `tree, file`.
- [ ] `-v value,source -p lines` → warning naming `value, source`.
- [ ] Warning text includes the dropped field names.
- [ ] Warning text points to `-p results` as the alternative.
- [ ] Warning goes to stderr, not stdout.
- [ ] Exit code is unchanged (non-zero only if the command itself failed).

#### 5.2 Warning does NOT fire

- [ ] `-v tree -p tree` (redundant overlap) → no warning.
- [ ] `-v tree,file -p results` (structural) → no warning.
- [ ] `-v tree,file -p report` (structural) → no warning.
- [ ] `-v tree,file -p summary` (metadata) → no warning.
- [ ] `-v tree,file -p totals` (metadata) → no warning.
- [ ] Default `-v` (no explicit flag) with `-p tree` → no warning.
- [ ] `-p tree` alone (no `-v`) → no warning.

### 6. Pipeline ordering — format-required fields survive

Format-layer adjustments run **after** `-p`'s `-v` replacement. The renderer still adds what it needs.

- [ ] `-p results -f json` → each match includes `file`, `line`, `column` attributes even if `-v` didn't list them (format adds them).
- [ ] `-p tree -f xml` in a check command → diagnostic extras (`severity`, `reason`, `origin`, `lines`) still appended per match if the match has them — via `render_fields_for_match` at `tractor/src/format/shared.rs:64`.
- [ ] `-p tree` does not suppress format-required fields in the per-match output.
- [ ] `-p tree --single` does not suppress format-required fields on the singular match.

### 7. Content-independence contract

Same flags → same shape regardless of result cardinality.

- [ ] `-p tree -f xml` emits `<results>` root for 0, 1, 2, 100 matches.
- [ ] `-p tree -f json` emits a JSON array for 0, 1, 2, 100 matches.
- [ ] `-p tree --single -f xml` emits a bare tree root for 1, 2, 100 matches (fails for 0).
- [ ] No flag combo produces a scalar for 1 match and a list for ≥2 matches.
- [ ] Output shape can be predicted from flags alone, before the query runs.

### 8. Parseability contract

- [ ] Every `-f xml` output parses as XML (no multi-root documents).
- [ ] Every `-f json` output parses as JSON.
- [ ] Every `-f yaml` output parses as YAML.
- [ ] Emptiness edge cases also parse: empty `-p tree -f xml` → valid XML; empty `-p tree -f json` → `[]`.

### 9. Interaction with other flags

- [ ] `-n 2 -p tree` → first 2 trees in `<results>` wrapper.
- [ ] `-n 0 -p tree` → empty results (validates content-independence for `-n`).
- [ ] `-m TEMPLATE -p tree` → behavior per the chosen resolution of open question #5 (proposal: `-p` wins; template ignored or errors).
- [ ] `--group FIELD -p tree` → per open question #3 (proposal: trees flattened across groups).
- [ ] `--group FIELD -p results` → `<results>` contains `<group>` elements.
- [ ] `--group FIELD -p report` → default grouped report shape.
- [ ] `-x QUERY -p tree` → works; `-x` unchanged.
- [ ] `-l LANG -p tree` → works; `-l` unchanged.
- [ ] `-d DEPTH -p tree` → depth still applies to tree serialization.
- [ ] `-W -p source` → whitespace still normalized in source field.

### 10. Migration — removing the count/schema short-circuit

- [ ] `-v count` without `-p`: produces a report with `<totals>/<matches>` (breaking change from bare scalar in text — document in CHANGELOG).
- [ ] `-v count -p totals --single -f text` → bare scalar (restores old UX for the user who wants it).
- [ ] `-v schema -f xml`: emits `<schema>` as a child of `<report>`, inside the envelope (not bypassed).
- [ ] `-v schema -p schema`: emits `<schema>` bare as the root.
- [ ] The short-circuit at `tractor/src/cli/query.rs:133-142` is removed.
- [ ] `todo/7-count-schema-short-circuit.md` is closed by this change.

### 11. Mode-specific behavior

- [ ] **Query mode** (`tractor … -x …`) `-p summary` → `<summary>` present, `<success>` absent (no verdict), other fields included if set.
- [ ] **Check mode** (`tractor check …`) `-p summary` → `<summary>` with `<success>`, `<totals>`, `<expected>`.
- [ ] **Test mode** (`tractor test …`) `-p summary` → similar to check.
- [ ] `-p totals` works in all modes.
- [ ] `-p schema` in query mode → emits schema of matches.
- [ ] `-p schema` in check mode → behavior per open question (emit schema? error?).

### 12. Error / edge cases

- [ ] `-p INVALID` → CLI rejects with enum error listing valid values.
- [ ] `-p tree --single -f xml empty.xml` → exit non-zero, empty stdout.
- [ ] `-p tree` with query that computes no trees (e.g. a query matching non-node values) → empty `<results>` or error (pick one).
- [ ] `-v '' -p tree` (empty explicit `-v`) → no warning (vacuous replacement).

### 13. Spec / documentation

- [ ] `specs/cli-output-design.md:305-355` updated: envelope is always present **when `-p` is omitted or `-p report`**; otherwise the projection determines shape.
- [ ] `specs/cli-output-design.md` documents the parseability and content-independence contracts.
- [ ] `--help` for `-p` lists all enum values with one-line descriptions.
- [ ] `--help` for `--single` documents "first, bare" semantics.
- [ ] `CHANGELOG` / release notes call out the `-v count` breaking change.

### 14. Snapshot regressions

- [ ] Existing language snapshots under `tests/integration/languages/*/.xml` regenerated with the new `<summary>` / `<matches>` shape.
- [ ] Existing format snapshots under `tests/integration/formats/snapshots/` regenerated.
- [ ] New snapshot cases added for each `-p` value × format (matrix from §1).
- [ ] New snapshot cases added for `-p X --single` cases.
- [ ] `tractor/src/bin/update_snapshots.rs` updated to include projection cases in `OUTPUT_FORMAT_CASES`.

## Out of scope

- **Full XPath on `-p`.** Closed enum only. Arbitrary XPath is a future extension, blocked on the open questions in `specs/report-model.md`.
- **Multi-document aggregation.** E.g. "sum match counts across files into one scalar". Not an envelope problem; needs separate aggregation design.
- **Deprecating `-v`.** `-v` keeps its current role and field set. The migration is additive.

## Summary

- Add `-p` with a closed enum of values, each naming a real element in the (revised) report.
- Add `--single` to emit one element bare, composable with `-p`.
- Refactor the report: wrap summary fields in `<summary>`, introduce `<schema>`, rename `<totals>/<results>` → `<totals>/<matches>` to resolve the naming collision.
- `-v` replacement rule: `-p X` replaces `-v` with `[X]` when X is a view-level field (`tree`/`value`/`source`/`lines`/`schema`). Schema is the cost-bearing case; structural projections (`results`/`report`) respect `-v` instead.
- Replace the `count`/`schema` ad-hoc short-circuit with the principled render path.
- No breaking changes to `-v`'s field set; report-shape changes are accepted as part of the refactor.
- Full XPath on `-p` deferred.
