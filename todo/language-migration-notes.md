# Language migration session notes

Running log of decisions and dilemmas during the autonomous
language-migration sweep. Created 2026-04-29.

Branch: `claude/simplify-node-names-SmonS`. Plan:
`C:\Users\Bouke\.claude\plans\all-good-but-the-binary-quasar.md`.

## Migrations completed before this session

- **Go** — full pilot (commits up through `479e90c`).
- **C#** — propagation (`ca45d9a` through `3b70b77`).
- **Architecture rename** — `kind.rs` → `input.rs`, `semantic.rs`
  → `output.rs`, extracted `rules.rs`, moved wrapper handlers
  to `transformations.rs` (`403b3a5`, `47a27a1`, `a8a7d54`).

## Migration ordering for this session

User requested order: java, php, python, rust, typescript, ruby.
I'll follow that. Compaction attempt after each language.

## Decisions and dilemmas

### Compaction trigger mechanism (resolved)

`/compact` is a built-in CLI command, not a skill — I cannot
invoke it. Trying via `Skill` tool returns an error explicitly
saying so. The user must run `/compact` themselves.

Workflow: at each clean break point (after a language migration
completes), I'll signal to the user that this is a good moment.
They run `/compact` if they want; otherwise I continue.

### TODO follow-up commits (running rule)

For each language, the plan template Step 7 is "optional" —
walking the rule table and grouping passthrough kinds with TODO
comments for future semantic upgrades. I'll do this for every
language since it's a small, focused commit and improves
discoverability of where the language has gaps.

### Promotion of `Rule::DefaultAccessThenRename` (anticipated)

C# has 9 declaration kinds sharing the "default-access marker
then rename" shape. Java will be the second user. The plan says
to promote `Rule::DefaultAccessThenRename` when Java migrates.

The variant body needs:
- the rename target (`&'static str`)
- a function pointer to determine the default access modifier
  (language-specific: depends on parent kind)
- a function pointer to check if an access modifier already exists

If this lands during Java's Step 3, it'll also refactor C#'s
rules.rs to use the new variant. Single promotion commit between
the two languages.

If it turns out Java's defaults are different enough that the
shape doesn't share cleanly, I'll keep both languages on per-kind
Custom helpers and revisit during the next language.

## Per-language progress log

### Java — COMPLETE

Commits (chronological):

- `0e74ad5` — Step 1: generate JavaKind enum (147 kinds).
- `0dce0ea` — Step 2: validate catalogue against JavaKind, drop 2
  dead entries (`else_clause`, `field_declaration_list`).
- `f7aeea5` — **Promotion**: add `Rule::DefaultAccessThenRename`
  variant, refactor C# to use it (replaces 9 per-kind Custom stubs
  with `da(XXX)` shorthand).
- `118cec8` — Step 3: rules.rs + transformations.rs.
- `34f2fad` — Step 4: swap dispatcher to rule()-driven (deletes
  401 lines from transform.rs).
- `833bd55` — Step 5: drop KINDS / rename_target.
- `5c98ee8` — Step 6: rename semantic.rs → output.rs.

Step 7 (TODO follow-up): TODOs were inlined into rules.rs during
Step 3 commit, grouped by theme (modules, annotation-types,
patterns, special-statements, try-with-resources, casts,
instanceof, update_expression, literal-not-yet-renamed, dimensions,
template-strings, annotated-types, misc structural). No separate
TODO commit needed.

#### Java-specific notes

- Java's `Modifiers` kind is a single text-bearing wrapper (vs
  C#'s individual `Modifier` nodes). The transformation walks the
  text content, splits on whitespace, and lifts known keywords as
  empty markers — same idea as C# but a different shape.
- Java's `default_access_for_declaration` resolver returns
  `Some(PUBLIC)` inside an interface declaration, `Some(PACKAGE)`
  otherwise — but only if the node has no `<modifiers>` child.
  When a `<modifiers>` child IS present, the modifiers handler
  itself inserts `<package/>` if no access keyword appeared.
- Method declarations don't fit the variant cleanly — Java's
  grammar tags the return type with `field="type"` (same as
  parameter types), so the builder can't wrap it generically.
  The Custom `method_declaration` handler does default-access +
  return-type wrapping + rename.

### PHP
(not started)

### Python
(not started)

### Rust (`rust_lang`)
(not started)

### TypeScript
(not started)

### Ruby
(not started)
