---
title: Group Option (-g, --group)
priority: 2
---

Controls how matches are structured in output: flat list or grouped by file.

## Values

- `file` — Group matches by source file. The file field moves from individual
  matches to the group container, eliminating redundancy.
- `none` — Flat list of matches, each carrying its own file field.

## Defaults

- **check** mode defaults to `-g file` (matches are grouped by file).
- **query**, **test**, and **set** modes default to no grouping (flat matches).
- An explicit `-g` flag always overrides the default.

## Effect on Output

When grouped, structured formats (JSON, YAML, XML) emit a `groups` array
instead of a `matches` array. Each group has a `file` field and a `matches`
array. Individual matches within a group omit the `file` field.

Line-oriented formats (gcc, github) resolve the file from the group context
and produce identical output regardless of grouping.

## Validation

Only `file` and `none` are accepted. Any other value produces an error.
