---
title: JSON Format
priority: 2
---

Output structured JSON array with objects containing:
- file: file path
- line: line number
- column: column number
- value: matched value
- message: custom message if provided

Useful for tool integration and parsing.

## Design Principles

- **Lossless round-trip**: The JSON format must be convertible back to XML without functional loss. Dropping syntax text (keywords, punctuation) and information not distinguished in queries is acceptable, but the semantic structure must survive the round-trip. This enables `tractor render` to accept JSON input as an alternative to XML.
