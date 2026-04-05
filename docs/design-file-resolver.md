# Design: Centralized File Resolution (FileResolver)

**Status**: Partially implemented
**Related PR**: #82 (root intersection, SharedFileScope, module split)
**Related issue**: #78 (implicit // in config)

---

## Current State (after #82)

File resolution is split across two dedicated modules:

- `tractor-core/src/files.rs` -- low-level primitives: glob expansion,
  language filtering, safety limits
- `tractor/src/pipeline/files.rs` -- high-level resolution: shared scope
  pre-computation, per-operation resolution, intersection, diff filtering

Supporting modules:
- `tractor/src/pipeline/git.rs` -- git diff integration
- `tractor/src/pipeline/input.rs` -- CLI-mode input source detection
  (stdin/inline/files), with its own separate glob expansion path
- `tractor/src/pipeline/matcher.rs` -- per-rule include/exclude matching

The CLI and run paths still duplicate glob expansion and language
filtering. Per-rule file matching uses the same pattern language as
operation-level globs but is handled by separate code.

---

## Design Goals

### 1. Unified pattern language across all levels

All file scope levels -- root, operation, rule, CLI -- use the same glob
pattern language. A rule's `include: ["**/*Controller*"]` is the same
language as an operation's `files: ["src/**/*.rs"]`. The system should
treat them uniformly. Whether a pattern triggers a filesystem walk or
an in-memory filter is an optimization choice, not a user-visible
distinction.

### 2. Composable scope operations

File scopes compose through set operations: intersect, exclude, include.
Each level narrows (or in future, re-includes) from its parent scope.
The system should support expressing these operations declaratively,
and choose the cheapest evaluation strategy internally.

### 3. Ordered evaluation (gitignore-style)

Like `.gitignore`, include/exclude patterns are not always commutative.
A later include can re-include files that an earlier exclude removed.
The current model (separate `files` and `exclude` fields) is a simplified
case. The internal model should not preclude ordered evaluation:

```yaml
# Future possibility:
files:
  - "src/**/*.rs"            # include
  - "!src/generated/**"      # exclude
  - "src/generated/api.rs"   # re-include specific file
```

### 4. Shared computation, no redundant work

Scopes that are shared across operations (root files, CLI files, global
diff, gitignore) should be computed once. Identical glob patterns across
operations should be expanded once (cached). The current `SharedFileScope`
achieves this for root/CLI/diff; a cache would extend it to operation
patterns.

### 5. Single entry point for callers

Callers should describe *what files they need* (patterns, excludes, diff
spec), not manage the resolution pipeline themselves. The current
`resolve_files` takes 8 positional parameters; a declarative request
struct would be clearer and extensible.

### 6. CLI and run paths share the same resolution

`resolve_input()` and `resolve_files()` should use the same underlying
system. CLI modes are just a simpler case (no root scope, no multi-operation
config).

### 7. Diagnosable

Verbose logging shows each resolution step with patterns, base
directories, and file counts. "Expanding ..." prints before the glob
walk starts so runaway expansions are visible and the user can Ctrl+C.
This is already implemented.

---

## Gitignore Support

Gitignored files are almost never targets for linting. The file resolver
should support excluding them, controlled by a flag:

```yaml
# In config:
gitignore: true
```

```bash
# Or from CLI:
tractor run config.yaml --gitignore
tractor check "src/**/*.rs" --gitignore
```

**Default**: off (current behavior -- explicit globs are respected as-is).

When enabled, gitignored files are excluded early in the pipeline as a
shared scope concern, computed once before operation-level resolution.

**Implementation is open.** Options include delegating to `git ls-files`,
using a Rust crate like `ignore`, or a combination. The key requirement
is correctness: the behavior should match what git considers ignored,
including nested `.gitignore` files, global config, and negation patterns.

---

## Per-rule Patterns

Currently, per-rule `include`/`exclude` is handled separately in
`run_rules()` via `GlobMatcher`. This should eventually be expressed
through the same file resolution system so that:

- The pattern language is guaranteed consistent across all levels
- Caching and optimization apply uniformly
- Verbose logging covers rule-level narrowing

---

## Resolution Pipeline

The full pipeline, from broadest to narrowest:

1. **Root scope** -- config-level `files` (shared across all operations)
2. **Operation scope** -- per-operation `files` (intersected with root)
3. **CLI scope** -- positional file args (intersected with above)
4. **Gitignore filter** -- when enabled
5. **Exclude patterns** -- root + operation excludes
6. **Language filter** -- only supported file types
7. **Diff-files filter** -- global (pre-computed) + per-operation
8. **Diff-lines filter** -- hunk-level result filtering
9. **Rule include/exclude** -- per-rule narrowing within operation files
10. **Safety limits** -- max_files, expansion limit, empty-set diagnostic

Steps 1-3 use set intersection. Steps 4-8 are subtractive filters.
Step 9 is per-rule narrowing. Step 10 is validation.

---

## Current Code -> Future FileResolver

| Current | Direction |
|---|---|
| `SharedFileScope` in `pipeline/files.rs` | Evolves into `FileResolver` |
| `resolve_files()` in `pipeline/files.rs` | Becomes `FileResolver::resolve()` |
| `resolve_op_files()` in `pipeline/files.rs` | Callers build a request struct |
| `resolve_input()` in `pipeline/input.rs` | Uses `FileResolver` for file resolution |
| `expand_globs_checked()` in `tractor-core/src/files.rs` | Low-level primitive, stays |
| Per-rule glob matching in `run_rules()` | Expressed through FileResolver |

## Not in Scope

- **Cross-operation parse deduplication**: Two operations targeting the same files still parse independently. A shared parse cache is a separate concern.
- **Async/streaming glob expansion**: Not needed for the current performance profile.
