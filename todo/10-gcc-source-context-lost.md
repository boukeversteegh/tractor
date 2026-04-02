# GCC format lost source context lines after pipeline refactor

## Problem

Before the "Clean pipeline separation" refactor, gcc output included source context lines
with underline markers below each `file:line:col: severity: reason` line:

```
src/main.rs:10:5: error: no foo allowed
10 | public class Foo {
       ^~~~~~~~~~~~~~~~
```

After the refactor, only the diagnostic line is emitted:

```
src/main.rs:10:5: error: no foo allowed
```

This happened because `source_lines` (the raw file lines) are now consumed at
report-build time by `match_to_report_match()` and are not stored in `ReportMatch`.
The gcc renderer constructs a minimal `Match` with empty `source_lines` for the
`append_source_context()` helper, which then has nothing to render.

## Why it matters

- **Regression**: the source context was useful for human-readable gcc output — editors
  and CI logs that show gcc-format diagnostics lose the "where exactly" context.
- **Parity with other linters**: gcc, clang, rustc, and most linters include source context
  in their diagnostic output. Omitting it makes tractor's output less useful in the same
  contexts.

## Recommendation

Add a `source_context: Option<String>` field to `ReportMatch`, populated at report-build
time when the output format is gcc or github (or when Source/Lines is in the ViewSet):

```rust
// In match_to_report_match:
let source_context = needs_gcc_context
    .then(|| format_source_context(&m));
```

The gcc renderer then uses `rm.source_context` directly instead of trying to reconstruct
from source_lines.

Alternatively, the gcc-format supplanted ViewSet (set in RunContext::build for
gcc/github formats) could include a `Source` or `Lines` field, and the gcc renderer could
use `rm.lines` or `rm.source` to build the context display. This avoids a new field but
reuses existing ones for a different presentation.

## Priority

Medium. Real UX regression for users of gcc-format output in editors and CI.
