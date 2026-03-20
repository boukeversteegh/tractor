---
title: JSON Format
type: group
priority: 2
---

Output structured JSON with objects containing match metadata and tree data.
When the tree view (`-v tree`) is active, the JSON output includes a `tree` field
with a structured representation of the semantic tree serialized from the XML-based
intermediate representation.

## Design Principles

- **Lossless round-trip**: The JSON format must be convertible back to XML without functional loss. Dropping syntax text (keywords, punctuation) and information not distinguished in queries is acceptable, but the semantic structure must survive the round-trip. This enables `tractor render` to accept JSON input as an alternative to XML.
