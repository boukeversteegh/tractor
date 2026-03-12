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
| `tractor-core/src/report.rs` | `Report`, `ReportMatch`, `FileGroup` model |
| `tractor-core/src/output/formatter.rs` | `format_matches`, `format_message`, text sub-formats |
