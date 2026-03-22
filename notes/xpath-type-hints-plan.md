# XPath Type Hints via Predicate Markers

## Problem

Currently, `tractor set` splices the user-provided `--value` directly into the
source. This forces the user to provide pre-formatted values:

```sh
# User must include JSON quotes for strings:
tractor set config.json -x "//name" --value '"Alice"'
# User must know to omit quotes for numbers:
tractor set config.json -x "//age" --value '120'
```

The caller has to know the target language's syntax. That's backwards.

## Idea: Type Hints as XPath Predicates

XPath predicates can encode the desired type as empty marker elements:

```sh
tractor set config.json -x "//age[number]" --value 120
tractor set config.json -x "//name[string]" --value Alice
tractor set config.json -x "//enabled[boolean]" --value true
```

In the XML data tree model, these predicates correspond to marker child
elements (`<number/>`, `<string/>`, etc.) — the same pattern already used
elsewhere in the data tree for type information.

## Unified Value Pipeline (Insert + Update)

Both insert and update would go through the same steps:

1. **Parse XPath** — extract the path and any type marker predicate
2. **Build a typed node** — xpath_xml_builder generates an XML fragment with
   the marker element (e.g. `<age><number/></age>`)
3. **Set the raw value** — the plain user-provided text (`120`, `Alice`) is set
   on the node
4. **Render** — the language renderer (JSON, YAML, etc.) produces correctly
   formatted output based on the type marker (`120` as bare number, `"Alice"`
   with quotes)
5. **Splice** — insert the rendered text at the insertion point (new node) or
   replace the matched span (existing node)

The only difference between insert and update is *where* the splice happens.
The value preparation is identical.

## Default Behavior

Without a type hint predicate:
- **Update path**: could infer the type from the existing node's `kind`
  attribute in the data tree
- **Insert path**: default to `string` (safest assumption)

## Benefits

- User never thinks about quoting or escaping
- `--value Alice` just works — the type hint + renderer handle formatting
- Same pipeline for insert and update
- Naturally extends to any language the renderer supports
