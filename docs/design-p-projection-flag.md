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
