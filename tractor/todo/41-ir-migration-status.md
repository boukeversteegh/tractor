# IR migration status (snapshot 2026-05-07)

## Done — IR-only end-to-end

All 9 programming-language flavours run through `crate::ir::*`
end-to-end via `parse_with_ir_pipeline`. Their imperative
`{rules,transformations,transform}.rs` files are deleted; only
`{input,output,post_transform}.rs` remain (input is the kind-
catalogue enum; output is the TractorNode vocabulary;
post_transform is the IR-pipeline polish layer for list-
distribution / chain inversion / slot-wrapping).

| Language        | LOC saved | Status |
|-----------------|-----------|--------|
| C#              | ~1500     | ✅ end-to-end |
| Python          | ~1400     | ✅ end-to-end |
| Java            | ~900      | ✅ end-to-end |
| TypeScript / JS / TSX / JSX | 1255 | ✅ end-to-end (via TS lower fn) |
| Rust            | ~1100     | ✅ end-to-end |
| Go              | ~1068     | ✅ end-to-end |
| Ruby            | ~700      | ✅ end-to-end |
| PHP             | ~872      | ✅ end-to-end |

Snapshot regen across 57 fixtures committed once IR shape was
stable. 274 transform tests + 18 catalogue + lib tests + 149
integration snapshots all green.

### `render_to_xot` per-arm extraction ✅

`src/ir/to_xot.rs` (renamed from `render.rs` to clarify it's the
Xot renderer) now extracts all 50 non-trivial match arms into
`#[inline(never)]` per-arm helpers. Dispatcher's match is a thin
jump table. Workspace tests pass at default opt-level=0.

A 16 MiB `rayon::ThreadPoolBuilder::stack_size` hack remains in
`cli/context.rs`, but its load-bearing reason is now the **xee
XPath evaluator's** recursive AST walks during query evaluation,
NOT `render_to_xot` (which has been split). Comment in context.rs
reflects this.

### Module reorganization ✅

- `src/ir/render.rs` → `src/ir/to_xot.rs`  (renders IR → Xot tree)
- `src/ir/json.rs` → `src/ir/to_json.rs`   (renders IR → serde_json::Value)
- `src/ir/source/` (was already there)     (renders IR → original source bytes)

Public function names (`render_to_xot`, `ir_to_json`) unchanged.

### Build hygiene ✅

Source-side build warnings reduced from 6 to 0 (only Cargo's
cosmetic PDB-filename collision warning remains, which is not
source-actionable).

## Pending — architectural

### Drop `list=`/`field=` from C# (and eventually all IR-pipelined
languages) XML output

**Blocked on:** `ir_to_json` not wired into the production JSON
output path.

The IR has typed children (`Vec<Comment>`, `Vec<Class>`, etc.) so
`list=` attributes are redundant for JSON projection. But the
production JSON path still flows through `xml_to_json`, which
reads `list=` to decide singleton-vs-array. Wiring `ir_to_json`
into that path requires:

1. Plumb `Option<Ir>` through `XotParseResult` → `XeeParseResult`
   → `Match::tree`. Today only the Xot tree carries semantic info.
2. In the JSON output renderer, when the parse came from the IR
   pipeline, dispatch to `ir_to_json` instead of `xml_to_json`.
3. Confirm parity vs current snapshots — tests/ir_csharp_json_parity.rs
   has a known divergence around singleton-vs-plural decisions
   (xml_to_json forces plural arrays via `list="X"` even for
   singletons; `ir_to_json` keeps singletons singular). Needs a
   per-element-name "always plural" rule table OR encoding plurality
   in the IR's typed structure.
4. Once `ir_to_json` is the production JSON path, drop
   `tag_multi_role_children` + `distribute_member_list_attrs`
   from C# `post_transform`.

Estimate: 4–8 hours.

### Data-language IR migration (JSON/YAML/TOML/INI/Markdown)

**Architectural blocker:** the IR's text-recovery + round-trip
invariants don't match the data-branch shape.

The data branch produces XML where element names are user-keyed
(e.g. `[database]\nhost = localhost` → `<database><host>localhost</host></database>`).
The bracket markup and `=` are dropped, not preserved as gap text.
The IR's invariants explicitly forbid that:
- `string(IR_root) == source`
- `to_source(ir, source) == source`

Two approaches:

A. **New data IR variants with relaxed invariants.** Define
   `Ir::DataElement { name: String, children: Vec<Ir> }` and
   `Ir::DataScalar { name: String, value_text: String }`,
   document that they break text-recovery, restrict them to the
   data branch.

B. **Keep data languages on the imperative path long-term.** A
   deliberate split: structural (programming) languages on IR,
   data languages on imperative. The IR architecture is for
   round-trip-preserving structure; data branch isn't the same
   problem.

Recommendation: do (B) unless there's a concrete need for
data-language IR mutation surface.

JSON's *syntax* branch (which preserves brackets etc.) could go
through IR using existing variants (Dictionary/List/Pair) with
parameterized `element_name` (currently the variants render with
fixed names: `<dict>`, `<list>`, `<pair>`, but JSON wants
`<object>`, `<array>`, `<property>`). Achievable but moderate
effort (~2–4 hours per language).

## What this PR delivers

- **8 programming language flavours** moved off the imperative
  pipeline onto a single, typed IR pipeline (~9 KLOC of imperative
  code deleted).
- **TSX / JSX migration** unifies all 4 TS/JS variants under one
  `lower_typescript_root` (the imperative pipeline already shared
  the same transform; the IR continues that unification).
- **`render_to_xot` decomposition** — wide-match → per-arm helpers
  fix the dev-build stack-overflow that was forcing
  `[profile.dev/test] opt-level = 1` and the 16 MiB rayon worker
  stack hacks. opt-level=1 is removed; rayon stack remains for
  xee's recursion.
- **Module reorg** clarifies that `ir/` has 3 render targets
  (`to_xot`, `to_json`, `to_source/`).
- **All-language kind catalogues** restored (`csharp/java/python`
  had lost theirs; regenerated via `task gen:kinds`). Drift detection
  via `task verify:gen-kinds` still works.
- **Build clean**: 0 source warnings (down from 6).
- **CI green** on Linux + Windows.
- **149 snapshot fixtures** regenerated to match IR shape.

850 commits ahead of `main`. +96K / -21K LOC net.
