# Unified report model for mixed operation batches

## Problem

When `execute_check()` validates rule examples, it generates `TestOperation`s
and runs them through `execute_test()`. The test results are then manually
hoisted into the check report by converting test failures into synthetic
`ReportMatch` entries (see `validate_rule_examples()` and
`example_failure_match()` in `executor.rs`).

This works but is a workaround. The check report was not designed to carry
test results, so the conversion loses information (e.g. expected vs actual
match counts) and uses synthetic `<example>` file paths.

## Desired state

A unified report model where a single batch execution can contain mixed
operation types (check, test, query) and the report renders them all
coherently. Example validation would just be test operations prepended to
the operation list — no special hoisting or conversion needed.

This would also benefit `tractor run`, which already executes mixed
operation batches but currently produces separate reports per operation.

## What needs to change

- `Report` needs to support heterogeneous result types, or the different
  report kinds (check, test, query) need a common base that renderers can
  handle uniformly
- The summary model needs to aggregate across operation types
- Renderers (gcc, github, json, yaml, xml, text) need to handle mixed
  content without losing type-specific fields (e.g. `expected` for tests,
  `severity` for checks)

## Location

- `tractor-core/src/report.rs` — report and summary types
- `tractor/src/executor.rs` — `validate_rule_examples()` conversion logic
- `tractor/src/pipeline/format/` — renderers

## Impact

Medium — this is an architectural improvement that simplifies the executor
and enables richer output for example validation. Not urgent since the
current hoisting approach works correctly.
