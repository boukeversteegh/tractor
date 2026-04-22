# Semantic tree: open questions

All currently-undecided naming, structural, and cross-language
questions in the semantic tree. Each entry has:

- **Source code** — what the construct looks like in the wild.
- **Current shape** — what tractor produces today (XPath notation).
- **Candidate shapes** — proposals in XPath notation, with pros/cons.
- **My lean** where I have one; blank where I genuinely don't.

**Notation**: shapes are written as XPath-style descriptors.

- `parent[a][b]` — `<parent>` has children `<a>` and `<b>` (presence only,
  values unspecified).
- `parent[a='T']` — `<parent>` has an `<a>` child whose text value is `T`.
  Used when the value matters to the example.
- `parent[a or b]` — `<parent>` has either an `<a>` or `<b>` child; used
  when the candidate is about choosing the child's name.
- Nesting: `parent[a[x]]` means `<parent><a><x/></a></parent>`.

Where a full XML example illustrates the shape better than the descriptor,
both are given.

Grouped cross-language first, then per-language.

---

## Cross-language


### #7 — Type parameter declaration inner shape

**Simple case:**

```typescript
class Box<T> { value: T }
```

**Current** (C# and TS after recent cleanups):
```xml
<class>
  <name>Box</name>
  <generic field="generics">
    <name>
      <type>T</type>      <!-- spurious wrapper -->
    </name>
  </generic>
  <field>
    <name>value</name>
    <type>T</type>
  </field>
</class>
```

Descriptor: `class[name='Box'][generic[name[type='T']]][field[name='value'][type='T']]`

The inner `<name><type>T</type></name>` is a relic. `T` is just the
name of the type parameter; the `<type>` wrapping inside the `<name>`
is spurious over-classification (the identifier landed in a type-slot
in tree-sitter, so its kind was `type_identifier`, but the role here
is "name of a declared type parameter", not a reference).

**Target:**

```xml
<class>
  <name>Box</name>
  <generic field="generics">
    <name>T</name>
  </generic>
  <field>
    <name>value</name>
    <type>T</type>
  </field>
</class>
```

Descriptor: `class[name='Box'][generic[name='T']][field[name='value'][type='T']]`

The type parameter declaration now mirrors every other declaration
shape — a `<name>` child holds the identifier as plain text.

**With a bound (Java-style):**

```java
class Box<T extends Comparable<T>> { T value; }
```

Target:
```xml
<class>
  <name>Box</name>
  <generic field="generics">
    <name>T</name>
    <bound>
      <type>
        <generic/>
        Comparable
        <type field="arguments">T</type>
      </type>
    </bound>
  </generic>
  <field>
    <name>value</name>
    <type>T</type>
  </field>
</class>
```

Descriptor: `class[name='Box'][generic[name='T'][bound[type[generic][type='T']]]][field[name='value'][type='T']]`

**Queries under the target**:
- `//generic[name='T']` — find the generic parameter named T.
- `//generic[bound]` — find constrained type parameters.
- `//generic[bound//type='Comparable']` — find generics bounded by a
  type whose reference text is `Comparable` (matches both bare and
  generic references).

Applied uniformly to C#, TS, Java, Rust. Python's flat-list already
does this for its type parameter form.

---

### Base-class / implements `<type>` wrapping (Principle #14)

Principle #14 says every type reference wraps in `<type>`. Today,
base-class slots carry the type as bare text.

```csharp
public class Foo : Bar, IBaz { }
```

```java
public class Foo extends Bar implements IBaz { }
```

**Current** (C#):
`class[name][extends="Bar"][implements="IBaz"]`
(extends/implements have bare text, not a `<type>` child)

**Target**: `class[name][extends[type]][implements[type]]`

Scope: cross-language — C#, Java, TS (`class_heritage`,
`extends_clause`, `implements_clause`).

---

## TypeScript / JavaScript

### `function_type` (arrow-function type annotations)

```typescript
type Handler = (event: Event) => void;
function setOn(h: (x: number) => string) { ... }
```

**Current**: `alias[name][function_type[param][type]]`
(the `function_type` tree-sitter kind leaks through)

**Candidates**:

| Shape | Pro | Con |
|---|---|---|
| `alias[name][type[signature[param][returns]]]` | `<signature>` reads cleanly in a type position | New element name |
| `alias[name][callable[param][returns]]` | Captures "this is a callable type" | More abstract |
| `alias[name][type[function[param][returns]]]` | Reuses `<function>` inside a `<type>` | `<function>` now means two things again (decl vs type) |

**My lean**: `alias[name][type[signature[param][returns]]]` — the
outer `<type>` honours Principle #14, the inner `<signature>` names
the function-shape semantic.

### JSX / TSX elements

```tsx
<Button onClick={handleClick}>Click me</Button>
```

**Current**: raw tree-sitter kinds leak through —
`jsx_element[jsx_opening_element[identifier][jsx_attribute[property_identifier][expression]]][text][jsx_closing_element]`

Needs a full design pass. No candidate shape yet — JSX attributes
map to component props and aren't plain strings, so the HTML-style
`element[name][attribute]` may not fit.

### `<arrow_function>` → `<lambda>` (applied, flagging for review)

Earlier I mapped `arrow_function` → `<lambda>`. Users may prefer
keeping the JS-native term:

- Current: `lambda[param][body]`
- Alternative: `arrow[param][body]`

Raising here for explicit sign-off.

---

## Python

### `expression_list`

```python
return x, y
yield a, b, c
```

**Current**: `return[expression_list[name][name]]`

Go flattens its `expression_list`; Python doesn't. Asymmetry.

**Proposal**: flatten Python's too → `return[name][name]`.

### f-string / multi-part strings

```python
message = f"hello {name}, you are {age}"
```

**Current**:
`string[string_start][string_content][interpolation[name]][string_content][interpolation[name]][string_end]`

**Proposal**: flatten start/content/end into plain text —
`string[interpolation[name]][interpolation[name]]` with surrounding
text as text-node siblings.

Lower priority; strings aren't a common query target.

---

## C#

### `where`-clause constraints

```csharp
class Repo<T, U>
    where T : class, IComparable<T>, new()
    where U : struct
{ }
```

**Current**:
`class[name][generic][generic][type_parameter_constraints_clause][type_parameter_constraints_clause]`
(clauses stand alone; bounds not attached to their generic)

**Candidates**:

| Shape | Pro | Con |
|---|---|---|
| `class[name][generic[name][bound[type]][bound[new]]]` (attach constraints to their `<generic>`) | Matches Java's design; a dev reads "T is a class and IComparable and has new()" as properties of T | Requires restructuring: resolving which constraint attaches to which generic |
| `class[name][generic][generic][where[name[ref]][constraint][constraint]]` | Keeps the clause grouping from the source | `where` collides with SQL/LINQ vocab; clause is a syntactic, not semantic, grouping |
| `class[name][generic][generic][constraints[constraint][constraint]]` | Preserves the clause-level wrapper without `where`-name collision | Same downside — wrapper is syntactic |

**My lean**: attach constraints to the `<generic>` element, with each
constraint as a `<bound>` child (matches Java). The `where` clause
grouping is syntactic, not something a developer thinks about.

---

## Go

### Struct / interface hoist

```go
type Hello struct {
    name string
}

type Greeter interface {
    Greet() string
}
```

**Current**: `type[name][struct[field]]`

**Target** (user-approved earlier): `struct[name][field]` and
`interface[name][method]`.

The `<type>` wrapper is Go-grammar bleed-through; a developer
thinks "I'm declaring a struct named Hello", not "I'm declaring a
type that happens to be a struct".

### Defined-type vs alias

```go
type MyInt int       // defined type — creates a distinct type with methods
type Color = int     // alias declaration — same type, new name
```

**Target** (user-approved earlier):

- `type MyInt int` → `type[name][type]` (outer `<type>` is Go's own
  spec term; inner `<type>` is the underlying-type reference)
- `type Color = int` → `alias[name][type]`

### `expression_list` asymmetry

Go flattens; Python doesn't (noted above). Decide once; apply
uniformly.

---

## Rust

### `struct_expression`

```rust
let p = Point { x: 1, y: 2 };
```

**Current**: `let[name][value[struct_expression[name][field][field]]]`

**Candidates**:

| Shape | Pro | Con |
|---|---|---|
| `let[name][value[new[name][field][field]]]` | Matches JS/Java/C# `new Foo()` mental model | Rust doesn't have `new` as a keyword; idiomatic `Point::new()` is a `<call>` |
| `let[name][value[literal[name][field][field]]]` | Devs call this a "struct literal" | Overloaded with Python's `<literal/>` marker (different context, though) |
| `let[name][value[init[name][field][field]]]` | Short, distinct | Not a Rust term |
| `let[name][value[struct[name][field][field]]]` | Symmetric with declaration | Collides with `<struct>` declaration element |

**My lean**: `literal[name][field]…` — the collision with Python's
`<literal/>` marker is not real (Python's is an empty marker *inside*
a collection element; Rust's would be an element with children in a
value position).

### `reference_type` → `<ref>`

```rust
fn foo(s: &str) -> &mut Vec<i32> { ... }
```

**Current**: `param[name][ref[type]]` (uses `<ref>` — collides
conceptually with the deprecated value-ref we removed in #73)

**Candidates**:

| Shape | Pro | Con |
|---|---|---|
| Keep `param[name][ref[type]]` | Shortest, matches Rust's `&` sigil | Latent name collision; confuses anyone reading history |
| `param[name][reference[type]]` | Clear, unambiguous | Long; not Rust's own term |
| `param[name][type[borrowed]]` | Fits Principle #14 (single `<type>`, marker distinguishes form) | Two-element nesting for a common construct |
| `param[name][borrow[type]]` | Rust-idiomatic ("borrow checker") | Non-standard; uncommon term for the tree element name |

**My lean**: `type[borrowed]` for Principle #14 consistency — `//type`
queries find every type reference including borrows.

### `match_block`

```rust
match x {
    1 => "one",
    _ => "other",
}
```

**Current**: `match[value][match_block[arm][arm]]`

**Proposal**: flatten `match_block` (Principle #12) →
`match[value][arm][arm]`.

---

## Ruby

### Identifier classification normalisation

```ruby
def foo
  x = 1
  x + y
end
```

**Current**: `method[name][body[assign[left[identifier]][right[int]]][binary[left[identifier]][right[identifier]]]]`
(Ruby's `identifier` isn't renamed to `<name>`)

**Proposal**: rename Ruby `identifier` → `<name>` →
`method[name][body[assign[left[name]][right[int]]][binary[left[name]][right[name]]]]`.

### Method-call shape

```ruby
arr.map { |x| x + 1 }
foo(bar, baz)
obj.method(arg).chain
```

Ruby has rich method-call variations (implicit receiver, blocks,
chained calls). Current `call` and `method_call` both rename to
`<call>`, but the `<callee>` / `<object>` / `<property>` shape used
for TS/JS isn't systematically applied.

Needs a design pass. No candidate shape yet — low priority until
Ruby sees heavier use.

---

## How to resolve

For each item: decide the shape, update the relevant per-language
transformation file in `specs/tractor-parse/semantic-tree/transformations/`,
implement in the language's `.rs`, regenerate fixtures, commit with
the decision cited.
