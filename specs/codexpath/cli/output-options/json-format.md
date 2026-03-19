---
title: JSON Format
priority: 2
---

Output structured JSON with objects containing match metadata and tree data.

## JSON Tree Serialization

When the tree view (`-v tree`) is active, the JSON output includes a `tree` field
with a structured representation of the semantic tree. This section defines how
the XML-based semantic tree is serialized to JSON.

### Design Principles

#### P1: Grammar-Based Singleton Detection

The decision to lift a child element as a direct JSON property is based on
tree-sitter's `field` attribute — a grammar-level signal that a child slot
appears at most once per parent. Elements with `field` are singletons and
become direct JSON properties. Elements without `field` go into a `children`
array.

This is purely grammar-based: no content-based heuristics, no inspecting
how many children exist at runtime, no reliance on `node-types.json`.

#### P2: Cardinality-Independence

The JSON shape of a node must not depend on how many children it has.
A class with 1 method and a class with 3 methods must produce the same
structure for the `body` field — always a `children` array. This ensures
that jq queries and JSON schemas work uniformly regardless of input.

**Consequence:** Singleton wrapper lifting (`lift_singleton_children`) must
only apply to wrappers whose child is grammatically guaranteed to be a
singleton (e.g. `value`, `returns`, `condition`). Wrappers like `body` whose
children can vary in count after transforms must not participate in singleton
child lifting.

### Serialization Rules

1. **`$type` instead of `type`**: Frees `type` for semantic use (e.g.
   property type annotations in C#). Only emitted on `children` array
   items, not on field-lifted properties.

2. **Modifiers/flags**: Self-closing elements become boolean properties.
   `<public/>` becomes `"public": true`.

3. **Leaf collapsing**: Field-backed text-only leaves collapse to plain strings.
   `<name field="name">Foo</name>` gives parent `"name": "Foo"`.

4. **Non-field text-only leaves**: Keep compact form with element name as key.
   `<accessor>get;</accessor>` becomes `{"accessor": "get;"}` (no `$type`).
   Only structural non-field children get `$type`.

5. **Field-backed structural nodes**: Become objects WITHOUT `$type`.
   `<body field="body">...</body>` gives parent `"body": { "children": [...] }`.

### Example

```json
{
  "$type": "class",
  "public": true,
  "name": "Foo",
  "body": {
    "children": [
      { "$type": "property", "public": true,
        "type": "int", "name": "Bar",
        "accessors": { "children": [
          { "accessor": "get;" },
          { "accessor": "set;" }
        ]}
      },
      { "$type": "method", "public": true,
        "name": "Baz",
        "returns": { "children": [{ "$type": "void" }] },
        "parameters": "()",
        "body": { "children": [{ "block": "{ }" }] }
      }
    ]
  }
}
```

Key observations:
- `name`, `body`, `returns`, `parameters` are lifted as direct properties (grammar singletons)
- `property` and `method` are in `children` array with `$type` (not singletons)
- `public` is a boolean flag (self-closing modifier)
- `"type": "int"` uses `type` as a semantic key (freed by `$type` for node identification)
- `accessor` uses compact text form (no `$type`)

### Implementation

- **Singleton detection**: `field` attribute on XML elements, set by `xot_builder.rs`
- **Singleton wrapper lifting**: `lift_singleton_children` in `xot_transform.rs`,
  controlled by `DEFAULT_SINGLETON_WRAPPERS` list (excludes `body`)
- **JSON conversion**: `xml_to_json.rs` partitions children by `field` attribute
- **YAML output**: Reuses the same JSON value tree, so all rules apply equally
