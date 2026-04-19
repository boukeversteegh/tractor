# Design Analysis: Multi-Level Hierarchical Extraction

*2026-03-16*

This document captures a design exploration for extracting hierarchical, multi-level data from source code using tractor. The analysis was motivated by a concrete use case but led to general insights about tree querying, projection, and join semantics.

## Motivating Use Case

An integration test project uses C# NUnit attributes to annotate test methods with metadata:

```csharp
[Test]
[Title("Standard Prospect Note Process")]
[Category(TestCategories.HappyFlow)]
[Description(@"Verifies that when a prospect note is created:
    1) It can also be fetched
    2) It can be updated")]
public async Task ProspectNoteHappyFlow() { ... }
```

A documentation generator (a C# test using .NET reflection) collects all test metadata and renders a markdown overview, grouped by namespace, then by class, then by method. The question: could tractor replace the reflection-based generator, making the extraction a standalone script that doesn't require building the C# project?

### What the generator needs at each level

| Level | Data needed |
|---|---|
| File / Namespace | Subdirectory path (maps to feature area) |
| Class | Class name (formatted as display name) |
| Method | Title, Category, Description, Issue keys |

The key requirement is **hierarchical structure**: the output must preserve which attributes belong to which method, which methods belong to which class, and which classes belong to which namespace/file.

## Current Tractor Capabilities

Tractor can extract all the individual pieces. For a single file:

```bash
# Class name
tractor file.cs -x "//class/name" -v value
# → ProspectNoteTests

# Namespace
tractor file.cs -x "//file_scoped_namespace_declaration/name" -v value
# → IntegrationTests.Tests.Integration

# Test titles
tractor file.cs -x "//method[...Test]/attributes/attribute[...Title]/arguments//string_literal_content" -v value
# → Standard Prospect Note Process

# Category values
tractor file.cs -x "//method[...Test]/attributes/attribute[...Category]/arguments//member/name/ref" -v value
# → HappyFlow
```

With `-f json -g file`, you can get structured output grouped by file.

### The gap

Tractor currently returns matches as a **flat list**. When you query `//method/attributes/attribute` across a file with multiple test methods, you get all attributes from all methods in one list, with no indication of which method each attribute belongs to. The hierarchical structure is lost.

## Approaches Explored

### Approach 1: `--sparse` flag on a single `-x`

**Idea**: Match leaf nodes (attributes), automatically retain all ancestors on the path from root to each match, prune everything else.

```bash
tractor "**/*Tests.cs" \
  -x "//method[attributes/attribute[name/ref='Test']]/attributes/attribute" \
  --sparse
```

Without `--sparse`, the current output is a flat list of the matched attribute subtrees:

```
Test
Title("Standard Prospect Note Process")
Category(TestCategories.HappyFlow)
Description(@"Verifies that when a prospect note is created:
    1) It can also be fetched
    2) It can be updated")
```

With `--sparse`, each match would instead be shown within its ancestor path from the document root, with non-matching siblings pruned away:

```
file ProspectNoteTests.cs
  namespace IntegrationTests.Tests
    class ProspectNoteTests
      method ProspectNoteHappyFlow
        attributes
          attribute Test                                          ← match
          attribute Title("Standard Prospect Note Process")       ← match
          attribute Category(TestCategories.HappyFlow)            ← match
          attribute Description(@"Verifies that ...")             ← match
```

The intermediate nodes (`file`, `namespace`, `class`, `method`, `attributes`) were not queried — they are retained only because they are ancestors of matches. The method body, other class members, using directives, and everything else is pruned.

For a file with multiple test methods, matches from different methods would share the common ancestry but branch at the method level:

```
file ProspectNoteTests.cs
  namespace IntegrationTests.Tests
    class ProspectNoteTests
      method ProspectNoteHappyFlow
        attributes
          attribute Test
          attribute Title("Standard Prospect Note Process")
          attribute Category(TestCategories.HappyFlow)
      method AnotherTestMethod
        attributes
          attribute Test
          attribute Title("Another Test")
          attribute Category(TestCategories.EdgeCase)
```

**Insight gained**: This conflates two orthogonal aspects:

1. **Direction from match**: Do you want the tree *above* the match (ancestors/supertree) or *below* the match (descendants/subtree)?
2. **Density**: Do you want the full tree or a pruned/sparse version?

Current `-x` gives **subtree + full** (everything below each match). The proposed `--sparse` would flip *both* axes simultaneously to **supertree + sparse**, which is conceptually muddy.

**Further insight**: What you actually want is *both* directions — the ancestry above (to see class and method context) and the subtree below (to get attribute argument values) — with non-matching siblings pruned. This is really **tree projection**: given a set of matched nodes, render them in situ within the original document tree, pruning branches that contain no matches.

This is essentially the Steiner tree of the matched nodes within the document AST: the minimal subtree that connects all matches.

### Approach 2: `-v outline` — new view mode with XPath union

**Idea**: Use XPath's union operator (`|`) to select nodes at multiple levels of the AST in a single query, and add a view mode that renders the matches in their structural context rather than as a flat list.

```bash
tractor "**/*Tests.cs" \
  -x "//class/name | //method/name | //attribute" \
  -v outline
```

The union selects three different node sets — class names, method names, and attributes — and `-v outline` would present them as a projected tree, preserving their containment relationships:

```
ProspectNoteTests
  ProspectNoteHappyFlow
    Test
    Title("Standard Prospect Note Process")
    Category(TestCategories.HappyFlow)
    Description(@"Verifies that...")
```

This differs from Approach 1 in an important way. With `--sparse`, you select only the leaf nodes and the tool infers the tree above them by retaining ancestors. Here, you **explicitly select** the nodes you want at each level via the union. The view mode just renders them nested by their containment in the source, rather than as a flat list.

**The generation-skipping problem**: The XPath selects `class/name`, `method/name`, and `attribute` — but in the real AST, a class doesn't directly contain methods. The actual path is `class > body > declaration_list > method > attributes > attribute`. The projected output skips all of those intermediate nodes, creating parent-child relationships (e.g., `class/name` directly above `method/name`) that never occur in the language grammar.

This *may* be a problem for tractor's grammar-aware JSON rendering. The rendering heuristics currently rely on parent-child relationships defined by the language grammar to decide whether nodes should be rendered as properties or collection members. Projected trees create novel parent-child combinations that don't exist in the grammar. However, it's an open question whether these heuristics actually need parent-child context, or whether they could make reasonable decisions based on individual node characteristics alone. If the latter, projected trees could still produce sensible JSON output. This hasn't been verified.

**Key realization**: The question of whether to skip intermediate nodes or retain them is itself a design choice:

- **Steiner tree**: Keep matched nodes *plus* all the connective tissue between them (every node on the path between any two matches). Intermediate nodes like `body` and `declaration_list` would appear in the output even though nobody asked for them. Grammar-aware rendering would still work, since all parent-child relationships are real.
- **Projected tree**: Keep *only* matched nodes, derive parent-child edges from ancestry in the original tree. Node X becomes parent of node Y in the output if X is an ancestor of Y in the original and no other matched node sits between them. Cleaner output, but breaks grammar-aware rendering.

For this particular use case, projected tree seems like the right choice — the `body` node between `class` and `method` adds no useful information. But this isn't always true. A user exploring an unfamiliar AST might want to see the full path to understand the actual structure. And if you choose the Steiner tree variant (retain all intermediate nodes), Approach 2 collapses into Approach 1 — you're just selecting leaves and having the tool fill in ancestors. The explicit union selection only adds something distinct when generations are actually skipped, which is the projected tree variant. So the two approaches are only meaningfully different in the projected case, which is also the case with the open question around grammar-aware rendering.

### Approach 3: Ancestor path per match

**Idea**: Keep the flat match list, but add a computed field to each match entry containing its ancestor path.

```json
{
  "matches": [
    {
      "tree": { ... },
      "ancestors": ["ProspectNoteTests", "ProspectNoteHappyFlow"]
    }
  ]
}
```

Computed only when requested (e.g., `-v ancestors`). Fits the existing model — `-v` picks which field to render.

**Assessment**: Pragmatic and non-breaking, but pushes all the structural nesting work to the consumer. The Node script would still need to group matches by their ancestry arrays to reconstruct the hierarchy. It solves the "which method does this attribute belong to?" problem but doesn't give you a hierarchical output directly.

### Approach 4: Multi-level grouping (inside-out)

**Idea**: Generalize the existing `-g file` to accept XPath expressions, and allow stacking multiple `-g` flags for nested grouping. Each `-g` evaluates an XPath relative to each match to determine its group key.

```bash
tractor "**/*Tests.cs" \
  -x "//method[attributes/attribute[name/ref='Test']]/attributes/attribute" \
  -g file \
  -g "ancestor::class/name" \
  -g "ancestor::method/name" \
  -f json
```

Output:
```json
{
  "groups": [{
    "file": "ProspectNoteTests.cs",
    "groups": [{
      "group": "ProspectNoteTests",
      "groups": [{
        "group": "ProspectNoteHappyFlow",
        "matches": [...]
      }]
    }]
  }]
}
```

**Assessment**: Conceptually clean — natural generalization of an existing feature. But it works **inside-out**: you match the deepest nodes first, then group upward by ancestors. This is technically difficult because tractor currently detaches matches from their document context before report rendering and grouping. Reconstructing ancestry from detached leaf nodes would require maintaining full document context per match without memory overload.

**Key insight**: Inside-out querying where you match leaves and then group by sparse ancestor paths is just a difficult way to describe what is fundamentally a **top-down multi-level join**.

### Approach 5: Chained `-x` / `-X` (outside-in) [SELECTED]

**Idea**: Allow multiple `-x` flags where each narrows within the results of the previous one. The first `-x` selects the outermost nodes, the second `-x` is evaluated within each match of the first, and so on.

```bash
tractor "**/*Tests.cs" \
  -x "//class" \
  -x ".//method[attributes/attribute[name/ref='Test']]" \
  -x "./attributes/attribute"
```

Each `-x` produces a level of nesting in the output. Matches at each level retain their full subtree, so the next query has all the context it needs.

**Text output**:
```
backend/.../ProspectNoteTests.cs
  class ProspectNoteTests
    method ProspectNoteHappyFlow
      Test
      Title("Standard Prospect Note Process")
      Category(TestCategories.HappyFlow)
      Description(@"Verifies that when a prospect note is created: ...")
```

**JSON output**:
```json
{
  "matches": [
    {
      "file": "backend/.../ProspectNoteTests.cs",
      "line": 7,
      "tree": { "$type": "class", "name": "ProspectNoteTests" },
      "matches": [
        {
          "line": 8,
          "tree": { "$type": "method", "name": "ProspectNoteHappyFlow" },
          "matches": [
            { "line": 9,  "tree": { "$type": "attribute" } },
            { "line": 10, "tree": { "$type": "attribute" } },
            { "line": 11, "tree": { "$type": "attribute" } },
            { "line": 12, "tree": { "$type": "attribute" } }
          ]
        }
      ]
    }
  ]
}
```

Same recursive shape at every level. Each `matches` array holds the next level's results within its parent.

### The projection problem

Chained `-x` solves the hierarchy, but not the "with the information you want at each level" part. Each `-x` says *what to match* but not *what to extract from each match*. If every level retains its full AST subtree, the output is massively redundant — the class-level tree already contains all methods and attributes, then the method level repeats a subset, and the attribute level repeats it again.

What you actually need at non-leaf levels is minimal: just enough to identify the node (its name, perhaps its type). The full subtree is only useful at the leaf level. But deciding *what* to extract is language-specific — C# classes have a `name` child, but SQL statements, YAML mappings, and JSON objects identify themselves differently. A built-in heuristic for "extract the name" would break down across languages.

This calls for a **per-level projection** — a way to specify what to extract from each match:

```bash
tractor "**/*Tests.cs" \
  -x "//class" -p "./name" \
  -x ".//method[attributes/attribute[name/ref='Test']]" -p "./name" \
  -x "./attributes/attribute"
```

`-x` says which nodes to match. `-p` (projection) says what to extract from each matched node, as a relative XPath. When `-p` is omitted (the leaf level), the full subtree is returned. Output becomes:

```json
{
  "matches": [
    {
      "value": "ProspectNoteTests",
      "line": 7,
      "matches": [
        {
          "value": "ProspectNoteHappyFlow",
          "line": 8,
          "matches": [
            { "line": 9,  "tree": { "$type": "attribute", ... } },
            { "line": 10, "tree": { "$type": "attribute", ... } }
          ]
        }
      ]
    }
  ]
}
```

No language heuristics, fully user-controlled.

### The two-languages tension

There is an uncomfortable tension here: we're using XPath in two different roles — once for matching (`-x`) and once for projecting (`-p`). Combined with the chaining semantics (join pipeline) and the join type modifier (`-X` for outer join), we've essentially built a mini query language *on top of* a query language. Each level has a FROM/WHERE clause (`-x`) and a SELECT clause (`-p`). It's SQL for trees, expressed through CLI flags.

XPath 3.1 can actually express projections and nested structures natively — it has `map{}`, `array{}`, and `let` bindings. The entire query could theoretically be written as:

```xpath
//class / map {
  "name": ./name,
  "methods": .//method[attributes/attribute[name/ref='Test']] / map {
    "name": ./name,
    "attributes": ./attributes/attribute
  }
}
```

But this is XQuery territory. Tractor uses XPath for node selection, not for constructing output. Implementing XQuery would be a massive leap in complexity.

The `-x`/`-p` chaining can be seen as the **smallest possible shim** to bridge the gap between XPath's selection power and structured output, without going full XQuery. The user isn't learning two different languages — they're learning one language (XPath) used in two positions, with CLI flags providing the structural glue that XPath alone can't express. Whether that pragmatic compromise is acceptable, or whether it's a sign that tractor should eventually support XQuery-style output construction, is an open question.

## Join Semantics

Chained `-x` is fundamentally a **join pipeline**:

```
files  ⟕  query1  ⟕  query2  ⟕  query3
       -x         -x         -x
```

Every boundary between levels has the same semantics. The file level is implicitly the first level, and current single-`-x` tractor is already `files -x query` — a single inner join, which is why files with zero matches are never shown.

### Inner vs outer join

By default, chained `-x` is an **inner join**: a parent is only shown if the next level produces results within it. This is the natural behavior for extraction ("show me classes that have test methods").

But sometimes you want a **left outer join**: show all matches at this level even if deeper levels are empty ("show me all classes, and within each, show me test methods if any").

Since `-x A -x B` is *not* the same as `-x A/B` (the former keeps intermediate state; the latter flattens to only B results), the distinction matters. A single XPath `A/B` already functions as an inner join — it only returns results where B exists under A. The chained form can do either.

**Proposed syntax**: `-X` (capital) for left outer join at that level:

```bash
# Inner join: only classes with test methods
tractor "**/*Tests.cs" \
  -x "//class" \
  -x ".//method[attributes/attribute[name/ref='Test']]" \
  -x "./attributes/attribute"

# Outer join at class level: all classes, test methods if any
tractor "**/*Tests.cs" \
  -X "//class" \
  -x ".//method[attributes/attribute[name/ref='Test']]" \
  -x "./attributes/attribute"
```

`-X` is a light visual cue: "this level is wider, it keeps everything." The join type is specified per-level, so you can mix inner and outer joins in the same pipeline.

### Consistency with file grouping

This model implies that `-g file` should be the **default** behavior, not an opt-in. The file is always the first level of the join. Current behavior (flat match list with file info per match) is the odd special case where the file level has been collapsed away.

Consistent behavior:
```
files -x query → grouped by file (it's a join, files are the first level)
```

This would be a breaking change, but it's the conceptually correct default. The flat list could be an opt-out (`--flat`).

## Approach 6: XPath `map{}` expressions (in progress)

The two-languages tension in Approach 5 points toward a solution that was hiding in plain sight: XPath 3.1 already has a `map{}` construct for building structured output. Rather than inventing CLI-level projection flags, the matching *and* the shaping can live in a single XPath expression:

```bash
tractor "**/*Tests.cs" -x "//class / map {
  'name': ./name,
  'methods': .//method[attributes/attribute[name/ref='Test']] / map {
    'name': ./name,
    'attributes': ./attributes/attribute
  }
}"
```

This is a single `-x` that produces nested structured output. Each `map{}` selects a node set and projects named fields from each match. The nesting is expressed inline — no chained flags, no separate projection syntax, no join semantics encoded in CLI conventions.

**What this resolves**:
- The two-languages problem disappears. There's one language (XPath), used in one place (`-x`), doing both selection and projection.
- Per-level projection is native — each `map{}` declares exactly what fields to extract.
- The hierarchy is explicit in the expression itself, not inferred from flag ordering.
- Inner/outer join semantics fall out of XPath's existing behavior: if the method selector returns nothing, the `methods` key gets an empty sequence (outer join), or you can wrap it in a predicate to filter (inner join).

**What it requires**:
- Tractor needs to evaluate XPath `map{}` expressions and serialize the resulting maps to JSON. This is a significant extension to XPath evaluation, but it's a well-specified part of XPath 3.1, not an ad-hoc invention.
- The output is no longer a list of AST node matches — it's a constructed data structure. This changes what a "result" is in tractor's model: from "here are the nodes I found" to "here is the data I built."
- Text rendering of maps needs consideration — JSON is a natural fit, but the indented text output that works well for node lists may need a different approach for arbitrary maps.

**Comparison with Approach 5**: Where chained `-x`/`-p` encodes the query structure in CLI flags (the *how* is outside the query), `map{}` encodes it in the expression itself (the *how* is the query). This is more expressive and more composable, but also more complex for simple cases. A user who just wants to find all classes still writes `-x "//class"` — `map{}` is the tool you reach for when flat results aren't enough.

This approach is currently being implemented.

## Design Goals and How Each Approach Meets Them

| Goal | #1 sparse | #2 outline | #3 ancestors | #4 grouping | #5 -x chain | #6 map{} |
|---|---|---|---|---|---|---|
| Easy to understand | Moderate | Moderate | Simple | Simple | Simple | Moderate |
| Conceptually consistent | Conflates axes | New concept | Additive | Extends existing | Extends existing | Native XPath |
| Preserves grammar rendering | Breaks it | Open question | No impact | No impact | No impact | N/A (new output type) |
| Implementation feasibility | Hard | Hard | Easy | Hard (ancestry) | Natural (top-down) | Moderate (XPath 3.1) |
| Hierarchical output | Yes | Yes | Consumer's job | Yes | Yes | Yes |
| Controls join type | No | No | No | No | Yes (-x/-X) | Yes (XPath predicates) |
| Per-level projection | No | No | No | No | Yes (-p) | Yes (map keys) |
| Single language | Yes | Yes | Yes | Yes | No (-x + -p) | Yes |

## Summary

The core need is extracting a **hierarchical structure** from source code: selecting nodes at multiple levels of the AST and preserving their containment relationships.

The exploration revealed two orthogonal dimensions in tree querying that are easy to conflate:
- **Direction**: supertree (ancestors above match) vs subtree (descendants below match)
- **Density**: full tree vs sparse/pruned tree

The inside-out approach (match leaves, reconstruct ancestry) and the outside-in approach (match outermost, drill down) produce equivalent results but have very different implementation profiles. Inside-out querying with sparse parent trees is fundamentally just a convoluted way to describe a top-down multi-level inner join — which is exactly what chained `-x` expresses directly.

The chained `-x`/`-X` design (Approach 5) provides strong pragmatic value:
- A consistent mental model: everything is a join, from files down to leaf nodes
- Natural top-down implementation: each level has the full subtree available for the next query
- Per-level join type control with a minimal syntax distinction (`-x` inner, `-X` outer)
- Per-level projection control (`-p`) so each level carries only the data the user wants
- No interference with grammar-aware rendering, since each match retains its full AST subtree
- Recursive output shape in JSON, trivially consumable by scripts

However, it introduces a tension: XPath is used in two roles (matching via `-x` and projecting via `-p`), with CLI flags providing the structural glue. This amounts to a mini query language built on top of a query language.

The XPath `map{}` approach (Approach 6, currently in progress) resolves this tension by keeping everything in a single language. The matching, projection, and hierarchical shaping are all expressed in one XPath expression. This is more expressive and more composable, though it shifts complexity from CLI ergonomics to query authoring. For simple node selection, single `-x` remains the right tool. For structured multi-level extraction — the use case that motivated this entire analysis — `map{}` appears to be the natural fit.
