# Design: Centralized File Resolution (FileResolver)

**Status**: Partially implemented
**Related PR**: #82 (root intersection, SharedFileScope, module split)
**Related issue**: #78 (implicit // in config)

---

## Current State (after #82)

File resolution is now split across two dedicated modules:

- `tractor-core/src/files.rs` -- low-level primitives: `expand_globs_checked()`,
  `filter_supported_files()`, `GlobExpansion`, safety limits
- `tractor/src/pipeline/files.rs` -- high-level resolution: `SharedFileScope`
  (pre-computes root, CLI, and global diff once), `resolve_op_files()`,
  `resolve_files()` (expansion, intersection, exclusion, diff, limits)

Supporting modules:
- `tractor/src/pipeline/git.rs` -- `git_changed_files()`, `intersect_changed()`, `DiffHunkFilter`
- `tractor/src/pipeline/input.rs` -- CLI-mode input source detection (stdin/inline/files),
  with its own glob expansion path separate from `pipeline/files.rs`
- `tractor/src/tractor_config.rs` -- `merge_scope()` merges exclude/diff settings per operation;
  no longer touches `files` (root files handled by `SharedFileScope`)
- `tractor/src/pipeline/matcher.rs` -- `run_rules()` per-rule include/exclude glob matching

## Remaining Problem

The two file resolution paths (CLI modes via `pipeline/input.rs` and run mode
via `pipeline/files.rs`) still duplicate glob expansion and language filtering.
A `FileResolver` would unify them behind a single API with caching.

## Proposed: FileResolver

### 1. Single FileResolver struct owns all file resolution

A `FileResolver` holds the shared state (root files, CLI files, global diff, base dir, limits) and provides a single `resolve()` method that operations call with their per-operation patterns.

```rust
pub struct FileResolver {
    base_dir: Option<PathBuf>,
    max_files: usize,
    expansion_limit: usize,
    verbose: bool,

    // Cache: original glob patterns -> expanded file set.
    // Identical patterns across operations expand only once.
    cache: HashMap<Vec<String>, Arc<HashSet<String>>>,

    // Pre-resolved shared scopes.
    root: Option<Arc<HashSet<String>>>,
    cli: Option<Arc<HashSet<String>>>,
    global_diff: Option<HashSet<PathBuf>>,
}
```

**Why**: One place for all expansion, caching, intersection, exclusion, diff filtering, limits, and verbose logging. Callers describe what they need; the resolver decides how.

### 2. Declarative FileRequest instead of positional arguments

Operations build a `FileRequest` describing their scope:

```rust
pub struct FileRequest<'a> {
    pub files: &'a [String],         // operation-level glob patterns
    pub exclude: &'a [String],       // operation-level exclusion patterns
    pub diff_files: Option<&'a str>, // per-operation diff spec
    pub diff_lines: Option<&'a str>, // per-operation diff-lines spec
}
```

**Why**: The current `resolve_files` takes 8 positional parameters. A struct is self-documenting and extensible without breaking callers.

### 3. Automatic glob expansion cache

`FileResolver` caches expanded glob sets keyed by the original pattern list. Two operations with identical `files: ["**/*.ts"]` expand once.

```rust
fn expand_cached(&mut self, patterns: &[String]) -> &HashSet<String>
```

**Why**: In a config with 10 operations sharing the same file patterns, the current code expands globs 10 times. The cache eliminates redundant filesystem walks automatically.

### 4. resolve() handles all three cases

The `resolve()` method contains the full resolution pipeline:

1. **Determine base files**: operation files (expand, possibly cached) or root files (pre-computed) as fallback
2. **Intersect with root** (when operation has its own files and root is defined)
3. **Intersect with CLI files** (when provided)
4. **Apply excludes** (union of root and operation excludes)
5. **Filter to supported languages**
6. **Apply diff-files** (global from pre-computed set, per-operation runs git)
7. **Apply diff-lines result filters**
8. **Check max_files limit**
9. **Check empty result** (fatal diagnostic)

**Why**: All steps in one pipeline, in one place. The three cases (both root+op, op only, root only) are handled at step 1-2 instead of being spread across `merge_scope` and `resolve_files`.

### 5. CLI modes can use FileResolver too

`resolve_input()` in `pipeline/input.rs` currently duplicates glob expansion and language filtering. It could construct a `FileResolver` with no root/CLI scope and call `resolve()` with the CLI-provided patterns.

**Why**: Unifies the two file resolution paths (CLI and run) under one implementation. Not required initially -- can be done incrementally.

### 6. Per-rule include/exclude stays separate

Rule-level `include` and `exclude` in `run_rules()` use `glob::Pattern::matches()` against an already-resolved file list. This is in-memory pattern matching, not filesystem glob expansion.

**Why**: Different operation -- no filesystem walk, no caching benefit. Mixing it into FileResolver would add complexity without value. The rule-level matching operates on the output of FileResolver, not alongside it.

## Mapping: Current Code -> FileResolver

| Current | Proposed |
|---|---|
| `SharedFileScope::build()` in `pipeline/files.rs` | `FileResolver::new()` |
| `resolve_files()` in `pipeline/files.rs` | `FileResolver::resolve()` |
| `resolve_op_files()` in `pipeline/files.rs` | Removed -- callers build `FileRequest` directly |
| `merge_scope()` exclude/diff logic | Stays (or moves into `FileRequest` construction) |
| `resolve_input()` in `pipeline/input.rs` | Optional: can use `FileResolver` for consistency |
| `expand_globs_checked()` in `tractor-core/src/files.rs` | Still the low-level primitive, called only from `FileResolver` |
| `filter_supported_files()` in `tractor-core/src/files.rs` | Called inside `FileResolver::resolve()` |
| Per-rule glob matching in `run_rules()` | Expressed as FileResolver operations (see below) |

## Where It Lives

`tractor/src/pipeline/files.rs` -- evolves the existing module. The `SharedFileScope` + `resolve_files` code there is the natural starting point for the `FileResolver` struct.

The low-level primitives stay in `tractor-core/src/files.rs` because they are also used by the WASM web build.

## Verbose Logging

All file resolution logging goes through `FileResolver`. The log output follows a pipeline narrative:

```
  files: resolving relative to /home/user/project
  files: max 10000 files, expansion limit 100000
  files: expanding root scope "src/**/*.js" ...
  files: root scope has 342 file(s)
  files: expanding operation "src/core/**/*.js" ...
  files: operation has 48 file(s)
  files: 48 file(s) after root intersection (was 48)
  files: 45 file(s) after excludes
  files: 42 file(s) after language filter
  files: 12 file(s) after diff-files filter
```

The "expanding ..." line prints before the glob walk starts, so runaway expansions are diagnosable (the user sees which pattern is stuck and can Ctrl+C).

## Glob Pattern Arithmetic

All file scope levels use the same glob pattern language. Whether a pattern
triggers a filesystem walk or an in-memory filter is an optimization detail,
not a semantic distinction. A rule's `include: ["**/*Controller*"]` is the
same language as an operation's `files: ["src/**/*.rs"]`.

The FileResolver should provide glob pattern operations that compose:

```
root_scope("src/**/*.rs")          -- filesystem expansion
  .intersect("src/core/**/*.rs")   -- narrow (filesystem or in-memory)
  .exclude("**/*_test.rs")         -- subtract
  .intersect("**/*Controller*")    -- narrow further (rule include)
  .exclude("**/generated/**")      -- subtract (rule exclude)
```

### Order matters

Like `.gitignore`, include/exclude patterns are not commutative. A later
include can re-include files that an earlier exclude removed. The current
model (all excludes are subtractive, applied once) is simpler but less
expressive. The FileResolver should support ordered filter chains:

```yaml
# Hypothetical future syntax:
files:
  - "src/**/*.rs"        # include
  - "!src/generated/**"  # exclude
  - "src/generated/api.rs"  # re-include specific file
```

This is a future direction. The current implementation uses separate
`files` and `exclude` fields without ordering. But the FileResolver's
internal model should not preclude ordered evaluation.

### Implementation strategy

Given a resolved parent set, the resolver can choose the cheapest strategy:
- **No parent set**: expand against filesystem (`glob::glob()`)
- **Has parent set, pattern is a filter** (e.g. `**/*Controller*`):
  in-memory `glob::Pattern::matches()` against the parent set
- **Has parent set, pattern adds new paths**: expand against filesystem,
  then union with parent set

The caller doesn't need to know which strategy is used. They express intent
(intersect, exclude, include); the resolver picks the implementation.

### Per-rule patterns through FileResolver

Currently, per-rule `include`/`exclude` patterns are handled by
`run_rules()` via `GlobMatcher` in `tractor-core`. These should eventually
be expressed as FileResolver operations so that:
- The same caching and optimization applies
- Verbose logging shows rule-level file narrowing
- The pattern language is guaranteed consistent across all levels

## Not in Scope

- **Cross-operation file deduplication for parsing**: Two operations targeting the same files still parse them independently. A shared parse cache is a separate optimization.
- **Async/streaming glob expansion**: The `glob` crate is synchronous. Changing this would require a different crate and is not needed for the current performance profile.
