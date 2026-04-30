# Stable Expression Hosts and Broad-to-Narrow Query Design

## Purpose

This note captures the design rationale for updating Tractor's semantic tree design documents.

The goal is to support Tractor's mission:

> Write a rule once. Enforce it everywhere.

That means the semantic tree should make durable, reliable rules easy to write. In particular, users should be able to start with a broad query, inspect the result set, and narrow the query incrementally. The tree should avoid shapes that make simple first queries unintentionally narrow, because those lead to silent false negatives and weaken confidence in Tractor rules.

The main design issue addressed here is how to represent modified expressions such as:

```rust
foo()?;
await foo();
```

or:

```ts
obj!.foo();
obj?.foo();
```

Tree-sitter grammars often represent these as modifier-specific wrapper nodes, for example `try_expression`, `await_expression`, or `non_null_expression`. Those wrappers steal structural identity from the operand. A `?`-suffixed call is no longer a direct call expression in its enclosing body; it becomes a call nested under a try wrapper. This breaks otherwise natural XPath queries.

The recommended solution is to introduce stable expression hosts.

---

## Recommended Decision

Use a uniform `<expression>` host at every expression position. The concrete operand keeps its specific semantic element inside the host: `<call>`, `<member>`, `<binary>`, `<name>`, `<literal>`, `<lambda>`, etc.

Expression-level modifiers attach to the `<expression>` host as empty markers.

Preferred shape:

```xml
<expression>
  <call>...</call>
  <try/>
</expression>
```

Not:

```xml
<try>
  <call>...</call>
</try>
```

And not:

```xml
<call>
  <try/>
  ...
</call>
```

The `<expression>` node is not intended as an abstract type hierarchy wrapper. It is a stable host for expression-position semantics. The concrete operand remains the primary queryable concept.

This gives two complementary query surfaces:

```xpath
//call
```

Finds all calls regardless of whether they are awaited, try-propagated, null-forgiven, conditionally accessed, etc.

```xpath
//body/expression[call]
```

Finds expression statements in a body whose root operand is a call.

So users learn the distinction:

- Use concrete nodes for concept queries: `//call`, `//member`, `//binary`, `//lambda`.
- Use expression hosts for position-sensitive queries: `//body/expression[call]`, `//argument/expression[call]`, `//return/expression[call]`.

---

## Design Goal to Add: Broad-to-Narrow Query Refinement

Add a new design goal or guiding principle to `@specs/tractor-parse/semantic-tree/design.md`.

Suggested text:

```md
### Broad-to-Narrow Query Refinement

Simple queries should match the broadest natural concept that a developer is
likely to mean. More specific variants should be expressed by adding predicates
or markers.

A user should be able to start with a broad selector, inspect the result set,
and narrow the query incrementally. The tree shape should therefore avoid making
surface variants the primary node name when those variants belong to a larger
developer-recognized concept.

Good:

```xpath
//variable
//variable[const]
//variable[const][name='foo']
```

Bad:

```xpath
//const
```

because it silently excludes `let` and `var` declarations unless the user already
knows to search for them separately.

This supports iterative rule authoring: rules are first tested against an
existing codebase, then narrowed until the result set matches the intended
convention. False positives are visible during authoring and can be refined away.
False negatives caused by unintentionally narrow tree shapes are much harder to
discover and undermine confidence in the rule.
```

This principle reframes an existing tension in the design document. The current document says that concrete concepts are primary and abstractions build on top of concrete things. That remains mostly correct, but it needs refinement.

The better rule is:

> Primary nodes should be concrete developer concepts, not raw grammar variants.

Examples:

```xml
<variable>
  <const/>
  <name>foo</name>
</variable>
```

is still concrete, because developers do think in terms of variables. `const`, `let`, and `var` are declaration-kind variants.

Similarly:

```xml
<expression>
  <call>...</call>
  <try/>
</expression>
```

is not over-abstracting. It says: this expression position contains a call expression with a try/propagation modifier.

---

## The Core Dilemma: Operand Identity vs Position Identity

There are two different identities that need to be preserved.

### 1. Operand Identity

A call should remain queryable as a call.

This query should work broadly:

```xpath
//call
```

It should match all of these source forms:

```rust
foo();
foo()?;
foo().await;
await foo();
```

and analogous forms in other languages:

```ts
foo();
await foo();
obj!.foo();
obj?.foo();
```

Modifier-specific wrappers break this because the call is no longer structurally in the same place.

### 2. Position Identity

The thing occupying a syntactic position should also remain stable.

For example, these two statements should have sibling nodes of the same kind:

```rust
xot.with_a(node)?;
xot.with_b();
```

The stable sibling should not be `<try>` in the first case and `<call>` in the second. It should be:

```xml
<body>
  <expression>
    <call>...</call>
    <try/>
  </expression>

  <expression>
    <call>...</call>
  </expression>
</body>
```

Then a rule can target adjacent expression statements whose root operand is a call.

---

## Practical Example: Consecutive `xot.with_*` Calls

Problem source:

```rust
xot.with_a(node)?;
xot.with_b();
```

This should be flagged as two unchained fluent calls that should probably have been chained.

A natural but fragile query against a no-host tree might be:

```xpath
//body/call[...]/following-sibling::call[1][...]
```

But this silently misses the modified case if `xot.with_a(node)?` becomes:

```xml
<try>
  <call>...</call>
</try>
```

because the two calls are no longer siblings.

With stable expression hosts, the shape becomes:

```xml
<body>
  <expression>
    <call>
      <callee>
        <member>
          <object>
            <expression>
              <name>xot</name>
            </expression>
          </object>
          <name>with_a</name>
        </member>
      </callee>
      <argument field="arguments">
        <expression>
          <name>node</name>
        </expression>
      </argument>
    </call>
    <try/>
  </expression>

  <expression>
    <call>
      <callee>
        <member>
          <object>
            <expression>
              <name>xot</name>
            </expression>
          </object>
          <name>with_b</name>
        </member>
      </callee>
    </call>
  </expression>
</body>
```

The rule becomes position-stable:

```xpath
//body/expression[
  call/callee/member[
    object/expression/name = 'xot'
    and starts-with(name, 'with_')
  ]
]
/following-sibling::expression[1][
  call/callee/member[
    object/expression/name = 'xot'
    and starts-with(name, 'with_')
  ]
]
```

This is slightly longer than querying `//body/call`, but it is much more robust.

The mental model is:

> Find adjacent expression statements whose root expression is an `xot.with_*` call.

---

## Evaluation of Possible Tree Shapes

### Option 1: Keep Modifier-Specific Wrappers

Example:

```xml
<body>
  <try>
    <call>...</call>
  </try>
  <call>...</call>
</body>
```

Advantages:

- Close to Tree-sitter.
- Minimal transformation work.
- Preserves modifier grammar structure directly.

Disadvantages:

- Modifier wrappers steal identity from operands.
- Position-sensitive queries become brittle.
- Every new modifier introduces another special case.
- Users must write defensive disjunctions such as:

```xpath
self::call or self::try or self::await or self::non_null
```

or switch from direct child queries to descendant queries.

This is a poor fit for Tractor's mission because it makes rules easy to under-scope accidentally.

Verdict: reject.

---

### Option 2: Push Markers Directly Onto the Operand

Example:

```xml
<body>
  <call>
    <try/>
    ...
  </call>
  <call>...</call>
</body>
```

Advantages:

- Keeps the original simple sibling query working.
- Very convenient for complex operands such as `<call>`, `<member>`, and `<binary>`.
- Avoids extra `<expression>` wrappers in many common cases.

Disadvantages:

- Fails for text-only leaves such as `<name>foo</name>`.
- To represent `x?` or `await x`, we would need something like:

```xml
<name>
  <try/>
  x
</name>
```

which breaks the JSON shape of text-only leaves.

- The modifier belongs to the expression as a whole, not always to the operand node as a concept.
- For nested/chained expressions, it can become ambiguous which node owns the modifier.

This conflicts with the existing principle that markers should not be added to text-only leaves.

Verdict: reject as the general model, even though it is attractive for calls.

---

### Option 3: Use `<expression>` Only When a Modifier Is Present

Example:

```xml
<body>
  <expression>
    <call>...</call>
    <try/>
  </expression>
  <call>...</call>
</body>
```

Advantages:

- Avoids adding hosts to unmodified expressions.
- Gives modifiers a safe host when needed.

Disadvantages:

- Does not solve the sibling problem.
- Modified and unmodified expressions still have different parent-facing shapes.
- Queries still need to handle both:

```xpath
//body/*[self::call or self::expression[call]]
```

This is only a partial fix.

Verdict: reject.

---

### Option 4: Reuse Existing Role Nodes as Hosts

Example:

```xml
<argument>
  <try/>
  <call>...</call>
</argument>

<return>
  <await/>
  <call>...</call>
</return>
```

Advantages:

- Avoids adding an extra expression wrapper where a role wrapper already exists.
- Efficient and compact.
- Could work reasonably well for arguments, return values, assignment values, binary sides, etc.

Disadvantages:

- Expression modifiers become scattered across many host types.
- There is no single expression-level query surface.
- Finding all try-propagated expressions becomes structurally sloppy:

```xpath
//*[try]
```

- It weakens discoverability: users must learn that modifiers attach to many different parent shapes.

Verdict: viable as an optimization, but weaker than a uniform expression host. Not recommended as the primary design.

---

### Option 5: Uniform `<expression>` Host at Every Expression Position

Example:

```xml
<body>
  <expression>
    <call>...</call>
    <try/>
  </expression>

  <expression>
    <call>...</call>
  </expression>
</body>
```

Advantages:

- Preserves operand identity.
- Preserves position identity.
- Gives expression modifiers one consistent home.
- Avoids markers on text-only leaves.
- Supports broad-to-narrow query refinement.
- Avoids future special cases for new expression modifiers.
- Keeps concrete concept queries simple:

```xpath
//call
//member
//binary
```

- Keeps position-sensitive queries stable:

```xpath
//body/expression[call]
//argument/expression[call]
//return/expression[call]
```

Disadvantages:

- Adds many `<expression>` nodes.
- Makes deeply nested expression trees more verbose.
- Requires users to learn that expression positions are queried through expression hosts.

Verdict: recommended.

---

### Option 6: Custom XPath Transparency or Virtual Nodes

Example idea: keep the tree as:

```xml
<try>
  <call>...</call>
</try>
```

but make Tractor's query engine treat the call as if it were visible through the wrapper.

Advantages:

- Potentially compact tree.
- Could preserve some simple queries.

Disadvantages:

- Breaks the simple mental model that Tractor uses XML plus XPath.
- Users would need to learn Tractor-specific XPath semantics.
- The tree shown to users would not exactly match the tree queried by users.
- Debugging becomes harder.

Verdict: reject for the core semantic model. Could be explored later as optional query sugar, but not as the foundational representation.

---

## Cost in Complex Expression Trees

The real cost of uniform expression hosts appears in large, deeply nested expressions.

Example:

```ts
totalPrice = Math.max(0, orders.reduce(x => x.price, (a, b) => a + b)) - user.credits;
```

A simplified host-heavy tree might look like:

```xml
<expression>
  <assign>
    <left>
      <expression>
        <name>totalPrice</name>
      </expression>
    </left>

    <right>
      <expression>
        <binary>
          <left>
            <expression>
              <call>
                <callee>
                  <expression>
                    <member>
                      <object>
                        <expression>
                          <name>Math</name>
                        </expression>
                      </object>
                      <name>max</name>
                    </member>
                  </expression>
                </callee>

                <argument field="arguments">
                  <expression>
                    <number>0</number>
                  </expression>
                </argument>

                <argument field="arguments">
                  <expression>
                    <call>
                      <callee>
                        <expression>
                          <member>
                            <object>
                              <expression>
                                <name>orders</name>
                              </expression>
                            </object>
                            <name>reduce</name>
                          </member>
                        </expression>
                      </callee>

                      <argument field="arguments">
                        <expression>
                          <lambda>
                            <parameter field="parameters">
                              <name>x</name>
                            </parameter>
                            <body>
                              <expression>
                                <member>
                                  <object>
                                    <expression>
                                      <name>x</name>
                                    </expression>
                                  </object>
                                  <name>price</name>
                                </member>
                              </expression>
                            </body>
                          </lambda>
                        </expression>
                      </argument>

                      <argument field="arguments">
                        <expression>
                          <lambda>
                            <parameter field="parameters">
                              <name>a</name>
                            </parameter>
                            <parameter field="parameters">
                              <name>b</name>
                            </parameter>
                            <body>
                              <expression>
                                <binary>
                                  <left>
                                    <expression>
                                      <name>a</name>
                                    </expression>
                                  </left>
                                  <right>
                                    <expression>
                                      <name>b</name>
                                    </expression>
                                  </right>
                                </binary>
                              </expression>
                            </body>
                          </lambda>
                        </expression>
                      </argument>
                    </call>
                  </expression>
                </argument>
              </call>
            </expression>
          </left>

          <right>
            <expression>
              <member>
                <object>
                  <expression>
                    <name>user</name>
                  </expression>
                </object>
                <name>credits</name>
              </member>
            </expression>
          </right>
        </binary>
      </expression>
    </right>
  </assign>
</expression>
```

This is undeniably more verbose.

However, this cost is acceptable because very complex expression trees are rarely the direct target of durable rules. Tractor rules are usually written for repeated codebase patterns, not exact one-off expression shapes.

Rules usually target patterns like:

```xpath
//method[public][async]
//class[not(attribute/name='GeneratedCode')]
//variable[const][value/expression/call]
//body/expression[call]
```

They rarely target an exact expression like:

```ts
totalPrice = Math.max(0, orders.reduce(x => x.price, (a, b) => a + b)) - user.credits;
```

So the verbosity cost appears mostly when inspecting/debugging complex one-off expressions, while the reliability benefit applies to repeated rule-worthy patterns.

The design should therefore optimize for durable rule reliability, not for making every possible expression tree visually minimal.

---

## Repeated Patterns Over One-Off Expressions

Add this rationale near the stable expression host principle.

Suggested text:

```md
### Repeated Patterns Over One-Off Expressions

Tractor rules are primarily written for repeating codebase patterns, not for
deeply specific one-off expressions. Very complex expression trees may become
more verbose with stable `<expression>` hosts, but these shapes are rarely the
direct target of durable rules.

The important case is that repeated patterns remain stable under local surface
variation. A rule that targets call expressions in a body should keep working
when one call gains `await`, `?`, `!`, `?.`, or another expression modifier.

This means the tree should prefer stable repeated query surfaces over compact
rendering of uncommon deeply nested expressions.
```

This supports the mission directly. A compact tree that silently misses real-world variants is worse than a verbose tree whose shape remains reliable.

---

## Exception Case: LINQ and SQL-Like DSLs

There is one important exception to the claim that complex expressions are rarely queried: fluent DSLs such as LINQ, SQL builders, validation builders, route builders, dependency-injection registration chains, and test setup builders.

Example:

```csharp
query
    .Include(x => x.Customer.Address)
    .Include(x => x.Items.Product)
    .Include(x => x.Payments.Method)
    .AsSplitQuery();
```

A team may want a rule like:

> Queries with more than 3 `Include` calls where each include path is two or more levels deep must call `.AsSplitQuery()`.

This is a complex expression, but it is rule-worthy because it is a repeated DSL shape.

The stable expression host design still helps. The query author gets a consistent structure for calls, member access, arguments, lambdas, and modifiers across the whole chain.

A broad first query might be:

```xpath
//expression[call/callee/expression/member/name = 'Include']
```

Then it can be narrowed to include depth:

```xpath
//expression[
  call/callee/expression/member/name = 'Include'
  and count(call/argument/expression/lambda//member) >= 2
]
```

Then grouped by containing fluent chain or statement and checked for `AsSplitQuery`.

The important point: even in this exception case, consistency is more valuable than compactness. DSL chains are only queryable because their repeated shape is predictable.

Suggested rationale:

```md
When complex expressions are rule-worthy, they are usually complex because they
belong to a repeated DSL shape — for example LINQ, query builders, validation
builders, routing DSLs, dependency injection registration chains, or test setup
builders. In those cases, consistency matters more than compactness: the rule
author benefits from a stable shape for calls, members, arguments, and modifiers
across the whole chain.
```

---

## Relationship to Existing Principles

### Relation to “Concrete Is Primary”

The current design document says that concrete concepts are primary and abstractions build on top of concrete things.

This should be refined, not discarded.

Recommended refinement:

> Primary nodes should be concrete developer concepts, not raw grammar variants.

Examples:

- `variable` is the concrete developer concept.
- `const`, `let`, and `var` are declaration-kind variants.
- `expression` is the stable expression-position host.
- `call`, `member`, `binary`, and `name` remain the concrete operands.
- `try`, `await`, `non_null`, `conditional`, `ref`, and `deref` are expression-level modifiers.

So the design does not become “abstract-first.” It becomes “stable concept first, variants as markers.”

### Relation to Principle #11: Specific Names Over Type Hierarchies

`<expression>` should not be treated as a rejected type hierarchy wrapper like:

```xml
<expression>
  <binary>...</binary>
</expression>
```

where the user is expected to query `//expression[binary]` instead of `//binary`.

Users should still query:

```xpath
//binary
//call
//member
```

for concrete concept queries.

The `<expression>` host is justified because it does real work:

- it gives expression modifiers a safe home;
- it avoids markers on text-only leaves;
- it preserves parent-position identity;
- it makes modified and unmodified expressions structurally comparable.

It is not merely encoding an “is-a” hierarchy.

### Relation to Principle #13: Annotation Follows Node Shape

Stable expression hosts are a direct consequence of the leaf-shape rule.

Because text-only leaves like `<name>foo</name>` should not grow child markers, modifiers cannot attach directly to all possible operands. A uniform complex host gives modifiers somewhere to live without regressing JSON shape.

---

## Proposed New Principle: Stable Expression Hosts

Suggested text for `@specs/tractor-parse/semantic-tree/design.md`:

```md
### Stable Expression Hosts

Every expression position is represented by an `<expression>` host. The concrete
operand keeps its specific semantic element inside the host: `<call>`, `<member>`,
`<binary>`, `<name>`, `<literal>`, `<lambda>`, etc.

Closed-set expression modifiers attach to the `<expression>` host as empty
markers:

```xml
<expression>
  <await/>
  <call>...</call>
</expression>

<expression>
  <call>...</call>
  <try/>
</expression>
```

Modifier markers never become wrapper heads and never attach directly to
text-only leaves.

This preserves two invariants:

1. **Concrete concept queries remain broad.**
   `//call` finds calls regardless of `await`, `?`, `!`, `?.`, or other expression
   modifiers.

2. **Position-sensitive queries remain stable.**
   Expression positions have a consistent parent-facing shape. A modified call
   expression and an unmodified call expression are both `<expression>` children
   of their containing body, argument, return value, binary side, etc.

Users narrow expression queries by adding predicates:

```xpath
//expression
//expression[call]
//expression[call][try]
//body/expression[call]
```

Do not use modifier-specific expression wrappers such as `<try>`, `<await>`, or
`<non_null>` as expression heads. They steal identity from the operand and force
users to patch otherwise simple queries with disjunctions.
```

---

## Suggested Updates to Target User / Product Assumptions

The broad-to-narrow argument depends on an assumption about how Tractor is used.

Custom linting is most useful when a team already has a moderately large codebase and has accumulated conventions from experience. Rules are authored against existing code, inspected, and refined until the result set looks right.

Suggested addition to a target-user or product-assumptions document:

```md
### Target Usage Context

Tractor is designed primarily for teams working in moderately large, existing
codebases. These teams have already accumulated local conventions, lessons from
incidents, and maintainability preferences that are worth applying consistently.

Rule authoring is expected to be iterative: users write a broad query, inspect
matches in the current codebase, and narrow the query until it captures the
intended pattern. This means false positives are usually visible during rule
authoring and can be refined away. False negatives caused by unintentionally
narrow tree shapes are more dangerous, because they may only appear later when a
real-world case drifts past the rule.
```

This supports the design preference for broad first queries.

---

## Rendering / Inspection Mitigation

Uniform expression hosts make XML more verbose. Do not weaken the semantic model just to reduce visual noise. Instead, improve rendering.

Suggested text for rendering or CLI documentation:

```md
For human-readable tree output, unmarked expression hosts may be visually
collapsed with their single concrete operand:

```text
expression/call
expression/member
expression/name
```

Marked hosts should show markers inline:

```text
expression[try]/call
expression[await]/call
expression[non_null]/member
```

This preserves the queryable structure while reducing visual noise during
inspection. Users can still type what they see: `//expression[try]/call` or
`//body/expression[call]`.
```

This is especially important for the existing goal that users should be able to infer queries by looking at the rendered tree.

---

## Implementation Implications

### General Transformation Rule

For every language transform:

1. Identify expression positions.
2. Emit an `<expression>` host for each expression position.
3. Put the concrete operand inside the host.
4. Convert closed-set expression modifiers into empty markers on the host.
5. Preserve source order of markers for renderability.
6. Do not attach markers to text-only leaves.
7. Do not represent closed-set expression modifiers as wrapper heads.

### Expression Positions Include

Likely expression positions include:

- expression statements in a body;
- return values;
- assignment left and right sides;
- binary left and right sides;
- unary operands;
- ternary condition/then/else branches;
- call callees;
- call arguments;
- member objects;
- array/list elements;
- object/property values;
- lambda expression bodies when expression-bodied;
- conditions in `if`, `while`, `for`, etc.;
- interpolated expression holes;
- language-specific expression slots.

### Modifiers That Should Become Expression Markers

Candidates:

- Rust `?` → `<try/>`
- Rust `.await` or await expression → `<await/>`
- JavaScript / TypeScript / Python / C# `await` → `<await/>`
- TypeScript non-null assertion `!` → `<non_null/>`
- Optional/conditional access where represented as expression-level modifier → `<conditional/>`
- dereference where it behaves as a closed-set expression modifier → `<deref/>`
- reference/borrow where it behaves as a closed-set expression modifier → `<ref/>`

Some constructs may remain named expression operands instead of markers when they carry data or introduce structure.

Examples:

- `<binary>` remains a concrete operand because it has left/op/right structure.
- `<unary>` may remain concrete if the operator is open-set or structurally significant.
- `<cast>` may remain concrete because it carries a target `<type>` child.
- statement-form `try` remains distinct from expression-level Rust `?`.

---

## Query Guidance to Document

Add user-facing guidance somewhere near examples:

```md
Use concrete nodes when asking “where does this concept occur?”

```xpath
//call
//member
//binary
//lambda
```

Use expression hosts when asking “what occupies this position?”

```xpath
//body/expression[call]
//argument/expression[call]
//return/expression[call]
```

Expression modifiers are predicates on the host:

```xpath
//expression[await]
//expression[try]
//expression[non_null]
//body/expression[call][try]
```
```

This should prevent confusion when users see extra `<expression>` hosts.

---

## Final Rationale

Stable expression hosts are worth the extra nodes because Tractor optimizes for reliable, reusable rule authoring.

A compact tree that preserves raw grammar wrappers makes simple queries accidentally narrow. Accidental narrowness causes silent false negatives. Silent false negatives are especially damaging for Tractor, because the whole point is to encode hard-won team lessons once and have them keep applying.

A slightly more verbose but stable tree lets users start broad, inspect results, and narrow naturally. False positives are visible at authoring time. The extra query precision needed is discovered incrementally from the result set, not guessed in advance from possible grammar variants.

In short:

> Prefer stable repeated query surfaces over compact rendering of rare deep expression trees.

or:

> Surface variants narrow a stable semantic concept; they do not replace it.

This applies to declarations:

```xml
<variable>
  <const/>
  <name>foo</name>
</variable>
```

not:

```xml
<const>
  <name>foo</name>
</const>
```

And it applies to expressions:

```xml
<expression>
  <call>...</call>
  <try/>
</expression>
```

not:

```xml
<try>
  <call>...</call>
</try>
```

This is the shape most aligned with Tractor's mission: write a rule once, and let it keep guiding the codebase as local expression details drift.

