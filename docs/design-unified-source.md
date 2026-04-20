# Unified `Source`: virtual paths as a first-class citizen

> **Status: shipped** on `claude/separate-factors-6meGf` (commits `63c6b97`, `ff8cb29`, `8d2b376`). Closes #133. The UX decision settled on: **reuse the positional `files` argument (single value) to name the virtual path when inline content is active**. No new flag like `--stdin-filename` was introduced.

## Context

Issue #133. Pre-commit / pre-edit hook workflows want to lint *proposed* content before it lands on disk. Today, stdin / `-s` content has no path, so:

- `include:` globs never match
- Directory-scoped rules silently produce zero results
- Diagnostics say `<stdin>`
- `--diff-lines` can't isolate the edited hunk

Prior art: ruff, prettier, eslint all expose `--stdin-filename`. Tractor does it more elegantly because the CLI already has a positional `files` argument — we reuse it to name the virtual path when content is inline, avoiding a redundant flag surface.

The root cause is that the operation layer carries two *structurally different* input shapes: `files: Vec<String>` (disk) and `inline_source: Option<String>` (stdin/`-s`), and every executor branches on them. The magic sentinel `"<stdin>"` is a symptom — it's the string we need because `inline_source` has no path field.

**The fix is to unify.** A source is a source: a path, a language, and some way to get its bytes. Whether those bytes live on disk or in memory is a property of the source, not a structural split at the operation level. Once unified, "virtual path" stops being a special case — it's just a `Source` whose content is held in memory instead of on disk.

## The one type

```rust
// tractor/src/input/source.rs  (new)
pub struct Source {
    pub path: NormalizedPath,
    pub language: String,
    pub content: SourceContent,
}

pub enum SourceContent {
    /// Read from disk lazily at parse time (existing file flow).
    Disk,
    /// Content is already in memory (from stdin or -s/--string).
    /// Arc so it can be cheaply shared across parallel workers / diff builders.
    Inline(Arc<String>),
}

impl Source {
    pub fn is_virtual(&self) -> bool {
        matches!(self.content, SourceContent::Inline(_))
    }

    /// Returns the bytes to parse — borrows from memory for inline,
    /// reads from disk for Disk. Called inside the parallel worker,
    /// preserving today's laziness.
    pub fn read(&self) -> io::Result<Cow<'_, str>> {
        match &self.content {
            SourceContent::Disk => fs::read_to_string(self.path.as_path()).map(Cow::Owned),
            SourceContent::Inline(s) => Ok(Cow::Borrowed(s.as_str())),
        }
    }
}

/// Sentinel for a path-less inline source (nothing piped with a positional path).
/// Only exists to keep existing display/formatting behaviour for that narrow case.
pub const PATHLESS_LABEL: &str = "<string>";
```

Each `OperationPlan` drops `files: Vec<String>`, `inline_source: Option<String>`, `language: Option<String>`, and the per-op diff/exclude fields, and gains:

```rust
pub sources: Vec<Source>,
pub filters: Vec<Box<dyn ResultFilter>>,
```

Both fields are **pre-resolved before the `OperationPlan` is constructed**. The executor is a pure consumer — it never expands globs, never calls `FileResolver`, never touches raw pattern strings for file resolution. (Per-rule `include:`/`exclude:` globs still live on `Rule` and are applied at match time inside `run_rules`; that is a different concern — "which rules apply to this source" — from file resolution.)

## UX: no new flag — reuse the positional `files` arg

**Decision (shipped):** when inline content is active (stdin or `-s`), the positional `files` arg, if present, must be exactly one entry and names the virtual path. No `--stdin-filename`-style flag is added — the CLI already accepts positional paths, and giving one while piping content is unambiguously "label this content with this path".

```bash
# Path-less (today's behaviour):
cat foo.cs | tractor check -l csharp -x "..."

# Path-tagged (new):
cat proposed.cs | tractor check -l csharp -x "..." src/Foo.cs
tractor check -s "..." -l csharp src/Foo.cs
```

Validation in `resolve_input` (`tractor/src/input/mod.rs`):

| Inline active? | `files.len()` | Produces |
|---|---|---|
| yes | 0 | `vec![Source::inline_pathless(content, lang)]` |
| yes | 1 | `vec![Source::inline_at(path, content, lang)]` |
| yes | >1 | Error: "inline source accepts at most one path" |
| no  | 0 | Error (unchanged: no input) |
| no  | ≥1 | Glob-expanded `Vec<Source>` with `SourceContent::Disk` and pre-detected languages |

Language is resolved at the input boundary: `-l` overrides, otherwise `detect_language(path)` per source. Downstream never re-detects.

## Data flow

```
                           INPUT BOUNDARY
 ┌───────────────────┐
 │ argv              │
 │  -l / -s / stdin  │
 │  + [files...]     │
 └─────────┬─────────┘
           │
           ▼
 ┌─────────────────────────────────┐
 │ resolve_input()                 │
 │                                 │
 │  inline?  → one Source::Inline  │
 │  files    → glob-expand, one    │
 │             Source::Disk per    │
 │             file                │
 │                                 │
 │  normalize paths (NormalizedPath)
 │  resolve language (once)        │
 └─────────────────┬───────────────┘
                   │
                   ▼
 ┌─────────────────────────────────┐
 │ OperationPlan {                 │
 │   sources: Vec<Source>,         │◀── ONE unified list
 │   rules / xpath / ...           │
 │ }                               │
 └─────────────────┬───────────────┘
                   │
        ┌──────────┴─────────┐
        ▼                    ▼
 ┌──────────────┐    ┌──────────────────────────┐
 │ run_rules    │    │ DiffHunkFilter           │
 │              │    │                          │
 │ for source   │    │ for each virtual source, │
 │  in sources: │    │  hunks = diff(<spec>:    │
 │   glob check │    │    path, inline content) │
 │   parse()    │    │ for disk sources:        │
 │   xpath      │    │  today's git diff logic  │
 └──────┬───────┘    └──────────────────────────┘
        │               filter attached via existing
        ▼               ResultFilter plumbing
 ┌──────────────────────────────┐
 │ Match.file = source.path     │ ← always a path.
 │                              │   No origin tracking.
 └──────────┬───────────────────┘
            ▼
 ┌──────────────────────────────┐
 │ format / reporter            │ ← path-agnostic.
 │ (suppresses prefix only when │   Only checks sentinel for
 │  file == PATHLESS_LABEL)     │   truly path-less case.
 └──────────────────────────────┘

 SET mutation write-side (the one place SourceContent still matters):
 ┌──────────────────────────────┐
 │ match source.content {       │
 │   Disk      => fs::write()   │
 │   Inline(_) => report output │
 │ }                            │
 └──────────────────────────────┘
```

## Concrete changes

| File | Change |
|---|---|
| `tractor/src/input/source.rs` | **NEW**. `Source`, `SourceContent`, `PATHLESS_LABEL`, constructors `Source::disk`, `Source::inline_at`, `Source::inline_pathless`, `Source::read()`. |
| `tractor/src/input/mod.rs` | `InputMode` → `Vec<Source>` (drops the `Files` vs `InlineSource` split). `resolve_input` validates inline+files≤1, glob-expands disk files, pre-detects language, returns a unified `Vec<Source>`. |
| `tractor/src/cli/{check,query,set,test}.rs` | Build each `OperationPlan` with a single `sources: Vec<Source>` field. Remove the Phase-2-vs-Phase-3 dispatch. |
| `tractor/src/executor/{check,query,set,test}.rs` | `OperationPlan` struct loses `files`, `inline_source`, `language`; gains `sources`. Execution body drops the inline branch. Parsing goes through `source.read()` + `parse_string_to_documents(&content, &source.language, source.path.as_str(), ...)`. |
| `tractor/src/matcher.rs:187` (`run_rules`) | Signature: `&[Source]` instead of `&[NormalizedPath]`. Per-rule glob match still on `&source.path`. Parsing switches from `parse_file_to_xot` to `source.read()` piped into `parse_string_to_xot`. Parallel iteration preserved. |
| `tractor/src/input/file_resolver.rs` | `FileResolver::resolve` returns `(Vec<Source>, Vec<Box<dyn ResultFilter>>)` directly — no adapter. Takes a `SourceRequest` (single `inline_source: Option<&Source>` field, no dual `inline`/`has_inline`). Constructor takes a new `ResolverOptions` struct (split off from `ExecuteOptions`). |
| `tractor/src/input/git.rs` | `DiffHunkFilter::from_spec_with_sources(spec, cwd, sources)` — for any `is_virtual()` source, replace that path's hunks with `diff(<spec>:path, inline content)`; disk sources unchanged. |
| `tractor/src/executor/set.rs` | Write-side branches on `source.content`: `Disk` writes to file, `Inline` routes to `report.add_output()`. One `match`, replaces the entire Phase-2/Phase-3 duplication. |
| `tractor/src/mutation/replace.rs:216` | Rejection check uses the single sentinel: `if m.file == PATHLESS_LABEL`. `PATHLESS_LABEL` moved to the library crate (`tractor/src/model/report.rs`) so library and binary share one constant — replaces the prior two-string defensive check. |
| `tractor/src/format/text.rs:101,173` | `file != "<stdin>"` → `file != tractor::PATHLESS_LABEL`. Narrow: only the truly path-less case. Imports from library crate, not from the `input` sibling module. |

## `--diff-lines` + virtual source

When both are present, hunks for a virtual source come from `diff(<spec>:<source.path>, <inline content>)`, reflecting the caller's proposed edit. Other sources use today's `git diff <spec>` output. No new filter abstraction — the existing `ResultFilter` machinery is content-source-agnostic; we just feed it a better-populated `hunks` map at construction time.

## Unmatched-glob behaviour

A virtual source whose path doesn't match any rule's `include:` produces zero matches with no error. This mirrors today's file-mode behaviour — we're not inventing new semantics, just preserving what `run_rules` already does on a per-file basis.

## What the unification buys

- **Four executor bodies shrink.** The Phase-2 (inline) vs Phase-3 (files) branch disappears in `check`, `query`, `set`, `test`. One loop, one code path.
- **The sentinel is quarantined.** It appears in exactly one place — as a display label when a user pipes content without a positional path — and the formatter's `file != PATHLESS_LABEL` check remains as a local, narrow concern. `PATHLESS_LABEL` itself lives in the library crate and is the single source of truth.
- **Glob matching, diff-lines, filtering: free.** All of it already keys off `NormalizedPath`. Unifying sources means the virtual path participates in these pipelines exactly like a real path, with no new code.
- **Factor separation made structural, not conventional.** Today "is this inline?" is answered by inspecting the operation's shape (is `inline_source` Some?). After: answered by `source.is_virtual()` — a property of the thing itself, not the container.
- **File resolution lifted out of operations.** As a direct consequence of unifying to `Vec<Source>`, `FileResolver` now runs **before** the `OperationPlan` is constructed (in `cli/*.rs` and `cli/config.rs`). `OperationPlan`s are pure consumers of pre-resolved `sources`/`filters`. Executors no longer import `FileResolver`. Config loading goes through an intermediate `ConfigOperation { inputs: OperationInputs, op: … }` so each op-specific parser declares its pattern shape (an `OperationInputs` bag of `files`/`exclude`/`diff_*`/`language`/`inline_source`) and a single runner in `input::plan` drives `resolver.resolve(...)` uniformly, then calls `config_op.into_parts()` to split off the `Operation` and hands the resolved sources/filters to `Operation::into_plan(...)`. The resolver knows nothing about operation types; operations contain no resolution logic.

## Verification

Tests in `tractor/tests/cli.rs` using the existing `cli_case!` + `.stdin()` pattern:

1. **Include glob matches via virtual path**: pipe C# with a violation; config has `include: ["src/**/*.cs"]`; `... src/Foo.cs` → violation reported. Without path → zero (regression guard).
2. **Non-matching virtual path**: same config, `other/Foo.cs` → zero, no error.
3. **Display uses the virtual path**: GCC-format output shows `src/Foo.cs:L:C:`, never `<string>`.
4. **More than one path rejected**: `cat x | tractor check -l csharp a.cs b.cs` → clear error.
5. **`-s` + path**: `tractor check -s "..." -l csharp src/Foo.cs` behaves identically to the stdin case.
6. **`--diff-lines` + virtual path**: git repo fixture with a committed baseline; piping modified content with `--diff-lines HEAD src/Foo.cs` reports only violations inside modified hunks. Move the violating line outside the hunk → zero.
7. **Set + virtual path**: `tractor set -s "..." -l yaml config.yaml` writes mutated content to stdout with label `config.yaml`; working tree untouched.
8. **Query + virtual path**: smoke — virtual path in output where a real path would appear.
9. **Parallel file flow regression**: `cargo test -p tractor` green, confirming the `run_rules` signature change didn't disturb the disk flow.

Manual sanity:
```bash
cargo build
cargo test -p tractor cli
cat proposed.cs | tractor check --config tractor.yml -l csharp src/Foo.cs
```

## Deviations from the original plan

The unification shipped, but the original plan was scoped narrowly around the `Source` type. Implementation surfaced a larger structural opportunity, and the final design deviates from this document's earlier sketches in several ways. Captured here so readers can reconcile the doc with current code.

### Type shapes

- **`Operation` is now a pre-resolution type; `OperationPlan` is the executor-ready form.** The original plan showed a single `Operation` with `sources: Vec<Source>` gained. Shipped: `Operation` (pre-resolution, aligns with user-facing YAML `operations:` key) is constructed from CLI args or config parsing and holds everything *except* sources and filters. `plan_single` / `plan_multi` resolve it into `OperationPlan`, which the executor consumes. The pre-resolution type never carries empty-placeholder sources/filters.
- **`filters: Filters` struct, no trait object.** The original plan showed `Vec<Box<dyn ResultFilter>>`. Shipped: `Filters { diff_hunks: Vec<DiffHunkFilter> }` — a concrete envelope with the single real filter impl inlined. No `dyn` dispatch, no boxing, and `#[derive(Debug, Clone)]` works on every `OperationPlan`. The envelope leaves room for named additional filter fields if they ever emerge.
- **Two draft-like types per op variant became one: the `Operation`.** There is no `*Draft` type in shipped code. `CheckOperation`, `SetOperation`, `QueryOperation`, `TestOperation`, `UpdateOperation` are each "the pre-resolved form"; `*::into_plan(sources, filters [, base_dir])` produces the corresponding `*OperationPlan`.
- **`ExecutionPlan` is the resolved-operations envelope.** The container returned by `plan_single`/`plan_multi` was briefly called `InputPlan`; final name is `ExecutionPlan` to pair cleanly with the existing `OutputPlan` on the rendering axis.

### Placement decisions

- **`PATHLESS_LABEL` lives in the library crate** (`tractor/src/model/report.rs`), not the binary. This lets `mutation/replace.rs` import the constant directly instead of inlining the string.
- **`ReportMatch::is_pathless()` and `is_pathless_file(&str)` encapsulate the sentinel check.** Consumers ask the match / file, not the constant. Direct `== PATHLESS_LABEL` comparisons live only inside the library's own module.
- **Per-rule glob patterns compile at construction, not at match time.** `compile_ruleset(...)` builds a `Vec<CompiledRule>` where each entry bundles rule metadata with a pre-compiled `GlobMatcher` and pre-resolved effective language (ruleset default + rule override collapsed). `run_rules` stops compiling on every call and just iterates compiled rules.
- **`Source::disposition() -> SourceDisposition { Disk, InlineWithPath, InlinePathless }`** is the three-variant domain ADT. `is_virtual()` / `is_pathless()` remain as thin derivations. The set executor matches on `disposition` to pick write-mode / filter-slice / disk-write-gate in one local binding instead of three scattered boolean checks.
- **`FileResolver` never enters the executor.** Lifted entirely upstream into a single `input::plan` module. Operations are pure consumers of `sources: Vec<Source>` + `filters: Filters`. `ResolverOptions { ... }`, `FileResolver::new`, `SourceRequest { ... }`, and `resolver.resolve(...)` each appear in exactly one production call site.
- **`ExecuteOptions` is gone.** Collapsed into `ExecCtx<'a>` — a borrowed view over `RunContext`, the single owner of `verbose` / `base_dir`. No caller can populate the two fields in two structs with different values.

### Write-side predicate

The original plan proposed `if source.is_virtual() { reject }` in `mutation/replace.rs`. The shipped check is `if m.file == PATHLESS_LABEL` (or equivalently `m.is_pathless()`). **Different semantics, deliberately.** A virtual-with-path source *could* in principle have its mutation captured (the output path is known); only a genuinely pathless source has nowhere to route the result. The check gates on "does the match have a writable target?" — and for set operations, virtual-with-path is routed through Capture mode instead of being rejected, so the mutated content still reaches the caller.

### Parse entry points

Not in the original plan: the three `parse_to_documents` / `parse_string_to_documents` / `parse_string_to_documents_with_options` functions are unified behind `parse(ParseInput, ParseOptions) -> Result<...>`. `Source::parse` constructs a `ParseInput` from `self.content` + `self.path`. The old three functions remain as thin shims for the 20+ call sites that pre-existed the refactor.

### Scoping contract: all intersectional

Explicitly stated after-the-fact, but a first-class invariant of the shipped design: **every file/line scoping layer composes intersectionally.** Global CLI `--diff-lines` / `--diff-files`, config-root `diff-lines` / `diff-files`, per-operation `diff-lines` / `diff-files`, root `files` / `exclude`, per-op `files` / `exclude`, CLI positional files, ruleset `include` / `exclude`, rule `include` / `exclude` — all AND-compose when narrowing; excludes union-reject. No layer overrides another. The `Filters.diff_hunks: Vec<DiffHunkFilter>` shape, sequential `intersect_changed` on `diff_files`, and `OperationInputs.diff_files`/`diff_lines: Vec<String>` (concatenated from root + per-op) all follow from this contract. Regression tests guard the intersection at each boundary.

`--diff-lines` with pathless inline input is rejected at plan time with a fatal diagnostic — there's no git baseline to compute hunks against. This makes filter application uniform across all executors (set no longer needs a per-source filter bypass).

### Config ceremony

- **`ConfigOperation { Check { inputs, op: CheckOperation }, ... }`** — carries the `OperationInputs` bag alongside the pre-resolved `Operation`. `into_parts()` splits into `(OperationInputs, Operation)` for the planner.
- **Single normalization seam.** Every CLI subcommand and the config-mode runner go through `plan_single` or `plan_multi`. Policy about valid inputs, intersection semantics, resolver construction, and `Operation → OperationPlan` materialisation lives in one module.
