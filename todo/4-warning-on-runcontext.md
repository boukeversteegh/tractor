# `warning` field on RunContext is test-mode specific

## Problem

`RunContext.warning` is only meaningful for `tractor test --warning` (makes test failures
non-fatal). It is always `false` for query and check. Having it on the shared context is
a mild layering violation — test-specific semantics on a shared struct.

## Fix options

1. Remove `warning` from `RunContext`. Pass it directly from `run_test` to
   `render_test_report` (it's already a parameter there). `RunContext::build` would
   no longer take `warning`.
2. Leave as-is. It's harmless and the cost of fixing is low value.

## Priority

Very low. No behavior impact, just a minor cleanliness concern.
