# Architecture Reorganization

## Problem

The file structure doesn't tell the architecture story. Key issues:

1. **`executor.rs`** is a 1700-line monolith containing all operation types + execution logic
2. **`pipeline/`** mixes actual pipeline stages (matcher, format) with unrelated code (git, context)
3. **`modes/`** is named poorly — these are CLI commands, not "modes"
4. **`tractor-core`** exists as a separate crate solely for WASM compatibility, but the name implies it's the architectural core. It's really "the WASM-safe subset"
5. **`context.rs`** does CLI arg processing but lives in `pipeline/`

## Target Structure

### tractor/src/

```
main.rs

cli/
  mod.rs              — Cli, Command, SharedArgs, DocsCommand
  help.rs             — help text injection, parse error hints
  query.rs            — QueryArgs + run_query (args → executor call)
  check.rs            — CheckArgs + run_check
  test.rs             — TestArgs + run_test
  set.rs              — SetArgs + run_set
  update.rs           — UpdateArgs + run_update
  render.rs           — RenderArgs + run_render
  run.rs              — RunArgs + run_run (config file execution)
  config.rs           — tractor config file parsing (YAML/TOML rule files)
  languages.rs        — `docs languages` subcommand

executor/
  mod.rs              — execute() entry point, ExecuteOptions, shared types
  query.rs            — QueryOperation + execution
  check.rs            — CheckOperation + execution
  test.rs             — TestOperation, TestAssertion + execution
  set.rs              — SetOperation, SetMapping + execution
  update.rs           — UpdateOperation + execution

input/
  mod.rs              — InputMode, resolve stdin vs files
  file_resolver.rs    — glob expansion, file discovery
  filter.rs           — diff-files/diff-lines filtering
  git.rs              — git diff helpers

matcher.rs            — parallel parse + XPath dispatch (uses rayon)

format/
  mod.rs              — OutputFormat, ViewField, ViewSet, GroupDimension, render dispatch
  options.rs          — option types and parsing
  text.rs             — human-readable text renderer
  json.rs             — JSON report renderer
  yaml.rs             — YAML report renderer
  xml.rs              — XML report renderer
  gcc.rs              — GCC-style one-line renderer
  github.rs           — GitHub Actions annotation renderer
  claude_code.rs      — Claude Code hook JSON renderer
  shared.rs           — shared rendering utilities

version.rs
```

### Merge tractor-core into tractor

The two-crate split exists solely for WASM. Instead:

1. Move all `tractor-core/src/` modules into `tractor/src/` (under logical groupings or a `core/` namespace)
2. Use a Cargo feature flag to gate OS-dependent modules:
   ```toml
   [features]
   default = ["cli"]
   cli = ["rayon", "walkdir", "clap"]
   ```
3. `#[cfg(feature = "cli")]` on modules that need filesystem/threads (cli/, input/, matcher, etc.)
4. Create a thin `tractor-wasm/` crate that depends on `tractor` with `default-features = false` and exposes `#[wasm_bindgen]` bindings
5. Update `web/` to import from the new WASM crate

### What's WASM-safe (no `cli` feature needed)

From current tractor-core:
- parser/, languages/, xpath/ — parsing and querying
- output/ — XML/tree rendering
- render/ — XML→source code (reverse rendering)
- report.rs, rule.rs — report model
- replace.rs, xpath_upsert.rs, declarative_set.rs — mutation
- normalized_xpath.rs, normalized_path.rs, glob_pattern.rs — types
- source_utils.rs, tree_mode.rs — utilities

From current tractor:
- format/ (mostly) — report renderers (text, json, gcc, etc.)
- executor/ operation types — pure data structs

### What needs `cli` feature

- cli/ — clap, CLI args
- input/ — filesystem, walkdir, glob
- matcher.rs — rayon parallelism
- input/git.rs — shells out to `git`

## Execution Plan

### Phase 1: Reorganize tractor/src (no crate boundary changes) ✅ Done
1. ✅ Rename `modes/` → content moves into `cli/`
2. ✅ Split `executor.rs` (1708 lines) into `executor/` directory with per-operation files
3. ✅ Move `pipeline/` contents to their logical homes:
   - `pipeline/format/` → `format/`
   - `pipeline/git.rs`, `pipeline/input.rs` + `file_resolver.rs` + `filter.rs` → `input/`
   - `pipeline/matcher.rs` → `matcher.rs`
   - `pipeline/context.rs` → `cli/context.rs`
4. ✅ Delete empty `pipeline/` and `modes/`

### Phase 2: Merge tractor-core into tractor ✅ Done
1. ✅ Add feature flags (`native`, `wasm`) to `tractor/Cargo.toml`
2. ✅ Move tractor-core modules into `tractor/src/` (flat, alongside CLI modules)
3. ✅ Update all imports (`tractor_core::` → `tractor::`)
4. ✅ WASM exports via `#[cfg(feature = "wasm")]` in the same crate (no separate tractor-wasm crate needed)
5. ✅ Update web/ build to use `tractor` crate with `--features wasm`
6. ✅ Remove `tractor-core/` from workspace

### Phase 3: Cleanup
1. Update CLAUDE.md / architecture docs
2. Verify `task test` passes (cargo tests, web vitest, integration tests)
3. Verify WASM build still works
