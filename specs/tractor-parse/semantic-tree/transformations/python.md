---
title: Python Transformations
---

Per-node decisions for the Python transform
(`tractor/src/languages/python.rs`).

## Summary

Python's transform has three distinctive features:

1. **Collection unification** — list / dict / set literals *and*
   comprehensions both render under the same element, distinguished
   by an exhaustive `<literal/>` vs `<comprehension/>` marker
   (Principle #9). Generators have no literal form so they stay
   bare `<generator>`.
2. **Operator-extract** for binary, comparison, boolean, unary, and
   augmented-assignment operators (uniform with other languages).
3. **`async` marker** on `function_definition` — Python's async
   keyword is lifted to a `<async/>` empty marker, mirroring the
   same treatment now applied in TypeScript.

## Element names

| tree-sitter kind | semantic name | rationale |
|---|---|---|
| `module` | `<module>` | Python spec term. |
| `class_definition` | `<class>` | Language keyword. |
| `function_definition` | `<function>` | Developer mental model (Goal #5). |
| `decorated_definition` | `<decorated>` | Short, reads well. |
| `decorator` | `<decorator>` | Language term. |
| `default_parameter`, `typed_parameter`, `typed_default_parameter` | `<param>` | All unified; distinctions are in the children (type annotation, default value). |
| `return_statement` | `<return>` | Language keyword. |
| `if_statement` | `<if>` | Language keyword. |
| `elif_clause` | `<else_if>` | Cross-cutting conditional shape — see below. |
| `else_clause` | `<else>` | Language keyword. |
| `for_statement`, `while_statement` | `<for>`, `<while>` | Language keywords. |
| `for_in_clause` | `<for>` | Comprehension's `for`; same conceptual element as a loop's for — developer mental model (Goal #5). |
| `try_statement` | `<try>` | Language keyword. |
| `except_clause` | `<except>` | Language keyword. |
| `finally_clause` | `<finally>` | Language keyword. |
| `with_statement` | `<with>` | Language keyword. |
| `raise_statement` | `<raise>` | Language keyword. |
| `pass_statement` | `<pass>` | Language keyword. |
| `import_statement` | `<import>` | Language keyword. |
| `import_from_statement` | `<from>` | Language keyword (reads more natural than `import_from`). |
| `call` | `<call>` | Already canonical. |
| `attribute` | `<member>` | Matches C#/Java/TS shape for `obj.attr`. |
| `subscript` | `<subscript>` | Python-specific; the `obj[key]` syntax. |
| `assignment` | `<assign>` | Consistent. |
| `augmented_assignment` | `<assign>` | Unified with plain assignment — the `<op>` child (e.g., `+=`) distinguishes them (Principle #5 / Goal #5). |
| `binary_operator` | `<binary>` | Consistent. |
| `unary_operator` | `<unary>` | Consistent. |
| `comparison_operator` | `<compare>` | Python-specific category for `<`, `<=`, `in`, `is`, etc. |
| `boolean_operator` | `<logical>` | For `and`, `or`. |
| `conditional_expression` | `<ternary>` | Language concept. |
| `lambda` | `<lambda>` | Already a Python keyword. |
| `await` | `<await>` | Language keyword. |
| `generator_expression` | `<generator>` | See Collection unification below. |
| `string` | `<string>` | Language concept. |
| `integer` | `<int>` | Matches Python's `int` type. |
| `float` | `<float>` | Matches Python's `float` type. |
| `true`, `false`, `none` | `<true>`, `<false>`, `<none>` | Python keywords (note: lowercase — spec convention). |
| `identifier` | `<name>` | Namespace vocabulary (Principle #14). |
| `as_pattern` | `<as>` | Read natural: `except E as e`, `with f as x`. |
| `list_splat_pattern` | `<splat>` | Short; `*args` style. |
| `dictionary_splat_pattern` | `<kwsplat>` | Short; `**kwargs` style. Parallel with `splat`. |

## Structural transforms

### Flatten (Principle #12)

- `expression_statement` → Skip (absorb children).
- `block` — purely structural.
- `as_pattern_target` — the target name slot in an `as_pattern`;
  drop the wrapper so the name is a direct child.
- `pattern_list` — the `a, b = …` LHS of an unpacking assignment;
  drop so the underlying patterns are siblings.

### Flat lists with field distribution (Principle #12)

- `parameters` → children get `field="parameters"`.
- `argument_list` → children get `field="arguments"`.
- `type_parameter` — Python's tree-sitter calls the `[X]` in
  `List[X]` a `type_parameter`, but it's actually the list of
  type arguments. Children get `field="arguments"` (the generic
  `<type>` it lives inside makes it a type argument list).

### Generic type references

Python generic annotations like `List[int]` go through the shared
`rewrite_generic_type` helper (name-kinds: `identifier`,
`type_identifier`):

```xml
<type>
  <generic/>
  List
  <type field="arguments">int</type>
</type>
```

The `type` tree-sitter wrapper that Python puts around type
expressions is collapsed when it contains exactly a single
`generic_type` (to avoid double-nesting `<type><type>…`).

### Collection unification (inverted-shape design)

Each collection is its *produced type* as the outer element, with
the construction form as an exhaustive marker inside.

```xml
<list><literal/>1, 2, 3</list>                 <!-- [1, 2, 3] -->
<list><comprehension/>x for x in xs</list>     <!-- [x for x in xs] -->
<dict><literal/>"a": 1</dict>                  <!-- {"a": 1} -->
<dict><comprehension/>k: v for k,v in xs</dict>
<set><literal/>1, 2</set>                      <!-- {1, 2} -->
<set><comprehension/>x for x in xs</set>
<generator>x for x in xs</generator>           <!-- (x for x in xs) -->
```

Rationale:
- A list literal and a list comprehension both *produce* a list.
  That's the same thing in the developer's mental model (Goal #5) —
  the list is what you get; the construction method is secondary.
- Queries become powerful: `//list` finds every list production;
  `//list[comprehension]` narrows to comprehensions only;
  `//list[literal]` narrows to literals only.
- `<generator>` has no marker because Python has no generator
  literal (parens around a comma-separated list produce a tuple,
  not a generator). Principle #9 requires markers only for
  *mutually exclusive* variations; generators have only one form.

### `async` function marker

`function_definition` extracts the `async` keyword when present
and prepends `<async/>` as an empty marker child. Same pattern as
TS/JS.

### Operator extraction

For all binary/unary/comparison/boolean/augmented forms, the
operator text is pulled out into an `<op>` child:

```xml
<binary><op>+</op><left>...</left><right>...</right></binary>
<compare><op>in</op><left>x</left><right>xs</right></compare>
<assign><op>+=</op><left>x</left><right>1</right></assign>
```

## Language-specific decisions

### `dictionary` → `<dict>`

Tree-sitter calls the dict literal `dictionary`. Python developers
always say `dict`. Rename aligns with Goal #5 and with Python's own
built-in type name.

### `<splat>` / `<kwsplat>` instead of tree-sitter's compound names

`list_splat_pattern` (`*args`) and `dictionary_splat_pattern`
(`**kwargs`) are overly long. Python doesn't have a universal term
for them; "splat" is common in REST-language-adjacent developer
speech. `splat` / `kwsplat` keeps the symmetry (both are splats;
`kw` = "keyword") and is short.

### `<augmented_assignment>` → `<assign>`

Python's `x += 1` is syntactically and semantically an assignment
with a compound operator. Separating `<assign>` from
`<augmented_assignment>` doubles the vocabulary for a distinction
the `<op>` child (`=` vs `+=`) already makes (Goal #5).

### `for_in_clause` → `<for>`

Inside a list comprehension, the `for x in xs` clause is called
`for_in_clause` by tree-sitter. Developer speech is just "the for"
— the `in` is grammatical, not a separate concept. Using `<for>`
matches the same element as loop-statement for (Principle #5).

### Conditional shape (`<if>` / `<else_if>` / `<else>`)

Python's tree-sitter grammar already emits `elif_clause` and
`else_clause` as flat siblings of `if_statement` (each tagged with
field `alternative`), so no structural collapse is needed. The
implementation:

- `PYTHON_FIELD_WRAPPINGS` (in `tractor/src/languages/mod.rs`)
  deliberately omits the `("alternative", "else")` entry that the
  cross-cutting convention ships with. Wrapping every `elif_clause`
  and `else_clause` in `<else>` would bury the flat chain under a
  redundant wrapper. The `consequence` → `<then>` rename stays,
  because Python's grammar has no literal `then` kind.
- `map_element_name` renames `elif_clause` → `<else_if>` and
  `else_clause` → `<else>` as plain kind-to-name mappings.
- No post-transform is registered for Python: after walk_transform
  the tree already matches the target shape.

See the cross-cutting "Conditional shape" convention in the index
[`transformations.md`](../transformations.md).
