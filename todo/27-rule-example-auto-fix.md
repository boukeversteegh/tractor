# Derive auto-fix suggestions from rule `valid`/`invalid` examples

## Context

Rules in Tractor already carry paired `valid` and `invalid` example
snippets (see `tractor/tractor-lint.yaml`). Today these are used for
documentation and for verifying that a rule's XPath matches the
invalid case but not the valid case.

If the two examples are a **minimal pair** — i.e. they differ in one
localized structural way — we have enough information to derive an
**auto-fix**: when the rule fires against user code, we suggest (or
apply) the transformation that turns the invalid shape into the
valid shape.

## POC status

A working proof of concept lives at
`tractor-core/tests/ast_diff_poc.rs`. It:

- parses both examples via `parser::parse_string_to_xot` with
  `TreeMode::Raw`,
- walks the two xot trees in parallel using
  `xot_transform::helpers`,
- records each divergence point as `(path, before_kind, after_kind,
  before_text, after_text)` where the text fields are slices of the
  original source located via the `line`/`column`/`end_line`/
  `end_column` attributes the builder already attaches.

Three passing tests on the motivating C# examples:

| Rule | Divergence | Quality |
|---|---|---|
| always-use-braces | `return_statement` → `block` | surgical, clean |
| no-null-comparison | `if_statement` → `expression_statement` | surgical, clean |
| primary-constructor | entire `class_declaration` replaced | coarse — documented limit case |

## Two candidate architectures

The POC proved the **diff walker** works. The remaining question is
how the derived patch is represented and applied. Two distinct
architectures:

### A. Source-splicing auto-fix (what the POC does)

The AST walk *locates* divergences; the AFTER text is a byte-range
slice of the valid example's source; at apply-time we splice that
text into the user's source at the corresponding location.

Pros:
- No renderer work required. The slice *is* the output.
- Formatting preserved exactly as the rule author wrote it.
- Matches how production linters do auto-fix
  (ESLint's `fixer.replaceTextRange`, Clippy suggestions, etc.).
- Fast; multi-point patches compose cleanly if applied in reverse
  byte order.

Cons:
- No structural guarantee — a bad placeholder substitution can
  produce syntactically invalid source. Needs a re-parse as a
  sanity check.
- Inherits the example's whitespace style; may clash with user's
  formatting. (Mitigation: run the formatter after the fix; most
  ecosystems do.)
- Not composable as a first-class object; the "patch" is just a
  (range → replacement-text) tuple.

### B. SetOp-based auto-fix

Derive a `Vec<SetOp>` that transforms the invalid AST into the valid
AST; store it as structured data; apply it via `xpath_upsert` and
`render`.

Pros:
- Patches are first-class, serialisable, composable objects.
- Structural validity is guaranteed by construction.
- Fits the existing `tractor set` + `render` infrastructure cleanly.
- Usable outside of auto-fix (e.g. codemods, batch refactors).

Cons:
- Requires `SetValue::Subtree(...)` (or a new `ReplaceOp`) —
  current SetValue is scalar-only.
- Requires the renderer to cover every node kind that can appear
  on either side of a diff. For C# that means roughly full
  statement- and expression-level coverage (see next section).
- More moving parts overall.

### Recommendation

**Ship A first.** It's what the POC proved; it doesn't block on
renderer work; it covers the auto-fix use case realistically. B is
the right destination if/when patches need to be composable across
rules or used as codemods. Don't couple the two — a rule can have
both a splicing-style fix (shipped today) and a SetOp-style fix
(added later) without either blocking the other.

## The renderer gap (applies only to architecture B)

Current C# renderer (`tractor-core/src/render/csharp.rs:13-28`)
covers only class-level structural nodes: `class`, `struct`,
`property`, `field`, `unit`, `namespace`, `import`, `comment`.
Methods, statements, and expressions all fall through to
`RenderError::UnsupportedNode`.

| Rule | Extra node kinds needed in the renderer |
|---|---|
| always-use-braces | `method_declaration`, `parameter_list`, `parameter`, `block`, `if_statement`, `return_statement`, `binary_expression`, `identifier` |
| no-null-comparison | above + `expression_statement`, `invocation_expression`, `member_access_expression`, `throw_statement`, `object_creation_expression` |
| primary-constructor | `constructor_declaration`, `primary_constructor_base_type`, `base_list`, `assignment_expression` |

So architecture B effectively requires a full statement/expression
renderer for each target language — a substantial subproject.
Architecture A avoids this entirely.

## Placeholder complexity: three tiers

The diff walker gives us BEFORE/AFTER text slices. To generalize to
real user code, identifier tokens inside those slices need to vary.
There are three progressively harder cases:

### Tier 0 — Literal patches (no placeholders)

The AFTER text has no identifiers that depend on user code.
Example: `deserialize-deny-unknown-fields` — the fix is always
literally `#[serde(deny_unknown_fields)]`. Works today.

### Tier 1 — Bounded-identifier correspondence

Identifier tokens appearing in the *same* position on both sides of
the minimal pair are bound together. At apply time, we read the
user's token at that position in the invalid match and substitute
it into the AFTER text.

Example: `no-null-comparison` — `foo` appears on both sides. At
apply time, user writes `if (customer == null) throw …;`; the rule
binds `$1 := customer`, the AFTER text `$1.IsNotNullOrThrow();`
becomes `customer.IsNotNullOrThrow();`.

Mechanics: for each identifier leaf in the BEFORE slice, record its
relative byte range within the slice. At apply time, read the
user's text at those ranges and rewrite the AFTER slice accordingly.
No template engine needed — it's ranges, not string matching.

This tier handles the three motivating examples and most real
lint-style rules.

### Tier 2 — Derived identifiers (textual transformation)

The AFTER text contains an identifier *computed from* the BEFORE
identifier — not equal to it. Examples:

- `serde-field-rename-dashes` — field `tree_mode` becomes a rename
  attribute `"tree-mode"` (underscore → dash).
- `field-to-getter-method` — field `Name` becomes method `GetName`.
- `rename-interface-i-prefix` — class `Foo` becomes interface `IFoo`.

No amount of identifier-correspondence bookkeeping captures these —
the AFTER text is *computed*, not *matched*.

#### Reuse XPath itself as the transformation language

We don't need a new template DSL. Tractor already ships an XPath 3.1
engine (xee), which has:

- maps (`map { "k": v, ... }`) and higher-order functions
  (`map:for-each`, `map:merge`),
- string manipulation (`replace()`, `upper-case()`,
  `lower-case()`, `substring()`, `concat()`, `string-join()`,
  `tokenize()`),
- path navigation and variable bindings against a match context.

Express each placeholder as an **XPath expression evaluated against
the match node**. Bindings live in a map:

```yaml
fix:
  template: '#[serde(rename = "$1")]'
  bindings:
    $1: replace(@name, '_', '-')
```

This one mechanism covers every tier:

| Tier | Binding expression | Example |
|---|---|---|
| 0 | literal constant | `$1: 'dimensions'` |
| 1 | identity / direct reference | `$1: @name` |
| 2 | derived via XPath string fn | `$1: replace(@name, '_', '-')` |

No new language, no new engine, no new interpreter. The fix reuses
the same XPath that already runs for matching.

#### Implications for auto-derivation

The diff walker can auto-fill the bindings map for Tiers 0 and 1:

- A leaf with no corresponding identifier on the other side → emit
  a literal binding.
- A leaf corresponding to an identifier at a mirrored position →
  emit an identity binding like `@name` (or an indexed path into
  the match context).

Tier 2 is the only case that requires manual work from the rule
author, and the work is small: replace the auto-emitted identity
binding with an XPath string expression. The rest of the fix
(template, location, everything else) is still auto-generated.

**Revised scope decision for v1:** auto-generate Tiers 0 and 1;
emit a stub binding map for Tier 2 cases with a warning that the
author should replace the identity expressions with a real
transformation. Do not ship Tier 2 fixes without author sign-off
(avoid silently-wrong auto-fixes).

## Requirements on the examples

- **Parseable.** Both sides must parse cleanly.
- **Minimally different.** Valid differs from invalid *only* in the
  way the rule enforces. Add surrounding boilerplate only as needed
  to make both parse.
- **Localized.** Divergence lives in a single subtree or a small
  number of sibling subtrees. Whole-sale restructurings produce
  coarse patches.
- **Bounded identifiers (Tier 1) auto-generate cleanly.** If the
  transformation derives a new name from an existing one (Tier 2),
  the rule author replaces the auto-emitted identity binding with
  an XPath transformation (e.g. `replace(@name, '_', '-')`).

## Open questions

- **Placeholder ranges across multi-line slices.** When the BEFORE
  text spans multiple lines, identifier sub-ranges need to be
  tracked as (line, col)→(line, col) not flat byte offsets. Cheap
  to do; just needs care.
- **Overlapping diff points.** Multiple adjacent divergences under
  one parent: apply in reverse byte order. Easy.
- **Re-parse validation.** After splicing, re-parse the user's file
  and verify no new parse errors appeared. Cheap safety net.
- **How to surface fixes in the report.** Need a `suggestion` field
  on `ReportMatch` carrying the byte range and replacement text.
- **Multi-file / multi-match fixes.** A rule may fire at many sites
  in a file. Fixes must be collected and applied in reverse order
  to avoid byte-offset invalidation.

## Why this matters

- **Rules become self-documenting fixes.** The investment rule
  authors already make in writing good examples now also powers the
  fix.
- **Closes the AI guardrailing loop (todo/26).** Combined with AST
  delta monitoring, rules can be *learned* from observed edits AND
  carry their own fix derived from the same delta.
- **Keeps the implementation small.** No DSL, no template engine,
  no renderer expansion. A parallel walker + byte-range splicing +
  re-parse guard.

## Related

- `tractor-core/tests/ast_diff_poc.rs` — working POC
- `tractor-core/src/parser/mod.rs` — parse pipeline
- `tractor-core/src/xot_transform.rs` — tree helpers
- `tractor-core/src/declarative_set.rs` — SetOp (relevant only for architecture B)
- `tractor-core/src/xpath_upsert.rs` — upsert_typed (architecture B)
- `tractor-core/src/render/mod.rs` — render (architecture B)
- `tractor/tractor-lint.yaml` — real valid/invalid examples
- todo/26 — AST delta monitoring (upstream "learn rules from edits" idea)
