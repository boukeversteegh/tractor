# Chain Inversion

## Purpose

Tractor's mission — *"Write a rule once. Enforce it everywhere."* — depends on the semantic tree mirroring the developer's mental model of the source. Where the tree disagrees with how a programmer reads code, rules become awkward to write and silently miss cases.

Member-access and method-call chains (`a.b.c.d()`) are one of the largest such mismatches. Tree-sitter parses them right-deep by operator precedence — the LAST source token (the invocation) becomes the outermost element, and the FIRST source token (the receiver) is the deepest leaf. Programmers read them left-to-right: "start with `a`, then access `.b`, then `.c`, then call `.d()`". This document specifies the inverted shape that tractor emits, ratified by the project owner mid-iter-234.

## Current right-deep shape

For TypeScript `a.b.c.d()`:

```
call/
  ├─ callee/member/
  │   ├─ object/member/
  │   │   ├─ object/member/
  │   │   │   ├─ object/name = "a"   ← deepest = first in source
  │   │   │   ├─ "."
  │   │   │   └─ property/name = "b"
  │   │   ├─ "."
  │   │   └─ property/name = "c"
  │   ├─ "."
  │   └─ property/name = "d"
  └─ "()"
```

The `//call/callee/member/object/member/object/member/object/name='a'` query reaches the receiver. Adding or removing a chain link changes the depth of every receiver query. Cross-language queries are language-specific because Java emits a flat call (`<call><object/>NAME...args</call>`), Python/Go nest via `<member>`, and TypeScript wraps in `<callee>`.

## Inverted shape — `<chain>` wrapper, nested step spine

For 2+ link chains, `<chain>` has exactly two children: the **receiver** (any expression element) followed by the **first step**. Subsequent steps nest as the LAST child of the previous step.

`console.stdout.write()`:
```xml
<chain>
  <name>console</name>          <!-- receiver -->
  <member>                       <!-- step 1: .stdout -->
    <name>stdout</name>
    <call>                        <!-- step 2 (terminal): .write() -->
      <name>write</name>
    </call>
  </member>
</chain>
```

For a bare identifier `a` (no chain), the result is just `<name>a</name>` — no `<chain>` wrapper. The wrapper appears only when there is at least one access or invocation step.

### Step element types

- `<member>` — `.foo` access. Children: `<name>foo</name>` plus optional next-step element.
- `<call>` — `.foo(...)` method call OR `(args)` result-invocation. Children: `<name>foo</name>` (absent for result-invocation) + zero or more `<argument>` siblings + optional next-step element.
- `<subscript>` — `[expr]` index access. Children: index expression + optional next-step element.
- `<cascades>` *(future, Dart only)* — wrapper holding sibling cascade steps. See "Cascades" section below.

### Examples

`a.b.c.d` (pure access):
```xml
<chain>
  <name>a</name>
  <member>
    <name>b</name>
    <member>
      <name>c</name>
      <member>
        <name>d</name>
      </member>
    </member>
  </member>
</chain>
```

`a.b().c.d()` (mixed mid-chain calls):
```xml
<chain>
  <name>a</name>
  <call>
    <name>b</name>
    <member>
      <name>c</name>
      <call>
        <name>d</name>
      </call>
    </member>
  </call>
</chain>
```

`a[0].b` (subscript in chain):
```xml
<chain>
  <name>a</name>
  <subscript>
    <int>0</int>
    <member>
      <name>b</name>
    </member>
  </subscript>
</chain>
```

`(x as Foo).b` (complex receiver):
```xml
<chain>
  <cast>
    <name>x</name>
    <type>Foo</type>
  </cast>
  <member>
    <name>b</name>
  </member>
</chain>
```

`f()(args)` (result-invocation):
```xml
<chain>
  <call>
    <name>f</name>
  </call>
  <call>
    <argument>...</argument>
  </call>
</chain>
```

The discriminator for "result invocation" vs "method call" is presence/absence of a `<name>` child.

`a?.b?.c()` (optional chaining):
```xml
<chain>
  <name>a</name>
  <member>
    <optional/>
    <name>b</name>
    <call>
      <optional/>
      <name>c</name>
    </call>
  </member>
</chain>
```

The marker rides on the step element where the operator appears, not on the receiver name.

## Why nested rather than flat

The first proposal (iter 232) was a FLAT shape with sibling chain segments under `<chain>`. After comparing query patterns side-by-side, NESTED won on the strength of **declaration-call query symmetry**:

| Query | Declaration | Chain (nested) | Chain (flat) |
|---|---|---|---|
| Specific path | `//class[name='Foo']/method[name='bar']` | `//chain[name='console']/member[name='stdout']/call[name='write']` | `//chain[name[1]='console' and name[2]='stdout' and call/name='write']` |
| Receiver match | `//class[name='Foo']` | `//chain[name='Foo']` | `//chain[name[1]='Foo']` |
| Middle pattern | n/a | `//member[name='foo']/member[name='bar']` | `//chain[name='foo']/following-sibling::*[1][.='bar']` |

The nested form lets a developer write XPath against chain expressions exactly the way they write it against declarations. Tractor's whole rule library benefits from one consistent navigation idiom rather than two.

The trade-off is that depth queries (Law-of-Demeter detection) become `count(.//member | .//call | .//subscript) >= 3` instead of FLAT's `count(*) >= 3`. Acceptable cost: depth queries are written once into a rule library; path-matching queries are written every time someone authors a rule.

## Element name: `<chain>`

Distinct from existing `<path>`:
- `<path>` — compile-time namespace lookup (`com.example.Foo`, `os::env`, `App\Models\User`). Each segment is purely declarative.
- `<chain>` — runtime member-access / method-call sequence. Each segment may have effects (calls, subscripts, optional checks).

Keeping them separate avoids forcing one element to carry both meanings — Principle #5 (Unified Concepts within a language) and Principle #11 (Specific Names Over Type Hierarchies).

## Cascades (Dart) — future extension, not implemented

> **Status: design only.** Cascades are *not* part of the current
> rollout. This section exists so the chain shape is forward-
> compatible: when Dart joins the supported languages, the
> extension below can land without redesigning what's already
> shipped. The current 8 languages (TS, Python, Java, C#, Go,
> Rust, Ruby, PHP) only have linear chains and don't need any of
> this.

Most languages have linear chains. Dart's cascade operator `..` breaks the assumption that "step N operates on step N-1's result" — every cascade step operates on the **same receiver**, the leftmost expression. The whole cascade expression evaluates to that receiver.

`paint..color = c..strokeCap = s` is semantically:
```dart
paint.color = c; paint.strokeCap = s; /* return */ paint;
```

The nested chain shape would be wrong for cascades because each step is *not* the result of the previous one. The clean extension is a `<cascades>` wrapper holding sibling steps — they're independent operations on the same receiver:

```xml
<chain>
  <call><name>Paint</name></call>
  <cascades>
    <member><name>color</name><assign>c</assign></member>
    <member><name>strokeCap</name><assign>s</assign></member>
  </cascades>
</chain>
```

For mixed cascade and normal chain `obj..a().b..c()..d()` (cascade `a()` on obj, then normal `.b` access on obj, then cascades `c()` and `d()` on `obj.b`):

```xml
<chain>
  <name>obj</name>
  <cascades>
    <call><name>a</name></call>
  </cascades>
  <member>
    <name>b</name>
    <cascades>
      <call><name>c</name></call>
      <call><name>d</name></call>
    </cascades>
  </member>
</chain>
```

`<cascades>` blocks are siblings of regular chain steps. They compose with the rest of the spine.

When Dart arrives, the inverter gains a `ChainSegment::Cascade` variant and `emit_chain` groups consecutive cascades under a single `<cascades>` element. No code lives for cascades in the current implementation — `chain_inversion.rs` does not have a `Cascade` variant, the helper does not recognize `..`, and no tests cover this path. The design is captured here only so the eventual implementer knows the target shape.

## Helper API

Module: `tractor/src/transform/chain_inversion.rs`.

```rust
pub enum ChainSegment {
    Receiver(XotNode),
    Member { name_node: XotNode, markers: Vec<XotNode> },
    Call {
        name_node: Option<XotNode>,
        args: Vec<XotNode>,
        markers: Vec<XotNode>,
    },
    Subscript { index_node: XotNode, markers: Vec<XotNode> },
    // future: Cascade { ... }
}

/// Walk a right-deep canonical input and produce a segment list
/// in source order (leftmost-first). Non-mutating.
pub fn extract_chain(xot: &Xot, node: XotNode) -> Vec<ChainSegment>;

/// Build the inverted `<chain>` tree from a segment list. Returns
/// the new `<chain>` element. Pre: ≥2 segments, first is Receiver.
pub fn emit_chain(xot: &mut Xot, segments: Vec<ChainSegment>)
    -> Result<XotNode, xot::Error>;

/// In-place: extract → detach → emit → replace. Returns the new
/// `<chain>` on success, or None if the input wasn't a useful
/// chain (and was left untouched).
pub fn invert_chain_nesting(xot: &mut Xot, node: XotNode)
    -> Result<Option<XotNode>, xot::Error>;
```

### Canonical input shape

The extractor expects a right-deep input with these conventions:

```
<member>
  <object>RECEIVER</object>            -- receiver subtree (recursive)
  <property><name>X</name></property>  -- the .X access
</member>

<call>
  CALLEE                               -- first non-marker element child:
                                         (a) <member> for method call
                                         (b) <call> for result-invocation
                                         (c) any other element for top-level call
  <argument>...</argument>*            -- args follow as siblings
</call>
```

Languages whose current shape doesn't match (Java's flat call, TypeScript's `<callee>` wrapper) need a small per-language normalization pass before invoking `invert_chain_nesting`. Languages whose shape already matches (Python, Go) can adopt directly.

### Useful-chain guard

`invert_chain_nesting` refuses to wrap when:
- there are fewer than 2 segments (just a receiver, e.g. a bare identifier), or
- the only step is a nameless top-level Call (e.g. `f(args)`).

Wrapping these in `<chain>` would add noise without informational value.

### Source-location threading

- `<chain>` inherits `line`/`column`/`end_line`/`end_column` from the receiver node (the leftmost source token).
- Each step element inherits from its primary node — the access name for `<member>`, the method name for `<call>`, the index expression for `<subscript>`.
- For result-invocation `<call>` segments (no `name_node`), the step has no source location attached automatically — callers can attach later if needed.

## Test coverage

`tractor/src/transform/chain_inversion.rs` ships with 36 unit tests covering the 16 design-doc edge cases and the round-trip pipeline:
- 14 emit cases (receiver-only chain, terminal call, multi-link, mixed, args, markers, subscript, complex receiver, result-invocation, source-location threading, pre-condition guards).
- 11 extract cases (the same shapes, walked from right-deep input).
- 11 round-trip cases (full extract → detach → emit → replace pipeline, including no-op guards for non-chains and idempotency on already-inverted input).

Future Dart cascade work will add tests in the same module.

## Per-language rollout

Each language's post-transform pass needs a chain-root finder + `invert_chain_nesting` invocation per chain root. A "chain root" is a `<member>` or `<call>` whose parent is NOT inside another chain — specifically:
- `<member>` is a chain root iff its parent is not `<object>` and not the callee position of a `<call>`.
- `<call>` is a chain root iff its parent is not `<object>`.

The walker visits chain roots top-down; each invocation extracts the full chain into a flat segment list, so nested chains (e.g. `obj.method(x).other.thing()` where the inner `obj.method(x)` is also a chain) are consumed as part of the outer extraction. There's no double-processing.

### Languages with canonical input (no normalization needed)

- **Python** — `<call><member><object/><property/></member>...args</call>`.
- **Go** — same shape as Python.

### Languages needing a normalization pass first

- **TypeScript** — currently wraps the callee in `<callee>`. Unwrap before extraction.
- **Java** — currently flat `<call><object/>NAME...args</call>`. Wrap NAME in a `<member>` synthetic element first.
- **C#** — TBD (sample blueprint to be reviewed during the C# pilot iter).
- **Rust** — `obj.method().field` — TBD.
- **Ruby** — TBD.
- **PHP** — `$obj->method()->prop` — `->` operator instead of `.`, but otherwise similar shape — TBD.

### Render module update

The renderer (`tractor/src/render/<lang>.rs`) currently reconstructs source from the right-deep shape. After inversion, each per-language renderer needs an `<chain>` traversal that emits source with the correct operator (`.`, `->`, `::`) between segments. One render iter per language, batched as needed.

### Design.md update

After the per-language rollout completes, the global design.md needs a Decision section documenting the chain shape as canonical. Per the self-improvement loop's rules, design.md edits require explicit user approval — that iter is held until the rollout is mature enough to commit.

## References

- `tractor/src/transform/chain_inversion.rs` — implementation + tests.
- `todo/40-chain-inversion-implementation.md` — running implementation checklist.
- `specs/tractor-parse/semantic-tree/design.md` — Principles #5, #11, #15 (rationale grounding).
- `docs/design-transformation-expression-hosts-analysis.md` — companion design note for stable expression hosts (Principle #15), the cousin transform that this work composes with.
