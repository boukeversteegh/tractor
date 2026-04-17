# Unified `Source`: virtual paths as a first-class citizen

## Context

Issue #133. Pre-commit / pre-edit hook workflows want to lint *proposed* content before it lands on disk. Today, stdin / `-s` content has no path, so:

- `include:` globs never match
- Directory-scoped rules silently produce zero results
- Diagnostics say `<stdin>`
- `--diff-lines` can't isolate the edited hunk

Prior art: ruff, prettier, eslint all expose `--stdin-filename`. Tractor can do it more elegantly because the CLI already has a positional `files` argument — we let it name the virtual path when content is inline.

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

Each `Operation` drops `files: Vec<String>`, `inline_source: Option<String>`, and `language: Option<String>`, and gains:

```rust
pub sources: Vec<Source>,
```

## UX: no new flag — reuse the positional `files` arg

When inline content is active (stdin or `-s`), the positional `files` arg, if present, must be exactly one entry and names the virtual path.

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
 │ Operation {                     │
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
| `tractor/src/cli/{check,query,set,test}.rs` | Build each `Operation` with a single `sources: Vec<Source>` field. Remove the Phase-2-vs-Phase-3 dispatch. |
| `tractor/src/executor/{check,query,set,test}.rs` | `Operation` struct loses `files`, `inline_source`, `language`; gains `sources`. Execution body drops the inline branch. Parsing goes through `source.read()` + `parse_string_to_documents(&content, &source.language, source.path.as_str(), ...)`. |
| `tractor/src/matcher.rs:187` (`run_rules`) | Signature: `&[Source]` instead of `&[NormalizedPath]`. Per-rule glob match still on `&source.path`. Parsing switches from `parse_file_to_xot` to `source.read()` piped into `parse_string_to_xot`. Parallel iteration preserved. |
| `tractor/src/input/file_resolver.rs` | `FileResolver::resolve` returns `Vec<Source>` (all `SourceContent::Disk`) instead of `Vec<NormalizedPath>`. Or: keep `resolve` producing paths and add a thin `into_sources()` adapter — decide during implementation based on call-site count. |
| `tractor/src/input/git.rs` | `DiffHunkFilter::from_spec_with_sources(spec, cwd, sources)` — for any `is_virtual()` source, replace that path's hunks with `diff(<spec>:path, inline content)`; disk sources unchanged. |
| `tractor/src/executor/set.rs` | Write-side branches on `source.content`: `Disk` writes to file, `Inline` routes to `report.add_output()`. One `match`, replaces the entire Phase-2/Phase-3 duplication. |
| `tractor/src/mutation/replace.rs:213` | Rejection check: `if source.is_virtual() { reject }` — wired via the caller passing source context, not a string comparison. |
| `tractor/src/format/text.rs:101,173` | `file != "<stdin>"` → `file != PATHLESS_LABEL`. Narrow: only the truly path-less case. |

## `--diff-lines` + virtual source

When both are present, hunks for a virtual source come from `diff(<spec>:<source.path>, <inline content>)`, reflecting the caller's proposed edit. Other sources use today's `git diff <spec>` output. No new filter abstraction — the existing `ResultFilter` machinery is content-source-agnostic; we just feed it a better-populated `hunks` map at construction time.

## Unmatched-glob behaviour

A virtual source whose path doesn't match any rule's `include:` produces zero matches with no error. This mirrors today's file-mode behaviour — we're not inventing new semantics, just preserving what `run_rules` already does on a per-file basis.

## What the unification buys

- **Four executor bodies shrink.** The Phase-2 (inline) vs Phase-3 (files) branch disappears in `check`, `query`, `set`, `test`. One loop, one code path.
- **The sentinel is quarantined.** It appears in exactly one place — as a display label when a user pipes content without a positional path — and the formatter's `file != PATHLESS_LABEL` check remains as a local, narrow concern.
- **Glob matching, diff-lines, filtering: free.** All of it already keys off `NormalizedPath`. Unifying sources means the virtual path participates in these pipelines exactly like a real path, with no new code.
- **Factor separation made structural, not conventional.** Today "is this inline?" is answered by inspecting the operation's shape (is `inline_source` Some?). After: answered by `source.is_virtual()` — a property of the thing itself, not the container.

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
