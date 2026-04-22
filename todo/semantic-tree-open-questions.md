# Semantic tree: open questions

All currently-undecided naming, structural, and cross-language
questions in the semantic tree. Each entry has:

- **Source code** — what the construct looks like in the wild.
- **Current tree shape** — what tractor produces today (with the
  element names that leak through / feel wrong).
- **Candidate shapes** — proposals with pros/cons.
- **My lean** where I have one; blank where I genuinely don't.

Grouped by: cross-language first, then per-language. Cross-language
items should be decided once and applied uniformly.

---

## Cross-language

### #3 — `condition` / `consequence` for `if`

"Consequence" is functional-language jargon; no mainstream
imperative developer says it.

```python
# Python
if x > 0:
    return x
else:
    return -x
```

```typescript
// TypeScript
if (x > 0) { return x } else { return -x }
```

**Current tree** (all languages):

```xml
<if>
  <condition><binary>...</binary></condition>
  <consequence><return>...</return></consequence>
  <alternative><return>...</return></alternative>
</if>
```

**Candidates**:

| Shape | Pro | Con |
|---|---|---|
| `<condition>` / `<then>` / `<else>` | `then`/`else` match natural speech and the actual `else` keyword | `<else>` collides with the standalone `else_clause` element used for for-else in Python / else-if chains |
| `<condition>` / `<body>` / `<else>` | `body` matches how function/class bodies are named | `body` usually implies a larger scope; a one-liner if doesn't feel like it has a "body" |
| `<test>` / `<then>` / `<else>` | `test` is shorter than `condition` and matches Python AST vocabulary | Less self-explanatory for non-Pythonists |

**My lean**: `<condition>` stays, rename `consequence` → `<then>`,
keep `<alternative>` → `<else>` (accept the namespace collision with
for/while `else_clause` — query context disambiguates).

---

### #7 — Type parameter declaration inner shape

```typescript
class Box<T> { value: T }
```

```java
class Box<T extends Comparable<T>> { T value; }
```

**Current tree** (C# and TS after recent cleanups):

```xml
<class>
  <name>Box</name>
  <generic field="generics">
    <name><type>T</type></name>     <!-- over-classified -->
  </generic>
  ...
</class>
```

The inner `<name><type>T</type></name>` wrapping is a relic —
`T` is the name of the type parameter, which a developer thinks of as
just "T". The `<type>` inside is spurious.

**Target shape**:

```xml
<class>
  <name>Box</name>
  <generic field="generics">
    <name>T</name>
    <bound><type>Comparable</type></bound>   <!-- Java bounds -->
  </generic>
</class>
```

Applied uniformly to C#, TS, Java, Rust. Already how Python's
type_parameter flat-list works.

---

### Base-class / implements `<type>` wrapping

Principle #14 says every type reference wraps in `<type>`. Today,
some type-reference slots (notably base classes in C#/Java) carry the
type as bare text, not wrapped.

```csharp
public class Foo : Bar, IBaz { }
```

```java
public class Foo extends Bar implements IBaz { }
```

**Current tree** (after agent's compound-name cleanup):

```xml
<!-- C# -->
<class>
  <name>Foo</name>
  <extends>Bar</extends>
  ...
</class>
```

Target (Principle #14):

```xml
<class>
  <name>Foo</name>
  <extends><type>Bar</type></extends>
  <implements><type>IBaz</type></implements>
</class>
```

Plus: Java `super_interfaces` → `<implements>` is done; each child
should be wrapped in `<type>` too.

**Scope**: cross-language — C#, Java, TS (`class_heritage`,
`extends_clause`, `implements_clause`).

---

## TypeScript / JavaScript

### `function_type` (arrow-function type annotations)

```typescript
type Handler = (event: Event) => void;
function setOn(h: (x: number) => string) { ... }
```

**Current tree**:

```xml
<alias>
  <name>Handler</name>
  <function_type>
    (<param><name>event</name><type>Event</type></param>)
    =>
    <type>void</type>
  </function_type>
</alias>
```

**Candidates**: `<signature>` inside `<type>`, `<callable>`,
`<functiontype>`. `<signature>` reads cleanest in a type position.

### JSX / TSX elements

```tsx
<Button onClick={handleClick}>Click me</Button>
```

**Current tree**: raw tree-sitter kinds (`jsx_element`,
`jsx_opening_element`, `jsx_attribute`, etc.). Not yet consolidated.

Needs a full design pass — probably `<element>` with `<name>`,
`<attribute>` children (matching XML/HTML vocabulary), but attributes
in JSX map to props which aren't just strings.

### `<arrow_function>` → `<lambda>` (already done, flagging for review)

Earlier I mapped `arrow_function` → `<lambda>`. Users may prefer
keeping `<arrow>` since it's the JS-native term. Raising here for
explicit sign-off.

---

## Python

### `expression_list`

```python
return x, y
yield a, b, c
```

**Current tree**:

```xml
<return>
  return
  <expression_list>
    <name>x</name>, <name>y</name>
  </expression_list>
</return>
```

Go flattens its `expression_list`; Python doesn't. Asymmetry.

**Proposed**: flatten Python's too. `<return><name>x</name>,<name>y</name></return>`.

### f-string / multi-part strings

```python
message = f"hello {name}, you are {age}"
```

**Current tree**:

```xml
<string>
  <string_start>"</string_start>
  <string_content>hello </string_content>
  <interpolation>{<name>name</name>}</interpolation>
  <string_content>, you are </string_content>
  <interpolation>{<name>age</name>}</interpolation>
  <string_end>"</string_end>
</string>
```

Start/end/content nodes are tree-sitter machinery that could be
flattened into plain text siblings of `<interpolation>`, giving:

```xml
<string>
  "hello
  <interpolation><name>name</name></interpolation>
  , you are
  <interpolation><name>age</name></interpolation>
  "
</string>
```

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

**Current tree**:

```xml
<class>
  <name>Repo</name>
  <generic field="generics"><name>T</name></generic>
  <generic field="generics"><name>U</name></generic>
  <type_parameter_constraints_clause>
    where T : class, IComparable&lt;T&gt;, new()
  </type_parameter_constraints_clause>
  <type_parameter_constraints_clause>
    where U : struct
  </type_parameter_constraints_clause>
</class>
```

**Candidates**:

| Element name | Pro | Con |
|---|---|---|
| `<where>` | Matches the keyword; short, queryable | `where` is SQL/LINQ keyword too — potential future cross-language query clash |
| `<constraint>` / `<constraints>` | Literal translation of the C# docs term | Two-level vocabulary (clause vs individual constraint) |
| `<bound>` / `<bounds>` | Matches Java's `type_bound` naming | Feels off for the clause wrapper — `where T : class, new()` is more than a bound, it's multiple |

Plus: inner shape. Should constraints attach to the generic they
constrain, rather than being separate clauses? e.g.:

```xml
<generic field="generics">
  <name>T</name>
  <bound><type>class</type></bound>
  <bound><generic/>IComparable<type field="arguments">T</type></bound>
  <bound><new/></bound>
</generic>
```

That matches Java's design (bounds attach to the generic directly).

**My lean**: attach constraints to the `<generic>` element, with
each constraint as a `<bound>` child. Lose the `where`-clause
grouping (it's syntactic, not semantic — a developer thinks "T is
constrained to be a class + IComparable + have a default ctor").

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

**Current tree**:

```xml
<type>
  <name>Hello</name>
  <struct>
    <field><name>name</name><type>string</type></field>
  </struct>
</type>
```

**Target** (user-approved earlier):

```xml
<struct>
  <name>Hello</name>
  <field><name>name</name><type>string</type></field>
</struct>

<interface>
  <name>Greeter</name>
  <method>...</method>
</interface>
```

The `<type>` wrapper is Go-grammar bleed-through; a developer
thinks "I'm declaring a struct named Hello", not "I'm declaring
a type that happens to be a struct".

### Defined-type vs alias

```go
type MyInt int       // defined type — creates a distinct type with methods
type Color = int     // alias declaration — same type, new name
```

**Target** (user-approved earlier):

```xml
<!-- type MyInt int -->
<type>
  <name>MyInt</name>
  <type>int</type>
</type>

<!-- type Color = int -->
<alias>
  <name>Color</name>
  <type>int</type>
</alias>
```

The outer `<type>` in the defined-type form matches Go's own spec
term "type definition". Nested `<type>` is OK — the outer has a
`<name>` child (declaration), the inner is a bare reference.

### `expression_list` asymmetry

Go flattens; Python doesn't (already noted above). Decide once:
flatten both, or wrap both.

---

## Rust

### `struct_expression`

```rust
let p = Point { x: 1, y: 2 };
```

**Current tree**:

```xml
<let>
  <name>p</name>
  <value>
    <struct_expression>
      <name>Point</name>
      <field><name>x</name><value><int>1</int></value></field>
      <field><name>y</name><value><int>2</int></value></field>
    </struct_expression>
  </value>
</let>
```

**Candidates**:

| Element | Pro | Con |
|---|---|---|
| `<new>` | Matches JS/Java/C# `new Foo()` mental model | Rust doesn't have `new` as a keyword — idiomatic is `Point::new()` which is a `<call>` |
| `<struct>` | Symmetric with declaration | Collides with `<struct_item>` declaration |
| `<literal>` | "It's a struct literal" — common term | Overloaded with Python collection marker `<literal/>` |
| `<init>` | Short, reads well, distinct | Not a Rust term |

**My lean**: `<literal>` as an element (not a marker) — developers
call it a "struct literal". The collision with Python's marker is
not actual — Python's `<literal/>` is an empty marker inside
`<list>`/`<dict>`/`<set>`, while Rust's would be an element with a
name and field children. Different contexts.

### `reference_type` → `<ref>`

```rust
fn foo(s: &str) -> &mut Vec<i32> { ... }
```

**Current tree** (after cleanup):

```xml
<function>
  ...
  <param>
    <name>s</name>
    <ref><type>str</type></ref>
  </param>
  <returns>
    <ref><mut/><generic/>Vec<type field="arguments">i32</type></ref>
  </returns>
</function>
```

`<ref>` is evocative of Rust's `&` but collides conceptually with
the old deprecated value-ref element name we removed earlier (#73).

**Candidates**:

| Element | Pro | Con |
|---|---|---|
| Keep `<ref>` | Shortest, matches Rust's `&` sigil | Latent collision; confuses anyone reading our history |
| `<reference>` | Clear, unambiguous | Long, and the type is borrowing, not just referring |
| `<type><borrowed/>...</type>` | Fits Principle #14 (single `<type>` element, marker distinguishes form) | Two-element nesting; `<borrowed/>` is uncommon term |
| `<borrow>` | Rust-idiomatic — "borrow checker", "a borrow" | Non-standard; not a single language's spec term |

**My lean**: `<type><borrowed/>...</type>` — matches the Principle
#14 pattern (type-as-reference with a marker distinguishing the
borrow form) and makes `//type` queries find all type references
including borrows.

### `match_block`

```rust
match x {
    1 => "one",
    _ => "other",
}
```

**Current tree**:

```xml
<match>
  ...
  <match_block>
    <arm>...</arm>
    <arm>...</arm>
  </match_block>
</match>
```

**Proposal**: flatten `match_block` — it's a purely-grouping wrapper
(Principle #12). Result: `<match><arm>...</arm><arm>...</arm></match>`.

---

## Ruby

### Identifier classification normalisation

```ruby
def foo
  x = 1
  x + y
end
```

**Current tree**:

```xml
<method>
  <name>foo</name>
  <body>
    <assign>
      <left><identifier>x</identifier></left>
      <right><int>1</int></right>
    </assign>
    <binary>
      <left><identifier>x</identifier></left>
      <right><identifier>y</identifier></right>
    </binary>
  </body>
</method>
```

Ruby's `identifier` isn't renamed to `<name>` like in other
languages. Minor inconsistency with Principle #14.

**Proposal**: rename Ruby `identifier` → `<name>` in the transform.

### Method-call shape

```ruby
arr.map { |x| x + 1 }
foo(bar, baz)
obj.method(arg).chain
```

Ruby has rich method-call variations (implicit receiver, blocks,
chained calls). Currently tractor's Ruby transform renames `call`
and `method_call` to `<call>`, but hasn't systematically applied
the `<callee>`/`<object>`/`<property>` shape that TS/JS uses.

Needs a design pass. Low priority until Ruby sees heavier use.

---

## How to read / resolve

For each item: decide the shape, update the relevant per-language
transformation file in `specs/tractor-parse/semantic-tree/transformations/`
to document the new decision, implement in the language's `.rs`
file, regenerate fixtures, commit with the decision cited.
