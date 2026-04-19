# Design Exploration: Call Graph and Cross-File Name Resolution

A survey of existing libraries that could provide tractor with the
cross-file name-resolution primitives needed to make cross-file rules
(see `design-cross-file-rules.md`) *precise* rather than *heuristic*.

This document does not propose an implementation. It records findings
about prior art, identifies the best-fitting library, and sketches how
it would slot into the cross-file rules pipeline.

---

## Why this matters

`design-cross-file-rules.md` defers "import path resolution" and notes
that users can approximate cross-file checks with `contains()` /
`matches()` on raw strings. That works for coarse rules (layer
violations on clearly-named folders) but fails for anything that
depends on knowing *what a name actually refers to*:

- Aliased imports — `import { foo as bar } from "./m"` — string match
  on `bar` never finds the export `foo`.
- Re-exports — `export * from "./internal"` — the symbol's canonical
  file is not its import path.
- Namespace imports — `import * as M; M.foo()` — the call site
  mentions `M.foo`, not `foo`.
- Overloads and same-named symbols in different files.
- Method receivers — which `.save()` is being called?

All of these are forms of *name resolution*: given a use of an
identifier at a specific AST location, return the definition it
resolves to. Call graph construction is a straightforward consequence
once name resolution exists — every call site becomes an edge to its
resolved target.

Without name resolution, the cross-file rules in
`design-cross-file-rules.md` use cases 4 (cross-file fan-out counting)
and 5 (dead exports) are unreliable; use case 3 (interface/impl
matching) works only for trivial naming conventions. The other three
use cases (layer violations, file correspondence, naming/structure
consistency) can be served adequately by string matching and
filesystem metadata.

---

## Prior art surveyed

### engram (`NickCirv/engram`)

Marketed as a tree-sitter-based code indexer with "call-graph
precision." Inspection of the source (`src/miners/ast-miner.ts`)
shows the current implementation is regex-only. Tree-sitter is a
declared dependency but not imported anywhere. No call edges are ever
emitted; the graph consists of file→function, file→class, and
file→import edges. A comment in the source explicitly defers
tree-sitter to "Phase 2."

**Verdict**: Not a useful reference. Engram demonstrates how to ship a
structural-extraction MVP while deferring name resolution entirely —
it sidesteps the problem rather than solving it.

### stack-graphs (`github/stack-graphs`) ⭐

A Rust library from GitHub that implements scope graphs for
file-by-file name resolution on top of tree-sitter. Used in production
for GitHub's Code Navigation feature.

Key properties:

- **Tree-sitter native.** Consumes a tree-sitter parse tree directly;
  no separate grammar infrastructure.
- **Declarative language definitions.** Per-language name-binding
  rules are written in `.tsg` (tree-sitter graph) files, not code. A
  `.tsg` file pattern-matches on AST nodes and emits graph nodes and
  edges. This matches tractor's "users write declarative rules"
  philosophy.
- **Incremental by design.** Each file produces an independent
  *partial* stack graph. Partials are stitched at query time. Changing
  one file invalidates only that file's partial — no full re-index.
  This matches tractor's existing per-file parallel-parse model and
  watch-mode story.
- **Rust crate ecosystem on crates.io:**
  - `stack-graphs` — core graph and path-finding.
  - `tree-sitter-stack-graphs` — runs `.tsg` rules against a parse
    tree to produce a partial graph.
  - `tree-sitter-stack-graphs-typescript`, `-python`, `-java`,
    `-javascript` — prebuilt language packs.
  - SQLite-backed partial-graph storage is provided upstream.
- **License**: MIT OR Apache-2.0 — compatible with tractor.

**Verdict**: The best-fitting library surveyed. Same parser, same
philosophy (declarative, language-agnostic), incremental by design.

### SCIP (`sourcegraph/scip`)

SCIP is a **protobuf-based index format**, not an indexing engine.
The per-language indexers that produce SCIP data (`scip-typescript`,
`scip-python`, `scip-java`, `scip-go`, `scip-ruby`) wrap each
language's native compiler or type-checker — they are not tree-sitter
based and are not reusable as a library. `scip-syntax` is
tree-sitter-based but provides only syntactic/local information; it
does not resolve cross-file references.

**Verdict**: Wrong layer. SCIP is useful if tractor wanted to
*consume* externally-produced indexes, but not as a building block on
top of tree-sitter. Adopting SCIP would mean shelling out to
per-language binaries — the opposite of tractor's approach.

### Glean (`facebook/glean`)

Meta's code indexing system. Datalog-like fact schema, Haskell server,
designed for monorepo-scale deployments. Powerful but requires heavy
infrastructure.

**Verdict**: Wrong shape. Glean is a platform, not an embeddable
library.

### Others covered in `design-cross-file-rules.md`

CodeQL, Semgrep, ESLint, and ArchUnit/NetArchTest are discussed in
that document's Prior Art section as *rule systems*. They are not
reusable name-resolution libraries for a third-party tool.

---

## Why stack-graphs is the best match for tractor

| Property | Tractor needs | stack-graphs provides |
|-|-|-|
| Parser | tree-sitter | tree-sitter |
| Language | Rust | Rust crate |
| Style | declarative rules | `.tsg` DSL rules |
| Index scope | per-file + stitched | partial graphs + stitching |
| Watch mode | incremental per file | invalidation is per file |
| Storage | embeddable | SQLite module provided |
| License | permissive | MIT / Apache-2.0 |

No other surveyed library matches along all these axes.

---

## Integration shape

Stack-graphs would slot into the cross-file rules pipeline (from
`design-cross-file-rules.md`) as an *enrichment pass* between
per-file parsing and the skeleton merge:

```
files → parse → AST ──┬── extract skeleton ───┐
                      │                        ├── merge → project doc → XPath → report
                      └── stack-graphs pass ──┘
                           partial graph,
                           resolved refs
```

The resolved references become additional XML elements on each
`<file>` skeleton. A query's `<call>` or `<import>` element can carry
a `<resolved-to>` child that points at the canonical definition
location.

Hypothetical skeleton after enrichment:

```xml
<file path="src/handlers/login.ts" language="typescript">
  <import>
    <source>./user</source>
    <resolved-to>src/models/user.ts</resolved-to>
  </import>
  <call>
    <name>save</name>
    <resolved-to file="src/models/user.ts" symbol="User.save"/>
  </call>
  <call>
    <name>log</name>
    <resolved-to status="unresolved"/>
  </call>
</file>
```

Cross-file XPath rules then query `<resolved-to>` elements instead of
text-matching on raw names. The query language stays XPath 3.1; no
new DSL is introduced. This respects design principle #1 of the
cross-file rules document.

### Use cases unlocked

Mapped against the six use cases in `design-cross-file-rules.md`:

| # | Use case | Benefit from name resolution |
|---|---|---|
| 1 | Layer violations | String matching works for trivial folder structures; resolution is needed once path aliases or re-exports are involved. |
| 2 | File correspondence | Pure filesystem — no benefit. |
| 3 | Interface/implementation matching | String heuristics (`I`-prefix) break down quickly; resolution gives the real `implements` edge. |
| 4 | Cross-file fan-out counting | **Required.** Text matching on call names produces false positives for any two functions sharing a name across files. |
| 5 | Dead exports | **Required.** Aliased imports, namespace imports, and re-exports defeat text matching. |
| 6 | Naming/structure consistency | Filename + single-file AST — no benefit. |

Net: three of six documented use cases become reliable; one becomes
more robust under realistic codebases.

---

## Risks and limitations

- **Language coverage.** Prebuilt stack-graphs definitions exist for
  TypeScript, JavaScript, Python, and Java. Rust, Go, C#, Ruby, and
  others require writing new `.tsg` files. This is not trivial work —
  a correct definition must model scope, imports, modules, and
  classes for the target language.
- **`.tsg` DSL learning curve.** The DSL is a separate skill to
  acquire. Upstream language definitions are the best reference, but
  they are substantial (thousands of lines).
- **Partial results.** Some names will fail to resolve (dynamic
  imports, reflection, missing dependencies). The design must
  represent and surface "unresolved" outcomes without misleading
  users.
- **Memory and speed at scale.** Per-file partials are small, but
  stitching across thousands of files has costs. Benchmarking is
  required before committing to stack-graphs for large monorepos.
- **Coupling.** Adopting stack-graphs introduces a substantial
  external dependency. Its API is not stable in the SemVer sense.

---

## What's deferred

- **Implementation.** This document is a survey, not a plan.
- **New-language `.tsg` authoring.** Out of scope for the initial
  experiment; start with languages that have prebuilt packs.
- **Storage layer decisions.** Whether to use the upstream SQLite
  module, reuse tractor's own report persistence, or something else
  is left open.
- **Unresolved-name UX.** How to present unresolved references in
  tractor reports needs its own design pass.

---

## Open questions

1. **Prototype scope.** What's the smallest end-to-end experiment
   that would validate the integration? A candidate: run
   `tree-sitter-stack-graphs-typescript` over a ~50-file project,
   emit a `<resolved-to>`-enriched XML project document, run an
   existing cross-file XPath rule (e.g., dead-exports) against it,
   and compare to the string-matching version on the same corpus.

2. **Incremental update fit.** Does stack-graphs partial-graph
   invalidation align cleanly with tractor's existing change-driven
   pipeline, or does it require a second persistent store?

3. **Report-location semantics.** When a cross-file rule fires on a
   `<resolved-to>` element that points *across* files, which file
   should own the diagnostic — the use site, the definition site, or
   both?

4. **`.tsg` as a tractor authoring surface.** Should tractor ever
   expose `.tsg` to users, or keep it as an internal implementation
   detail with a curated set of supported languages?

5. **Fallback path.** When stack-graphs has no definition for a
   language, should tractor silently fall back to string-matching, or
   refuse to run cross-file rules that require resolution?

---

## Next steps (suggested, not committed)

1. Prototype stack-graphs against a TypeScript test fixture; emit the
   enriched project document format sketched above.
2. Port one representative cross-file rule (dead exports) to the
   enriched document and compare precision against the string-matching
   version on a real codebase.
3. Decide on the fallback-and-coverage policy before broadening to a
   second language.

---

## References

- `design-cross-file-rules.md` — the cross-file rules design this
  document supports.
- `github/stack-graphs` — the candidate library.
- Niko Matsakis & Douglas Creager, *Introducing stack graphs* (GitHub
  blog) — accessible introduction to the underlying model.
