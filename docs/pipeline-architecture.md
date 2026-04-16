# Pipeline Architecture (As-Is)

This document describes the actual data processing pipeline of the tractor CLI as it exists today, including parallelism, branching points, and standardization gaps.

## Full Pipeline Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│  CLI ARGS  (clap parse)                                             │
└───────────────────────────┬─────────────────────────────────────────┘
                            │
                  ┌─────────▼──────────┐
                  │   RunContext::build │  SerFormat, ViewSet,
                  │   (context.rs)     │  XPath, color, concurrency
                  └──────┬─────────────┘
                         │
          ┌──────────────▼──────────────┐
          │      resolve_input()        │
          └──────┬───────────────┬──────┘
                 │               │
    ┌────────────▼──┐      ┌─────▼──────────┐
    │  InlineSource  │      │  Files          │  (glob expand +
    │  (--string or  │      │  Vec<String>    │   lang filter)
    │   stdin+lang)  │      └─────┬───────────┘
    └────────┬───────┘            │
             │                   │
   ┌─────────▼──────┐   ┌────────▼──────────────────────────────────┐
   │ parse_string_  │   │  query_files_batched()                    │
   │ to_documents() │   │                                           │
   └─────────┬──────┘   │  batch 0: [files 0..T]    ← T = threads  │
             │          │  batch 1: [files T..3T]   ← 2×           │
             │          │  batch 2: [files 3T..7T]  ← 4×           │
             │          │  ...capped at 8×T per batch               │
             │          │                                           │
             │          │  ┌────────────────────────────────────┐  │
             │          │  │ per batch: rayon par_iter()        │  │
             │          │  │  ┌──────────┐  ┌──────────┐        │  │
             │          │  │  │ file A   │  │ file B   │  ...   │  │
             │          │  │  │ parse()  │  │ parse()  │        │  │
             │          │  │  │ query()  │  │ query()  │        │  │
             │          │  │  └────┬─────┘  └────┬─────┘        │  │
             │          │  │       └──────┬───────┘              │  │
             │          │  │          flatten                     │  │
             │          │  │          sort (file,line,col)        │  │
             │          │  │          truncate to limit           │  │
             │          │  └────────────────────────────────────┘  │
             │          └────────────────┬──────────────────────────┘
             │                          │
             └──────────────┬───────────┘
                            │  Vec<Match>
                            │  { file, line, col, value, xml_fragment, ... }
             ┌──────────────▼──────────────────────────────────────────┐
             │                   MODE SPLIT                            │
             └───┬──────────────┬────────────────┬──────────┬──────────┘
                 │              │                │          │
          ┌──────▼──┐    ┌──────▼──┐    ┌───────▼──┐  ┌───▼────┐
          │  QUERY  │    │  CHECK  │    │   TEST   │  │  SET   │
          │         │    │         │    │          │  │        │
          │ wrap as │    │ wrap as │    │ wrap as  │  │apply_  │
          │ Report- │    │ Report- │    │ Report-  │  │replace-│
          │ Match + │    │ Match + │    │ Match +  │  │ments() │
          │ message │    │ reason  │    │ message  │  │(in-    │
          │ template│    │severity │    │ template │  │ place  │
          │         │    │ message │    │          │  │ edit)  │
          └──────┬──┘    └──────┬──┘    └───────┬──┘  └───┬────┘
                 │              │                │          │
                 │       .with_groups()          │      print
                 │       (drains matches         │      summary
                 │        into FileGroup[])      │
                 │              │                │
                 └──────────────┴────────────────┘
                                │  Report { kind, matches|groups, summary? }
                                │
             ┌──────────────────▼──────────────────────────────────────┐
             │                 SerFormat dispatch                       │
             └──┬───────┬──────┬──────┬──────────┬──────────┬──────────┘
                │       │      │      │           │          │
             Text      Gcc   Github  Json        Yaml       Xml
                │       │      │      │           │          │
          format_  render_ render_ render_    render_   render_
          matches  gcc()  github() json_      yaml_     xml_
          (core)           report() report()  report()
                │       │      │      │           │          │
                │       │      │      └───────────┘          │
                │       └──────┘            │                │
                │            │         view-filtered         │
                │       source ctx     JSON/YAML/XML:        │
                │       + underline    matches or groups     │
                │                           │                │
                └──────────────────────────┴────────────────┘
                                │
                           stdout (text/structured)
                         + stderr (summaries, errors)
```

## Standardization Gaps

| Gap | Description |
|-----|-------------|
| `.with_groups()` | Only called in `check`. `query` and `test` produce a `Report` but skip grouping entirely. |
| Count / Schema | Short-circuited in `run_query` before the `Report` is built — these views never enter the report pipeline. |
| `explore_*` (no-XPath) | Completely separate output path in query mode. Bypasses `Vec<Match>`, `Report`, `ReportMatch`, and all format renderers. Produces its own direct stdout output. |
| `set` mode | Completely bypasses the `Report` pipeline. Unique output path with its own summary print. |
| Summary always `Some` | `Report` has `summary: Option<Summary>` but all constructors (`query`, `check`, `test`) always set it to `Some`. The renderer suppresses it for query kind, not the model. |

## Parallelism Summary

| Stage | Parallel? | Notes |
|-------|-----------|-------|
| Input resolve | No | Sequential glob expand and lang filter |
| Parse + Query | **Yes** | `rayon par_iter` per batch, per file |
| Batch ordering | Partial | Within-batch sort by `(file, line, col)`; cross-batch order is stable only because batches are processed sequentially |
| Report build | No | Sequential match wrapping and HashMap grouping |
| Rendering | No | Single-threaded string building |
| File writes (`set`) | No | Files written sequentially |

## Key Branching Points

1. **Command type** (`main.rs`): `Query` | `Check` | `Test` | `Set`
2. **InputMode** (`context.rs`): `Files` vs `InlineSource`
3. **XPath present or absent** (`run_query`): query path vs explore path
4. **SerFormat** (`report_output.rs`): 6 output formats — `Text`, `Gcc`, `Github`, `Json`, `Yaml`, `Xml`
5. **ViewSet fields** (`context.rs`): composable field selection; `Count` and `Schema` short-circuit before report construction
6. **OutputFormat** (`formatter.rs`): `Xml` | `Lines` | `Source` | `Value` | `Count` | `Schema` (text sub-formats)
7. **Grouping** (`report.rs`): `matches` present vs `groups` present — mutually exclusive after `.with_groups()`

## Key Files

| File | Role |
|------|------|
| `tractor/src/main.rs` | Entry point, command routing |
| `tractor/src/cli.rs` | All argument definitions |
| `tractor/src/pipeline/context.rs` | `RunContext` builder, `SerFormat`, `ViewSet` |
| `tractor/src/pipeline/input.rs` | Input mode resolution |
| `tractor/src/pipeline/query.rs` | Parallel query core, explore functions |
| `tractor/src/modes/check.rs` | Check mode, violation wrapping |
| `tractor/src/modes/query.rs` | Query mode, render dispatch |
| `tractor/src/modes/test.rs` | Test mode, expectation checking |
| `tractor/src/modes/set.rs` | Set mode, in-place file editing |
| `tractor/src/pipeline/report_output.rs` | All report renderers |
| `tractor/src/report.rs` | `Report`, `ReportMatch`, `FileGroup` model |
| `tractor/src/output/formatter.rs` | `format_matches`, `format_message`, text sub-formats |

---

# Pipeline Architecture (After Clean Pipeline Separation)

This section describes the pipeline after the refactor committed in "Clean pipeline separation". Compare with the As-Is diagram above to see what changed.

## What Changed

| Concern | Before | After |
|---------|--------|-------|
| `ViewSet` backing type | `HashSet<ViewField>` — unordered | `Vec<ViewField>` — preserves `-v` declaration order |
| `-v` with `gcc`/`github` | Silently ignored | Errors at `RunContext::build()` |
| `ReportMatch` shape | Nested: `rm.inner.file`, `rm.inner.line` | Flat: `rm.file`, `rm.line` |
| Content fields populated | Always all fields | Only fields in resolved `ViewSet` |
| `source_lines` lifetime | Carried inside `Match` all the way to the renderer | Consumed at report-build time; not in `ReportMatch` |
| `xml_fragment` in renderers | Passed as part of `Match` inside `ReportMatch` | Stored as `rm.tree: Option<String>` — `None` if not in ViewSet |
| Field order in JSON/YAML/XML | Fixed canonical order | Follows `-v` declaration order |
| `view.has()` in renderers | Branching on content fields | Only used for structural decisions (summary, grouping) |
| `is_count_format` in engine | Rendering concern inside `query_files_batched` | Removed; count short-circuits in command layer |
| `render_match_text()` | Streaming text render inside matcher | Removed |
| GCC source context lines | Rendered from `source_lines` in `Match` | Suppressed (source_lines not stored in `ReportMatch`) |

## Full Pipeline Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│  CLI ARGS  (clap parse)                                             │
└───────────────────────────┬─────────────────────────────────────────┘
                            │
                  ┌─────────▼──────────────────────────────────────┐
                  │   RunContext::build()  (context.rs)            │
                  │                                                │
                  │  1. parse -f → OutputFormat                    │
                  │  2. if gcc/github AND -v given → error         │
                  │  3. parse -v → ViewSet (Vec, ordered)          │
                  │     or use command default if -v absent        │
                  │  ViewSet fully resolved here; never modified   │
                  └──────┬─────────────────────────────────────────┘
                         │
          ┌──────────────▼──────────────┐
          │      resolve_input()        │
          └──────┬───────────────┬──────┘
                 │               │
    ┌────────────▼──┐      ┌─────▼──────────┐
    │  InlineSource  │      │  Files          │
    └────────┬───────┘      └─────┬───────────┘
             │                   │
   ┌─────────▼──────┐   ┌────────▼──────────────────────────────────┐
   │ parse_string_  │   │  query_files_batched()                    │
   │ to_documents() │   │                                           │
   └─────────┬──────┘   │  batch 0: [files 0..T]    ← T = threads  │
             │          │  batch 1: [files T..3T]   ← 2×           │
             │          │  ...capped at 8×T per batch               │
             │          │                                           │
             │          │  ┌────────────────────────────────────┐  │
             │          │  │ per batch: rayon par_iter()        │  │
             │          │  │  ┌──────────┐  ┌──────────┐        │  │
             │          │  │  │ file A   │  │ file B   │  ...   │  │
             │          │  │  │ parse()  │  │ parse()  │        │  │
             │          │  │  │ query()  │  │ query()  │        │  │
             │          │  │  └────┬─────┘  └────┬─────┘        │  │
             │          │  │       └──────┬───────┘              │  │
             │          │  │       flatten, sort, truncate        │  │
             │          │  └────────────────────────────────────┘  │
             │          └────────────────┬──────────────────────────┘
             │                          │
             └──────────────┬───────────┘
                            │  Vec<Match>
                            │  { file, line, col, value, source_lines, xml_fragment }
                            │
             ┌──────────────▼──────────────────────────────────────────┐
             │   MODE SPLIT  +  match_to_report_match()               │
             │   (query.rs / check.rs / test.rs)                      │
             │                                                         │
             │   For each Match, populate ONLY ViewSet fields:         │
             │     tree     → xml_fragment  (if Tree ∈ ViewSet)       │
             │     value    → m.value       (if Value ∈ ViewSet)      │
             │     source   → extract_source_snippet()  (if Source)   │
             │     lines    → get_source_lines_range()  (if Lines)    │
             │     reason   → from check rule  (None in query/test)   │
             │     severity → from check rule  (None in query/test)   │
             │     message  → format_message(template, m)  (if -m)   │
             │   file/line/column always populated (identity fields)  │
             │   Match (source_lines, xml_fragment) dropped here ◄─── │
             └───┬──────────────┬────────────────┬──────────┬──────────┘
                 │              │                │          │
          ┌──────▼──┐    ┌──────▼──┐    ┌───────▼──┐  ┌───▼────┐
          │  QUERY  │    │  CHECK  │    │   TEST   │  │  SET   │
          │         │    │         │    │          │  │        │
          │ Count/  │    │ with_   │    │ check_   │  │apply_  │
          │ Schema  │    │ groups()│    │ expecta- │  │replace-│
          │ short-  │    │         │    │ tion()   │  │ments() │
          │ circuit │    │         │    │          │  │        │
          └──────┬──┘    └──────┬──┘    └───────┬──┘  └───┬────┘
                 │              │                │          │
                 └──────────────┴────────────────┘      print
                                │                       summary
                                │  Report { kind, matches|groups, summary }
                                │  ReportMatch: flat struct, Option<> per field
                                │
             ┌──────────────────▼──────────────────────────────────────┐
             │              OutputFormat dispatch  (format/mod.rs)     │
             └──┬───────┬──────┬──────┬──────────┬──────────┬──────────┘
                │       │      │      │           │          │
             Text      Gcc   Github  Json        Yaml       Xml
                │       │      │      │           │          │
                │    fixed   fixed  iterate     iterate   attrs +
                │   template schema view.fields view.fields iterate
                │       │      │      │           │          │
                │   rm.file  rm.file  field       field    view.fields
                │   rm.line  rm.line  Some→emit   Some→emit  for children
                │   rm.reason rm.reason
                │   rm.severity rm.severity
                │       │      │      │           │          │
                │       │      │      └───────────┘          │
                │       └──────┘            │                │
                │                      tree: JSON obj   tree: XML child
                │                      (xml_fragment    (xml_fragment
                │                       → json conv)     verbatim)
                │                           │                │
                └──────────────────────────┴────────────────┘
                                │
                           stdout (text/structured)
                         + stderr (summaries, errors)
```

## Remaining Standardization Gaps

| Gap | Description |
|-----|-------------|
| Count / Schema | Still short-circuited in `run_query` before `Report` is built. |
| `set` mode | Completely bypasses the `Report` pipeline. |
| `.with_groups()` | Still only called in `check`. `query` and `test` use flat `matches`. |
| GCC source context | `source_lines` not stored in `ReportMatch`; gcc context lines suppressed. Could be restored by adding a `source_context: Option<String>` field computed at build time. |
| Summary always `Some` | All `Report` constructors always set `summary: Some(...)`. The `Option<>` wrapper is vestigial. |

## Key Files

| File | Role |
|------|------|
| `tractor/src/main.rs` | Entry point, command routing |
| `tractor/src/cli.rs` | All argument definitions |
| `tractor/src/pipeline/context.rs` | `RunContext::build()` — format+view normalization, gcc/github validation |
| `tractor/src/pipeline/input.rs` | Input mode resolution |
| `tractor/src/pipeline/matcher.rs` | `query_files_batched`, `query_inline_source`, `match_to_report_match` |
| `tractor/src/modes/check.rs` | Check mode — reason/severity injection |
| `tractor/src/modes/query.rs` | Query mode — count/schema short-circuit, report build |
| `tractor/src/modes/test.rs` | Test mode — expectation checking |
| `tractor/src/modes/set.rs` | Set mode — in-place file editing (separate pipeline) |
| `tractor/src/pipeline/format/mod.rs` | Render dispatch for query/check/test |
| `tractor/src/pipeline/format/options.rs` | `ViewSet` (ordered Vec), `ViewField`, `OutputFormat` |
| `tractor/src/report.rs` | `Report`, flat `ReportMatch`, `FileGroup`, `Summary` |
| `tractor/src/output/formatter.rs` | `render_source_precomputed`, `render_lines_precomputed` |
