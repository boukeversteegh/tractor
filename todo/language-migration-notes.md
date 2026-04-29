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

### PHP — COMPLETE

Commits (chronological):

- `49a1ce1` — Step 1: generate PhpKind enum (159 kinds).
- `b278333` — Step 2: validate catalogue against PhpKind, drop 6
  dead entries (`anonymous_function_creation_expression`,
  `class_modifier`, `elseif_clause`, `exit_intrinsic`,
  `formal_parameter`, `type_cast_expression`).
- `9de906f` — Step 3: rules.rs + transformations.rs (uses
  promoted `Rule::DefaultAccessThenRename` for method + property,
  PHP's class members default to public).
- `5ef3dbe` — Step 4: swap dispatcher to rule()-driven (deletes
  292 lines from transform.rs).
- `4c91b40` — Step 5: drop KINDS / rename_target.
- `07f7e9f` — Step 6: rename semantic.rs → output.rs.

Step 7 TODOs were inlined into rules.rs during Step 3 (10+
groupings: nullsafe operators, heredoc/nowdoc, intersection-types,
anonymous-class, update/augmented assignment, special-statements,
php-specific values like error_suppression / shell_command,
declaration variants, cast-type / dynamic-variable / text,
traits-use clauses, property-hooks).

#### PHP-specific notes

- **PHP grammar exposes `PHP_NODE_TYPES`** (not `NODE_TYPES`) at
  the crate root since the crate ships two grammars (PHP and
  PHP_only). The `LangCodegen` struct already takes any
  `&'static str`, so no infrastructure change needed.
- **PHP's `default_access_for_declaration` is the simplest of the
  three users so far** (C#, Java, PHP): always returns
  `Some(PUBLIC)` when no visibility marker is present. Class
  members default to public regardless of enclosing scope.
- **PHP's `name_wrapper` is more involved than other languages**:
  variable_name (`$foo`) and qualified_name (`App\Hello\Greeter`)
  need distinct inlining/flattening logic. Single-element child
  with namespace_name / qualified_name kind triggers Flatten so
  segments hoist to the enclosing namespace/use. Multiple element
  children also Flatten (qualified-name segments + separators).
- **PHP has many TODO passthroughs** (~50) — the grammar has lots
  of niche kinds (heredoc/nowdoc, anonymous class, nullsafe
  operators, property hooks, error suppression, shell command,
  intersection types, etc.). Each is a small isolated PR.

### Python — COMPLETE

Commits (chronological):

- `acd6729` — Step 1: generate PyKind enum (127 kinds initially).
- `d51f8f5` — **gen-kinds improvement**: also recurse into
  `fields.*.types` and `children.types`. PyKind grew to 128
  (added `as_pattern_target`, which only appears in
  `as_pattern.fields.alias.types`).
- `5e8cb4c` — Step 2: validate catalogue, drop 2 dead entries
  (`async_function_definition`, `async_if_clause` — both absorbed
  into the regular function/if kinds with text-level async).
- `51a4101` — Step 3: rules.rs + transformations.rs.
- `8b8b516` — Step 4: swap dispatcher (deletes 486 lines from
  transform.rs).
- `a07b341` — Step 5: drop KINDS / rename_target.
- `7f7d698` — Step 6: rename semantic.rs → output.rs.

#### Python-specific notes

- **Codegen needed extension** to also recurse into
  `fields.*.types` and `children.types` arrays. Some kinds (like
  Python's `as_pattern_target`) appear only in those nested type
  lists — never declared at the top level or in `subtypes`.
  Without this, the typed enum misses kinds the parser actually
  emits, and the new dispatcher's `<Lang>Kind::from_str` returns
  None for them. The fix benefits all future languages too;
  Go/C#/Java/PHP unchanged.
- **No `Rule::DefaultAccessThenRename` use** — Python uses
  underscore convention for visibility (encoded into the
  `function_definition` Custom handler, not declarative). Class
  members still get `<public/>` / `<protected/>` / `<private/>`
  markers via the function_definition handler walking
  `is_inside_class_body`.
- **Decorated_definition is a Custom handler** that hoists
  `@decorator` children INTO the inner class/function so the
  cross-language topology (`<class><decorator/>...`) holds. Then
  the wrapper flattens.
- **Collection construction is exhaustive (Principle #9)**:
  `<list>` always has `<literal/>` or `<comprehension/>`;
  same for `<dict>` and `<set>`. The handlers prepend the marker
  before any rename.
- **f-string interpolation kinds are passthrough TODO** —
  `format_expression`, `escape_interpolation`, etc. The catalogue
  didn't handle them either; they survive as raw kind names.

### Rust (`rust_lang`)
(not started)

### Rust (`rust_lang`)
(not started)

### TypeScript
(not started)

### Ruby
(not started)
