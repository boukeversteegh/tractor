# JSON/YAML property lifting: field-based singleton detection

## Context

JSON/YAML output is verbose and hard to query. The root cause: everything goes into a flat
`{"type": "...", "children": [...]}` structure. Querying with jq requires painful chains like
`.children[] | select(.type=="method") | .children[] | select(.type=="name")`.

Tree-sitter distinguishes **fields** (named slots, always 0-1 per parent) from **children**
(unnamed, can repeat). Fields map to tractor's WRAPPED_FIELDS (`name`, `body`, `parameters`,
etc.) and non-wrapped `field="..."` attributes. This is exactly the singleton vs list boundary.

By lifting field-backed elements to direct JSON properties, we get clean jq paths like
`.body.children[].name` while keeping `children` arrays only where elements genuinely repeat.

## Design decisions

1. **Singleton detection**: Element has `field` attribute -> singleton -> lift to property.
   No `field` -> `children` array. No content-based guessing, no node-types.json.
2. **`$type` instead of `type`**: Frees `type` for semantic use (e.g. property type annotation).
   Only emitted on `children` array items, not on field-lifted properties.
3. **Modifiers/flags**: Unchanged. `<public/>` -> `"public": true`.
4. **Leaf collapsing**: Field-backed text-only leaves collapse to plain strings.
   `<name field="name">Foo</name>` -> parent gets `"name": "Foo"`.
5. **Non-field text-only leaves**: Keep compact form `{"accessor": "get;"}` (element name
   as key, no `$type`). Only structural non-field children get `$type`.

## Expected output

Current:
```json
{
  "type": "class",
  "public": true,
  "children": [
    { "name": "Foo" },
    { "type": "body", "children": [
      { "type": "property", "public": true, "children": [
        { "type": "int" },
        { "name": "Bar" },
        { "type": "accessors", "children": [
          { "accessor": "get;" },
          { "accessor": "set;" }
        ]}
      ]},
      { "type": "method", "public": true, "children": [
        { "type": "returns", "children": [{ "type": "void" }] },
        { "name": "Baz" },
        { "parameters": "()" },
        { "type": "body", "children": [{ "block": "{ }" }] }
      ]}
    ]}
  ]
}
```

After:
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

## Implementation

### Step 1: Add `field` attribute to WRAPPED_FIELDS wrapper elements

**File:** `tractor/src/xot_builder.rs`

Currently, wrapper elements (line 216-230 native, ~340 WASM) don't get a `field` attribute.
Non-wrapped fields do (line 232-234). Add `field="<fieldname>"` on wrapper elements too, after
creating the wrapper and copying location attributes:

```rust
// After copying location attrs to wrapper:
let field_attr = self.get_name("field");
self.xot.attributes_mut(wrapper).insert(field_attr, field.to_string());
```

Do this in both `build_node()` (native) and `build_serialized_node()` (WASM).

**Note:** `rename()` in `xot_transform.rs` (line 167-172) strips `field` when it matches the
new element name. Wrapper elements are never passed to `rename()` by transforms (transforms
target inner TS-backed nodes by `kind`), so this is safe.

### Step 2: Include `field` attribute in XML fragment output

**File:** `tractor/src/output/xml_renderer.rs`

Line 436-442 strips `kind`, `start`, `end`, etc. when `include_locations=false`. Currently
`field` is NOT in this strip list (it's already included in output). Verify this — no change
may be needed.

Actually, checking the code: only `start|end|startLine|startCol|endLine|endCol|kind` are
stripped. `field` survives. But double-check that `xot.to_string()` (used in engine.rs line
170 to produce the xml_fragment) also includes it — `to_string()` serializes all attributes.

### Step 3: Rewrite `xml_to_json.rs` for field-based partitioning

**File:** `tractor/src/output/xml_to_json.rs`

This is the core change. Three sub-changes:

**3a. Change `KEY_TYPE` constant:**
```rust
const KEY_TYPE: &str = "$type";  // was "type"
```

**3b. Add `field` to `JsonNode` and introduce `ChildEntry`:**
```rust
struct ChildEntry {
    field: Option<String>,
    value: Value,
}

struct JsonNode {
    name: String,
    field: Option<String>,           // NEW
    flags: Vec<String>,
    content_children: Vec<ChildEntry>, // CHANGED from Vec<Value>
    children_truncated: bool,
}
```

**3c. Read `field` attribute during XML parsing:**
In `Event::Start(e)` handler, extract the `field` attribute:
```rust
let field = e.attributes()
    .filter_map(|a| a.ok())
    .find(|a| a.key.as_ref() == b"field")
    .and_then(|a| String::from_utf8(a.value.to_vec()).ok());
```

**3d. Rewrite `into_value()`:**

The key logic change. When building the JSON object:
- Field-backed text-only leaf: return just `Value::String(text)` (parent lifts it)
- Field-backed structural node: return object WITHOUT `$type` (parent lifts by field name)
- Non-field node: return object WITH `$type` (goes in children array, needs identification)
- Partition children: field-backed children become direct properties, rest go in `children` array
- Anonymous text tokens: still dropped in structural content (syntactic noise)

When popping from stack on `Event::End`:
```rust
let node = stack.pop().unwrap();
let field = node.field.clone();
let val = node.into_value();
parent.content_children.push(ChildEntry { field, value: val });
```

### Step 4: Update snapshots and tests

Run `task test:snapshots:update` to regenerate all JSON/YAML snapshots.

Add unit tests in `xml_to_json.rs` for:
- Field-backed text leaf -> string value
- Field-backed structural node -> object without $type
- Non-field child -> object with $type in children array
- Mixed field + non-field children
- Flags still work as boolean properties

### Step 5: Verify YAML

YAML renderer reuses `match_to_value()` which calls `xml_fragment_to_json()`. Changes
propagate automatically. Run YAML tests to confirm.

## Key files

- `tractor/src/xot_builder.rs` — add `field` attr to wrappers (lines 216-230, ~340)
- `tractor/src/output/xml_to_json.rs` — core rewrite
- `tractor/src/output/xml_renderer.rs` — verify `field` not stripped
- `tractor/src/pipeline/format/json.rs` — verify no changes needed
- `tests/integration/formats/json/query.json` — primary snapshot to verify

## Verification

1. `cargo test` — all unit tests pass
2. `task test:snapshots:update` — regenerate snapshots
3. `task test` — full suite including integration tests
4. Manual: `tractor -x "//class" samples/sample.cs -f json -v tree` — verify output matches expected
5. Manual: `tractor -x "//method[1]" samples/sample.cs -f yaml -v tree` — verify YAML
6. Spot check jq: `tractor ... -f json | jq '.matches[0].tree.body.children[0].name'`

## Related

- todo/5: C# generic_name transform loses metadata (upstream fix, separate from this work)
