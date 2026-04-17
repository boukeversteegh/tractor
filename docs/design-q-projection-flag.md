# `-q` Projection Flag

**Date:** 2026-04-17
**Status:** Design proposal
**Related:** [#120](https://github.com/boukeversteegh/tractor/issues/120), `specs/report-model.md`

## Goals

- Let users emit a projection of the report (e.g. just the tree, just the summary) without the `<report>/<results>/<match>` envelope.
- Preserve two contracts: (a) structured-format output is always valid (XML parses, JSON parses); (b) output structure is determined by input flags, never by result cardinality.
- Keep the flag surface small — one new flag with a closed set of values.
- Stay compatible with existing `-v` behavior. No breaking changes.

## Non-goals

- Do **not** support arbitrary XPath in `-q` yet. A closed enum of shorthands only.
- Do **not** resolve the larger `-q` design questions in `specs/report-model.md` (multi-`-q` chaining, map-vs-reduce semantics, AST boundary implementation, full XPath). Those remain open.
- Do **not** replace or deprecate `-v`. `-v` keeps its current role.
- Do **not** auto-switch output shape based on result count. Same flags → same shape.

## Problem

Today, `-v tree` narrows *which fields* render inside each match, but structured formats (`xml`/`json`/`yaml`) still wrap the output in the full report envelope:

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

The user wanted just `<a>1</a>`. Text mode already does this by accident of implementation (no envelope). `-v count` and `-v schema` also bypass the envelope today, but via an ad-hoc short-circuit in `tractor/src/cli/query.rs:133-142` — not by design.

The envelope problem isn't tree-specific. The same issue applies to `-v value`, `-v source`, `-v lines`, and to multi-field views where users want `[{tree, file}, ...]` instead of `{results: [{tree, file}, ...]}`.

## Core insight

The report is structured data (conceptually XML, per `specs/report-model.md`). The envelope question dissolves if we separate two concerns:

- **`-v` — report construction.** Which fields are computed and included in each `<match>`. Controls *what's in* the report.
- **`-q` — report projection.** Which nodes from the built report are emitted. Controls *what comes out*.

Both flags coexist. Neither replaces the other. They operate at different pipeline stages.

```
source → parse → -x (query source ASTs) → build report (using -v) → project (-q) → serialize (-f)
                                           ↑                          ↑
                                           what's in the report       what gets emitted
```

## The `-q` flag

### Values

A closed enum of shorthands. No XPath expressions, no composition, no predicates.

**Per-match content fields** — project to the contents of that field across all matches:

| Value | Emits | Auto-enables on `-v` |
|---|---|---|
| `tree` | The `<tree>` contents per match | Yes (`-v +tree`) |
| `value` | The `<value>` contents per match | Yes (`-v +value`) |
| `source` | The `<source>` contents per match | Yes (`-v +source`) |
| `lines` | The `<lines>` contents per match | Yes (`-v +lines`) |

**Report-level aggregates** — project to a single report-level node:

| Value | Emits | Auto-enables on `-v` |
|---|---|---|
| `summary` | The `<summary>` element (totals + verdict) | No (summary is in the report when command has a verdict) |
| `count` | Scalar match count (just the number) | No (`totals.results` is always cheap) |
| `schema` | Structural overview of matched trees | **Yes** (`-v +schema`) — computation cost |

**Structural** — project to report structure without drilling into fields:

| Value | Emits | Auto-enables on `-v` |
|---|---|---|
| `matches` | List of `<match>` elements with current `-v` fields | No (uses whatever `-v` built) |
| `report` | The entire report (default when `-q` is omitted) | No |

### Auto-enable rule

`-q <field>` implicitly sets `-v +<field>` when `<field>` is a view-level name and isn't already present. This guarantees the projection finds its target.

Schema is the only field with real computation cost — walking all matched trees to build the overview. All others are either cheap (`tree`/`value` are slices of already-parsed data) or free (`count` is `totals.results`, `summary` is metadata). The auto-enable rule matters for correctness in all cases and for performance specifically for `schema`.

### When `-q` is omitted

Default: whole report. Equivalent to `-q report`. This preserves today's behavior exactly.

## Interaction with other flags

### `-v`

Independent. `-v` controls what fields are computed; `-q` controls which elements are emitted. Combining them is valid:

```bash
-v file,tree -q tree    # file is computed but not emitted (projection drops it)
-v file,tree            # default -q report emits everything
-q tree                 # auto -v +tree; projection emits just trees
```

The auto-enable rule means users rarely need both flags — most of the time `-q tree` alone is enough.

### `-f` (format)

Each format serializes a sequence of nodes according to its rules:

| Format | Single-node result | Multi-node result |
|---|---|---|
| `text` | Rendered inline | Newline-separated |
| `json` | Top-level value or array ¹ | Top-level array |
| `yaml` | Top-level value or sequence ¹ | Top-level sequence |
| `xml` | Single root element | Always wrapped in `<results>…</results>` ² |

¹ Single-node JSON/YAML: we still emit an array of length 1 for content-independence. `-q count` is an exception because count is inherently scalar — one number, not a one-element sequence.

² XML needs a single root. `<results>` is the stable wrapper for any multi-node projection, regardless of what `-q` value was used.

### `-n` (limit)

Unchanged. `-n` limits the number of matches processed. Projection applies after limiting. `-q tree -n 1` emits a one-element array / `<results>` with one `<tree>` child.

Note: `-q` alone does not provide a "bare single value" form. Producing exactly one unwrapped element is a separate concern (see "Open questions" below).

## Behavior examples

### Issue #120 — snapshot use case

```bash
$ echo '<root><a>1</a></root>' | tractor -l xml -x '//a' -q tree -f xml
<?xml version="1.0" encoding="UTF-8"?>
<results>
  <a>1</a>
</results>

$ echo '<root><a>1</a></root>' | tractor -l xml -x '//a' -q tree -f json
[
  {"a": "1"}
]

$ echo '<root><a>1</a></root>' | tractor -l xml -x '//a' -q tree -f text
a = "1"
```

### Count / schema — replacing the ad-hoc short-circuit

```bash
$ tractor src/**/*.cs -x '//method' -q count -f text
42

$ tractor src/**/*.cs -x '//method' -q count -f json
42

$ tractor src/**/*.cs -x '//class' -q schema -f text
<class>
  <name/>
  <body>
    <method/>
  </body>
</class>
```

Today, `count` and `schema` bypass the renderer via `tractor/src/cli/query.rs:133-142`. With `-q`, they go through the normal pipeline: build report → project → serialize. The short-circuit can be removed.

### Summary-only projection

```bash
$ tractor check src/**/*.cs -x '//comment[contains(.,"TODO")]' --reason TODO -q summary -f json
{
  "passed": false,
  "total": 3,
  "files": 2,
  "errors": 3
}
```

### Match list without envelope

```bash
$ tractor src/**/*.cs -x '//function' -q matches -f json
[
  {"file": "src/a.cs", "line": 5, "column": 1, "tree": {...}},
  {"file": "src/b.cs", "line": 12, "column": 1, "tree": {...}}
]
```

## Migration of existing behavior

### The `count` / `schema` short-circuit (cleanup)

`tractor/src/cli/query.rs:133-142` short-circuits before the renderer when `-v count` or `-v schema` is requested, writing the value directly to stdout. This becomes unnecessary once `-q count`/`-q schema` route through the standard render path. Remove the short-circuit, close `todo/7-count-schema-short-circuit.md`.

`-v count` and `-v schema` remain valid for backwards compatibility — they produce the same output as `-q count` / `-q schema` respectively, via the auto-enable rule applied in reverse (`-v schema` is both the build-side *and* the projection when no `-q` is specified).

### Spec update

`specs/cli-output-design.md:305-355` declares "the report envelope is always present, in every format, for every command." This claim becomes inaccurate once `-q` ships. Update that section to note:

- The envelope is always present when `-q` is omitted (default).
- `-q` projects the report; the emitted output is whatever nodes the projection returns.
- For XML, a stable `<results>` root wrapper is used for multi-node projections to preserve XML validity.

## Open questions

1. **Bare-single-value output.** The snapshot use case often wants exactly one tree, emitted bare (no `<results>` wrapper, no JSON array). `-q tree -n 1` still produces a one-element list. Options to decide later:
   - A separate flag like `--one` or `--single` that implies `-n 1` and emits bare.
   - A modifier on `-q` values (e.g. `-q first:tree`).
   - Accept that the stable-wrapper form is the answer and snapshot tooling strips the wrapper.
2. **Field naming: `tree` vs `ast`.** `specs/report-model.md` uses `ast` for the element name in several places; the current `-v` flag uses `tree`. Keep `tree` for consistency with `-v`, or rename to `ast` to match the report-model doc? Proposal: keep `tree`.
3. **XML root wrapper name.** Proposal: always `<results>`. Alternative: field-pluralized (`<trees>`, `<values>`) — rejected because downstream XPath becomes view-dependent, which violates content-independence.
4. **Interaction with `--group`.** Grouped output changes the report structure (groups wrap matches). Does `-q tree` inside a grouped report emit trees flat, or preserve group structure? Proposal: flat — `-q` is a projection, groups are envelope structure. If the user wants grouped projection, they keep the full report (`-q report`).
5. **Ordering with auto-enable.** When `-q tree` auto-enables `-v +tree`, where does `tree` land in the field order? Proposal: append if absent; do not reorder if present. Matches existing `-v +tree` semantics.

## Out of scope

- **Full XPath on the report.** `-q` accepts only the enum values listed above. Arbitrary XPath expressions (`-q //match[@file='x']`) are a future extension, blocked on the open questions in `specs/report-model.md` (multi-`-q`, map/reduce, AST boundary).
- **Multi-document aggregation.** E.g. "sum match counts across files into one scalar". Not an envelope problem; needs separate aggregation design.
- **Deprecating `-v`.** `-v` keeps its current role and field set. The migration story is *additive* — users start using `-q` where they need projection, `-v` stays for report-construction control.

## Summary

- Add `-q` as a new flag with a closed set of shorthand values.
- Two clean roles: `-v` builds the report, `-q` projects it.
- Auto-enable rule (`-q X` implies `-v +X` for view-level fields) guarantees projections find their target and makes `schema` pay its cost only when requested.
- Each format serializes the projection naturally; XML uses a stable `<results>` root for multi-node results.
- Replaces the `count`/`schema` ad-hoc short-circuit with the principled render path.
- No breaking changes to `-v`.
- Full XPath on `-q` and "bare single value" output are deliberately deferred.
