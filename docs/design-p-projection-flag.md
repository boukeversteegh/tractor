# `-p` / `--project` Projection Flag

**Date:** 2026-04-17
**Status:** Implemented
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

### On the `results` naming overlap

`results` appears twice in the report — once at `/summary/totals/results` as a scalar count ("total number of results"), and once at `/results` as the list wrapper containing matches/groups. They sit at different paths and don't collide, but the shared name is conceptually awkward.

**Decision for this refactor:** keep both as they are. `/summary/totals/results` means "total number of results" and that meaning is stable. `-p results` projects to `/results`, the naturally expected target. A future cleanup could rename the match element to `result` for symmetry, but that is **out of scope here** — no change to match-element naming.

### Issue 1: Summary fields are loose at the top level

Today `/success`, `/totals`, `/expected`, `/query` are all top-level paths, alongside `/results`. There's no container that groups them, so `-p summary` can't map to a real element.

**Change:** introduce `/summary`, moving `/success`, `/totals`, `/expected`, `/query` under it as `/summary/success`, `/summary/totals`, `/summary/expected`, `/summary/query`.

### Issue 2: `schema` is not part of the report

Today `-v schema` bypasses the report entirely — it short-circuits in `tractor/src/cli/query.rs` and prints the text-mode schema rendering directly. There is no `/schema` node in the report, so `-p schema` has nothing to project.

**Change:** when schema is computed, include it at `/schema` as an **opaque string** — the same text rendering produced today, stored verbatim as the node's text content. No structured per-format serialization yet. Concretely:

- XML: `<schema>…text schema…</schema>` (string content, XML-escaped).
- JSON/YAML: `"schema": "…text schema…"` (string value).
- Text: printed as-is (the current behavior).

A future iteration can make schema a structured node, but that's out of scope here. The point of this change is to get schema into the pipeline as addressable data so the short-circuit can be removed and `-p schema` works uniformly.

### Revised report structure

```xml
<report>
  <summary>
    <success>true</success>
    <totals>
      <results>2</results>
      <files>1</files>
      <errors>0</errors>
    </totals>
    <expected>2</expected>
    <query>//a</query>
  </summary>
  <schema>&lt;a&gt;...&lt;/a&gt;</schema>  <!-- opaque text rendering, escaped -->
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
| `count` | Scalar total match count (= `/totals/results`) | **Replace** with `[count]` |
| `summary` | The `<summary>` container | None — `-v` doesn't apply |
| `totals` | The `<totals>` element inside summary | None — `-v` doesn't apply |
| `results` | The `<results>` list wrapper (list of matches) | None — respects user's `-v` |
| `report` | The whole `<report>` (default when `-p` omitted) | None — respects user's `-v` |

### The `-v` replacement rule

When `-p` is a **view-level field** (`tree`, `value`, `source`, `lines`, `schema`, `count`), it **replaces** the view set with exactly `[that field]`. Reason: `-p tree` means "I just want trees" — keeping `-v file,source` alongside would compute data that's thrown away. If the user wanted multiple fields per match, they'd use `-p results` (which respects `-v`) instead.

`count` is a sugar alias for the scalar `/totals/results` — no `<count>` element exists in the report. This matches the existing `-v count`, which is sugar in the same way.

`-m TEMPLATE` is a view-level field too: it contributes a `message` field to each match (rendered inline in text, as a `message` key in structured formats). It composes with `-p` via the same rules — `-p message` would be a hypothetical future addition, but today `-m` simply adds `message` to the view set. `-p results`/`-p report` preserves it; any view-level `-p X ≠ message` discards it (with the warning below); `-p summary`/`-p totals` drops it as an unreachable per-match field (also warned).

When `-p` is **structural** (`results`, `report`), `-v` is respected — those projections emit per-match content, so `-v` still drives what each match contains.

When `-p` is a **metadata container** (`summary`, `totals`), `-v` is untouched but irrelevant — these elements don't contain per-match fields, so the view set has nothing to influence.

Schema is notable: it's the only view-level field with real computation cost. The replacement rule ensures `-p schema` always triggers schema computation, and `-p tree` (or any other projection) never does.

### Warning on discarded view fields

Any explicitly-requested view field (via `-v` or `-m`) that won't appear in stdout should be reported. The user wrote it, expected to see it, and won't — that's worth surfacing.

**Rule:** warn on stderr when an explicitly-passed view field is dropped from the output. Two cases produce drops:

1. **Replacement drop** — `-p X` is view-level and the user's `-v`/`-m` contains fields other than `X`. Those extras are replaced away.
2. **Unreachable drop** — `-p` is `summary` or `totals` (metadata containers with no per-match rendering). Any explicit `-v`/`-m` field is unreachable under these projections.

Warning text should name the dropped fields and point to the fix:

```
warning: -v fields {file, source} were discarded because -p tree replaces the view set.
  To keep -v intact, use `-p results` (respects -v) instead of `-p tree`.
```

```
warning: -m message template has no effect with -p summary (no per-match rendering).
```

No warning for:
- Redundant overlap: `-v tree -p tree` — user's intent is preserved.
- Structural `-p`: `-p results`, `-p report` honor `-v`/`-m` in full.
- Default view (no explicit `-v` or `-m`): the replacement can't surprise a user who didn't specify anything.

This is a warning, not an error — malformed combinations still produce output.

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

`--single` is a modifier on sequence projections. It:

1. Limits match processing to the first match (implicit `-n 1`).
2. Strips list wrappers from the output — the projected element is emitted bare, no `<results>` root, no JSON array, no YAML sequence.

`--single` applies only to projections that return a sequence — `tree`, `value`, `source`, `lines`, `results`. For singular projections (`summary`, `schema`, `totals`, `report`, `count`), `--single` is a no-op and emits a warning on stderr. There is no list to flatten, so the flag has nothing to do.

`--single -n N` (for any N ≠ 1) is a contradiction — `--single` means "first match only", and `-n` setting a different bound contradicts that. Treat as a CLI error with a clear message. `--single -n 1` is redundant but accepted.

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
| `-p summary --single` | `<summary>…</summary>` (no-op + warning — already singular) |
| `-p schema --single` | `<schema>…</schema>` (no-op + warning) |
| `-p totals --single` | `<totals>…</totals>` (no-op + warning) |
| `-p report --single` | `<report>…</report>` (no-op + warning) |
| `-p count` | Bare number, e.g. `3` (already singular) |
| `-p count --single` | Bare number (no-op + warning) |

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

¹ XML requires a single root for validity. The stable wrapper is `<results>` for any multi-node per-match projection (e.g. `-p tree`). For singular projections (`-p summary`, `-p schema`, `-p totals`), the element itself is the root. `-p count` is a bare scalar in text/json/yaml, and emits a synthetic `<count>N</count>` root in XML (same name as the flag, keeps XML parseable).

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
{"results": 42, "files": 7}

$ tractor src/**/*.cs -x '//class' -p schema -f text
<class>
  <name/>
  <body>
    <method/>
  </body>
</class>

$ tractor src/**/*.cs -x '//class' -p schema -f xml
<?xml version="1.0" encoding="UTF-8"?>
<schema>&lt;class&gt;
  &lt;name/&gt;
  &lt;body&gt;
    &lt;method/&gt;
  &lt;/body&gt;
&lt;/class&gt;</schema>
```

Today `-v count` / `-v schema` bypass the renderer via `cli/query.rs:133-142`. With `-p`, they go through the normal pipeline: build report → project → serialize. The short-circuit can be removed. Closes `todo/7-count-schema-short-circuit.md`.

### Summary-only projection

```bash
$ tractor check src/**/*.cs -x '//comment[contains(.,"TODO")]' --reason TODO -p summary -f json
{
  "success": false,
  "totals": {"results": 3, "files": 2, "errors": 3},
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

Introducing `<summary>` and `<schema>` changes the XML/JSON/YAML shape of reports. Snapshot tests under `tests/integration/languages/*/.xml` and `tests/integration/formats/snapshots/` regenerate. No code-level compatibility layer needed — the report model is an output contract, not a stable API surface.

### Spec update

`specs/cli-output-design.md:305-355` declares "the report envelope is always present, in every format, for every command." Update to state:

- The envelope is always present when `-p` is omitted (default `-p report`).
- `-p` projects the report; the output is whatever nodes the projection returns.
- For XML, a `<results>` root wrapper is used for multi-node per-match projections to preserve XML validity.
- `--single` drops list wrappers and emits one element bare.

## Open questions

1. **Field naming: `tree` vs `ast`.** `specs/report-model.md` uses `<ast>` in several places; the current `-v` uses `tree`. This doc keeps `tree` for consistency with the existing flag. If the report-model doc ever migrates to `<ast>`, `-p` follows.
2. **`-p summary` in query mode.** Query reports have no verdict (no `/summary/success`), so `/summary` may be empty or near-empty. Proposal: emit `/summary` with whatever fields are present (e.g. just `/summary/query` if `-v query` was set). If nothing is present, `-p summary` emits an empty element.

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

- [x] `-p tree -f text multi.xml` → 3 bare trees, newline-separated, no envelope.
- [x] `-p tree -f xml multi.xml` → `<results>` root containing 3 `<tree>` children. Parses as XML.
- [x] `-p tree -f json multi.xml` → top-level JSON array of 3 tree objects. Parses as JSON.
- [x] `-p tree -f yaml multi.xml` → top-level YAML sequence of 3 tree mappings. Parses as YAML.
- [x] `-p tree -f xml one.xml` → `<results>` root with 1 `<tree>` child (not a bare `<tree>` — content-independence).
- [x] `-p tree -f json one.xml` → top-level array of 1 tree (not a bare object).
- [x] `-p tree -f xml empty.xml` → `<results/>` or empty `<results></results>`. Parses.
- [x] `-p tree -f json empty.xml` → `[]`.
- [x] Same eight cases with `-p value`.
- [x] Same eight cases with `-p source`.
- [x] Same eight cases with `-p lines`.

#### 1.2 Singular projections

- [x] `-p schema -f text` → bare text schema rendering (same as today's `-v schema -f text`).
- [x] `-p schema -f xml` → `<schema>…</schema>` root, text content is the rendering (XML-escaped). Parses.
- [x] `-p schema -f json` → bare JSON string, e.g. `"…schema text…"`.
- [x] `-p schema -f yaml` → bare YAML scalar string.
- [x] Schema content is byte-identical across formats after format-specific escaping (no structured serialization).
- [x] `-p summary -f xml` → `<summary>` root element.
- [x] `-p summary -f json` → top-level JSON object with summary fields.
- [x] `-p summary -f yaml` → top-level YAML mapping.
- [x] `-p totals -f xml` → `<totals>` root element containing `<results>`, `<files>`, `<errors>`.
- [x] `-p totals -f json` → top-level JSON object, e.g. `{"results": 3, "files": 1, "errors": 0}`.
- [x] `-p totals -f yaml` → top-level YAML mapping.
- [x] `-p count -f text` → bare number, e.g. `3`.
- [x] `-p count -f json` → bare number, e.g. `3`.
- [x] `-p count -f yaml` → bare number, e.g. `3`.
- [x] `-p count -f xml` → `<count>3</count>` root element. Parses.
- [x] `-p count` value equals `-p totals -f json | jq .results`.

#### 1.3 Structural projections

- [x] `-p results -f xml` → `<results>` root containing `<match>` children with `-v`-driven fields.
- [x] `-p results -f json` → top-level array of match objects.
- [x] `-p results -f yaml` → top-level sequence of match mappings.
- [x] `-p report -f xml` → full `<report>` envelope (same as no `-p`).
- [x] `-p report -f json` → full report object.
- [x] `-p report` omitted ≡ `-p report` explicit — output byte-identical.

### 2. `--single` flag

- [x] `-p tree --single -f xml one.xml` → bare `<a/>` (no `<results>` wrapper).
- [x] `-p tree --single -f xml multi.xml` → bare first `<a/>`.
- [x] `-p tree --single -f xml empty.xml` → empty stdout, non-zero exit.
- [x] `-p tree --single -f json` → bare tree object (not array).
- [x] `-p tree --single -f yaml` → bare mapping.
- [x] `-p value --single -f text` → single value, no newline list.
- [x] `-p results --single` → single `<match>` bare, no `<results>` wrapper.
- [x] `--single` with `-p` omitted → treated as `-p results --single`.
- [x] `-p summary --single` → `<summary>…</summary>` unchanged, warning on stderr.
- [x] `-p schema --single` → `<schema>…</schema>` unchanged, warning on stderr.
- [x] `-p totals --single` → `<totals>…</totals>` unchanged, warning on stderr.
- [x] `-p report --single` → `<report>…</report>` unchanged, warning on stderr.
- [x] `-p count --single` → bare number unchanged, warning on stderr.
- [x] Warning text names the projection as "already singular" and suggests dropping `--single`.
- [x] `-n 1 --single` → same as `--single` (redundant but accepted, no warning).
- [x] `--single -n 2` → CLI error with a message explaining the contradiction. Non-zero exit, no output.
- [x] `--single -n 3` (or any N ≠ 1) → same error.
- [x] `--single` never emits a list wrapper, in any format, for any projection it applies to.

### 3. Report-structure refactor

Paths below are absolute XPaths into the report.

- [x] `/success`, `/totals`, `/expected`, `/query` move to `/summary/success`, `/summary/totals`, `/summary/expected`, `/summary/query`. No loose summary fields remain at the top level.
- [x] `/summary/totals/results`, `/summary/totals/files`, `/summary/totals/errors` preserved with original meanings — no rename inside totals.
- [x] `/results` still exists as the list wrapper for matches (separate from `/summary/totals/results`).
- [x] `/schema` is a direct child of `/` (not inside `/summary`) when `-v schema` or `-p schema` is set.
- [x] JSON/YAML shapes mirror the XML paths (e.g. `report.summary.totals.results`).

### 4. `-v` replacement rule

The intent here is to verify that `-p` replaces / respects / ignores `-v`. The assertions are about which fields are **present in the output**, regardless of how each format renders them. Concretely:

- "field X is absent" = no corresponding XML element, no JSON/YAML key, no text-mode line for X.
- "field X is present" = the field's data appears wherever the format puts it (wrapped element, key, or rendered line).

For projections where output shape differs across formats (e.g. `-p tree` → `<tree>` wrappers in XML but raw tree values in JSON), these items assert *whether file-path information or other unrequested data appears anywhere*, not the wrapper shape (that's §1).

#### 4.1 Replace cases (view-level `-p`)

Each `-v tree,file -p X` case below *also* triggers the discarded-`-v`-fields warning (verified in §5.1). §4.1 asserts the replacement took effect in stdout; §5.1 asserts the warning fires on stderr.

- [x] `-v tree,file -p tree` → output contains no file-path data (no `<file>` in xml, no `file` key in json/yaml, no path prefix in text).
- [x] `-v tree,file -p value` → output contains neither tree nor file data — just values.
- [x] `-v tree,file -p source` → output contains neither tree nor file data — just source text.
- [x] `-v tree,file -p lines` → output contains neither tree nor file data — just line snippets.
- [x] `-v tree,file -p schema` → output contains neither tree nor file data — just the schema element.
- [x] `-v tree,file -p count` → output contains neither tree nor file data — just the scalar count.
- [x] Default `-v` with `-p tree` → output contains only tree data per match (no file, line, column, etc. — default view fields suppressed).
- [x] `-p schema` always computes schema regardless of `-v` (schema data appears).
- [x] `-p tree` never computes schema even if default `-v` would include it (no schema data in output).

#### 4.2 Respect cases (structural `-p`)

- [x] `-v tree,file -p results` → each emitted match contains **both** tree data and file-path data.
- [x] `-v tree,file -p report` → same (under the `<report>` envelope).
- [x] Default `-v` with `-p results` → each match contains the default view fields.

#### 4.3 Unreachable cases (metadata `-p`)

These also trigger the discarded-fields warning (§5.1), since the requested `-v`/`-m` fields can't appear in the output.

- [x] `-v tree,file -p summary` → output is the summary; no tree or file data anywhere. Warning names `tree, file`.
- [x] `-v tree,file -p totals` → output is totals; no tree or file data anywhere. Warning names `tree, file`.
- [x] `-m 'TEMPLATE' -p summary` → output is the summary; no message rendered. Warning mentions the message template.
- [x] `-m 'TEMPLATE' -p totals` → same.

### 5. Warning on discarded `-v` fields

Warning fires on stderr; output still produced on stdout.

#### 5.1 Warning fires

Rule: any explicitly-requested view field (via `-v` or `-m`) that won't appear in stdout → warning on stderr naming the dropped field(s). Two drop modes:

- **Replacement** — `-p X` is view-level (`tree`/`value`/`source`/`lines`/`schema`/`count`), and the explicit `-v`/`-m` set contains fields other than `X`.
- **Unreachable** — `-p` is `summary` or `totals` (no per-match rendering), and any `-v`/`-m` field is explicitly requested.

Canonical check (replacement): `-v tree,file -p tree` produces approximately this on stderr:

```
warning: -v fields {file} were discarded because -p tree replaces the view set.
  To keep -v intact, use `-p results` (respects -v) instead of `-p tree`.
```

Canonical check (unreachable): `-m 'TEMPLATE' -p summary` produces approximately:

```
warning: -m message template has no effect with -p summary (no per-match rendering).
```

- [x] Canonical replacement case above prints an equivalent warning.
- [x] Canonical unreachable case above prints an equivalent warning.
- [x] Same rule holds for any other view-level `-p` and any `-v`/`-m` superset (e.g. `-v tree,file,source -p tree` names `file, source`; `-v tree,file -p schema` names `tree, file`; `-m TMPL -v tree -p value` names `tree` and `message`).
- [x] Warning text lists **all** dropped field names (not just the first).
- [x] Replacement warning points to `-p results` as the alternative that respects `-v`.
- [x] Warning lines begin with `warning:` (lowercase) — matches tractor's convention.
- [x] Warnings go to stderr; stdout is unchanged.
- [x] Exit code is unchanged (non-zero only if the command itself failed).

#### 5.2 Warning does NOT fire

- [x] Redundant overlap — `-v tree -p tree`: user's intent is preserved, no warning.
- [x] Structural `-p` — `-v tree,file -p {results | report}`: all `-v`/`-m` fields appear in the output, no warning.
- [x] Default view — `-p tree` with no explicit `-v` or `-m` (or `-p tree` alone): nothing the user asked for is dropped, no warning.
- [x] `-m TEMPLATE -p results` → `message` field appears per match, no warning.
- [ ] `-m TEMPLATE -p message` (if `-p message` is ever added) → no warning. (N/A today — included for future-proofing.)

### 6. Pipeline ordering — format-required fields survive

Format-layer adjustments run **after** `-p`'s `-v` replacement. The renderer still adds what it needs.

- [x] `-p results -f json` → each match includes `file`, `line`, `column` attributes even if `-v` didn't list them (format adds them).
- [x] `-p tree -f xml` in a check command → diagnostic extras (`severity`, `reason`, `origin`, `lines`) still appended per match if the match has them — via `render_fields_for_match` at `tractor/src/format/shared.rs:64`.
- [x] `-p tree` does not suppress format-required fields in the per-match output.
- [x] `-p tree --single` does not suppress format-required fields on the singular match.

### 7. Content-independence contract

Same flags → same shape regardless of result cardinality.

- [x] `-p tree -f xml` emits `<results>` root for 0, 1, 2, 100 matches.
- [x] `-p tree -f json` emits a JSON array for 0, 1, 2, 100 matches.
- [x] `-p tree --single -f xml` emits a bare tree root for 1, 2, 100 matches (fails for 0).
- [x] No flag combo produces a scalar for 1 match and a list for ≥2 matches.
- [x] Output shape can be predicted from flags alone, before the query runs.

### 8. Parseability contract

- [x] Every `-f xml` output parses as XML (no multi-root documents).
- [x] Every `-f json` output parses as JSON.
- [x] Every `-f yaml` output parses as YAML.
- [x] Emptiness edge cases also parse: empty `-p tree -f xml` → valid XML; empty `-p tree -f json` → `[]`.

### 9. Interaction with other flags

- [x] `-n 2 -p tree` → first 2 trees in `<results>` wrapper.
- [x] `-n 0 -p tree` → empty results (validates content-independence for `-n`).
- [x] `-m TMPL -p results -f json` → each match contains a `message` key.
- [x] `-m TMPL -p report -f json` → each match (inside `/results`) contains a `message` key.
- [x] `-m TMPL -p tree` → warning fires (§5.1 replacement); no message in output.
- [x] `-m TMPL -p summary` → warning fires (§5.1 unreachable); output is summary only.
- [ ] `--group FIELD -p …` → out of scope for this design (see "Out of scope"); behavior is undefined / CLI may reject.
- [x] `-x QUERY -p tree` → works; `-x` unchanged.
- [x] `-l LANG -p tree` → works; `-l` unchanged.
- [x] `-d DEPTH -p tree` → depth still applies to tree serialization.
- [x] `-W -p source` → whitespace still normalized in source field.

### 10. Migration — removing the count/schema short-circuit

- [x] `-v count` without `-p`: produces a report with `/totals/results` (breaking change from bare scalar in text — document in CHANGELOG).
- [x] `-v count -p totals --single -f text` → bare scalar (restores old UX for the user who wants it).
- [x] `-v schema -f xml`: emits `<schema>` as a child of `<report>`, inside the envelope (not bypassed).
- [x] `-v schema -p schema`: emits `<schema>` bare as the root.
- [x] The short-circuit at `tractor/src/cli/query.rs:133-142` is removed.
- [x] `todo/7-count-schema-short-circuit.md` is closed by this change.

### 11. Mode-specific behavior

- [x] **Query mode** (`tractor … -x …`) `-p summary` → `<summary>` present, `<success>` absent (no verdict), other fields included if set.
- [x] **Check mode** (`tractor check …`) `-p summary` → `<summary>` with `<success>`, `<totals>`, `<expected>`.
- [x] **Test mode** (`tractor test …`) `-p summary` → similar to check.
- [x] `-p totals` works in all modes.
- [x] `-p schema` in query mode → emits schema of matches.
- [ ] `-p schema` in check mode → behavior per open question (emit schema? error?).

### 12. Error / edge cases

- [x] `-p INVALID` → CLI rejects with enum error listing valid values.
- [x] `-p tree --single -f xml empty.xml` → exit non-zero, empty stdout.
- [ ] `-p tree` with query that computes no trees (e.g. a query matching non-node values) → empty `<results>` or error (pick one).
- [x] `-v '' -p tree` (empty explicit `-v`) → no warning (vacuous replacement).

### 13. Spec / documentation

- [ ] `specs/cli-output-design.md:305-355` updated: envelope is always present **when `-p` is omitted or `-p report`**; otherwise the projection determines shape.
- [ ] `specs/cli-output-design.md` documents the parseability and content-independence contracts.
- [x] `--help` for `-p` lists all enum values with one-line descriptions.
- [x] `--help` for `--single` documents "first, bare" semantics.
- [ ] `CHANGELOG` / release notes call out the `-v count` breaking change.

### 14. Snapshot regressions

- [x] Existing language snapshots under `tests/integration/languages/*/.xml` regenerated with the new `<summary>` / `<schema>` shape.
- [x] Existing format snapshots under `tests/integration/formats/snapshots/` regenerated.
- [x] New snapshot cases added for each `-p` value × format (matrix from §1).
- [x] New snapshot cases added for `-p X --single` cases.
- [x] `tractor/src/bin/update_snapshots.rs` updated to include projection cases in `OUTPUT_FORMAT_CASES`.

## Out of scope

- **Full XPath on `-p`.** Closed enum only. Arbitrary XPath is a future extension, blocked on the open questions in `specs/report-model.md`.
- **Multi-document aggregation.** E.g. "sum match counts across files into one scalar". Not an envelope problem; needs separate aggregation design.
- **Deprecating `-v`.** `-v` keeps its current role and field set. The migration is additive.
- **Single-scalar projection of other summary fields** (e.g. `-p success`, `-p query`, `-p expected`). Acknowledged inconsistency: `-p count` is supported because it mirrors `-v count` and covers the most common use case, but the other scalar summary fields are not addressable. Revisit once dotted paths or XPath on `-p` is on the table.
- **Grouping with projection.** `--group FIELD` combined with any `-p` value is not supported in this design. Behavior is undefined; the CLI may reject the combination or produce a reasonable default. Revisit once the grouped-report shape interacts cleanly with projections.

## Summary

- Add `-p` with a closed enum of values, each naming a real element in the (revised) report.
- Add `--single` to emit one element bare, composable with `-p`.
- Refactor the report: wrap summary fields in `<summary>` and introduce `<schema>`. Keep `/totals/results` as-is (no rename).
- `-v` replacement rule: `-p X` replaces `-v` with `[X]` when X is a view-level field (`tree`/`value`/`source`/`lines`/`schema`). Schema is the cost-bearing case; structural projections (`results`/`report`) respect `-v` instead.
- Replace the `count`/`schema` ad-hoc short-circuit with the principled render path.
- No breaking changes to `-v`'s field set; report-shape changes are accepted as part of the refactor.
- Full XPath on `-p` deferred.
