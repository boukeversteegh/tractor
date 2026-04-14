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
