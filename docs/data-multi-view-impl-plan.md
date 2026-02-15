# Implementation Plan: Multi-view XML for Data Sources

## Summary

Add dual-branch XML output (`<ast>` + `<data>`) for JSON and YAML files.
This gives users two views of the same source: a lossless normalized AST for
structural inspection, and a query-friendly data projection for value extraction.

**Scope**: JSON and YAML only. TOML/INI/.env can follow later.

---

## Architecture Overview

### Current pipeline

```
Source → TreeSitter → raw xot tree → walk_transform(lang_fn) → single tree
```

The raw tree is modified **in-place** by the transform. For YAML, this
destructively converts the syntax tree into a data view (keys→elements).
For JSON, no transform runs at all.

### New pipeline (data-aware languages only)

```
Source → TreeSitter → raw xot tree
                          │
                          ├─ clone_node() → AST copy → walk_transform(ast_transform) → <ast>
                          │
                          └─ original tree → walk_transform(data_transform) → <data>
                          │
                          └─ assemble: <File kind="data" format="json|yaml">
                                         <ast>...</ast>
                                         <data>...</data>
                                       </File>
```

**Key insight**: xot's `clone_node()` does a deep clone of a subtree within
the same arena. We clone the content root, apply the AST-normalizing transform
to the clone, apply the data-projecting transform to the original, then
wrap each in `<ast>`/`<data>` under the `<File>` element.

---

## Detailed Steps

### Step 1: Introduce "syntax category" concept for language dispatch

**File**: `tractor-core/src/languages/mod.rs`

Add a function to classify languages:

```rust
pub enum SyntaxKind {
    /// Programming language (single transform, current behavior)
    Code,
    /// Data/config format (dual-branch: ast + data)
    Data { format: &'static str },
    /// No transform (raw TreeSitter passthrough)
    Raw,
}

pub fn get_syntax_kind(lang: &str) -> SyntaxKind {
    match lang {
        "json" => SyntaxKind::Data { format: "json" },
        "yaml" | "yml" => SyntaxKind::Data { format: "yaml" },
        // future: "toml", "ini", "env"
        "typescript" | "ts" | ... => SyntaxKind::Code,
        _ => SyntaxKind::Raw,
    }
}
```

Also add a second dispatch function for data languages that returns **two**
transform functions:

```rust
pub fn get_data_transforms(lang: &str) -> Option<(TransformFn, TransformFn)> {
    match lang {
        "json" => Some((json::ast_transform, json::data_transform)),
        "yaml" | "yml" => Some((yaml::ast_transform, yaml::data_transform)),
        _ => None,
    }
}
```

### Step 2: Create JSON transform module

**New file**: `tractor-core/src/languages/json.rs`

TreeSitter JSON grammar produces these node kinds:
- `document`, `object`, `pair`, `array`, `string`, `number`, `true`, `false`, `null`
- Fields: `key` (on pair→string), `value` (on pair→value node)

#### `json::ast_transform`

Normalizes TreeSitter nodes into the spec's unified AST vocabulary:

| TreeSitter kind | Target element | Notes |
|-----------------|---------------|-------|
| `document`      | (flatten)     | Remove wrapper, promote children |
| `object`        | `<object>`    | Keep as-is (already named correctly) |
| `array`         | `<array>`     | Keep as-is |
| `pair`          | `<property>`  | Rename; existing `key`/`value` field wrappers from TreeBuilder already present |
| `string`        | `<string>`    | Strip surrounding quotes from text content |
| `number`        | `<number>`    | Keep |
| `true`/`false`  | `<bool>`      | Rename, keep text content |
| `null`          | `<null>`      | Rename, keep text content |

Also: remove all punctuation text nodes (`{`, `}`, `[`, `]`, `,`, `:`).

The result for `{"name": "John", "age": 30}`:
```xml
<ast>
  <object start="1:1" end="1:30">
    <property start="1:2" end="1:15">
      <key start="1:2" end="1:8">
        <string start="1:2" end="1:8">name</string>
      </key>
      <value start="1:10" end="1:15">
        <string start="1:10" end="1:16">John</string>
      </value>
    </property>
    <property start="1:18" end="1:28">
      <key start="1:18" end="1:23">
        <string start="1:18" end="1:23">age</string>
      </key>
      <value start="1:25" end="1:27">
        <number start="1:25" end="1:27">30</number>
      </value>
    </property>
  </object>
</ast>
```

Note: the `key` and `value` wrapper elements already exist because
`TreeBuilder::WRAPPED_FIELDS` includes both "name" and "value". However,
TreeSitter JSON uses `key` as the field name for the key child of a pair,
which is NOT in `WRAPPED_FIELDS`. We need to either:
- Add `"key"` to `WRAPPED_FIELDS` (affects all languages), OR
- Handle it in the JSON AST transform by creating `<key>` wrappers manually

**Decision**: Handle in the JSON AST transform. The TreeSitter `pair` node
has a `key` field (→ adds `field="key"` attribute) and a `value` field
(→ creates `<value>` wrapper since it's in WRAPPED_FIELDS). The transform
will create the `<key>` wrapper element for the key child.

#### `json::data_transform`

Converts to query-friendly data view:

1. For each `pair` node: extract the key string, sanitize to XML name,
   rename the pair element to the key name. Remove the key child and
   punctuation.
2. For arrays: items under a named key get repeated using the parent key name
   (handled structurally — the array element is flattened and its items
   become children of the parent key element). Anonymous array items get
   wrapped in `<item>`.
3. For scalars: strip quotes, flatten to text content.
4. Remove `object`/`array`/`document` wrappers (flatten).

The result for `{"name": "John", "tags": ["math", "science"]}`:
```xml
<data>
  <name start="1:2" end="1:15">John</name>
  <tags start="1:25" end="1:31">math</tags>
  <tags start="1:33" end="1:42">science</tags>
</data>
```

### Step 3: Split YAML transform into ast + data

**File**: `tractor-core/src/languages/yaml.rs`

The current `yaml::transform` is already a data transform. We need to:

1. **Rename** existing `transform` → `data_transform` (or keep as `transform`
   and add `ast_transform`).

2. **Add `yaml::ast_transform`** that normalizes TreeSitter YAML nodes into
   the unified vocabulary:

| TreeSitter kind | Target element |
|-----------------|---------------|
| `stream`        | (flatten) |
| `document`      | (flatten for single-doc; keep for multi-doc) |
| `block_node`, `flow_node` | (flatten) |
| `block_mapping`, `flow_mapping` | `<object>` |
| `block_mapping_pair`, `flow_pair` | `<property>` |
| field="key" child | `<key>` wrapper |
| field="value" child | `<value>` wrapper |
| `block_sequence`, `flow_sequence` | `<array>` |
| `block_sequence_item` | (flatten, children become direct array items) |
| `plain_scalar`, `double_quote_scalar`, `single_quote_scalar` | `<string>` (strip quotes) |
| `integer_scalar` | `<number>` |
| `float_scalar` | `<number>` |
| `boolean_scalar` | `<bool>` |
| `null_scalar` | `<null>` |
| `comment` | remove |
| `anchor`, `tag`, `alias` | flatten/remove |

3. **Existing `data_transform`**: Keep current logic (mostly unchanged).
   Small adjustments may be needed for array handling to match the spec's
   repeated-element pattern.

### Step 4: Modify XeeBuilder for dual-branch assembly

**File**: `tractor-core/src/xot_builder.rs`

Add a new method to `XeeBuilder`:

```rust
pub fn build_data_with_options(
    &mut self,
    ts_node: TsNode,
    source: &str,
    file_path: &str,
    lang: &str,
    format: &str,          // "json" or "yaml"
    ast_transform: TransformFn,
    data_transform: TransformFn,
    ignore_whitespace: bool,
    max_depth: Option<usize>,
) -> Result<DocumentHandle, xot::Error>
```

Implementation:
1. Call `build_raw_with_options()` to get the initial tree (with `<Files><File>`
   wrapper).
2. Find the content root under `<File>` (first element child).
3. Clone the content root with `xot.clone_node(content_root)`.
4. Apply `ast_transform` to the original content root via `walk_transform`.
5. Apply `data_transform` to the cloned content root via `walk_transform`.
6. Create `<ast>` element, move the original (now AST-transformed) content
   into it.
7. Create `<data>` element, move the cloned (now data-transformed) content
   into it.
8. Append `<ast>` and `<data>` as children of `<File>`.
9. Set `kind="data"` and `format="json|yaml"` attributes on `<File>`.

**Important detail**: `walk_transform` currently calls `find_content_root()`
which skips `Files`/`File` wrappers. For this step we need to pass the
content root directly (not the document root). We may need a variant
`walk_transform_from(xot, content_root, transform_fn)` that doesn't skip
wrappers, or we apply the transform before wrapping in `<ast>`/`<data>`.

**Recommended approach**: Apply transforms to the raw subtrees BEFORE wrapping:
1. Build raw tree, get content_root (first child of `<File>`)
2. Clone content_root → `cloned_root`
3. Detach content_root from `<File>`
4. Walk-transform content_root with ast_transform (using the node directly,
   not through find_content_root)
5. Walk-transform cloned_root with data_transform
6. Create `<ast>` element, append transformed content_root to it
7. Create `<data>` element, append cloned_root to it
8. Append `<ast>` and `<data>` to `<File>`

This requires a new walker entry point that doesn't skip wrappers.
Add to `xot_transform.rs`:

```rust
/// Walk and transform starting from a specific node (no wrapper skipping)
pub fn walk_transform_node<F>(xot: &mut Xot, node: XotNode, transform_fn: F)
    -> Result<(), xot::Error>
where F: FnMut(&mut Xot, XotNode) -> Result<TransformAction, xot::Error>
{
    walk_node(xot, node, &mut transform_fn)
}
```

### Step 5: Update build_with_options dispatch

**File**: `tractor-core/src/xot_builder.rs`

Modify `XeeBuilder::build_with_options()` to detect data languages and
route to the new dual-branch builder:

```rust
pub fn build_with_options(...) -> Result<DocumentHandle, xot::Error> {
    // ... existing raw build ...

    if !raw_mode {
        if let Some((ast_fn, data_fn)) = crate::languages::get_data_transforms(lang) {
            let format = match lang {
                "json" => "json",
                "yaml" | "yml" => "yaml",
                _ => "unknown",
            };
            // Build dual-branch tree
            self.apply_data_transforms(doc_handle, format, ast_fn, data_fn)?;
        } else {
            // Existing single-transform path for code languages
            let transform_fn = crate::languages::get_transform(lang);
            crate::xot_transform::walk_transform(self.documents.xot_mut(), doc_node, transform_fn)?;
        }
    }

    Ok(doc_handle)
}
```

Where `apply_data_transforms` is a private method that does steps 1-8
from Step 4 above.

### Step 6: Handle `--raw` mode

When `--raw` is passed, skip all transforms and return the raw TreeSitter
tree as today. No `<ast>`/`<data>` branches. This is unchanged.

### Step 7: Update language_info.rs

**File**: `tractor-core/src/language_info.rs`

Set `has_transforms: true` for JSON (currently `false`).

### Step 8: Integration tests

Add integration tests that verify:

1. **JSON dual-branch**: Parse JSON, verify both `/ast` and `/data` branches
   exist under `<File>`.
2. **YAML dual-branch**: Parse YAML, verify both branches.
3. **AST vocabulary**: Verify JSON AST uses `object/array/property/key/value/
   string/number/bool/null` naming.
4. **YAML AST vocabulary**: Same unified vocabulary.
5. **Data view**: Verify `/data/name` returns `"John"` for `{"name":"John"}`.
6. **Array handling**: Verify repeated elements for arrays under named keys.
7. **Nested structures**: Deep objects, arrays of objects, arrays of arrays.
8. **XPath queries**: `//data/user/name`, `//ast//property`, etc.
9. **Output modes**: `-o value`, `-o source`, `-o xml` all work from both
   branches.
10. **Source spans**: Verify `start`/`end` attributes present on both AST
    and data nodes with correct values.

### Step 9: Edge cases to handle

1. **Top-level arrays**: JSON `[1,2,3]` — the data view has no named parent
   key. Use `<item>` elements:
   ```xml
   <data>
     <item start="1:2" end="1:2">1</item>
     <item start="1:4" end="1:4">2</item>
     <item start="1:6" end="1:6">3</item>
   </data>
   ```

2. **Nested arrays (array of arrays)**: `[[1,2],[3,4]]` — inner arrays
   also use `<item>`:
   ```xml
   <data>
     <item>
       <item>1</item>
       <item>2</item>
     </item>
     <item>
       <item>3</item>
       <item>4</item>
     </item>
   </data>
   ```

3. **Empty objects/arrays**: `{}` and `[]` become empty `<data/>` or
   empty parent element.

4. **Mixed object keys**: Keys that are invalid XML names get sanitized
   with simple underscore replacement (e.g., `"first name"` → `first_name`).
   Add `<key>first name</key>` child to preserve original key (current
   YAML approach).

5. **Duplicate keys**: JSON technically allows them, TreeSitter parses both.
   Both appear in AST. In data view, both appear as sibling elements
   (last-wins semantics deferred to v2).

6. **Multi-document YAML**: Each YAML document (`---` separator) gets its
   own `<ast>`/`<data>` pair, OR the `<ast>` contains multiple `<document>`
   children while `<data>` flattens them. **Recommendation**: Keep a
   `<document>` wrapper in both branches for multi-doc YAML.

7. **Null values**: `"key": null` → AST: `<null>null</null>`, Data:
   `<key>null</key>` (text content "null").

8. **Boolean values**: `"key": true` → AST: `<bool>true</bool>`, Data:
   `<key>true</key>` (text content "true").

---

## Span handling on data nodes

The spec says data nodes carry source spans. This works naturally:

- The raw TreeSitter tree already has `start`/`end` on every element.
- When we clone the tree for the data branch, the cloned nodes retain
  their `start`/`end` attributes.
- The data transform renames elements and flattens wrappers, but the
  surviving elements keep their span attributes.
- For elements renamed from mapping pairs, the span covers the full
  key-value pair (which is correct — that's the extent of the data).

No special work needed here.

---

## What does NOT change

- `<Files><File>` wrapper structure (multi-file support intact)
- `start="line:col"` / `end="line:col"` attribute format
- Programming language transforms (TypeScript, Python, etc.)
- XPath engine (queries both branches transparently via standard XPath)
- Output modes (`-o value`, `-o source`, `-o xml`) — they work based on
  span attributes and string-value, both of which are present on all nodes
- `--raw` mode
- XML passthrough for `.xml` files

---

## Implementation order

1. **`xot_transform.rs`**: Add `walk_transform_node()` (public, no wrapper skipping)
2. **`languages/mod.rs`**: Add `SyntaxKind`, `get_syntax_kind()`, `get_data_transforms()`
3. **`languages/json.rs`**: New file with `ast_transform` and `data_transform`
4. **`xot_builder.rs`**: Add `apply_data_transforms()` private method, modify `build_with_options()` dispatch
5. **Test JSON end-to-end**: Parse JSON, verify dual branches, query with XPath
6. **`languages/yaml.rs`**: Add `ast_transform`, rename existing to `data_transform`
7. **Test YAML end-to-end**: Verify both branches
8. **`language_info.rs`**: Set `has_transforms: true` for JSON
9. **Integration tests**: Comprehensive test suite

Estimated touch points: 5 files modified, 1 new file created.
