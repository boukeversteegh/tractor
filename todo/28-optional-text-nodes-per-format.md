# Optional text-node output per format

## Context

While reviewing snapshot sizes for the v1 release, the JSON and YAML
tree outputs consistently carry text nodes that machine consumers
don't need — punctuation tokens (`"{"`, `"}"`, `","`, `";"`, `"="`),
keyword literals, and source whitespace. The text is useful for
`-v value` and for the human-readable text tree, but in JSON / YAML
it's noise that bloats the output and forces consumers to filter it
out themselves.

Source-backed markers (`<public/>` + dangling "public" sibling text)
are one visible symptom: JSON gets a clean `"public": true` flag but
also keeps scattered structural text around it that isn't
semantically meaningful to a data consumer.

## Problem

Text nodes serve two audiences:

1. **Tree / schema / XML renderers, and `-v value`** — the text is
   the source token, essential for source-accurate output and for
   queries that pull the original text.
2. **JSON / YAML consumers** — treating the AST as data, they want
   structural properties (names, types, markers) and not the
   punctuation that the source happened to contain.

Today both get the same text, and JSON / YAML consumers have to
strip it manually.

## Desired state

A per-format default for whether text nodes are emitted:

- **Tree (text)**: on — text is part of the visual rendering.
- **Schema**: on — the schema output cites sample values.
- **XML**: on — XML is the lossless reference representation.
- **JSON / YAML**: off by default — text nodes are dropped unless the
  text IS the element's content (e.g. `<name>foo</name>` still
  serializes as `"name": "foo"`).

A CLI flag (`--text-nodes={on,off,auto}`, default `auto`) overrides
the per-format default when someone needs the other behavior —
e.g. round-tripping JSON back to source, or inspecting what the
tree actually contains.

## Notes

- Source: `tractor/src/output/xml_to_json.rs` already collects
  text fragments; the work is in the decision of whether to include
  them in the final object.
- Text-only leaf elements (`<name>foo</name>` → `"name": "foo"`)
  must still work with text-nodes off — those are structural
  values, not incidental source tokens. The distinction is
  roughly: if the text is the element's *only* non-flag content,
  it's structural; if it's mixed with element children, it's
  incidental.
- Will change almost every JSON / YAML snapshot; plan accordingly
  (regen after the default flips, or add a migration flag).
