# Derive auto-fix suggestions from rule `valid`/`invalid` examples

## Context

Rules in Tractor already carry paired `valid` and `invalid` example
snippets (see `tractor/tractor-lint.yaml` for real examples). Today
these are used only for documentation and for verifying that a rule's
XPath matches the invalid case and not the valid case.

If the two examples are a **minimal pair** — i.e. they differ in one
localized structural way — we have enough information to derive an
**auto-fix patch**: when the rule fires against user code, we can
suggest (or apply) the transformation that turns the invalid shape
into the valid shape.

## Core insight

**We do not need a custom AST diff algorithm.** The diff IS a sequence
of Set operations:

```
diff(before_ast, after_ast) = Vec<SetOp> such that
    apply(before_ast, diff) == after_ast
```

Set already knows how to mutate an AST (`xpath_upsert::upsert_typed`)
and can insert missing nodes. Render already knows how to serialize a
(possibly-modified) AST back to source (`render::render`). The only
missing piece is the "find-where-they-differ" walker that emits the
SetOp sequence.

## Pipeline

1. **Parse** both example snippets via
   `parse_string_to_documents()` → two AST trees (invalid, valid).
2. **Walk in parallel**. Where subtrees are structurally equal,
   continue. Where they diverge, emit a SetOp whose xpath selects the
   divergence point in the invalid tree and whose value is the
   corresponding valid subtree (serialized via `render()` on the
   sub-tree).
3. **Abstract placeholders**. Identifier-kind leaves appearing inside
   SetOp values are replaced with stable slots (`$1, $2, …`) based on
   first-seen order across both sides. Identifiers that occur on both
   sides map to the same slot — this is what lets the patch generalize
   to user code that uses different variable names.
4. **Attach to rule**. The resulting `Vec<SetOp>` (with placeholders)
   becomes the rule's `fix` field. When the rule fires on user code,
   each SetOp's xpath is rebased relative to the match node, the
   placeholders are bound to the user's actual identifier bindings at
   that site, and `upsert_typed` + `render` produce the suggested
   edit.

## Requirements on the examples

For this to work, a `valid`/`invalid` pair must be:

- **Parseable** in the same language. (Both must parse cleanly — a
  failed parse can't be diffed.)
- **Minimally different.** The user's job, when writing examples, is
  to make the valid case differ from the invalid case *only* in the
  way the rule wants to enforce. Add/remove surrounding boilerplate
  only as needed to make both compile/parse.
- **Localized.** The divergence should live in a single subtree, or a
  small number of sibling subtrees under a common parent. Wholesale
  restructurings (e.g. traditional constructor → primary constructor)
  are borderline and may produce a patch that replaces a large
  ancestor rather than a surgical edit.
- **Placeholder-consistent.** Identifiers that should generalize
  (variable names, type names) should be written the same way in both
  sides where they refer to the same thing, and differently where
  they shouldn't cross-map. (E.g. in the "no null comparison" rule,
  the variable `foo` should appear on both sides.)

## Test cases to try (C#)

1. **always-use-braces** — `if (x) return;` → `if (x) { return; }`.
   Expect a single SetOp wrapping the `if_statement`'s body child into
   a `block`. Small, localized, clean case.

2. **no-null-comparison** — `if (foo == null) throw new ...();` →
   `foo.IsNotNullOrThrow();`. The whole `if_statement` becomes an
   `expression_statement`. Expect one SetOp replacing the statement.
   `foo` must bind to the same slot on both sides.

3. **primary-constructor** — traditional constructor + backing field
   → primary constructor. Multiple coordinated changes (class
   declaration gains params, constructor node is removed, field
   declaration is removed). Stresses the algorithm — may produce a
   "replace class body" patch rather than three surgical edits. Good
   limit case to document.

## Renderer coverage is the real gating concern

The pipeline only works if `render()` can serialize every node type
that might appear on either side of a diff. The C# renderer today
(`tractor-core/src/render/csharp.rs:13-28`) only handles
**class-level structural nodes**: `class`, `struct`, `property`,
`field`, `unit`, `namespace`, `import`, `comment`. Methods are not
rendered, let alone method bodies. Anything inside a method body
(statements, expressions) falls through to
`RenderError::UnsupportedNode`.

Mapping the three test cases against that gap:

| Test case | Additional node kinds the renderer must support |
|---|---|
| always-use-braces | `method_declaration`, `parameter_list`, `parameter`, `block`, `if_statement`, `return_statement`, `binary_expression`, `identifier` |
| no-null-comparison | all of the above + `expression_statement`, `invocation_expression`, `member_access_expression`, `throw_statement`, `object_creation_expression` |
| primary-constructor | `constructor_declaration`, `primary_constructor_base_type`, `base_list`, `assignment_expression` |

In other words, to make this usable on realistic C# rules the
renderer needs roughly full statement- and expression-level coverage.
That is a substantial subproject on its own — likely larger than the
diff walker.

**Scoping options**, rough ordering from cheapest to most ambitious:

1. **Round-trip shortcut for unchanged subtrees.** When the diff
   walker decides a subtree on the valid side is the "replacement
   value" for a SetOp, it could serialize it not by calling
   `render()` but by slicing the original source text using the
   subtree's byte span (the span is already tracked during parsing).
   This completely sidesteps the rendering problem for subtrees that
   came verbatim from the user's example — which is the common case
   when examples are minimal pairs. The renderer only needs to be
   invoked if the diff synthesizes a new subtree, which this
   algorithm doesn't do.
2. **Incremental renderer growth driven by tests.** If we *do* need
   the renderer (e.g. because we want the patch to emit canonically
   formatted C# rather than whatever the example used), grow the
   renderer one node kind at a time, with each rule's auto-fix test
   pulling in the next batch.
3. **Full statement/expression renderer.** Only commit to this if
   the source-slicing shortcut turns out to be insufficient (e.g.
   placeholder substitution means we can't just slice — we need to
   regenerate with identifier replacements).

Option (1) is probably the right POC path: it lets us prove the diff
walker and patch-apply loop work before we sink effort into renderer
expansion. Placeholder substitution can still be done by text-level
splicing inside the sliced source, since tree-sitter gives us spans
down to individual identifiers.

## Proof-of-concept plan

A minimal POC, before renderer work:

1. Take the "always-use-braces" example pair (smallest, cleanest
   minimal pair).
2. Parse both sides via `parse_string_to_documents`.
3. Implement a parallel walker that returns `Vec<DiffPoint>` where
   `DiffPoint = { invalid_xpath, valid_subtree_span }`.
4. For each diff point, slice the valid subtree directly out of the
   valid-side source via the byte span, and construct a SetOp whose
   value is that slice.
5. Apply the SetOps to a *third* invalid snippet (not the example —
   real user code) and check the result matches an expected output.
6. Only then tackle placeholder abstraction and renderer expansion.

## Open questions / risks

- **Removals.** Set supports upsert but not delete. Removals have to
  be modeled as "replace the smallest common ancestor that contains
  only the deletion with a version that lacks the deleted subtree."
  Works but produces larger patches.
- **How to serialize a SetOp value that is a subtree, not a scalar?**
  Current `SetValue` is `String | Number | Boolean | Null`. We likely
  need a `SetValue::Subtree(XmlNode)` variant (or a separate
  `ReplaceOp`) to carry structural replacements.
- **Match rebasing.** The patch's xpath is relative to the invalid
  example's root. When the rule fires against user code, we need to
  rebase xpaths relative to the match node. Tractor's XPath engine
  can handle relative paths — confirm this works.
- **Placeholder binding at apply-time.** When the rule matches user
  code, how do we know which user identifier fills `$1`? The simplest
  answer: the rule's XPath must bind placeholders as captures (named
  XPath variables), and the fix template references them.
- **Multi-point divergence.** Several independent changes under a
  common parent should stay as separate SetOps, not collapse into one
  "replace the parent" op. The walker needs to recurse past a
  divergent node only when all its children are individually
  reconcilable.

## Why this matters

- **Rules become self-documenting fixes.** Writing a good
  `valid`/`invalid` pair is already expected of rule authors; that
  investment now also powers an auto-fix.
- **Closes the AI guardrailing loop (todo/26).** Combined with AST
  delta monitoring, rules could be *learned* from observed edits AND
  carry the fix derived from the same delta — no separate authoring
  step.
- **Keeps the implementation small.** No new AST diff engine, no new
  transformation DSL. Just a parallel walker on top of the existing
  set+render machinery.

## Related

- `tractor-core/src/declarative_set.rs` — SetOp, SetValue, parse_set_expr
- `tractor-core/src/xpath_upsert.rs` — upsert_typed (core mutation)
- `tractor-core/src/render/mod.rs` — render, render_with_spans
- `tractor/src/executor.rs:696-837` — existing parse → set → render pipeline
- `tractor/tractor-lint.yaml` — real valid/invalid examples to test against
- todo/26 — AST delta monitoring (the upstream "learn rules from edits" idea)
