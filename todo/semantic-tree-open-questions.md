# Semantic tree: open questions

Tracks design decisions for the semantic tree. Each entry has:

- **Source code** — what the construct looks like in the wild.
- **Current shape** — what tractor produces today (XPath notation).
- **Decision** — the shape that's been agreed / implemented.
- **Alternatives** — shapes we considered but didn't pick, saved for
  later re-evaluation.

**Notation**: shapes are written as XPath-style descriptors.

- `parent[a][b]` — `<parent>` has children `<a>` and `<b>`.
- `parent[a='T']` — `<parent>` has an `<a>` child whose text value is `T`.
- Nesting: `parent[a[x]]` means `<parent><a><x/></a></parent>`.

---

## Still open

### Ruby — method-call shape

```ruby
arr.map { |x| x + 1 }
foo(bar, baz)
obj.method(arg).chain
```

Ruby has rich method-call variations (implicit receiver, blocks,
chained calls). Currently `call` and `method_call` both rename to
`<call>`, but the `<callee>` / `<object>` / `<property>` shape used
for TS/JS isn't systematically applied.

Needs a full design pass. Deferred until Ruby sees heavier use.

### JSX / TSX — element shape

```tsx
<Button onClick={handleClick}>Click me</Button>
```

**Current**: raw tree-sitter kinds leak through —
`jsx_element[jsx_opening_element[identifier][jsx_attribute[property_identifier][expression]]][text][jsx_closing_element]`

**Decision** (for when we implement): full shape below. JSX deferred
for v1; TSX out of scope for v1.

```
element/
  ├─ name = "Button"                    # tag or component name
  ├─ prop/                              # field="props" on each
  │   ├─ name = "onClick"
  │   └─ value/...                      # expression or string literal
  ├─ "Click me"                         # text child
  └─ element/…                          # nested JSX
```

Rules:
- `jsx_element` / `jsx_self_closing_element` → `<element>`.
- `jsx_opening_element` / `jsx_closing_element` → flattened (grammar
  wrappers, Principle #12).
- `jsx_attribute` → `<prop>` with `<name>` + `<value>` children; bare
  props (no value) render as `<prop><name>x</name></prop>`.
- `jsx_text` → plain text nodes (no wrapper).
- `jsx_expression` inside attribute value → children of `<value>`.

Query examples:
- `//element[name='Button']` — find Button usages.
- `//element[prop/name='onClick']` — elements with an onClick prop.
- `//element[name='div']//element` — nested elements inside divs.

Deferred as too early: intrinsic tags (`button`) vs. components
(`Button`) via marker; JSX namespaces (`<foo.Bar>`); fragment shorthand
(`<></>`); typed generics on components.

---

## Landed, open for re-evaluation

### Python — f-string / multi-part strings

```python
plain = "hello"
greeting = f"hello {name}"
status = f"hello {name}, you are {age}"
```

**Current shape** (after landing):

```
string/
  ├─ "f\"hello"
  ├─ interpolation/{ "{", name="name", "}" }
  ├─ ", you are"
  ├─ interpolation/{ "{", name="age", "}" }
  └─ "\""
```

`string_start` / `string_content` / `string_end` grammar wrappers are
flattened (Principle #12); `<interpolation>` is preserved so
`//string/interpolation/name='age'` finds every interpolation of the
`age` variable regardless of the surrounding literal text.

Plain (non-interpolated) strings collapse to a text-only `<string>`
element.

**Alternative to revisit**: keep `<string_content>` as an element
(but still flatten `string_start` / `string_end`). That would let you
query for specific literal-text fragments of a string —
`//string_content[. = "hello"]` — at the cost of a more verbose tree
for plain strings. Dropped on first pass because strings are rarely
a precise query target; if users do start writing such queries, we
can restore the wrapper.

---

## How to resolve

For each item: decide the shape, update the relevant per-language
transformation file in
`specs/tractor-parse/semantic-tree/transformations/`, implement in the
language's `.rs`, regenerate fixtures, commit with the decision cited.
