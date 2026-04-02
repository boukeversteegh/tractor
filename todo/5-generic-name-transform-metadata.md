# C# generic_name transform loses metadata needed for JSON output

## Problem

The C# transform for `generic_name` (e.g. `List<T>`) extracts the identifier text into
an anonymous text node and drops the kind. The `type_argument_list` child has no `field`
attribute in tree-sitter's grammar, so the JSON converter can't tell it should be a
property rather than a `children` array item.

Current XML after transform:
```xml
<type kind="generic_name">
  <generic/>
  List              <!-- anonymous text, was identifier — kind lost -->
  <arguments kind="type_argument_list">   <!-- no field attr -->
    <type>T</type>
  </arguments>
</type>
```

Current JSON (ugly):
```json
{ "$type": "type", "generic": true, "children": [{ "$type": "arguments", "children": [{ "$type": "type", "text": "T" }] }] }
```

Text "List" is dropped entirely (anonymous text in mixed content gets filtered).

## Desired outcome

The transform should preserve enough metadata for the JSON converter to produce
something like:
```json
{ "generic": true, "text": "List", "arguments": ["T"] }
```

## Suggested fix

In the `generic_name` handler in `csharp.rs`:
1. Keep the type name as a proper element (e.g. `<name>List</name>`) or as a `text`
   attribute, instead of anonymous text
2. Add `field="arguments"` to the `type_argument_list` element so the JSON converter
   knows it's a singleton property

This is an upstream fix — the transform should produce clean semantic XML so downstream
renderers (JSON, YAML) don't need workarounds.

## Context

Discovered while exploring a JSON/YAML output format that uses field attributes to
distinguish singleton properties from children arrays. See also: the broader initiative
to make JSON output more compact and queryable (property lifting based on field attrs).
