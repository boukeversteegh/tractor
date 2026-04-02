# Unified pipeline: separate operation building from rendering

## Context

While working on generalized error reporting (PR #64), the `main()`
fallback error renderer needed format/color settings to render errors
in the user's requested format. But by the time an error reaches
`main()`, the args have been moved into `run_*()`. The workaround was
a pre-dispatch match that extracts format/color from every command
variant — duplicating the dispatch structure.

This revealed a deeper issue: each `run_*()` function currently owns
the full pipeline — it parses args into operations, executes them,
builds a report, *and* renders output. This coupling means output
concerns (format, color, grouping, message templates, view projection)
are tangled with operation logic in every mode.

## Problem

Each mode function (`run_query`, `run_check`, `run_set`, etc.) repeats
the same pipeline:

1. Parse CLI args into an `Operation`
2. Build a `ReportBuilder`, execute, call `build()`
3. Apply view projection, message templates, grouping
4. Render in the requested format
5. Determine exit code from `report.success`

Steps 3-5 are identical across modes. Step 2 is nearly identical (only
`set_no_verdict()` and `set_expected()` differ). Yet each mode
reimplements the full chain, making it hard to add cross-cutting
features (format-aware errors, streaming output, progress reporting).

## Desired state

A single `run()` function in `main()` that owns the pipeline:

```
parse_cli() → (Vec<Operation>, OutputConfig)
                    ↓                ↓
              execute(&ops)     render(report, config)
                    ↓
             ReportBuilder.build()
```

Each mode function becomes a pure **arg-to-operations translator** —
it reads CLI args and produces `Vec<Operation>` plus any mode-specific
builder config (no-verdict, expected value). It does not touch
rendering, formatting, or exit codes.

`OutputConfig` holds format, color, view, group-by, message template —
everything currently in `RunContext` that isn't operation-specific.
This is extracted once from CLI args, used for both normal output and
error fallback rendering.

Benefits:
- Format-aware error rendering falls out naturally (no pre-extraction hack)
- Cross-cutting concerns (streaming, progress, timing) live in one place
- Adding a new command only requires implementing the arg-to-ops translation
- `RunContext` splits cleanly into `OutputConfig` (rendering) and operation
  fields (files, xpath, rules)

## Notes

- PR #64's `ReportBuilder` collector is a prerequisite — it unifies
  the executor side. This todo addresses the rendering/orchestration side.
- Supersedes todo/9 (set bypasses report) — set would go through the
  same pipeline like everything else.
- Partially supersedes todo/19 (unified report model) — already landed
  in PR #64 via the builder pattern.
- The inline set stdout path (`build_set_inline_report`) is the hardest
  case: it produces a pre-grouped report with `output_content` at the
  group level. The unified pipeline needs a way to express this without
  special-casing.
- `run_render` (XML rendering mode) may remain separate since it's not
  an operation-based pipeline at all.
