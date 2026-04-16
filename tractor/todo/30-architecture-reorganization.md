# Architecture Reorganization

## Problem

The file structure doesn't tell the architecture story. Key issues:

1. **`executor.rs`** is a 1700-line monolith containing all operation types + execution logic
2. **`pipeline/`** mixes actual pipeline stages (matcher, format) with unrelated code (git, context)
3. **`modes/`** is named poorly — these are CLI commands, not "modes"
4. **`tractor-core`** exists as a separate crate solely for WASM compatibility, but the name implies it's the architectural core. It's really "the WASM-safe subset"
5. **`context.rs`** does CLI arg processing but lives in `pipeline/`

## Final Structure

### tractor/src/

```
lib.rs                — library entry point (public API + re-exports)
main.rs               — CLI entry point

  ── CLI (the tool, native-only) ──

cli/                  — CLI argument parsing & command handlers
executor/             — parallel execution per operation type
format/               — output format renderers (text, json, gcc, ...)
input/                — stdin/file resolution, diff filtering, git
matcher.rs            — parallel parse + XPath dispatch (uses rayon)
tractor_config.rs     — YAML/TOML config file parsing
version.rs            — version info

  ── Library (the engine, native + WASM) ──

parser/               — tree-sitter parsing (native-only)
xot/                  — xot XML tree building & transformation
  builder.rs          — TreeSitter AST → xot document
  transform.rs        — generic tree walker & xot helpers
languages/            — per-language semantic transforms + metadata
  info.rs             — language metadata (names, extensions, features)
  csharp.rs, go.rs, ... — per-language transforms
xpath/                — XPath 3.1 query engine
output/               — XML/tree rendering, syntax highlighting
  source_utils.rs     — source snippet extraction
render/               — reverse rendering (XML→source code)
model/                — core data types
  report.rs           — report, match, severity, totals
  rule.rs             — check rule with XPath + glob matchers
  tree_mode.rs        — raw/structure/data tree mode enum
  normalized_xpath.rs — auto-prefixed XPath expression wrapper
glob/                 — file discovery & path handling
  matching.rs         — glob pattern matching + filesystem walking
  pattern.rs          — normalized glob pattern wrapper
  normalized_path.rs  — forward-slash normalized path wrapper
  files.rs            — glob expansion with limits (native-only)
mutation/             — code modification
  replace.rs          — match-based text replacement
  xpath_upsert.rs     — XPath-based insert/update for data files
  declarative_set.rs  — declarative path=value set expressions
wasm/                 — WASM bindings for web playground
  ast.rs              — serializable AST types for JS interop
```

### Feature flags

```toml
[features]
default = ["native"]
native = ["tree-sitter parsers", "rayon", "clap", ...]  # CLI binary
wasm = ["wasm-bindgen", "console_error_panic_hook"]      # WASM cdylib
```

Binary targets have `required-features = ["native"]`. Library modules use
`#[cfg(feature = "native")]` or `#[cfg(feature = "wasm")]` as needed.

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

### Phase 3: Organize library modules into logical directories ✅ Done
1. ✅ `glob/` — matching.rs, pattern.rs, normalized_path.rs, files.rs
2. ✅ `model/` — report.rs, rule.rs, tree_mode.rs, normalized_xpath.rs
3. ✅ `mutation/` — replace.rs, xpath_upsert.rs, declarative_set.rs
4. ✅ `xot/` — builder.rs, transform.rs
5. ✅ `wasm/` — mod.rs (bindings), ast.rs (serialization types)
6. ✅ `languages/info.rs` — language metadata (was language_info.rs)
7. ✅ `output/source_utils.rs` — source snippet extraction
8. ✅ Backward-compatible `pub use` re-exports in lib.rs — zero import changes needed

### Phase 4: Remaining cleanup
1. Update CLAUDE.md / architecture docs
2. Verify `task test` passes (cargo tests, web vitest, integration tests)
3. Verify WASM build still works
