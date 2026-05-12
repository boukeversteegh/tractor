---
title: Chain Inversion
priority: 1
---

> **Status: shipped.** Left-deep `<object[access]>` chains are produced
> natively by each language's IR lowering (`Ir::Access { receiver,
> segments }`). There is no separate chain-inversion transform pass —
> the previous `transform::chain_inversion` post-walk was deleted once
> every language's lowering constructed the shape directly. See the
> design rationale below; cross-language uniformity is enforced by
> `tractor/tests/cross_language_index_access_chain_inverts.rs`.

## Purpose

Tractor's mission — *"Write a rule once. Enforce it everywhere."* — depends on the semantic tree mirroring the developer's mental model of the source. Where the tree disagrees with how a programmer reads code, rules become awkward to write and silently miss cases.

Member-access and method-call chains (`a.b.c.d()`) are one of the largest such mismatches. Tree-sitter parses them right-deep by operator precedence — the LAST source token (the invocation) becomes the outermost element, and the FIRST source token (the receiver) is the deepest leaf. Programmers read them left-to-right: "start with `a`, then access `.b`, then `.c`, then call `.d()`". This document specifies the inverted shape that tractor emits.

## Source right-deep shape (what tree-sitter produces)

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

## Inverted shape — `<object[access]>` wrapper, nested step spine

For 2+ link chains, the wrapper is `<object>` carrying an `<access/>` marker. The marker distinguishes runtime member-access from object literals (which share the `<object>` element name in TS/JS). The wrapper has the marker as its first child, then the receiver, then the first step. Subsequent steps nest as the LAST child of the previous step.

`console.stdout.write()`:
```xml
<object>
  <access/>                      <!-- marker: this is access, not literal -->
  <name>console</name>          <!-- receiver -->
  <member>                       <!-- step 1: .stdout -->
    <name>stdout</name>
    <call>                        <!-- step 2 (terminal): .write() -->
      <name>write</name>
    </call>
  </member>
</object>
```

The `<object>` name is chosen so a developer reading the source thinks the same thing about the tree: `foo.bar` is "an object foo, with member bar accessed on it" — not "a chain of foo and bar". The element name reflects the natural mental model.

For a bare identifier `a` (no chain), the result is just `<name>a</name>` — no wrapper. The `<object[access]>` wrapper appears only when there is at least one access or invocation step.

### Disambiguating chains from object literals

The `<object>` element name is also used for object-literal expressions (TS/JS `{a: 1, b: 2}`):
```xml
<object>
  <pair>...</pair>
  <pair>...</pair>
</object>
```

Two ways to distinguish in queries:
- **Marker-based:** `//object[access]` finds chains; `//object[not(access)]` finds literals.
- **Structural:** `//object[member or call or subscript]` finds chains; `//object[pair]` finds literals.

The marker form is recommended — explicit and short. Both are valid.

### Step element types

- `<member>` — `.foo` access. Children: `<name>foo</name>` plus optional next-step element.
- `<call>` — `.foo(...)` method call OR `(args)` result-invocation. Children: `<name>foo</name>` (absent for result-invocation) + zero or more `<argument>` siblings + optional next-step element.
- `<subscript>` — `[expr]` index access. Children: index expression + optional next-step element.
- `<cascades>` *(future, Dart only — see § Cascades)* — wrapper holding sibling cascade steps.

### Examples

`a.b.c.d` (pure access):
```xml
<object>
  <access/>
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
</object>
```

`a.b().c.d()` (mixed mid-chain calls):
```xml
<object>
  <access/>
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
</object>
```

`a[0].b` (subscript in chain):
```xml
<object>
  <access/>
  <name>a</name>
  <subscript>
    <int>0</int>
    <member>
      <name>b</name>
    </member>
  </subscript>
</object>
```

`(x as Foo).b` (complex receiver):
```xml
<object>
  <access/>
  <cast>
    <name>x</name>
    <type>Foo</type>
  </cast>
  <member>
    <name>b</name>
  </member>
</object>
```

`f()(args)` (result-invocation):
```xml
<object>
  <access/>
  <call>
    <name>f</name>
  </call>
  <call>
    <argument>...</argument>
  </call>
</object>
```

The discriminator for "result invocation" vs "method call" is presence/absence of a `<name>` child.

`a?.b?.c()` (optional chaining):
```xml
<object>
  <access/>
  <name>a</name>
  <member>
    <optional/>
    <name>b</name>
    <call>
      <optional/>
      <name>c</name>
    </call>
  </member>
</object>
```

The marker rides on the step element where the operator appears, not on the receiver name.

## Why nested rather than flat

The first proposal was a FLAT shape with sibling chain segments under a single wrapper. After comparing query patterns side-by-side, NESTED won on the strength of **declaration-call query symmetry**:

| Query | Declaration | Chain (nested) | Chain (flat) |
|---|---|---|---|
| Specific path | `//class[name='Foo']/method[name='bar']` | `//object[access][name='console']/member[name='stdout']/call[name='write']` | `//chain[name[1]='console' and name[2]='stdout' and call/name='write']` |
| Receiver match | `//class[name='Foo']` | `//object[access and name='Foo']` | `//chain[name[1]='Foo']` |
| Middle pattern | n/a | `//member[name='foo']/member[name='bar']` | `//chain[name='foo']/following-sibling::*[1][.='bar']` |

The nested form lets a developer write XPath against chain expressions exactly the way they write it against declarations. Tractor's whole rule library benefits from one consistent navigation idiom rather than two.

The trade-off is that depth queries (Law-of-Demeter detection) become `count(.//member | .//call | .//subscript) >= 3` instead of FLAT's `count(*) >= 3`. Acceptable cost: depth queries are written once into a rule library; path-matching queries are written every time someone authors a rule.

## Per-step markers carry information; `[access]` lives only at the root

The `[access]` marker on the `<object>` wrapper says "this is a member-access chain" — distinguishing it from object literals (TS `{a: 1}`), which share the `<object>` element name. **Per-step markers (`<member>`/`<call>`/`<subscript>`) only exist when they carry information that the root marker doesn't already imply.**

Examples of per-step markers that DO carry information:
- `<member[optional]>` / `<call[optional]>` — `?.` short-circuits on null. The semantic is per-step, not chain-wide (`a.b?.c.d` has only one optional step).
- `<call[nullsafe]>` — PHP `?->` (same idea, different operator).

Examples of per-step markers that should NOT exist (rejected as redundant):
- `<member[access]>` / `<call[access]>` — would just repeat the root.
- `<member[instance]>` (C#/PHP, rejected) — would be added unconditionally to every member-access derived from the `member_access_expression` tree-sitter kind, but since static-vs-instance is not actually distinguished (both syntactic forms get the marker), the marker carries no information. The chain-root `[access]` already says "this is access," and the element name `<member>` already says "this is `.`-style."

The general rule: **add a per-step marker only when there's a meaningful syntactic alternative that lacks it.** `[optional]` qualifies (some steps are nullable, others aren't). `[instance]` did not (every step is equally "instance" in the marker sense).

## Element name: `<object[access]>`

Distinct from existing `<path>`:
- `<path>` — compile-time namespace lookup (`com.example.Foo`, `os::env`, `App\Models\User`). Each segment is purely declarative.
- `<object[access]>` — runtime member-access / method-call sequence. Each segment may have effects (calls, subscripts, optional checks).

Keeping them separate avoids forcing one element to carry both meanings — Principle #5 (Unified Concepts within a language) and Principle #11 (Specific Names Over Type Hierarchies).

## Cascades (Dart) — future extension, not implemented

> **Status: design only.** Cascades are *not* part of the current
> rollout. This section exists so the chain shape is forward-
> compatible: when Dart joins the supported languages, the
> extension below can land without redesigning what's already
> shipped. The 8 IR-supported languages (TS, Python, Java, C#, Go,
> Rust, Ruby, PHP) only have linear chains and don't need any of
> this.

Most languages have linear chains. Dart's cascade operator `..` breaks the assumption that "step N operates on step N-1's result" — every cascade step operates on the **same receiver**, the leftmost expression. The whole cascade expression evaluates to that receiver.

`paint..color = c..strokeCap = s` is semantically:
```dart
paint.color = c; paint.strokeCap = s; /* return */ paint;
```

The nested chain shape would be wrong for cascades because each step is *not* the result of the previous one. The clean extension is a `<cascades>` wrapper holding sibling steps — they're independent operations on the same receiver:

```xml
<object>
  <access/>
  <call><name>Paint</name></call>
  <cascades>
    <member><name>color</name><assign>c</assign></member>
    <member><name>strokeCap</name><assign>s</assign></member>
  </cascades>
</object>
```

For mixed cascade and normal chain `obj..a().b..c()..d()` (cascade `a()` on obj, then normal `.b` access on obj, then cascades `c()` and `d()` on `obj.b`):

```xml
<object>
  <access/>
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
</object>
```

`<cascades>` blocks are siblings of regular chain steps. They compose with the rest of the spine.

When Dart arrives, the Dart `lower_dart_root` will need to recognise `..` and emit a `<cascades>` segment, consuming consecutive cascade operators into one wrapper. The design is captured here so the eventual implementer knows the target shape.

## Implementation notes (IR lowering)

The shape above is produced directly by each language's `Ir::Access { receiver, segments: Vec<AccessSegment> }` construction in its `lower_<lang>_root`. There is no separate transform pass.

`AccessSegment` (`tractor/src/ir/types.rs`) variants:

- `Member { name, optional }` — `.foo`, `?.foo`.
- `Call { name, args, optional }` — `.foo(args)`, `?.foo(args)`. `name = None` is the result-invocation case.
- `Subscript { index, optional }` — `[expr]`, `?.[expr]`.

The left-deep emission is mechanical: walk the right-deep CST shape from the outermost node inwards, push segments to a `Vec`, then construct `Ir::Access { receiver, segments }` with the segments in source order. `to_xot` translates each segment into the nested `<member>` / `<call>` / `<subscript>` step element in the standard way.

### Useful-chain guard

`Ir::Access` is constructed only when there is at least one access step. A bare identifier `a` lowers to `Ir::Name`, not `Ir::Access { receiver, segments: [] }`. A lone top-level `Ir::Call` with no name (e.g. `f(args)`) also stays as `Ir::Call`, not `Ir::Access`. Wrapping these would add noise without informational value.

### Source-location threading

- `<object[access]>` inherits `line`/`column`/`end_line`/`end_column` from the receiver (the leftmost source token).
- Each step element inherits from its primary node — the access name for `<member>`, the method name for `<call>`, the index expression for `<subscript>`.
- For result-invocation `<call>` segments (no `name`), the step has no source location attached automatically.

### Test coverage

- Per-language IR lowering tests: `tractor/tests/ir_<lang>_parity.rs`, `tractor/tests/ir_<lang>_missing_kinds.rs`.
- Cross-language uniformity: `tractor/tests/cross_language_index_access_chain_inverts.rs` exercises subscript-in-chain across the 7 IR languages (TS, Python, Java, C#, Go, Rust, Ruby, PHP) and pins them to the same shape.
- Per-language snapshot fixtures under `tractor/tests/fixtures/` cover the chain shape in real code.

## References

- `specs/tractor-parse/tree/design.md` — Principles #5, #11, #15; § "Hierarchical access nests top-down" (the high-level decision).
- `tractor/src/ir/types.rs` — `Ir::Access` + `AccessSegment` variants.
- `tractor/src/ir/<lang>.rs` — per-language lowering that constructs `Ir::Access` directly from right-deep CST.
