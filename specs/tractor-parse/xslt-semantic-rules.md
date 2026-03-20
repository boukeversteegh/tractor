# XSLT for Semantic Transformation Rules — Feasibility Analysis

## Context

Tractor's semantic transforms convert TreeSitter's raw syntax tree into
query-friendly XML. Today these rules are implemented as imperative Rust
functions (one per language) that walk the xot tree and mutate it in place.

This document explores whether XSLT could replace the imperative Rust code,
and what trade-offs that would involve.

## Current Architecture

```
Source Code → TreeSitter → Raw XML (xot tree) → walk_transform(lang_fn) → Semantic XML → XPath queries
```

Each language module (e.g. `languages/csharp.rs`) implements a
`transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction>` function.
The generic walker in `xot_transform.rs` calls this for every element node,
and the function returns one of:

| Action      | Meaning |
|-------------|---------|
| `Continue`  | Recurse into children normally |
| `Skip`      | Detach node, promote children to parent |
| `Flatten`   | Transform children first, then promote to parent |
| `Done`      | Node fully handled, don't recurse |

Within each function, the transform logic includes:
1. **Renaming** — `class_declaration` → `class`, `method_declaration` → `method`
2. **Flattening** — Remove wrapper nodes like `declaration_list`, `block`
3. **Skipping** — Remove nodes like `expression_statement`
4. **Modifier lifting** — `<modifier>public</modifier>` → `<public/>`
5. **Identifier classification** — Classify `<identifier>` as `<name>`, `<type>`, or `<ref>` based on parent/sibling context
6. **Operator extraction** — Extract `+`, `-`, `=` from text children into `<op>` elements
7. **Structural rewriting** — Generic types, nullable types, name inlining

## What XSLT Can Express Well

XSLT is a tree-transformation language purpose-built for converting one XML
structure into another. The following transform categories map naturally:

### 1. Element Renaming (trivial in XSLT)

```xslt
<!-- class_declaration → class -->
<xsl:template match="class_declaration">
  <class>
    <xsl:apply-templates select="@*|node()"/>
  </class>
</xsl:template>
```

This is XSLT's bread and butter. The `map_element_name()` lookup tables in each
language module (50-80 entries per language) translate directly to XSLT template
rules — one per mapping, or a single template with an `xsl:choose`.

### 2. Flattening / Skipping (natural in XSLT)

```xslt
<!-- Skip: promote children, discard wrapper -->
<xsl:template match="expression_statement">
  <xsl:apply-templates/>
</xsl:template>

<!-- Flatten: same effect — children bubble up to parent -->
<xsl:template match="declaration_list">
  <xsl:apply-templates/>
</xsl:template>
```

The distinction between `Skip` and `Flatten` (whether children are transformed
before or after promotion) is handled automatically by XSLT's recursive
`apply-templates` model.

### 3. Modifier Lifting (straightforward)

```xslt
<!-- <modifier>public</modifier> → <public/> -->
<xsl:template match="modifier[. = 'public' or . = 'private' or . = 'static'
                              or . = 'async' or . = 'abstract' or . = 'virtual'
                              or . = 'override' or . = 'sealed' or . = 'readonly'
                              or . = 'const' or . = 'partial']">
  <xsl:element name="{normalize-space(.)}"/>
</xsl:template>
```

### 4. Operator Extraction (possible but awkward)

```xslt
<!-- Extract operator text from binary_expression -->
<xsl:template match="binary_expression">
  <binary>
    <op>
      <xsl:value-of select="text()[not(. = '(' or . = ')' or . = ',' or . = ';')]"/>
    </op>
    <xsl:apply-templates select="*"/>
  </binary>
</xsl:template>
```

Filtering text nodes by content is doable in XSLT but less elegant than in Rust.

## What XSLT Struggles With

### 5. Context-Dependent Identifier Classification (the hard part)

The most complex logic in the Rust transforms is `classify_identifier()`, which
decides whether an `<identifier>` should become `<name>`, `<type>`, or `<ref>`
based on:

- Parent element kind (is it a `class_declaration`? `parameter`? `variable_declarator`?)
- Grandparent context (is the parent a `<name>` wrapper inside a declaration?)
- Following siblings (does a `parameter_list` follow this identifier?)
- Ancestor chain (are we inside a `namespace_declaration`?)
- Attribute values (`field="type"`)

In XSLT, this requires deeply nested XPath predicates:

```xslt
<!-- Identifier as name: in a declaration context -->
<xsl:template match="identifier[
    parent::class_declaration or parent::method_declaration
    or parent::variable_declarator or parent::parameter
    or (parent::name and (
        parent::name/parent::class_declaration
        or parent::name/parent::method_declaration
    ))
    or (following-sibling::parameter_list or following-sibling::parameters)
]">
  <name><xsl:apply-templates/></name>
</xsl:template>

<!-- Identifier as type: field="type" attribute or type context -->
<xsl:template match="identifier[@field='type']
                   | identifier[parent::type_argument_list]
                   | type_identifier | predefined_type">
  <type><xsl:apply-templates/></type>
</xsl:template>

<!-- Identifier default: ref -->
<xsl:template match="identifier">
  <ref><xsl:apply-templates/></ref>
</xsl:template>
```

This is expressible but fragile. The match patterns grow long and must be ordered
carefully for XSLT's conflict-resolution rules. Each language's classification
logic is different, and C#'s is particularly complex (checking grandparents,
walking ancestors for namespace context, checking sibling types).

### 6. Structural Rewriting (possible but verbose)

Complex rewrites like generic types and nullable types require multi-step
restructuring:

```
Input:  <generic_name><identifier>List</identifier><type_argument_list>...</type_argument_list></generic_name>
Output: <type><generic/>List<arguments>...</arguments></type>
```

In XSLT this requires extracting children selectively, creating new text nodes,
and reordering — doable but verbose compared to the imperative Rust version that
directly manipulates the tree.

### 7. Name Inlining (somewhat awkward)

Converting `<name><identifier>Foo</identifier></name>` to `<name>Foo</name>`
(only in declaration contexts) requires context-aware templates:

```xslt
<xsl:template match="name[parent::class_declaration or parent::method_declaration]">
  <name>
    <xsl:value-of select="identifier"/>
  </name>
</xsl:template>
```

Straightforward for this case, but the logic varies by language (Python checks
`function_definition | class_definition`, Go checks `type_spec`, etc.).

## XSLT Runtime Options in Rust

| Option | XSLT Version | Status | Fit for Tractor |
|--------|-------------|--------|-----------------|
| **[Xee](https://github.com/Paligo/xee)** | 3.0 (partial) | Active, backed by Paligo | **Best candidate** — tractor already depends on xee for XPath. Same bytecode VM. But XSLT support is highly incomplete (921/14,595 tests passing). |
| **[xrust](https://github.com/ballsteve/xrust)** | 1.0 complete, 3.0 WIP | Active | XSLT 1.0 is complete but uses its own data model (not xot). Would require serializing xot→XML→xrust→XML→xot round-trip. |
| **[libxslt](https://github.com/KWARC/rust-libxslt)** | 1.0 (via C FFI) | Dormant | C dependency (libxml2/libxslt). Proof of concept only. Not suitable. |

**Key insight**: Tractor already uses `xot` (the XML arena from xee) as its
internal tree representation, and `xee-xpath` for XPath evaluation. Xee's XSLT
engine operates on the same `xot` tree, meaning there would be **zero
serialization overhead** if xee's XSLT were used. However, xee's XSLT support
is early-stage and missing critical features.

## Feasibility Assessment

### What Percentage of Rules Can XSLT Handle?

Analyzing across all 14 language modules:

| Rule Category | % of total logic | XSLT fit | Notes |
|---------------|-----------------|----------|-------|
| Element renaming | ~40% | Excellent | Direct template match → new element |
| Skip/Flatten | ~15% | Excellent | `<xsl:apply-templates/>` without wrapper |
| Modifier lifting | ~10% | Good | `<xsl:element name="{.}"/>` |
| Operator extraction | ~10% | Adequate | Text node filtering is clunky |
| Identifier classification | ~15% | Poor | Complex context-dependent logic |
| Structural rewriting | ~10% | Adequate | Verbose but expressible |

**~65% of the logic** maps naturally to XSLT. The remaining ~35% is expressible
but results in complex, hard-to-maintain match patterns.

### Architecture: Pure XSLT vs Hybrid

**Option A: Pure XSLT** — Replace all Rust transform code with `.xslt` files.

- Pro: Single declarative language for all rules. Easy for non-Rust contributors.
- Con: Identifier classification becomes fragile XPath spaghetti. Each language
  needs a separate stylesheet (14 files). Testing/debugging XSLT is harder
  than Rust match arms.

**Option B: Hybrid** — Use XSLT for the ~65% that maps well (renaming, skipping,
flattening, modifier lifting), keep Rust for context-dependent logic
(identifier classification, complex rewrites).

- Pro: Best of both worlds. Declarative rules stay declarative, imperative
  logic stays imperative.
- Con: Two transformation passes (XSLT then Rust, or vice versa). More complex
  architecture. Debugging spans two systems.

**Option C: Declarative Rust DSL** — Instead of XSLT, define a Rust DSL or
data-driven config (TOML/JSON) for the declarative parts (rename maps,
skip/flatten lists, modifier lists), keep imperative Rust for the rest.

- Pro: No new runtime dependency. Same debugging story. Compile-time checked.
  Simpler than XSLT for what tractor actually needs.
- Con: Still Rust code (not a standard like XSLT). Less powerful than XSLT
  for future complex transforms.

## Performance Considerations

The current Rust transforms are **single-pass, in-place mutations** on the xot
arena — essentially pointer manipulation. This is extremely fast.

XSLT transforms, even with xee's shared xot representation, would:
1. Need to construct a new output tree (XSLT is functional, not mutating)
2. Evaluate XPath match patterns for every node against every template rule
3. Involve template conflict resolution overhead

For tractor's use case (small-to-medium source files, interactive CLI), the
performance difference is likely negligible. But it's worth noting that XSLT
adds overhead that doesn't exist today.

## Recommendation

**Not recommended at this time**, for three reasons:

1. **No production-ready XSLT engine in Rust** — Xee's XSLT passes only 6% of
   conformance tests. xrust would require a serialize/deserialize round-trip.
   libxslt is a dormant C FFI wrapper. None are ready for production use.

2. **Marginal benefit for the complex parts** — The parts of the transform logic
   that are most painful in Rust (identifier classification, context-dependent
   rewrites) are also the parts that are most painful in XSLT. Switching
   languages doesn't simplify the hard logic — it just changes which language
   it's hard in.

3. **The easy parts are already easy** — Element renaming is a lookup table in
   Rust (`map_element_name`). Skipping/flattening is a single enum variant.
   These are already concise and readable. XSLT would make them more
   _standardized_ but not meaningfully simpler.

### What To Watch

- **Xee XSLT maturity** — If xee reaches >80% conformance, revisit this
  analysis. Since tractor already depends on xee/xot, an XSLT integration
  would be nearly zero-cost in terms of dependencies and serialization.

- **Declarative Rust DSL** — If the number of languages grows significantly
  (beyond 22) and the rename/skip/flatten rules become burdensome, a
  data-driven approach (TOML config for rename maps, skip lists) within Rust
  could capture 80% of the benefit of XSLT without the runtime dependency.

### If We Were To Prototype

The lowest-risk way to test XSLT viability would be:

1. Pick a simple language (Go: ~195 lines, minimal context-dependent logic)
2. Write a `go.xslt` stylesheet that handles renaming + skip/flatten
3. Keep `classify_identifier` in Rust as a post-pass
4. Compare output against existing test snapshots
5. Measure: lines of code, readability, performance

This would validate whether the hybrid approach (Option B) delivers real value
before committing to a larger migration.
