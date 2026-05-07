# Transform pipeline — alternative-design exploration

Status: **exploration / experiment proposal**, not a decided plan.
Scope: rethink the entire transform pipeline with desired output shape held constant.
Audience: future-me (re-loading context) + the user (evaluating directions).

> **Direction picked by the user (mid-exploration):** abandon in-place Xot
> mutation entirely. Introduce a typed parallel **IR** of the target shape;
> render to XML afterwards. Possibly allow a `Bag` variant for pass-through
> nodes; possibly drop it if it makes the design cleaner. This direction
> supersedes Option C (§7.3) and reframes most of what follows. See §11
> below for the concrete proposal.

---

## 1. What we are doing, mathematically

We compute a function

    f_L : T(Σ_L)  →  T'(Σ', A')

per language `L`, where:

- `T(Σ_L)` is the labelled ordered tree tree-sitter emits — labels drawn from a
  per-language alphabet of grammar kinds Σ_L (≈ 200–400 kinds per language).
- `T'(Σ', A')` is the semantic tree we emit — labels drawn from a unified
  alphabet Σ' of ~100 names + a small attribute schema A' (markers,
  `list="X"`, `field="…"`).

The interesting structural question is **what class of transducer is `f_L`**:

- It is *not* a top-down finite-state tree transducer: chain inversion turns
  right-deep `member(member(member(a, b), c), d)` into a left-deep
  `object[access] / receiver=a / member=b / member=c / member=d`. A top-down
  transducer cannot rebalance.
- It is *not* a homomorphism: some output nodes have no input counterpart
  (slot wrappers, expression hosts, marker children).
- It *is* expressible as a **macro tree transducer with parameters** (Engelfriet
  & Vogler, 1985), or equivalently an **attribute grammar** with synthesised
  + inherited attributes plus a finite number of ordered passes.

We are, in other words, doing the well-studied problem of *concrete-syntax to
abstract-syntax-with-normalisation*. There is a pile of formal machinery for
this — most of which we are not using.

## 2. Inventory: where does the complexity actually live?

Hard numbers from the current tree (May 2026):

| Layer                                | LOC    | Notes                                        |
| ------------------------------------ | ------ | -------------------------------------------- |
| Total transform + languages code     | 26 679 | ≈ 76 % of the crate                          |
| Shared `transform/` infrastructure   |  6 515 | builder, chain_inversion, conditionals, etc. |
| Per-language `transformations.rs`    |  6 631 | imperative `Custom` handlers                 |
| Per-language `rules.rs`              |  3 425 | declarative dispatch tables                  |
| `Custom(...)` invocations in rules   |    285 | across 9 languages                           |
| Distinct Custom-handler functions    |    176 | many shared across kinds                     |
| Post-walk passes (cross-cutting)     |   ~94  | counted across languages × infra             |
| Chain-inversion adapters             |    17 | per-language pre-pass                        |

Two facts pop:

- **Custom handlers are 38 % bigger than the rule table they escape from.** The
  rules-table approach is technically the primary dispatch mechanism, but we
  spend more code on the escape hatch than on the rules themselves.
- **Each construct's logic is spread across 4–6 loci.** Take PHP `foreach`:
  `PhpKind::ForeachStatement` (input.rs) → `Custom(foreach_statement)`
  (rules.rs) → `pub fn foreach_statement` (transformations.rs) → field
  wrappings registered in mod.rs → expression-host post-walk (transform/mod.rs)
  → shape contract entry (transform/shape_contracts.rs) → cross-language test
  (tests/transform/loops.rs). Six places. To add the same construct in
  another language you do the same six-place edit.

That spread is the **dominant source of accidental complexity** — bigger than
"too many passes," bigger than "rules not expressive enough," bigger than any
single quirk. It is what makes whack-a-mole so easy and refactors so scary.

## 3. What is *essential* complexity?

These cannot be wished away by any redesign that keeps the output shape:

1. **Cross-language alphabet diff.** Each tree-sitter grammar slices the world
   differently. Some mapping work is per-language and irreducible.
2. **Non-local rewrites.** Chain inversion, expression-host wrapping,
   member/property collision avoidance, condition-arm flattening are all
   *necessarily* multi-node operations. They cannot be expressed as
   independent leaf renames.
3. **Per-language quirks tree-sitter grammar makes you eat.** `field=`
   metadata, optional vs required children, anonymous tokens vs named
   children, error nodes. Some normalisation is required *before* anything
   else can read the tree uniformly.
4. **Order matters.** Some passes assume previous passes have run. There is no
   way to make all rewrites simultaneously commutative without changing what
   we emit.

Everything else is potentially accidental.

## 4. What is *accidental* complexity?

In rough order of cost:

1. **Spread per construct.** As inventoried above. To learn how `foreach`
   works you read 6 files. To change it you edit 6 files. There is no single
   point of truth.

2. **Custom-handler proliferation as escape valve for rules-too-thin.** 176
   handlers exist because `Rule` only knows `Rename`, `RenameWithMarker`,
   `Flatten`, `Custom`. Anything mildly conditional ("if `else` is an
   `if_statement`, do A else B") falls through to `Custom`, where it loses
   the structural-test-friendliness of declarative rules.

3. **Pre-passes that *undo* tree-sitter quirks.** `csharp_normalize_conditional_access`,
   `php_wrap_member_call_slots`, `rust_normalize_field_expression`,
   `typescript_unwrap_callee` — every one of these is "tree-sitter handed me a
   shape I did not want; I rewrite it before the real transform starts."
   Conceptually these are *parser bug-fixes for tree-sitter*. They are not
   transform logic.

4. **Field-wrap is a *separate* sub-system** with its own table, scope rules,
   and pitfalls (iter 347's regression). It interacts with rules but is not
   *part of* the rule.

5. **Post-walk passes are imperative tree mutations, not first-class units.**
   They have no name, no declared dependencies, no declared invariant; you
   just call them in a particular order from a particular function. When one
   breaks, you cannot ask the system "which pass changed `<expression>`
   wrapping?" — you have to read the code.

6. **Shape contracts are post-hoc ratchets.** They capture shapes as
   regression catchers, not as primary specifications. The spec is what we
   wrote down; the code emits whatever it emits; the contract notices when
   the two diverge. A redesign could *invert* this: contracts drive code,
   code is generated or constrained by contracts.

7. **Cross-language patterns are not abstracted.** PHP foreach, Java foreach,
   Ruby for, Python for, Go for-range, Rust for-in, TS for-of all want the
   same shape. We re-derive that shape per language. There is no `Foreach`
   abstraction we instantiate per language with input patterns.

8. **No introspection or replay.** Cannot ask "which rule fired here?",
   cannot dump tree state between passes from the CLI, cannot replay a
   single rule against a fixture in isolation.

9. **17 chain-inversion adapters per language** doing essentially the same
   structural-pattern-matching against differently-named tree-sitter kinds.
   This is the canonical "should be a config, is currently 17 functions."

10. **Multiple passes over the tree** for cross-cutting work that *could* fuse.
    Not the biggest cost (perf isn't the bottleneck) but each pass is its
    own opportunity for ordering bugs.

## 5. Survey: how do other systems do this?

### 5.1 Stratego / Spoofax (ELEMRT term-rewriting)

Each rule is a pattern → replacement; strategies (`topdown`, `bottomup`,
`repeat`, `try`, `alltd`, `<+`) compose them. Rebalancing chain trees is one
rule-fixpoint:

    chain-flatten = repeat(\ Member(Member(o, p1), p2) -> ObjectAccess(o, [p1, p2]) \)

**What it gets right:** rules are local, composable, and the strategy combinators
turn pass ordering into first-class code instead of a comment. Cross-cutting
rewrites are not different from local ones.

**What it gets wrong (for us):** it's a DSL needing an interpreter or
compiler; performance is poor; debugging is hard ("which rule fired at this
node and why?"). Adopting straight Stratego semantics in Rust would be a big
build.

### 5.2 Rascal / SDF3

Same family as Stratego but with built-in concrete-syntax patterns:

    visit (tree) {
        case (Expr) `<Expr e1>.<Id name>`  =>  member(e1, name)
    }

Lets you write patterns *in the surface syntax of the source language* rather
than as AST constructors. Beautiful for compiler authors, but our 9 languages
would each need a Rascal grammar.

### 5.3 MLIR (LLVM project)

The closest thing to our problem in production use:

- Each *dialect* defines its own operations (kinds) and verifiers.
- *Lowering passes* convert one dialect into another.
- The pass manager schedules passes; verifiers run between passes;
  every pass is a named, addressable, replayable unit.
- A dialect declares its operations *with constraints*, not just rules. The
  framework auto-generates verifiers and parsers/printers from declarations.

We are doing approximately MLIR-without-the-infrastructure: dialects =
languages, lowering = transform, target = semantic-tree dialect. The
infrastructure is what we lack. Every Custom handler is the equivalent of
writing a one-off MLIR pass that has no name and no contract.

### 5.4 Babelfish (`bblfsh`)

The defunct sourced.tech project that *literally* tried to do what we are
doing: tree-sitter (well, ANTLR + libuast) parsers per language → unified
AST queryable as XPath. Their architecture:

- Per-language *annotators* in a small DSL (`Bblfsh DSL`, `UAST roles`).
- Each annotator is a query that fires on patterns and adds *roles* to nodes.
- Shape was driven by a published UAST schema with versioning.

**Lesson from Babelfish:** they ran into the same wall — shape divergence
between languages was the hardest part, and ad-hoc per-language annotators
did not compose. The project was archived in 2020.

We have implicitly rebuilt Babelfish. Worth knowing what they tried and
where they hit limits.

### 5.5 jscodeshift / Comby / Sourcegraph batch-changes

Per-language codemod tools. They operate on the *native* AST. They make no
attempt to unify across languages. They are easier to use because they
sidestep our hardest problem.

### 5.6 Attribute grammars (Lex/Yacc through tree-sitter-graph)

Compute synthesised + inherited attributes during one tree walk, then build
output from attributes. Single pass; context-dependent; well-studied.

**For us:** the attribute approach probably *can* be made to work, but
attributes proliferate fast (chain-membership, host-position, marker-list,
list-tag, …) and Rust lifetimes make the attribute-graph unpleasant.

### 5.7 General lessons

- **First-class passes** with names, contracts, and replay — universal in
  serious compiler infra. We don't have it.
- **Pattern languages** beat imperative escape-hatches for local rewrites —
  consistent across Stratego, Rascal, Spoofax, MLIR. We mostly use
  imperative.
- **Shape-as-contract** (verifiers between passes) — MLIR. We have it weakly,
  as a ratchet.
- **Per-construct units, not per-language units** — appears in all systems
  that scale to many languages. We are organised the other way.

## 6. Where the rule language is unexpressive

Custom handlers exist because `Rule` cannot say:

- *"Match this kind only when child `field=else` is itself an `if_statement`"*
  (conditional dispatch).
- *"Match this kind and lift the `cond` field as a sibling marker named X"*
  (attribute lift).
- *"Match this kind and wrap children with role R inside `<role>` host"*
  (per-kind field-wrap, currently global).
- *"Match this kind and project `list="X"` onto its children"* (list-tag rule).
- *"Match this kind only when its parent is kind P"* (parent-context dispatch).
- *"Match this kind but emit different shapes depending on grandchild count"*
  (cardinality-based shape).

Each of these patterns appears 5–30 times across `transformations.rs`. They
are not exotic — they're the bulk of our Custom code. **A richer rule
vocabulary alone could eliminate ~60–70 % of the imperative handlers** with
no paradigm change.

## 7. Three candidate redesigns

I see three architectures worth considering, ordered by ambition:

### 7.1 Option A — *Reorganise by construct, expand the rule language* (low risk)

No paradigm change. Two organisational moves:

1. **File-per-construct, not file-per-language.** Replace
   `tractor/src/languages/<lang>/transformations.rs` with
   `tractor/src/semantic/<construct>.rs`. Each construct file contains:
   - The cross-language target shape (executable spec, not just a comment).
   - Per-language input recognisers (small functions matching tree-sitter
     kinds + structure).
   - The construct's transformation as one match expression over those
     recognisers.
   - The cross-language test, co-located.

   Result: PHP foreach, Java foreach, Ruby for, Python for, Go for-range,
   Rust for-in, TS for-of are *one file* (`semantic/foreach.rs`). The
   shape is visible at the top. Adding a new language is a new arm in one
   match. Whack-a-mole regressions become local.

2. **Expand `Rule` so 60–70 % of Custom handlers become declarative.** New
   variants the survey above motivates:

       Rule::ConditionalRename { when: Predicate, then: N, else: N }
       Rule::WrapField { kind_field: (Kind, Field), wrap: N }
       Rule::ProjectListTag { kind: Kind, tag: &'static str }
       Rule::ContextRename { parent: Kind, child: Kind, then: N }
       Rule::CardinalityShape { kind: Kind, single: ShapeFn, multi: ShapeFn }

   Predicates are small typed expressions (`HasChildKind`, `ChildField`,
   `ParentKind`, `ChildCount`). The dispatcher reads them. Custom remains as
   the genuine escape hatch for the truly weird (≈ 30–40 handlers, not 176).

3. **First-class passes.** Wrap each post-walk in a named struct
   `pub struct Pass { name, deps, run }` and run via a pass manager. CLI
   gains `--print-after-pass <name>` for debugging.

**Cost:** large refactor, no novel CS, no new DSL. ~2–4 weeks of careful
work.

**Benefit:** kills the dominant accidental cost (item 1 in §4) without any
risk to output shape. Halves the Custom-handler count. Makes debugging
tractable.

This is a "boring" win. It is also probably the *highest expected-value*
move on the table, because the dominant cost is organisational, not algorithmic.

### 7.2 Option B — *Pattern-rewrite engine with explicit strategies* (medium risk)

Build a small rewrite engine in Rust:

    rule! {
        php::ForeachStatement[$iter, $bind, $body] =>
            foreach[ right[expression[$iter]],
                     left[expression[$bind]],
                     $body ]
    }

    rule! {
        member[member[$o, $p1], $p2] =>
            object[access][receiver: $o, member: $p1, member: $p2]
    }

    let pipeline = sequence![
        topdown(per_language_rename),
        bottomup(repeat(chain_flatten)),
        topdown(wrap_expression_hosts),
    ];

A `rule!` macro builds typed pattern data structures; a small interpreter
walks the tree and applies strategies. Custom remains for cases the patterns
genuinely cannot express, but should be much rarer.

**What this buys vs. Option A:**

- Cross-cutting rewrites (chain inversion, expression-host wrapping) become
  declarative rules instead of bespoke imperative passes.
- Adding a language reduces to "write input patterns for each construct"
  instead of "write 20 Custom handlers."
- The pipeline becomes a value (composable, printable, swappable per
  language) rather than a hand-coded function.

**Cost:** designing the pattern language and ensuring it covers our cases
without becoming Stratego (overkill) or being too thin (leaving Custom
handlers everywhere). The macro layer in Rust is non-trivial.

**Risk:** if the pattern language doesn't match our cases well, we end up
with a half-built DSL *and* a Custom escape hatch and the worst of both.

### 7.3 Option C — *Single-pass attribute grammar over typed tree* (high risk)

Replace the multi-pass tree-mutation pipeline with one walk that synthesises
attributes bottom-up + propagates inherited attributes top-down, then a
single rendering step builds output from attributes.

Strongly typed. Conceptually clean. Closer to how a compiler frontend would
do it.

**Cost:** very large refactor; Rust borrow checker fights you on attribute
graphs; cross-cutting rewrites (chain inversion) become attribute-flow
problems that can be subtle to express.

**Risk:** one of those redesigns that looks beautiful and stalls in the
middle, leaving the system worse than where it started. I would not pick
this without first prototyping it on one trivial language and proving the
ergonomics.

### 7.4 Recommendation

**Pursue Option A first.** It addresses the dominant cost (spread per
construct + thin rule language) with no novel infrastructure. The work is
unglamorous but high-value and low-risk.

**Prototype Option B in parallel on one fresh language** (suggestions
below) to learn whether a pattern-rewrite engine pays for itself.

**Do not start Option C** unless A+B prove organisational changes are not
enough.

## 8. Concrete experiment proposals

The user asked: *"we can try out a parallel implementation for a new or
existing language and see if it works better."* My picks:

### 8.1 Experiment 1 — Lua, from scratch under Option A

Lua is a good probe:

- Currently unsupported (no risk of regressing existing fixtures).
- Small surface area (~70 grammar kinds vs. ~400 for C#).
- Has all the interesting structures: chains, scope blocks, multiple
  return, table constructors, control flow.
- No exotic shapes (it would not stress-test the rule language enough by
  itself, but it lets us compare ergonomics fairly).

**Method:** implement Lua transform under the construct-organised structure
with the expanded rule language. Compare:
- LOC vs. Python (closest existing analogue).
- Files touched per construct (target: 1).
- Custom handler count (target: < 5).
- Time to add the canonical "foreach + chain + member access" set.

### 8.2 Experiment 2 — Re-implement TOML under Option B

TOML is small and we already have it, so we have a ground-truth output to
match. A from-scratch Option-B re-implementation gives us:
- A direct ergonomics A/B against the current code.
- Validation that the pattern-rewrite engine handles list-of-tables and
  inline tables (which currently use Custom handlers).
- A decision point: if Option B beats current TOML on LOC + readability +
  Custom-handler count, scale up; otherwise abandon.

### 8.3 Experiment 3 — Apply Option A's reorganisation to *one construct
across all 9 languages*, in place

Pick one well-understood construct (suggestion: `foreach` or `binary`)
and lift its logic out of every language's `transformations.rs` into a
single `semantic/<construct>.rs`. See whether the result is unambiguously
better. Roll back trivially if not.

This is the cheapest experiment and gives us the most realistic signal
about the dominant cost.

## 9. What we keep no matter what

- Output shape (cross-language semantic tree as currently emitted).
- Shape contracts (in some form — possibly stronger as a verifier between
  named passes).
- Cross-language transform tests in `tractor/tests/transform/`.
- The XPath query model.

What we may give up:

- The current rule-table dispatch (replaced by a richer rule type or a
  pattern engine).
- The per-language file organisation of `transformations.rs`.
- The implicit pass ordering (replaced by a pass manager with explicit
  deps).
- The "every Custom handler is its own snowflake" model.

## 10. Open questions for the user

Before committing engineering effort:

1. **Which experiment first?** Suggested order: 8.3 (cheapest), 8.1 (most
   informative on ergonomics for new languages), 8.2 (validates Option B).
2. **Stop the regular self-improvement loop while exploring?** I recommend
   yes — exploration and incremental backlog work pull in opposite
   directions.
3. **Acceptable to introduce one new dependency** for Option B (a
   small-pattern-DSL crate or pattern-matching macro), or do we keep
   tractor as macro-light Rust?
4. **Time-box for exploration** before deciding to commit: 3 iters? 5? 10?
   Worth picking now so we don't drift.

---

## 11. Leading direction — typed IR + pure lowering

The user's intuition (paraphrased): the dominant cost is in-place mutation of
the Xot tree across many passes. Replace the workspace tree with a **typed
intermediate representation** of the target shape. Each per-language
transform becomes a pure function `lower_L : tree-sitter CST → IR`. A single
final pass renders `IR → XML`.

This is how every modern compiler frontend works:

- rustc:    `ast::Crate → hir::Crate → mir::Body → llvm::Module`
- Roslyn:   `SyntaxTree → SemanticModel → IL`
- Babel:    `Source → AST (typed) → AST (rewritten) → Source`
- Clang:    `TU → Sema → CodeGen`

We have been doing the same problem (concrete syntax → normalised abstract
syntax) but using XML as both workspace *and* output, which means every
intermediate state is a different undocumented half-transformed shape. If
the workspace is *typed* and *separate from* the output, that whole class
of bugs vanishes.

### 11.1 Architecture sketch

```
                                  ┌──────────────┐
   tree-sitter CST (per-lang)     │              │   pure function
   ───────────────────────────►   │  lower_L(·)  │   per language L
                                  │              │
                                  └──────┬───────┘
                                         │
                                         ▼
                                  ┌──────────────┐   shared, typed,
                                  │   IR (Ir)    │   strongly enforces
                                  │              │   semantic shape
                                  └──────┬───────┘
                                         │
                                         │  optional: pure IR→IR
                                         │  normalisations
                                         │  (chain rebalancing,
                                         │   marker normalisation,
                                         │   etc.)
                                         ▼
                                  ┌──────────────┐
                                  │   render(·)  │   IR → Xot/XML
                                  └──────┬───────┘
                                         ▼
                                    XPath queryable
```

Concretely, `Ir` is one `enum` (or several layered enums) whose variants
are exactly the constructs we declare in the semantic tree spec:

```rust
enum Ir {
    // Statements
    Foreach   { right: Box<Ir>, left: Box<Ir>, body: Box<Ir> },
    If        { cond: Box<Ir>, body: Box<Ir>, else_: Option<Box<Ir>> },
    Function  { name: Option<Name>, params: Vec<Ir>, body: Box<Ir>,
                modifiers: ModifierSet },

    // Expressions
    Call      { callee: Box<Ir>, arguments: Vec<Ir> },
    Member    { object: Box<Ir>, property: Name, optional: bool },
    Index     { object: Box<Ir>, indices: Vec<Ir> },
    Binary    { op: BinOp, left: Box<Ir>, right: Box<Ir> },
    // …

    // Leaves
    Name(Name),
    Literal(Literal),

    // Markers (placed wherever the parent variant declares them)
    // Markers are *not* an Ir variant; they're attributes on the
    // structural variants. If a marker is genuinely standalone
    // (e.g. <yield/> in `for foo in yield_pipe()`), it gets a typed
    // variant.
    Yield,
    Async,
    // …

    // Source location is on every node, threaded through.
}
```

Per-language lowering is a single `match`:

```rust
fn lower_php(node: TsNode<'_>, src: &str) -> Ir {
    use PhpKind::*;
    match php_kind(node) {
        ForeachStatement => Ir::Foreach {
            right: Box::new(lower_php(field(node, "right"), src)),
            left:  Box::new(lower_php(field(node, "left"),  src)),
            body:  Box::new(lower_php(field(node, "body"),  src)),
        },
        FunctionCallExpression => { /* … */ }
        SubscriptExpression    => Ir::Index { /* … */ },
        MemberAccessExpression => Ir::Member { /* … */ },
        // …
    }
}
```

Cross-cutting passes (chain inversion etc.) become *pure IR → IR
rewrites*:

```rust
fn flatten_chains(ir: Ir) -> Ir {
    use Ir::*;
    match ir {
        Member { object: box Member { object: box inner, property: p1, optional: o1 },
                 property: p2, optional: o2 } =>
            // rewrite into ObjectAccess(inner, [(p1,o1), (p2,o2)])
        // …
        other => other.map_children(flatten_chains),
    }
}
```

Each rewrite is a function from `Ir` to `Ir`, testable in isolation.

### 11.2 Why this addresses the inventory in §4

| Accidental cost (§4)              | How typed IR + pure lowering fixes it                                      |
| --------------------------------- | -------------------------------------------------------------------------- |
| Spread per construct (1)          | Lowering for all languages clusters around `Ir::<Construct>` — one site.   |
| Custom-handler proliferation (2)  | Custom *vanishes*: every per-language lowering arm is an explicit handler. |
| Pre-passes that undo tree-sitter (3) | Eaten at the source: each language's `lower_L` handles its own quirks.  |
| Field-wrap as separate sub-system (4) | Field assignment is a normal arm of the lowering match.                |
| Post-walks as imperative passes (5)| Post-walks become typed pure functions `Ir → Ir` with declared input/output. |
| Shape contracts as ratchets (6)   | Shape **is** the type. Contract is `cargo check`.                          |
| Cross-language patterns un-abstracted (7) | Each construct has a single IR variant — abstraction is automatic.   |
| No introspection (8)              | `dbg!(ir)` between passes works. CLI `--print-ir-after pass-name` is trivial. |
| 17 chain-inversion adapters (9)   | One IR rewrite. The "adapter" was only needed because the pre-IR Xot was per-language. |
| Multiple tree passes (10)         | Pure IR→IR composes; a smart compiler can fuse, but we don't have to.       |

The *essential* costs of §3 remain:
- Per-language alphabet diff (handled in `lower_L`).
- Non-local rewrites (handled as IR→IR passes).
- Tree-sitter quirks (handled in `lower_L`).
- Pass ordering (now a typed pipeline; ordering is `let ir = … ; let ir = pass2(ir); …`).

### 11.3 The `Bag` question

Should `Ir` have a "miscellaneous bag of children" variant for nodes we
have not (yet) classified?

**Argument for keeping `Bag`:**
- Lets us land partial IR coverage and fall through unimportant nodes.
- Matches today's reality (the half-transformed Xot is effectively one big
  bag).
- Easier migration: bring up languages incrementally.

**Argument against `Bag` (drop it):**
- A `Bag` is exactly what creates whack-a-mole today. Untyped pass-through
  means transforms downstream don't know what's inside.
- Forcing typed coverage means every CST kind must be either *handled* or
  *explicitly dropped*. That is unambiguous; bags are not.
- The IR's value comes from being a contract. A `Bag` variant punctures the
  contract.
- "If it gets painful, add it later" — but a deliberate punt is much
  better than a stash-everywhere bag.

**My recommendation:** *no `Bag`. Replace it with two narrower variants:*

```rust
enum Ir {
    // … the typed variants …

    /// Explicit "this CST kind has no semantic meaning; we drop its node
    /// but keep its children inline at the parent level". For things
    /// like tree-sitter's anonymous tokens / wrapper nodes that carry
    /// no information.
    Inline(Vec<Ir>),

    /// Last-resort hatch for an un-handled kind. Renders as `<unknown
    /// kind="..."/>` — visible, queryable, and a regression signal that
    /// can be ratcheted to zero per language.
    Unknown { kind: &'static str, raw: String },
}
```

`Inline` says "I deliberately have no shape contribution"; `Unknown` says
"I do not yet know what to do here." Both are honest. Neither is a bag.

If `Inline` and `Unknown` together turn out to be unused once a language
is fully covered, we delete them.

### 11.4 What lowering loses that mutation has

In-place mutation has one genuine advantage over construct-by-construct
lowering: it is easy to write a transform that *only knows about the part
it cares about and leaves everything else alone*. With typed IR, you must
have a complete match for every CST kind in `lower_L`.

That is the work that has been quietly accreting in the imperative passes
all along. A typed IR forces it to the surface — which is a feature, not
a cost, but it is up-front work.

Mitigation: provide a default `lower_kind_unknown` that emits
`Ir::Unknown { kind, raw }` so partial coverage remains compilable and
testable.

### 11.5 IR shape and renderer responsibility

The IR variants are exactly the semantic-tree concepts in
`specs/tractor-parse/semantic-tree/design.md`. That spec becomes the IR
schema by construction.

The renderer (`render: Ir → Xot`) is **mechanical**: it walks the IR and
emits the corresponding XML element with attributes. No decisions live in
the renderer. If two outputs differ, two IRs must differ.

`list="X"` cardinality, marker placement, expression hosts, and chain
shape are all decided in IR-land and emitted faithfully. The XML format
becomes a presentation layer.

### 11.6 Migration strategy

This is the biggest open question. Three options:

**Migration A — All at once on one language.** Pick one language (say
TOML; small; we have ground-truth output). Implement IR + renderer
end-to-end *for that language only*. Compare LOC, ergonomics, regression
frequency. If positive, scale up language by language.

**Migration B — All at once on all languages, no shipping in between.**
Keep the current Xot pipeline working in `main`. Build IR+lowering in a
parallel module. Run *both* pipelines on every fixture; assert they
produce the same XML. When parity is reached, delete the old pipeline.

**Migration C — Construct by construct, gradually.** Pick one construct
(e.g. `Foreach`). Lift its handling out of the imperative pipeline into
typed-IR space *across all languages* at once. Plumb IR pieces back into
the Xot output. Repeat per construct.

**Tradeoffs:**

- A is the cheapest, but TOML doesn't exercise chains, so it under-tests.
  We'd want to follow up with at least one programming language before
  committing.
- B is cleanest but most effort. We get full A/B comparison. The risk is
  the parallel build never finishes.
- C is most incremental but messiest — each construct migration is a
  hybrid where IR handles part and Xot handles part.

**My recommendation:** **Migration A on Lua**, not TOML. Lua is small but
has chains, control flow, and table constructors — enough to stress the
design. New language so no regression risk. If the result is good,
follow with **Migration B** as the migration path for existing languages.

### 11.7 First concrete iteration

Smallest experiment that proves or disproves the direction:

1. Pick **a single tractor language slice** that exercises ≥3 cross-cutting
   concerns (chain, foreach, conditional). Suggestion: Python, *expressions
   only* (call, member, index, binary, unary, literal, name).
2. Define `Ir` enum covering exactly that slice.
3. Write `lower_python_expr: TsNode → Ir`.
4. Write `render_ir: &Ir → XotNode` mirroring current XML output.
5. Run: take an existing Python fixture, lower its expression-only
   subtrees, render to XML, **diff against current snapshot**.
6. Goal: zero diff for the covered subset; clean code; compelling LOC delta.
7. If parity achieved cleanly, expand scope. If not, document why and
   reconsider.

This experiment fits in 1–3 iterations. It commits to nothing
permanent — the parallel module is a sketch, deleted if it doesn't pay
off.

## 12. Experiment 1 — outcome

Code: `tractor/src/ir/{mod,types,render,python}.rs` + parity tests at
`tractor/tests/ir_python_parity.rs`.

**Result: 14/14 parity tests pass.** The IR pipeline produces a
shape-identical Xot tree to the imperative pipeline for the Python
expression slice covered:

| construct                       | source     | LOC in IR variant + lowering | parity |
| ------------------------------- | ---------- | ----------------------------:| :----: |
| literal — int / float / string  | `42`       | 6 + 6                        | ✓      |
| literal — `True` / `False` / `None` | `True` | 6 + 6                        | ✓      |
| identifier                      | `foo`      | 1 + 1                        | ✓      |
| member access                   | `a.b`      | 6 (Access + Member) + 26     | ✓      |
| member chain (2 deep)           | `a.b.c`    | (covered by the same code)   | ✓      |
| subscript                       | `a[0]`    | (Index segment) + 26         | ✓      |
| bare call — no args             | `f()`      | 5 + 16                       | ✓      |
| bare call — with arg            | `f(x)`     | (covered by same code)       | ✓      |
| binary `+`                      | `a + b`    | 7 + 16 + tiny op-marker map | ✓     |
| unary `-`                       | `-x`       | 5 + 14 + tiny op-marker map | ✓     |

Total slice: **946 lines including tests and module docs.** ~700 lines
non-test.

### What worked smoothly

- **Chain inversion fell out of the lowering.** No separate pass. The
  attribute-handler arm checks "is the lowered object already an
  `Access`?" and either appends or wraps. 26 lines covers single and
  multi-step chains *and* mixed member/index. In the existing pipeline
  this is a 750-line `chain_inversion.rs` module + 17 per-language
  adapters (≈ 1 100 LOC total) doing the same job for every language.
  The **architectural reduction is real**, not theoretical.
- **No `Bag` was needed.** `Inline` and `Unknown` covered every
  pass-through case; no temptation to stash arbitrary children.
- **No imperative passes ever ran.** The full pipeline is
  `lower → render`. Two functions. The "phase ordering" debugging
  surface from § 4 is gone for this slice.
- **Type-level parity invariants.** Every `Ir::*` variant has explicit
  required fields. The compiler refuses partial constructions.
  `marker-stays-empty`, `container-has-content`,
  `name-declared-in-semantic-module` shape rules — all
  unrepresentable by construction in this slice.
- **Tests are tiny and meaningful.** Each parity test is one line of
  source + an assertion; the harness compares the structural shape
  produced by both pipelines. Failures point at exactly which shape
  diverged.

### What needed thought

- **Operator markers.** A 4-entry inline `op_marker` map handled
  `+ - * /` for the experiment. At scale this is the existing
  cross-language `OPERATOR_MARKERS` table; the IR will reuse it.
- **Receiver-vs-segment shape.** The current pipeline emits chains
  with the *leftmost* atom as `<object>`'s primary receiver and
  deeper segments right-nested. Modelling that as `Access {
  receiver, segments: Vec<...> }` and a recursive segment-renderer
  fits cleanly. Different from the natural right-deep CST shape but
  *easier* to express in IR than via a separate `chain_inversion`
  pass.
- **Standalone-call vs chain-segment Call.** `f()` is `<call>` at
  top level; `a.b()` would be a `<call>` segment under `<object>`.
  The IR could either have two variants or one variant with a
  context flag — deferred. For now `Ir::Call` is standalone-only;
  chained calls fall to `Unknown` and will be added when the slice
  grows.

### What did *not* fight back

- Source-location threading. Each variant carries `Span`; the
  renderer emits `line/column/end_line/end_column` mechanically.
- The renderer is small (216 lines including the operator helper and
  segment recursion) and entirely mechanical.
- No global state, no `Xot` mutation outside the renderer, no
  cross-cutting passes, no field-wrap table.

### Shape-contract rationale comments — what survives

Per the user's instruction. The runtime shape rules in
`tractor/src/transform/shape_contracts.rs` exist to catch bugs
produced by imperative mutation. In the typed-IR world many of them
become unrepresentable. The rationale is preserved as docstring
comments on `Ir` and `AccessSegment` (see `tractor/src/ir/types.rs`),
mapping each rule to either:

- *Type-enforced* — the bug class can't compile (markers can't have
  children, containers can't be empty, names can't have grammar-kind
  suffixes, etc.).
- *Renderer-asserted* — runtime check at render-time on cardinality
  / sibling counts (e.g. `no-children-overflow`).
- *Lowering-enforced* — anonymous-keyword leaks, kind-attribute
  values, and similar are decisions made (or refused) at lowering
  time.

When a language fully migrates to IR, its rules-table entries in
`shape_contracts.rs` should retire — but the *rationale comments*
move (with attribution to the original iter) into the IR variant
docstrings so the institutional memory survives.

### Decision

The experiment **proves the architecture works** for a non-trivial
subset. The next steps are:

1. **Expand the slice** — chained calls, comprehensions, decorators,
   strings-with-substructure. Aim for full Python expression
   coverage as iter target.
2. **Statements and module structure** — add `Function`, `If`,
   `Foreach`, `Class`, etc. variants. Lower Python's statement-level
   constructs.
3. **Wire IR rendering into the production parser path under a
   feature flag**, so existing snapshot tests run side-by-side with
   IR-rendered output. Aim for snapshot parity on
   `tests/integration/languages/python/blueprint.py`.
4. **Once snapshot parity holds for Python**, extend to a second
   language with an existing fixture. Work language-by-language.
5. **When all languages reach parity, delete the imperative
   pipeline** and move shape-contract rationale comments into the
   IR variants.

This is consistent with the user's stated migration path: "make sure
shape contract rationale comments are ported, then switch and roll
out broadly."

## 13. Iteration 2 — byte ranges, gap text, source-content invariant

**Goal.** Make the IR support two requirements the user named explicitly:
1. **Source round-trip.** Recover the verbatim source slice for any IR
   sub-tree.
2. **XPath text-content matching.** `string(.)` on every rendered XML
   element must equal its source slice — so `//call[.='foo()']` works
   as a query mechanism.

### Design changes

- Added `ByteRange { start: u32, end: u32 }` to every `Ir` variant and
  every `AccessSegment` variant. Source is now the single source of
  truth — owned `text: String` fields on leaves were dropped; the
  renderer slices `&source[range]` instead.
- Added `op_range: ByteRange` to `Binary`/`Unary` so the renderer can
  derive the gaps around operators (e.g. the spaces in `a + b`).
- Renderer signature now takes `source: &str`. Two cooperating
  algorithms:
  - For containers, emit gap text *between* source-derived children
    based on byte ranges not covered by any child.
  - For chain segments (right-nested), each segment's range is its
    own slice (e.g. `.b`); deeper segments render *inside* the
    previous segment's element. The renderer's
    `render_segments_chain` walks both the segments and a moving
    cursor to keep gap calculation correct.
- Synthetic IR (the `<access/>` empty marker, slot wrappers like
  `<left>`/`<expression>`) contributes no text. The variant's
  renderer places it at a deterministic position; it does not
  participate in gap calculation.

### Test invariants (now asserted on every parity case)

1. **Round-trip identity.** `to_source(lower(parse(s)), s) == s`.
2. **Lossless XPath text recovery.** `string(IR_root) == source`.
3. **Structural parity (leaf-text view).** Element names + nesting +
   leaf-text agree with the existing pipeline, with gap text on
   containers ignored *because that's the dimension on which the
   existing pipeline is lossy and the IR pipeline is strictly
   better.*

### The surprising finding

Pulling the gap-filter off the structural view exposed a **lossiness
in the existing pipeline** I hadn't expected. For `a.b`, the
existing pipeline renders:

    <object><access/><name>a</name><member><name>b</name></member></object>

— note the *missing dot*. XPath `string(.)` on the existing pipeline's
`<object>` returns `"ab"`, not `"a.b"`. So `[.='a.b']` does not match
the existing tree.

The IR pipeline, with byte-range-driven gap rendering, emits:

    <object><access/><name>a</name><member>.<name>b</name></member></object>

XPath `string(.)` returns `"a.b"`. The IR satisfies the
text-content-by-source-slice contract; the existing pipeline does
not. Same observation for subscript (`[`/`]`).

This means the IR pipeline is not just "structurally equivalent" — it
is **strictly more powerful for query semantics** in the cases that
go through chain inversion. The user's stated reason for wanting
byte-range threading (powerful XPath text-matching queries) is
something the existing pipeline can't do.

### Result

All 14 parity tests pass with all three invariants. Total
experimental footprint:

```
tractor/src/ir/types.rs       295 LOC
tractor/src/ir/render.rs      302 LOC
tractor/src/ir/python.rs      266 LOC
tractor/src/ir/mod.rs          50 LOC
tractor/tests/ir_python_parity.rs   188 LOC
                            =======
total                       1 101 LOC
```

The added complexity for ranges + gap-rendering was modest (~200
LOC). The XPath-by-source-text capability is a real gain over the
existing pipeline for the same expression slice.

## 14. Iteration 3 — extending toward Python blueprint parity

### Status checkpoint
After the byte-range work, I extended the IR pipeline toward full
parity against the Python `blueprint.py` fixture (227 lines, ~5 KiB
of source, exercising most of Python's surface area).

**What is supported and shape-parity-verified against the existing
pipeline:**

- Module + expression-statement
- Identifiers + all primitive literals (int / float / string / true /
  false / none)
- Member access (single + chained)
- Subscript (single + chained)
- Bare calls
- Binary `+ - * /`
- Unary `+ -`
- Comparison `== != < <= > >= is in`
- Imports (plain, aliased, multi, from-import, relative-from,
  aliased-from-import)
- Assignments (plain, type-annotated, augmented `+= //= @= …`,
  multi-target tuple unpacking)
- Function definitions (with async, decorators, generic type
  parameters PEP 695, parameter shapes regular/default/typed/typed
  default/`*args`/`**kwargs`/positional-only-`/`/keyword-only-`*`,
  return-type annotation)
- Class definitions (with bases, generics, decorators)
- Body blocks (with pass-only optimisation)
- Decorators (hoisted from `decorated_definition` into the inner def
  with range expanded backward)
- Comments (default leading classification)
- Tuples, lists, sets, dictionaries, pairs
- Generic-type expressions `Foo[T, U]`
- if / elif / else
- for / while / break / continue
- Returns

**Coverage status against `blueprint.py`:**

- Round-trip identity: holds (every byte recoverable from IR).
- XPath text-content recovery: holds (`string(IR_root) == source`).
- Slice tests: 14/14 pass (all three invariants per case).
- Lib tests: 341/341 still green.
- Structural parity: ~60 % of blueprint output (18 851 / 31 837
  bytes). Diverges deeper into the file at type-alias-statement,
  lambda, comprehensions, with, try/except, yield, await, match.

### What remains (~30 constructs, bounded effort each)

| Group                | Constructs                                           |
| -------------------- | ---------------------------------------------------- |
| Type aliases (PEP 695)| `type_alias_statement`, `constrained_type`, `splat_type`, `union_type`, `parenthesized_expression` |
| Lambdas              | `lambda_expression`, `lambda_parameters`             |
| Comprehensions       | list / set / dict / generator + `for_in_clause` + `if_clause` |
| Conditional expr     | `conditional_expression` (ternary)                   |
| Walrus               | `named_expression`                                   |
| Strings              | `concatenated_string`, f-string `interpolation`, `format_specifier`, `type_conversion` |
| Splats               | `list_splat`, `dictionary_splat`, `list_splat_pattern`, `dictionary_splat_pattern` |
| with / try           | `with_statement`, `with_item`, `as_pattern`, `try_statement`, `except_clause`, `except_group_clause`, `finally_clause` |
| yield / await / raise | `yield`, `await`, `raise_statement`                 |
| assert / pass / del / global | `assert_statement`, `global_statement`, `nonlocal_statement` |
| Match-case           | `match_statement`, `case_clause`, `case_pattern`, `class_pattern`, `keyword_pattern`, `complex_pattern`, `tuple_pattern`, `list_pattern`, `dict_pattern`, `union_pattern`, `splat_pattern`, `wildcard_pattern` |
| Imports (residual)    | `wildcard_import`, `future_import_statement`        |
| Argument shapes       | `keyword_argument`                                  |
| Type expressions      | `union_type`, `constrained_type`, `splat_type` in non-generic contexts |

Each entry is roughly 30-100 LOC of lowering + rendering, following
the now-established pattern: define `Ir::*` variant → write lowering
arm → write rendering arm with byte-range gap weaving.

### Architectural position
The pattern is established. The remaining work is mechanical: every
new construct fits the existing lowering/rendering pattern. No new
architectural decisions have been needed since iter 2's gap-aware
renderer.

Snapshot of the pipeline today:
- `tractor/src/ir/types.rs`: ~700 LOC (variant declarations + docs)
- `tractor/src/ir/python.rs`: ~900 LOC (lowering)
- `tractor/src/ir/render.rs`: ~700 LOC (rendering)
- `tractor/tests/ir_python_parity.rs`: 188 LOC (slice parity)
- `tractor/tests/ir_python_blueprint.rs`: 165 LOC (blueprint parity)

Total experiment: ~2 800 LOC.

### Decision point
Three honest paths from here:

1. **Continue mechanically.** Add the ~30 remaining constructs,
   commit per logical group, until blueprint structural parity hits.
   No architectural insight expected; mostly typing.

2. **Stop and validate.** The architecture is proven. Spend
   remaining attention on (a) running snapshot tests across all
   languages with the existing pipeline to confirm no regressions,
   (b) writing an honest write-up for the user, (c) deciding whether
   the typed-IR direction is worth the multi-language migration cost.

3. **Pivot to a second language.** Implement a small slice of
   another language (Java? Rust?) under the same IR to validate
   that the IR vocabulary works cross-language. Discovers
   real-world cross-language tensions earlier than full Python
   parity would.

## 15. Iteration 4 — cross-language validation (C# slice)

### Why C#
Picked per the user's direction: *"the language that gave us the
most whack-a-moles to evaluate against."* By git-log commit count:

| Language    | Commits | Custom handlers | post_transform LOC |
| ----------- | ------- | --------------- | ------------------ |
| **C#**      | **86**  | 23              | 440                |
| Rust        | 67      | 33              | 469                |
| TypeScript  | 66      | 37              | 311                |
| Java        | 65      | 30              | 98                 |
| Go          | 65      | 26              | 172                |
| Ruby        | 52      | 7               | 416                |
| PHP         | 47      | 21              | 335                |

C# also carries the unsolved `?.` conditional-access design problem,
the chain-inversion adapter, and various operator-extraction quirks
— a strong stress test for the typed-IR architecture.

### What was added for C#

Code: `tractor/src/ir/csharp.rs` (~280 LOC).

**Reused from Python** (no changes needed): all access-chain
machinery (`Ir::Access`, `AccessSegment::Member`, `AccessSegment::Index`),
calls (`Ir::Call`), binary/unary operators (`Ir::Binary`,
`Ir::Unary`), all atoms (`Ir::Name`, `Ir::Int`, `Ir::Float`,
`Ir::String`, `Ir::True`, `Ir::False`), expression hosts
(`Ir::Expression`), passthrough (`Ir::Inline`), and escape hatch
(`Ir::Unknown`).

**Added exactly two things** for C#:

1. `Ir::Null` — the `null` keyword literal. Distinct from
   `Ir::None` (Python's keyword text differs).
2. `element_name: &'static str` field on `Ir::Module`. Python emits
   `<module>`, C# emits `<unit>`. (Cross-language unification of this
   name is a Principle #5 audit candidate but requires the existing
   pipeline's per-language choice to be revisited; we keep parity.)

The operator-marker map for C# reuses the same names (Python: `plus`,
`minus`, `multiply`, …) — Principle #5 working in our favour.

### Result

**13/13 C# tests pass** including against the full
`tests/integration/languages/csharp/blueprint.cs` (239 lines, ~7 KiB):

- Round-trip identity holds.
- XPath `string(.)` recovery holds.
- Five expression-subtree cases (`a.b.c`, `a[0]`, `f(x)`, `42`,
  `a.b`) round-trip and recover cleanly.

**Python tests still all green** (14/14 slice + 60% blueprint
parity, unchanged); **lib tests still green** (341/341).

### Architectural finding

The IR's **expression-core vocabulary is cross-language reusable as-is**.
This was the decisive question: does typed IR scale to multiple
languages without each one demanding its own variants? For the
expression core: **yes, almost entirely.**

The only divergences this slice surfaced:
- One language-keyword variant (`Ir::Null`).
- One language-specific element name on `Ir::Module`.

Both are *honest* divergences — they reflect real differences in
the source languages, not architectural problems. They show up at
type definition time, not as debugging-time surprises.

Compare this against the equivalent pain in the imperative pipeline:
- C# has its own 280-LOC `chain_inversion` adapter (one of 17 across
  languages).
- C#'s `pre_transform_hook` does `csharp_normalize_conditional_access`
  to undo a tree-sitter quirk before chain inversion can run.
- C#'s `transformations.rs` (827 LOC) duplicates many shapes that
  Python, Java, TypeScript also handle independently.

In the typed-IR pipeline, *one* `lower_csharp_root` (~280 LOC,
total) handles the same expression core that Python's
`lower_python_root` (~700 LOC, but covering more constructs) handles.
The shared `render` is unchanged.

### Honest limits of this slice

C# tree-sitter requires syntactic context (a class with a method)
before it accepts an expression statement. So the slice tests wrap
each expression in `class C { void M() { var x = <expr>; } }`. The
*surrounding* class/method/variable structure isn't yet in the IR —
expressions show up wrapped in `Ir::Unknown` for the
`class_declaration` outer scope. The architectural invariants still
hold (the Unknown's range covers the unhandled bytes), but
structural parity for the wrapper isn't tested yet.

To extend further: add `Ir::Class`, `Ir::Method`, `Ir::Variable`,
`Ir::Block` for C#. These are mostly cross-language reusable too —
adding modifiers (`internal`, `public`, etc.) is one new field on
`Ir::Class`, and Python's existing `Ir::Class` has all the rest.

### Decision point (user-directed)
The cross-language architectural validation is complete. The
architecture handles a *very different* language (different
tree-sitter grammar, different syntactic constraints, different
keyword set) with **two type-level changes** and **zero changes to
shared infrastructure.**

Three paths from here:

1. **Continue extending C# coverage** to match Python's level.
2. **Pivot to Ruby** — second-highest pain language in the existing
   pipeline. Ruby's grammar is fundamentally weirder (no
   `expression_statement`, scope-resolution quirks); a successful
   Ruby slice would close the architectural validation case.
3. **Stop and assess.** The architecture is proven cross-language;
   the rest is volume work. Time to plan the migration to
   production rather than to extend the experiment.

## 16. Design — `GapSource` abstraction (deferred)

A future-looking architectural decision that emerged during iter 4
discussion. Documented here so it's not forgotten; *not yet
implemented* (~80-300 LOC change, deferred until needed).

### The insight
The IR doesn't just enable parsing. It enables **rendering from
scratch** — i.e. a code generator builds an IR by hand (or converts
JSON / XML to IR), then the SAME render logic that we use for
parsed IR emits source code. The two cases differ only in *where
gap text comes from*:

- **Parsed mode**: gap text is `&source[start..end]` (the user's
  original code).
- **Bare mode**: gap text comes from per-variant defaults
  (`(`, `, `, `)`, ` {\n`, etc. — language style choices).

If both cases share the rendering core, we get bidirectionality
(parse ↔ render) for free.

### The design
Replace the renderer's `&str source` parameter with a trait:

```rust
pub trait GapSource {
    /// Return the text for byte range [start..end). Used for both
    /// inter-child gaps and leaf text. Cow because some
    /// implementations slice from a borrowed string while others
    /// build owned defaults.
    fn slice(&self, start: u32, end: u32) -> Cow<'_, str>;

    /// Override leaf-value lookup. For variants that carry an
    /// explicit `value: Option<String>` (added when needed), the
    /// renderer asks the gap source whether to use the override.
    /// Default implementation returns None (use byte-range slice).
    fn leaf_override(&self, _ir: &Ir) -> Option<&str> { None }
}

pub struct SourceGaps<'a>(pub &'a str);              // parsed mode
pub struct DefaultGaps<'a>(pub &'a DefaultsTable);   // bare mode

impl GapSource for SourceGaps<'_> {
    fn slice(&self, start: u32, end: u32) -> Cow<'_, str> {
        Cow::Borrowed(&self.0[start as usize..end as usize])
    }
}

impl GapSource for DefaultGaps<'_> {
    fn slice(&self, _start: u32, _end: u32) -> Cow<'_, str> {
        // Bare mode: byte ranges are zero-width; defaults are looked
        // up by IR variant + position. Implementation detail.
        unimplemented!("bare-mode lookup")
    }
}
```

### Leaf values: `Option<String>` override
Bare mode leaves can't slice source (there is none). Add an optional
override on leaf variants:

```rust
Ir::Int   { range: ByteRange, value: Option<String>, span: Span }
Ir::Name  { range: ByteRange, value: Option<String>, span: Span }
// ...
```

- Parsed: `value: None`, renderer uses `source[range]`.
- Bare: `value: Some("42")`, renderer uses it directly.
- **Edited**: `value: Some("99")` overrides the original — natural
  for codemod-style edits.

### Why this beats inline gap storage
An alternative is to store gap text *inline* on each variant
(`Ir::Class { opening_brace: String, ... }`) — what libcst and
Roslyn do. That works but has costs:

- **Schema explosion.** `Ir::Function` has 10+ gap positions
  (modifiers, return type, name, generics, params, body, braces).
  Multiply by ~50+ variants.
- **Cross-language tension.** `IRClass.opening_brace` is C#-flavored;
  Python uses `:` and indentation. The same `Ir::Class` can't carry
  both natively.
- **Couples IR to format.** The IR is currently *structural*; format
  is separable. Inline storage merges them.

`GapSource` keeps the IR purely structural. Format choices live in:
- `&str` (parsed mode), or
- `DefaultsTable` (bare mode), per-language and per-construct.

The same IR works for both, no schema duplication, no cross-language
divergence.

### When to implement
The refactor is small (~80 LOC for the trait + renderer plumbing,
~20 LOC for `Option<String>` overrides). But it's **not blocking
anything currently**. Defer until either:

1. The render-from-scratch feature ships (need bare mode).
2. Codemod-style edits are needed (need value overrides).
3. We touch the renderer for another reason (cheap to bundle).

### Done note
Once implemented, the IR architecture supports four use cases with
one rendering core:

| Mode | gap source | leaf source | use case |
|---|---|---|---|
| Parsed | `SourceGaps(&source)` | `source[range]` | current — `string(.) == source` |
| Bare | `DefaultGaps(&defaults)` | `value.as_deref().unwrap()` | codegen, JSON → source |
| Edited | hybrid impl | `value.or(source[range])` | codemod, refactor tools |
| Rewrite-test | bare with custom defaults | inject specific values | round-trip-checking tests |

This is the architectural payoff that makes the IR investment pay
back many times.

## 17. Coverage audit — public-facing language support metric

Round-trip identity (`to_source(ir, source) == source`) catches lost
*bytes*; it does not catch lost *structure*. A typed parent that
forgets to lower a child's CST kind silently buries those bytes in
gap text — round-trip passes, but XPath structural queries can't
find the kind.

`tractor/src/ir/coverage.rs` walks the CST and IR in lockstep and
classifies every named CST node into one of:

| Bucket | Meaning |
|---|---|
| **Typed** | An IR node exactly matches this byte range; `Ir::Unknown` excluded. The kind is structurally represented. |
| **Unknown** | `Ir::Unknown` exactly matches this byte range. Kind is explicitly punted. |
| **Under-typed** | Typed IR ancestor's range contains the node, no exact match. Common for chain-folded structure (`a.b` inside `a.b.c` is folded into `Ir::Access` segments — its inner `member_access_expression` CST has no own IR but the chain does represent it). |
| **Under-unknown** | Under an `Ir::Unknown` ancestor's range. Whole subtree is unhandled at a higher level. |
| **Dropped** | No IR range covers this node at all. *Should never happen if round-trip identity holds*; existence indicates a renderer bug. |

### Public metrics

- **Kind coverage** = (kinds with ≥ 1 typed instance) / (distinct kinds in corpus).
  Public-facing language support level.
- **Node coverage** = (typed + under_typed) / total named CST nodes.
  Real-world fraction of code we can structurally query.
- **Drop count** = always asserted == 0.

### Status (against blueprints)

| Language | Kind coverage | Node coverage | Dropped |
|---|---|---|---|
| Python | 44.2% (50/113 kinds) | 63.3% (698/1103 nodes) | 0 |
| C# (expression-only) | 0.7% (1/150 kinds) | 0.1% (1/1479 nodes) | 0 |

C#'s low number is *honest* — we typed the expression core but not
the structural surface (class/method/variable declarations). The
metric correctly reflects this.

### Why this matters

- **Honest reporting.** "Python: 44% kinds typed" beats "Python:
  works for our test cases" because the metric is computed against
  a kitchen-sink corpus.
- **Gradual rollout is fine.** New languages start at low %, climb
  with each iteration. Nothing forces 100% before shipping.
- **Drop count is a safety net.** Any silent-loss bug is caught
  immediately; rounded to a hard assertion in the audit test.
- **Per-kind detail drives prioritisation.** The summary's per-kind
  rows tell the next implementer exactly which kinds are most
  common in the corpus and unhandled.

### Implementation
~280 LOC in `coverage.rs` (no other files touched besides
test wiring). Walks IR collecting (range, is_unknown) pairs; walks
CST classifying each named node; aggregates into per-kind histograms.
The IR walk is exhaustive over current variants — adding a new IR
variant requires extending `collect_ir_ranges` (caught by Rust's
exhaustiveness check).

## 18. The IR is the contract

A clarifying architectural framing that emerged from the mutation
discussion:

> **The IR is the primary contract. XML and JSON are derived
> representations.**

This isn't a small framing change. It has concrete consequences:

### What "primary contract" means
- The Rust `Ir` enum and its variant fields are the public API.
- Adding/removing/renaming a variant or field is a breaking API
  change — same as any typed compiler frontend (rustc HIR, Roslyn
  syntax tree, Babel AST).
- Stability is provided by versioning the IR schema, not by stabilising
  XML element names.

### What XML / JSON become
- **Derived views** of the IR for querying. Same data, different shape
  for different query languages.
- XML view: optimised for XPath (existing).
- JSON view: optimised for JQ-style queries (could be added cheaply
  given the same IR underneath).
- Future views: SQL-like, Datalog, GraphQL — each maps from the
  same IR.

### Mutation operates on IR, not XML
- `tractor modify -x "method[name='M']" --set access=private` is
  *implemented as* "find IR by query → mutate enum field → re-render."
- The XPath is for *finding*, the IR mutation is for *changing*.
- Marker swaps happen automatically when the enum changes
  (demonstrated: `Access::Public → Access::Private` produces
  `<public/> → <private/>` with no XML editing).

### The "exhaustive variations" principle
The user-stated principle "variations must be marked exhaustively"
maps to: **encode every variation as a typed enum field on the IR.**

- Compile-time exhaustiveness — Rust's `match` forces the renderer
  and lowering to handle every variant.
- Stable mutation surface — `--set field=value` validates against the
  enum's domain.
- Marker-rendering is *derived*, not source-of-truth — the source of
  truth is the enum value.

Concrete example shipped in iter:
```rust
pub enum Access { Public, Private, Protected, Internal,
                  ProtectedInternal, PrivateProtected, File }

pub struct Class {
    access: Option<Access>,   // None for languages without (Python).
    /* ... */
}
```

Compound access modifiers (C# `protected internal`) emit *two*
markers (`<protected/><internal/>`) rather than one underscored name
(`<protected_internal/>`), per the "no underscore in node names"
rule. The enum tracks the semantic value; the renderer fans it out.

### Why this matters for the project
- **Easier to add query languages.** JQ over JSON projection is just
  another renderer; same IR, no separate transform pipeline.
- **Mutation is principled.** `--set access=private` validates against
  the enum's domain *at the IR level* before producing any output.
- **Tooling depth.** IDE-level refactor tools (rename, change-access,
  …) can be built on the IR directly without going through XML
  round-trips.
- **No "the XML schema is the API" lock-in.** The XML schema can
  evolve (add list-tags, change marker positions) without breaking
  IR consumers.

## Appendix A — current pipeline as a phase diagram

    ┌──────────────────┐  tree-sitter, per-language grammar
    │ tree-sitter CST  │
    └────────┬─────────┘
             │ build_raw (transform/builder.rs)
             ▼
    ┌──────────────────┐  XML mirror of CST, with field=X attrs
    │ raw xot tree     │
    └────────┬─────────┘
             │ apply_field_wrappings (transform/mod.rs + per-lang table)
             ▼
    ┌──────────────────┐  field-children lifted into role wrappers
    │ field-wrapped    │
    └────────┬─────────┘
             │ walk_transform(lang_fn) (per-language Rule + Custom)
             ▼
    ┌──────────────────┐  kinds renamed, markers added, structural Custom rewrites done
    │ per-kind output  │
    └────────┬─────────┘
             │ post_transform (per-language)
             │   → shared helpers in transform/{chain_inversion,
             │     conditionals, generic_type, operators, singletons, …}
             ▼
    ┌──────────────────┐  chains inverted, hosts wrapped, markers normalised
    │ semantic tree    │
    └────────┬─────────┘
             │ shape contracts verified (transform/shape_contracts.rs)
             ▼
        XPath queryable

The phase boundaries are real but undocumented anywhere structural — they
exist only as the call order inside `transform_xot`.

## Appendix B — references

- Engelfriet & Vogler, *Macro tree transducers*, 1985.
- *Stratego/XT*: Visser et al. — DSL for term rewriting and program
  transformation.
- *Spoofax / Rascal* — language workbench with concrete-syntax rules.
- *MLIR* — LLVM dialect & pass infrastructure (multi-level IR).
- *Babelfish (bblfsh)* — sourced.tech's Universal AST project, archived 2020.
- *Attribute grammars* — Knuth 1968, modern survey: Paakki 1995.
