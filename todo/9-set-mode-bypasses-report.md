# Set mode bypasses the Report pipeline

## Problem

`run_set()` calls `query_files_batched` to find matches, then passes them directly to
`apply_replacements()`. It prints a summary to stderr and exits. The `-f` flag is accepted
by the CLI parser (inherited from `SharedArgs`) but has no effect — output is always a
plain-text summary line.

```
matches = query_files_batched(...)
apply_replacements(&matches, &args.value)
eprintln!("Set {} matches in {} files", ...)
```

## Why it matters

- **Silent flag ignore**: `tractor set config.yaml -x "//host" "new.example.com" -f json`
  succeeds but produces plain text, not JSON. The user gets no indication that `-f` was
  irrelevant.
- **No structured output**: CI pipelines that want a machine-readable summary of what was
  modified (which files, which values changed) have to parse the stderr text.
- **Dry-run gap**: there's no `--dry-run` that shows what would be replaced. A Report-based
  pipeline would make dry-run trivial (build the report, render it, skip `apply_replacements`).

## Recommendation

Two options:

1. **Minimal**: make `set` reject `-f` (or at least warn). This is honest about the fact
   that set has a separate output path. Cheap to implement.
2. **Full**: build a `Report::set(...)` with the replacement results (old value, new value,
   file, line), render it through the standard pipeline. Add `--dry-run` for free. This is
   more work but aligns `set` with the rest of the pipeline.

Option 1 is pragmatic for now. Option 2 is worth doing if set mode gains more features
(multi-rule replacement, conditional transforms, etc).

## Priority

Low. Set mode is functional and rarely used in CI pipelines that need structured output.
