# Design: Centralized File Resolution (FileResolver)

**Status**: Proposed  
**Related PR**: #82 (root ∩ operation intersection)  
**Related issue**: #78 (implicit // in config)

---

## Problem Statement

File resolution logic is scattered across multiple functions and modules:

- `merge_scope()` in `tractor_config.rs` — merges exclude/diff settings per operation
- `SharedFileScope::build()` in `executor.rs` — pre-computes root, CLI, and global diff files
- `resolve_files()` in `executor.rs` — 80-line function handling expansion, intersection, exclusion, language filtering, diff filtering, and limit checks
- `resolve_op_files()` in `executor.rs` — wrapper that builds diff-lines filters then calls `resolve_files`
- `resolve_input()` in `pipeline/input.rs` — separate file resolution path for CLI modes (check, query, test, update)
- `run_rules()` in `pipeline/matcher.rs` — per-rule include/exclude glob matching against resolved files

This makes it hard to reason about file resolution as a whole, leads to duplicated glob expansion across CLI and run paths, and made the root ∩ operation intersection harder to implement than necessary.

## Decisions

### 1. Single FileResolver struct owns all file resolution

A `FileResolver` holds the shared state (root files, CLI files, global diff, base dir, limits) and provides a single `resolve()` method that operations call with their per-operation patterns.

```rust
pub struct FileResolver {
    base_dir: Option<PathBuf>,
    max_files: usize,
    expansion_limit: usize,
    verbose: bool,

    // Cache: original glob patterns → expanded file set.
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

**Why**: Unifies the two file resolution paths (CLI and run) under one implementation. Not required initially — can be done incrementally.

### 6. Per-rule include/exclude stays separate

Rule-level `include` and `exclude` in `run_rules()` use `glob::Pattern::matches()` against an already-resolved file list. This is in-memory pattern matching, not filesystem glob expansion.

**Why**: Different operation — no filesystem walk, no caching benefit. Mixing it into FileResolver would add complexity without value. The rule-level matching operates on the output of FileResolver, not alongside it.

## Mapping: Current Code → FileResolver

| Current | Proposed |
|---|---|
| `SharedFileScope::build()` | `FileResolver::new()` |
| `resolve_files()` | `FileResolver::resolve()` |
| `resolve_op_files()` | Removed — callers build `FileRequest` directly |
| `merge_scope()` files logic | Removed — `FileResolver` handles root fallback |
| `merge_scope()` exclude/diff logic | Stays (or moves into `FileRequest` construction) |
| `resolve_input()` in pipeline | Optional: can use `FileResolver` for consistency |
| `expand_globs_checked()` in tractor-core | Still the low-level primitive, called only from `FileResolver` |
| `filter_supported_files()` in tractor-core | Called inside `FileResolver::resolve()` |
| Per-rule glob matching in `run_rules()` | Unchanged |

## Where It Lives

New file: `tractor/src/file_resolver.rs`

The executor imports and uses it. The module is internal to the `tractor` crate (not tractor-core) because it depends on git integration and CLI concerns.

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

## Not in Scope

- **Per-rule file resolution through FileResolver**: Rule include/exclude is pattern matching on resolved files, not glob expansion. It stays in `run_rules()`.
- **Cross-operation file deduplication for parsing**: Two operations targeting the same files still parse them independently. A shared parse cache is a separate optimization.
- **Async/streaming glob expansion**: The `glob` crate is synchronous. Changing this would require a different crate and is not needed for the current performance profile.
