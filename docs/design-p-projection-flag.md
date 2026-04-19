# `-p` / `--project` Projection Flag

**Date:** 2026-04-19  
**Status:** Implemented  
**Related:** [#120](https://github.com/boukeversteegh/tractor/issues/120), `specs/report-model.md`, `todo/7-count-schema-short-circuit.md`

This document records the final implemented design for output projection. It replaces the earlier proposal framing and describes the behavior that shipped.

## Outcome

- `-p` selects which report element is emitted.
- `--single` turns sequence projections into "first, bare".
- `-v count` and `-v schema` no longer bypass the normal render pipeline.
- Output shape remains input-driven rather than cardinality-driven.
- Structured formats remain parseable.

## Final design

### Conceptual split

- `-v` controls which per-match fields are computed and carried on each match.
- `-p` controls which element from the built report is emitted.
- `--single` is a post-projection shape modifier for sequence projections.

The pipeline is:

```text
user flags
  -> normalize output plan
  -> execute query/check/test
  -> build Report
  -> apply grouping when the projection preserves match structure
  -> project the selected element
  -> format-specific render/serialize
```

### Report structure

The feature required two structural decisions in the report model:

- Summary is structural. `success`, `totals`, `expected`, and `query` live under `summary`.
- Schema is stored on the report as structured schema IR (`Vec<SchemaNode>`), not as pre-rendered text.

For CLI output, schema text is rendered from that IR at the output layer:

- Text renders schema as text and can colorize it.
- JSON/YAML/XML render the schema as rendered text in the CLI output contract.

The naming overlap between `/results` and `/summary/totals/results` remains unchanged. `results` still means the match/group list at the top level, and `totals.results` still means the total result count.

### Projection values

Implemented projection values:

| `-p` value | Meaning | `-v` interaction |
|---|---|---|
| `tree` | Project matched trees | Replaces `-v` with `tree` |
| `value` | Project matched values | Replaces `-v` with `value` |
| `source` | Project matched source snippets | Replaces `-v` with `source` |
| `lines` | Project matched line snippets | Replaces `-v` with `lines` |
| `schema` | Project the schema | Replaces `-v` with `schema` |
| `count` | Project total result count | Replaces `-v` with `count` |
| `summary` | Project summary | `-v` is irrelevant |
| `totals` | Project summary totals | `-v` is irrelevant |
| `results` | Project the results list | Preserves `-v` and `-m` |
| `report` | Project the full report | Preserves `-v` and `-m` |

`count` remains a synthetic convenience projection for the summary total count. It does not imply a persistent `<count>` node in the internal report model.

### `-v` and `-m` interaction

The normalization rules that shipped are:

- View-level projections (`tree`, `value`, `source`, `lines`, `schema`, `count`) replace the view set with the projected field.
- Structural projections (`results`, `report`) preserve the user's explicit `-v` and `-m`.
- Metadata projections (`summary`, `totals`) do not carry per-match fields. Explicit `-v` or `-m` inputs that become unreachable produce warnings on stderr.

This keeps the CLI honest about discarded user intent while avoiding unnecessary computation for projections that only need one field.

### `--single`

`--single` applies to sequence projections:

- `tree`
- `value`
- `source`
- `lines`
- `results`

Final behavior:

- If `-p` is omitted, `--single` implies `-p results`.
- `--single` means "first, bare".
- `--single` with `-n/--limit` other than `1` is rejected.
- `--single` on an already singular projection keeps the output unchanged and emits a warning.
- `--single` with no projected values exits silently with empty stdout and a non-zero exit code.

### Grouping

The implemented design supports grouping for match-preserving projections:

- `-p report` preserves grouping.
- `-p results` preserves grouping.

Grouping does not have independent meaning for scalar or metadata projections, so those projections naturally bypass grouped match rendering.

### Format-specific rendering

All formats share the same report model and projection plan, but their final rendering stages differ:

```text
Report + OutputPlan
  -> projection
     -> text: render directly to strings
     -> json/yaml: normalize into serializable values, then serialize
     -> xml: render XML elements/fragments directly
```

Important consequences:

- Full report rendering and projected rendering are not separate conceptual pipelines. They reuse the same report data and the same projection plan.
- JSON and YAML both have a normalization step before serialization.
- XML does not build a general-purpose intermediate object, but it still projects from the same report model.
- XML sequence projections stay valid by using a stable `<results>` root in the multi case.
- For XML multi-node field projections, the projected field name remains visible in the output shape. For example, `-p tree -f xml` emits `<results><tree>...</tree>...</results>`.
- For `--single`, XML emits the bare first projected value rather than the multi-case wrapper.

### Report shape

The report shape exposed by the CLI is effectively:

```xml
<report>
  <summary>
    <success>true</success>
    <totals>
      <results>2</results>
      <files>1</files>
    </totals>
    <expected>2</expected>
    <query>//a</query>
  </summary>
  <schema>...rendered schema text...</schema>
  <results>
    <match>...</match>
  </results>
</report>
```

Groups and captured outputs may also appear where applicable.

## Remaining scope boundaries

Still intentionally out of scope:

- Arbitrary XPath in `-p`
- Multi-`-p` chaining
- General aggregation semantics beyond the existing report elements
- Direct scalar projections such as `-p success` or `-p query`

## Deviations from the original proposal

### Discussed with the project owner

- Schema did not ship as an opaque pre-rendered string stored on the report. It ships as schema IR on the report, with text rendering deferred to the output layer.
- Schema color support was restored as part of that change by rendering text schema at the output layer.
- Grouping was not left undefined for projection. The shipped behavior preserves grouping for match-preserving projections (`results` and `report`).

### Not explicitly discussed with the project owner

- The `<summary>` wrapper is now owned structurally by the report model through `Summary<'a>` plus custom `Serialize for Report`, rather than existing only as a per-format wrapper convention.
- XML multi-node field projections keep the named projection element in the output shape (`<tree>`, `<value>`, `<source>`, `<lines>` under `<results>`), while `--single` still emits the bare first value.
- Snapshot coverage and snapshot generation were expanded beyond the original proposal text, including a dedicated projection matrix and schema-order stabilization for deterministic snapshots.
- The implementation kept `EmptySingle` as a renderer-level silent-exit path instead of moving that case entirely into earlier output-plan normalization.

None of those deviations change the public flag surface. They were implementation-level choices made to keep the model coherent and the format behavior consistent.

## Validation status

The original proposal checklist has been collapsed into shipped validation items.

- [x] Projection snapshots exist across `text`, `json`, `xml`, and `yaml` for `tree`, `value`, `source`, `lines`, `schema`, `count`, `summary`, `totals`, `results`, and `report`.
- [x] `--single` snapshots exist for the sequence projections and cover the bare-output behavior.
- [x] Schema text snapshots include color coverage so schema coloring remains protected.
- [x] Existing format and language snapshots were regenerated for the final summary/schema report shape.
- [x] `tractor/src/bin/update_snapshots.rs` includes projection cases in `OUTPUT_FORMAT_CASES`.
- [x] `cargo run --package tractor --bin update-snapshots -- --check` passed on 2026-04-19 (`155 fixtures checked`).

## Summary

- `-p` is implemented as report projection, not as a separate ad hoc rendering path.
- `--single` is implemented as "first, bare" for sequence projections.
- Summary is structural on the report model.
- Schema is stored as IR and rendered at the output layer.
- Grouping is preserved for match-preserving projections.
- The shipped implementation differs from the original proposal mainly in the schema model, grouping support, and where summary ownership lives.
