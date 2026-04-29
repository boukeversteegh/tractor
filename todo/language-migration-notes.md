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

### Rust (`rust_lang`) — COMPLETE

Commits (chronological):

- `70701f0` — Step 1: generate RustKind enum (163 kinds). Includes
  codegen sanitizer for the `Self` keyword conflict (Rust grammar
  emits `self` kind → would PascalCase to `Self`, a reserved keyword;
  resolved by suffixing with `_` → `RustKind::Self_`).
- `dc72a34` — Step 2: validate catalogue, drop 8 dead entries
  (break_statement, continue_statement, method_call_expression,
  raw_string_literal_content, send_statement, slice_type,
  spread_element, trait_type — all renamed/removed in current
  grammar).
- `308ac42` — Step 3: rules.rs + transformations.rs.
- `1bdddf2` — Step 4: swap dispatcher (deletes 407 lines).
- `f8b2929` — Step 5: drop KINDS / rename_target.
- (Step 6) — Rename semantic.rs → output.rs.

#### Rust-specific notes

- **Codegen sanitizer for `Self` keyword**: tree-sitter Rust emits
  `self` as a kind. Snake-to-pascal would produce `Self`, which is
  a reserved Rust keyword and cannot be used as an enum variant
  identifier (raw identifiers `r#Self` are also reserved). Added
  a sanitizer that suffixes `Self` with `_`. Variant compiles as
  `RustKind::Self_`.
- **`Rule::DefaultAccessThenRename` for 8 declarations** (function,
  struct, enum, trait, const, static, type, mod) — Rust's default
  access is always `private` (no `pub` modifier means item-private).
  Simplest of the four users so far (C#, Java, PHP, Rust).
- **`visibility_modifier` is a complex Custom**: rebuilds the
  `<pub>` element with `<crate/>` / `<super/>` / `<in path>`
  restriction marker children, dangles the original source token
  as a sibling so string-value stays source-accurate.
- **`reference_type` Custom**: hoists `mut` from `mutable_specifier`
  child to a marker, prepends `<borrowed/>`, renames TYPE.
- **`name_wrapper` handles `lifetime` specially**: inlines lifetime's
  descendant text directly so `<name><lifetime>'a</lifetime></name>`
  becomes `<name>'a</name>` rather than triple-wrapping.

### TypeScript — COMPLETE

Commits (chronological):

- `aeee159` — Step 1: generate TsKind enum (initially 183 kinds
  from TYPESCRIPT_NODE_TYPES only).
- `a6761fc` — **gen-kinds multi-source extension**: change
  `node_types` (single string) to `node_types_sources` (slice of
  strings) so a language can union multiple grammars. TsKind
  grows to 192 kinds (covers TYPESCRIPT + TSX).
- `3d5ad30` — Step 2: validate catalogue, drop 4 dead entries
  (field_definition, readonly_modifier, string_start, string_end).
- `dfd9b1f` — Step 3: rules.rs + transformations.rs.
- `6dd9640` — Step 4: swap dispatcher. Caught a behavior
  divergence: `predefined_type` was `Rename(TYPE)` in catalogue
  but the old catch-all also wrapped TYPE-renamed text in
  `<name>` (Principle #14). Fixed by switching PredefinedType to
  `Custom(type_identifier)` in same commit.
- `229900e` — Step 5: drop KINDS / rename_target.
- `6fcb3b3` — Step 6: rename semantic.rs → output.rs.

#### TypeScript-specific notes

- **Multi-source grammar support** (gen-kinds): the typescript
  crate ships TYPESCRIPT_NODE_TYPES + TSX_NODE_TYPES grammars.
  Both dispatch through the same `typescript::transform`. Codegen
  now unions both so `TsKind::from_str` recognises every JSX
  kind too (jsx_element, jsx_attribute, etc.).
- **No `Rule::DefaultAccessThenRename` use** — TypeScript's
  class-member visibility default-public is encoded inside Custom
  handlers (method_definition, public_field_definition,
  abstract_method_signature) along with other extraction logic
  (async/star/get/set markers). Promoting wouldn't simplify since
  these handlers do multi-step work beyond just the access marker.
- **`name_wrapper` flattens for destructuring patterns**:
  array_pattern / object_pattern as the single child means it's
  not really a name — flatten so the pattern surfaces directly
  under the declarator.
- **`private_property_identifier` (`#foo`)**: the leading `#` is
  stripped, and a `<private/>` marker is lifted onto the enclosing
  field/property by the name_wrapper inline logic.
- **Behavior parity: predefined_type**: caught by the
  `functions::typescript_arrow` / `parameters::typescript` /
  `generics::typescript_vocabulary` transform tests during Step 4
  — the original catch-all wrapped TYPE-renamed text in `<name>`
  when no marker was present. The pure `Rule::Rename(TYPE)`
  doesn't do this. Fix: route PredefinedType through Custom
  (transformations::type_identifier) which does both rename + wrap.
  Lesson: snapshot byte-identity is the load-bearing assertion
  — it caught the issue immediately.

### Ruby — COMPLETE (final programming language)

Commits (chronological):

- `3b5487f` — Step 1: generate RubyKind enum (134 kinds; uses
  Self_ sanitizer since Ruby has a `self` kind too).
- `8a3ae61` — Step 2: validate catalogue, drop 5 dead entries
  (break_statement, continue_statement, next_statement —
  superseded by `break`/`next`; method_call collapsed into `call`;
  `symbol` replaced by simple_symbol/delimited_symbol).
- `5d6a34d` — Step 3: rules.rs + transformations.rs.
- `b073fda` — Step 4: swap dispatcher (deletes 165 lines).
- `49465ec` — Step 5: drop KINDS / rename_target.
- `6941ff1` — Step 6: rename semantic.rs → output.rs.

#### Ruby-specific notes

- **Smallest Custom set** — only `comment`, `passthrough`, and
  `name_wrapper`. Most arms are pure Rename or Flatten.
- **No `Rule::DefaultAccessThenRename`** — Ruby's class/method
  visibility (`private`/`protected` keywords) appears as separate
  expression statements in source rather than declaration prefixes,
  so there's no implicit access marker pattern.
- **`Self_` keyword sanitizer reused** — the codegen sanitizer
  added in commit `70701f0` (Rust migration) handled Ruby's `self`
  kind as well, with no further changes.
- **Many already-matching passthrough kinds** (26): block, break,
  conditional, constant, do, false, in, lambda, nil, etc. Ruby's
  grammar uses lowercase keyword names that happen to match our
  semantic vocabulary, so passthrough is the correct rule.
- **`identifier` / `instance_variable` / `class_variable` /
  `global_variable` all rename to NAME** — Ruby's grammar tags
  variable references by sigil but at the semantic layer they're
  all "variable references" → `<name>`. The leading sigil
  (`@`, `@@`, `$`) survives as text.

## Final state

All 8 programming languages migrated:
  Go, C#, Java, PHP, Python, Rust, TypeScript, Ruby.

Only tsql (data-only) remains on the catalogue path. The plan's
final cleanup step — removing KindEntry/KindHandling from
`languages/mod.rs` — is pending tsql migration.

Total commits in this session: ~50, accumulated across the seven
languages migrated after Go and C#. 790+ tests pass throughout;
140 snapshot fixtures byte-identical at every commit.
