# Design: Centralized File Resolution (FileResolver)

**Status**: Implemented
**Related PR**: #82 (root intersection, SharedFileScope, module split),
#128 (canonical glob walker, path intersection fix)
**Related issue**: #78 (implicit // in config), #127 (path filter intersection)

---

## Current State (after #128)

File resolution uses a vertically integrated architecture: a single
custom glob walker handles filesystem traversal, pattern matching, and
path canonicalization in one pass. This replaced the `glob` crate
dependency.

Core modules:

- `tractor-core/src/glob_match.rs` -- custom glob engine:
  `CompiledPattern` for pattern matching (WASM-safe), `expand_canonical()`
  for filesystem walking with canonical path construction (native-only)
- `tractor-core/src/files.rs` -- expansion API: `expand_globs_checked()`
  delegates to `expand_canonical()`, applies limits
- `tractor/src/file_resolver.rs` -- `FileResolver`: centralized
  pre-computation of shared scopes (root, CLI, diff), per-operation
  resolution with intersection, filtering, and diagnostics

Supporting modules:
- `tractor/src/pipeline/git.rs` -- git diff integration
- `tractor/src/pipeline/input.rs` -- CLI-mode input source detection
  (stdin/inline/files)
- `tractor/src/pipeline/matcher.rs` -- per-rule include/exclude matching
  (uses `CompiledPattern` from glob_match)

Path canonicalization is consolidated into `NormalizedPath::absolute()`
which all entry points use. The `to_absolute_path()` helper in output
formatting delegates to it.

---

## Design Goals

### 1. Unified pattern language across all levels

All file scope levels -- root, operation, rule, CLI -- use the same glob
pattern language. A rule's `include: ["**/*Controller*"]` is the same
language as an operation's `files: ["src/**/*.rs"]`. `CompiledPattern`
handles matching uniformly. Whether a pattern triggers a filesystem walk
or an in-memory filter is an optimization choice, not a user-visible
distinction.

### 2. Canonical paths by construction

All discovered paths are canonical from the moment of creation. The
custom glob walker (`expand_canonical`) achieves this by:

- Canonicalizing the root prefix once (resolves symlinks, 8.3 short
  names on Windows, and true filesystem casing)
- Building child paths by appending `read_dir` entry names (which have
  true filesystem casing) with `/` separators
- Never constructing intermediate `PathBuf` values that could introduce
  backslashes or inconsistent casing

This eliminates the class of bugs where paths from different sources
(CLI, config, glob expansion) don't compare equal due to casing or
separator differences.

### 3. Composable scope operations

File scopes compose through set operations: intersect, exclude, include.
Each level narrows (or in future, re-includes) from its parent scope.
The system should support expressing these operations declaratively,
and choose the cheapest evaluation strategy internally.

### 4. Ordered evaluation (gitignore-style)

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

### 5. Shared computation, no redundant work

Scopes that are shared across operations (root files, CLI files, global
diff, gitignore) should be computed once. `FileResolver` pre-computes
root files, CLI files, and global diff once, then each operation calls
`resolve()` with a `FileRequest` describing what it needs. Overlapping
glob patterns are deduplicated after expansion.

### 6. Single entry point for callers

Callers describe *what files they need* via `FileRequest` (patterns,
excludes, diff spec). `FileResolver::resolve()` handles the entire
pipeline: expansion, intersection, filtering, limits, diagnostics.

### 7. CLI and run paths share the same resolution

`resolve_input()` feeds into `FileResolver` for file-based execution.
CLI modes are just a simpler case (no root scope, no multi-operation
config). Config-based execution uses the same `FileResolver` with
additional root scope and per-operation intersections.

### 8. Diagnosable

Verbose logging shows each resolution step with normalized paths and
counts. The format is consistent across all phases:

```
files: working directory D:/Work/Repo
files: resolving relative to D:/Work/Repo
files: max 10000 files
files: root scope "src/**/*.cs" expanded to 78 file(s)
files: CLI args "src/Example.cs" expanded to 1 file(s)
files: 1 file(s) after root/operation ∩ CLI intersection (was 78)
files: result 1 file(s)
```

### 9. Fast

The custom glob walker is ~7x faster than the `glob` crate on large
codebases (1.5s vs 10s on 205K files). The speedup comes from:

- Walking with `read_dir` and matching during traversal (no separate
  pattern-matching pass)
- Building paths as strings by appending entry names (no `PathBuf`
  allocation per entry)
- Single `canonicalize()` call at the root instead of per-file

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

## Module Architecture

| Module | Role |
|---|---|
| `tractor-core/src/glob_match.rs` | Custom glob engine: `CompiledPattern` (matching, WASM-safe) + `expand_canonical()` (walk, native) |
| `tractor-core/src/files.rs` | Expansion API: `expand_globs_checked()` with limits, delegates to `expand_canonical()` |
| `tractor-core/src/normalized_path.rs` | `NormalizedPath::absolute()` — single source of truth for path canonicalization |
| `tractor-core/src/glob_pattern.rs` | `GlobPattern` — normalized pattern string wrapper for storage/serialization |
| `tractor-core/src/rule.rs` | `GlobMatcher` — two-layer include/exclude matching using `CompiledPattern` |
| `tractor/src/file_resolver.rs` | `FileResolver` — centralized resolution: pre-compute shared scopes, per-operation `resolve()` |
| `tractor/src/pipeline/matcher.rs` | `run_rules()` — applies per-rule glob matching after file discovery |
| `tractor/src/pipeline/input.rs` | CLI-mode input source detection (stdin/inline/files) |

## Glob Walker Design (`expand_canonical`)

The walker is vertically integrated: filesystem traversal, pattern
matching, path construction, and canonicalization happen in a single
recursive pass.

1. Split pattern at first wildcard → literal root prefix + wildcard suffix
2. Canonicalize root prefix once (`std::fs::canonicalize`)
3. Compile wildcard suffix into `CompiledPattern`
4. Walk with `std::fs::read_dir` recursively:
   - Build each path by appending `entry.file_name()` (true casing) to
     parent path string with `/`
   - Match each file's relative path against compiled suffix
   - Return `Vec<NormalizedPath>` — canonical by construction
5. Symlinks: traversed using link name (not target); depth limit (100)
   prevents cycles
6. Permission-denied directories: skipped silently

Pattern syntax: `*` (single segment), `**` (zero or more segments),
`?` (single character). `[...]` rejected with error. Case-insensitive
on Windows.

## Not in Scope

- **Cross-operation parse deduplication**: Two operations targeting the same files still parse independently. A shared parse cache is a separate concern.
