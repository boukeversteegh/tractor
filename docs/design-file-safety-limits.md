# Design: File Safety Limits & Glob Diagnostics

**GitHub Issue**: #72  
**Branch**: `claude/review-tractor-issues-z9kjo`  
**Status**: Partially implemented — core limits work, intersection semantics not yet done

---

## Problem Statement

When tractor's file glob patterns match nothing, tractor silently succeeds — leaving users confused about whether it worked at all. When glob patterns accidentally match too many files (e.g., globbing an entire drive), tractor hangs indefinitely with no feedback.

## Decisions Made

### 1. Three fatal errors for file resolution

Tractor should emit **fatal diagnostics** (not warnings) through the Report pipeline for these cases:

| Error | When | Message |
|---|---|---|
| **Glob expansion limit** | A single glob pattern expands past `10 × max_files` paths during iteration | `pattern "/**/*.ts" expanded to over 100000 paths — ...` |
| **Final file count exceeded** | After all intersections/exclusions, resolved set exceeds `max_files` | `resolved 12345 files, exceeding the limit of 10000 — ...` |
| **Empty glob result** | Glob patterns matched 0 files (nothing to do) | `file patterns matched 0 files: "src/**/*.ts"` |

All three are **fatal** (not warnings), all flow through `ReportBuilder` as `Severity::Fatal` + `DiagnosticOrigin::Config`.

**Why fatal, not warning**: The empty case means tractor literally has nothing to do — continuing silently is misleading. The limit cases indicate misconfiguration that would cause poor performance or hangs.

### 2. Two-tier file limit with a single user-facing knob

- **`--max-files`** (default: 10,000) — the maximum number of files tractor will process
- **Expansion limit** = `10 × max_files` (internal, not exposed) — how far any single glob can expand before bailing

**Why two tiers**: The expansion limit protects against runaway filesystem walks (e.g., `/**/*.cs` crawling an entire disk). The final limit catches cases where multiple legitimate globs combine to an unexpectedly large set. Using a single `--max-files` knob keeps the UX simple.

**Why 10,000 default**: Typical projects have 50–2,000 files. Large monorepos reach 5,000–15,000. 10,000 covers normal use while catching mistakes. Users can increase with `--max-files 50000`.

**Why 10× ratio for expansion**: Allows orthogonal glob patterns (e.g., `**/*.cs` ∩ `frontend/**`) where individual expansions are large but the intersection is small. A user who sets `--max-files 50000` gets 500,000 expansion ceiling, which scales proportionally.

### 3. Diagnostics go through the Report pipeline, not stderr

**Decision**: All file resolution diagnostics are `ReportMatch` entries with `Severity::Fatal` and `DiagnosticOrigin::Config`.

**Why not `eprintln!`**: Tractor has multiple output formats (JSON, YAML, GCC, GitHub annotations). Raw stderr would be invisible in structured output. The Report pipeline ensures diagnostics appear correctly in all formats, including CI pipelines using `--format json` or `--format github`.

### 4. CLI files for `tractor run` intersect with config globs

**Decision**: `tractor run config.yaml [files...]` accepts positional file arguments. These are **intersected** with the config's glob patterns, not unioned.

**Why intersection**: The use case is "run this config's rules but only on these specific files." The config defines the broad scope; CLI args narrow it. This matches how `--diff-files` works — it narrows, never widens.

### 5. Glob expansion uses lazy iteration with early bailout

**Decision**: `expand_globs_checked()` counts files during the lazy `glob::glob()` iterator and bails immediately when the expansion limit is hit. No double-expansion needed.

**Why**: The `glob` crate returns a lazy iterator that walks the filesystem on each `.next()`. By counting during iteration, we catch runaway patterns mid-walk without waiting for completion.

### 6. Scope of empty-glob fatal: only the overall file set

**Decision**: The "0 files matched" fatal applies to the overall resolved file set for an operation. Per-rule `include` patterns matching nothing are **not** checked.

**Why**: A rule's include matching nothing within an operation's file set can be normal in multi-language configs. The "did it even work?" case is when the operation-level globs themselves match nothing on disk.

---

## Rejected Alternatives

### Warning instead of fatal for empty glob
**Rejected because**: If tractor has 0 files to process, continuing is pointless. A warning would still show "success" with 0 results, which is the exact confusion we're trying to fix.

### Timeout-based protection instead of file count limit
**Rejected because**: Timeouts depend on disk speed and are non-deterministic. A file count limit is predictable, produces actionable messages ("matched over 10,000 files"), and tells the user *what's wrong* rather than just *that it's slow*.

### Per-glob-layer independent limits
**Rejected because**: Two orthogonal globs (`**/*.cs` ∩ `frontend/**`) might each be large individually but intersect to a small set. A global limit on the final result is what matters. The 10× expansion limit still protects against runaway single-pattern walks.

### Single limit for both expansion and final count
**Rejected because**: Would be too restrictive for the orthogonal-glob case. If `--max-files=10000`, then `**/*.cs` expanding to 15,000 (which intersects down to 200) would be blocked unnecessarily.

### `eprintln!` for quick implementation
**Rejected because**: Would be invisible in JSON/YAML output formats. The Report pipeline already supports `Severity::Info` and `DiagnosticOrigin::Config` — using it is only slightly more work and much more consistent.

---

## Implemented (on branch)

- [x] `expand_globs_checked()` in `tractor-core/src/parallel.rs` — expansion with limit and empty-pattern tracking
- [x] `GlobExpansion` and `GlobExpansionError` types
- [x] `--max-files` flag on `SharedArgs` (all commands)
- [x] `files` positional arg on `RunArgs` (`tractor run`)
- [x] `ExecuteOptions` extended with `max_files` and `cli_files`
- [x] `resolve_files` / `resolve_op_files` — emit fatal diagnostics through `ReportBuilder`
- [x] `make_fatal_diagnostic()` helper
- [x] CLI path (`resolve_input`) also uses `expand_globs_checked` with expansion limit
- [x] All modes pass `max_files` from `SharedArgs`
- [x] Tests pass (318 tests)
- [x] Help text snapshots updated

---

## Still TODO (in scope for this ticket)

### Root ∩ Operation file intersection in `merge_scope`

**Current behavior**: `merge_scope` in `tractor_config.rs` **overrides** root-level `files` with operation-level `files` when the operation specifies its own.

**Desired behavior**: Operation `files` should **intersect** with root `files`, not override. Each level narrows scope, never widens it. The include chain should be: `root files ∩ operation files ∩ CLI files`, with excludes subtractive at all levels.

**Approach discussed**: 
- Pass root-level files through `ExecuteOptions` as `config_root_files` (similar to `cli_files`)
- Stop merging files in `merge_scope` — operations keep only their own files
- `resolve_files` handles the three-way intersection
- Pre-compute `CLI ∩ root` once before iterating operations (since it's shared across all operations), then intersect per-operation files with that result

**Implementation started**: A `LoadedConfig` struct was created to return root files separately from `load_tractor_config`, but the branch had issues (session crashed during testing) and this work may need to be verified/redone. Check git stash or working tree state.

### Manual testing of intersection semantics

The previous session crashed while trying to manually test intersection behavior. Need to verify:
- CLI files ∩ config root files works
- Root files ∩ operation files works  
- The three-way intersection works end-to-end
- Edge cases: one layer empty, non-overlapping patterns, etc.

### Integration tests for new diagnostics

No integration tests were written yet for the three fatal errors. Need tests for:
- Empty glob → fatal exit
- Exceeding `--max-files` → fatal exit
- Exceeding expansion limit (10×) → fatal exit
- CLI files intersection with config

---

## Out of Scope (future work)

1. **Per-rule `include` matching nothing** — warning when a rule's include pattern matches none of the operation's files. Deferred because it can be normal in multi-language configs.

2. **Diff filter narrowing to 0 files** — when `--diff-files` reduces the set to empty. This is often "nothing to do right now" (no relevant changes), not a config error. Could be a `--verbose` message later.

3. **`--verbose` showing resolved base_dir and glob expansion details** — nice for debugging but not critical.

4. **`tractor check` / `tractor set` CLI modes with file intersection** — these modes resolve files differently (through `resolve_input`, not the executor pipeline). The expansion limit is wired up for CLI modes, but intersection with config files doesn't apply (they don't use config files).

5. **Merging `tractor check` and `tractor run`** — tracked in todo #23. The check command will eventually be merged into the run command, so investing in check-specific features is deferred.

6. **Progress indicator during glob expansion** — could show "expanding patterns..." for long-running globs. The file count limit mostly eliminates this need.
