# Chain Inversion Design

## Background

Per user feedback (mid-iter-231 conversation):

> "the syntax actually prefers a structure where the last node is sort
> of on the root level, but everything before is a sort of deeply
> nested hierarchy. This is quite different from how programmers
> probably conceptually think about it. They probably think of the
> first node in the chained call to be the root, and the second one
> to be a child of the first one. So the hierarchy is pretty inverted
> ... I want you to think very deeply about if it's possible to invert
> the hierarchy or to construct the hierarchy in such a way that
> basically chained calls work like path segments. that means that
> the call chain will look similar to the object declaration chain
> so for example a class called foo with a method called bar would
> be defined as class slash method with a name bar etc so the method
> is in the class so the call chain should reflect that as well"

## Current shape (right-deep)

For `a.b.c.d()` (TypeScript example, sampled iter 232):

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

The outermost element (`<call>`) corresponds to the LAST source token
(the invocation). The deepest leaf (`<name>a</name>`) corresponds to
the FIRST source token (the receiver). This reflects operator
precedence — left-associative `.` binds tighter than `()`.

The user's observation: developers reading source `a.b.c.d()` think
"start with `a`, then access `.b`, then `.c`, then call `.d()`" —
left-to-right, root-then-children. The current right-deep shape
inverts this; the developer has to mentally unwind.

## Target shape — flat chain

For `a.b.c.d()`:

```
chain/
  ├─ name = "a"
  ├─ name = "b"
  ├─ name = "c"
  └─ call/
      └─ name = "d"
```

For `a.b().c.d()`:

```
chain/
  ├─ name = "a"
  ├─ call/
  │   └─ name = "b"
  ├─ name = "c"
  └─ call/
      └─ name = "d"
```

For pure access `a.b.c`:

```
chain/
  ├─ name = "a"
  ├─ name = "b"
  └─ name = "c"
```

For optional chaining `a?.b?.c()`:

```
chain/
  ├─ name = "a"
  ├─ name[optional] = "b"
  └─ call[optional]/
      └─ name = "c"
```

For subscript `a[0].b`:

```
chain/
  ├─ name = "a"
  ├─ subscript/
  │   └─ int = "0"
  └─ name = "b"
```

For type-cast in receiver `(x as Foo).b`:

```
chain/
  ├─ cast/
  │   ├─ name = "x"
  │   └─ type/name = "Foo"
  └─ name = "b"
```

For result-of-call `f()(args)`:

```
chain/
  ├─ call/
  │   └─ name = "f"
  └─ call/
      └─ args...
```

## Why FLAT and not nested

Two candidate shapes were considered:

1. **Nested (each link as parent of next)**:
   ```
   <member name="a">
     <member name="b">
       <member name="c">
         <call name="d"/>
       </member>
     </member>
   </member>
   ```
2. **Flat (single wrapper, sibling segments)** — chosen.

The user phrased the desire two ways:
- "chained calls work like **path segments**" (suggests flat)
- "the method is in the class so the call chain should reflect that
  as well" (suggests nested)

Flat wins because:
- **Path-segment analogy is closer to language reality.** `<path>`
  already exists for namespace paths (iter 151-153 flattened
  right-deep paths to flat segments via `flatten_nested_paths`).
  Chain inversion is the same operation applied to runtime member
  access, just generalised to mix `<name>` and `<call>` segments.
- **XPath ergonomics.** `//chain/name[1]='a'` finds the receiver;
  `//chain/call[last()]/name` finds the terminal method. Nested
  forms would need axis traversal (`/descendant::call/name`).
- **Cardinality uniformity.** Each chain has 2..N segments; with
  list= on each segment, JSON renders `chain.names: [...]` as a
  uniform array.
- **Mid-chain calls compose cleanly.** `a.b().c.d()` is a flat
  list of 4 segments (name, call, name, call). Nested form would
  need to encode call vs access at each level, mixing two element
  types in the spine.

The class/method analogy still applies conceptually — "the method
is in the class" maps to "the call segment is in the chain
container", just laterally rather than vertically.

## Element-name choice: `<chain>`

Distinct from `<path>` because the semantics differ:
- `<path>` = compile-time namespace lookup (`com.example.Foo`,
  `os::env`, `App\Models\User`). Each segment is purely declarative.
- `<chain>` = runtime member-access / method-call sequence. Each
  segment may have effects (calls, subscripts, optional checks).

Keeping them separate avoids forcing one element to carry both
meanings, matching Principle #5 (Unified Concepts within a
language) and Principle #11 (Specific Names Over Type Hierarchies).

## Where this applies

Languages with member-access / method-call chains:
- TypeScript / JavaScript: `a.b.c()`, `a?.b`, `a[0].b`, `f()()`
- Python: `a.b.c()` — currently right-deep `<call><callee>...`
- Ruby: `a.b.c` — varies by grammar
- Java: `a.b.c()`, `array[0].field`
- C#: `a.b.c()`, `a?.b.Property`
- Go: `obj.Method()`, `m["key"].Field`
- Rust: `obj.method().field`, `vec[0].len()`
- PHP: `$obj->method()->prop`, `$arr[0]['key']`

Each language has its own grammar shape for chains; the inverter
runs per-language as a post-transform pass.

## Helper signatures (planned)

```rust
// Module: tractor/src/transform/chain_inversion.rs

/// One link in a chain.
pub enum ChainSegment {
    /// `.foo` — bare property access. Holds the name node and
    /// any markers (optional, non-null, etc.).
    Access { name_node: XotNode, markers: Vec<XotNode> },
    /// `.foo(...)` — method invocation. Holds name + args + markers.
    Call { name_node: XotNode, args: Vec<XotNode>, markers: Vec<XotNode> },
    /// `[expr]` — subscript.
    Subscript { index_node: XotNode },
    /// Anything else as a leftmost receiver (cast, parens, complex
    /// expression). Kept as-is.
    Root { node: XotNode },
}

/// Walk a right-deep `<member>`/`<call>` chain rooted at `node`
/// and extract segments in source order. Returns None if `node`
/// isn't a chain root.
pub fn extract_chain(xot: &Xot, node: XotNode) -> Option<Vec<ChainSegment>>;

/// Build a flat `<chain>` element with one child per segment,
/// inserted in place of the original chain. Detaches the original.
pub fn emit_flat_chain(
    xot: &mut Xot,
    original: XotNode,
    segments: Vec<ChainSegment>,
) -> Result<(), xot::Error>;

/// In-place: walk every chain root in `root` and invert it.
pub fn invert_chain_nesting(
    xot: &mut Xot,
    root: XotNode,
) -> Result<(), xot::Error>;
```

## Edge cases for unit tests

Before per-language adoption, the helper must pass tests covering:

1. Pure simple call `foo()` — no chain, no-op.
2. Standalone identifier `foo` — no-op.
3. Single member access `a.b` → `<chain><name>a</name><name>b</name></chain>`.
4. Single chained call `a.b()` → `<chain><name>a</name><call><name>b</name></call></chain>`.
5. Multi-link member chain `a.b.c.d` → 4 `<name>` segments.
6. Multi-link call chain `a.b.c.d()` → 3 names + 1 call.
7. Mixed mid-chain calls `a.b().c.d()` → name/call/name/call.
8. Optional chaining `a?.b?.c()` — `[optional]` markers preserved.
9. Type-cast in receiver `(x as Foo).b` — receiver is `<cast>`, not bare name.
10. Result-of-call invocation `f()(args)` — call on call.
11. Args at every chain link `a.b(1).c(2).d(3)` — args preserved per call.
12. Markers preserved on links (`!`, `?`, etc.).
13. Generic instantiation `a.b<T>.c()` (TS-like) — generics carried.
14. Subscript in chain `a[0].b` — `<subscript>` segment.
15. Empty chain root (no transform) — guard.
16. Already-flat chain (idempotent) — second invocation no-op.

## Iter plan

- **iter 232 (this iter)** — design doc (this file). No code.
- **iter 233** — skeleton module + ChainSegment + extract_chain + ~6
  unit tests for extraction.
- **iter 234** — emit_flat_chain + 6 unit tests for emission.
- **iter 235** — invert_chain_nesting wrapper + idempotency tests.
- **iter 236** — pilot one language (Java suggested — already flat
  at the call layer, smallest blast radius). Snapshot review.
- **iters 237-243** — per-language adoption (TS, Python, Ruby, C#,
  Go, Rust, PHP). One iter each.
- **iter 244** — render module update (per-language source
  reconstruction from `<chain>` shape).

## Open questions for the user

1. **Element name `<chain>` vs `<member>` vs `<path>`** — defaulting
   to `<chain>` for distinctiveness from compile-time `<path>`. Open
   to other names.
2. **Does the receiver `<root>` segment get its own wrapper?** —
   currently planned: receivers are bare children of `<chain>` (a
   `<name>` for simple roots, or a `<cast>`/`<paren>` for complex
   roots). Alternative: always wrap in `<receiver>`.
3. **Mid-chain non-call expressions** — `await a.b()` and similar
   where the chain is interrupted by a non-access operator. Plan:
   the chain ends at the operator boundary; the outer expression
   keeps its own host.
4. **Decision on what to do with the `<callee>` wrapper TS uses.** —
   Plan: drop it. The chain wrapper IS the callee context.
