---
priority: 1
type: note
---

# Naming Conventions

Naming rules for all user-facing identifiers in tractor. One consistent system
so users never have to guess.

## Design Principles

1. **Lowercase everywhere** — no capitalization to remember
2. **Prefer single words** — avoid compound names when a clear single word exists
3. **When compounding is necessary**, use the separator appropriate to the context (see below)
4. **XPath-safe** — any name that may appear in an XPath expression must not contain dashes (XPath parses `-` as minus)

## Separators by Context

| Context | Separator | Examples |
|---|---|---|
| **CLI arguments** | dash (`-`) | `--diff-files`, `--no-color`, `--max-files` |
| **XML attributes** (meta) | underscore (`_`) | `line`, `column`, `end_line`, `end_column` |
| **XML element names** (parse tree) | underscore (`_`) | `function`, `class`, `line_comment` |
| **Configuration fields** (YAML) | dash (`-`) | `diff-files`, `diff-lines` |
| **Environment variables** | underscore (`_`) | `TRACTOR_MAX_FILES` |

## Rationale

- **CLI arguments use dashes** because it's the universal convention. Arguments
  already start with `--`, so dashes flow naturally: `--diff-files`.
- **Configuration fields use dashes** to match the CLI flags they correspond to.
  `diff-files:` in YAML reads the same as `--diff-files` on the command line.
  CLI and config are the same namespace — one convention for both.
- **XML attributes and element names use underscores** because these names appear
  in XPath expressions, where dashes would be parsed as subtraction. Underscores
  are also compatible with environment variable names and variable names in most
  programming languages.
- **Element names from tree-sitter grammars** already use underscores
  (e.g., `function_declaration`, `class_body`). Using underscores for tractor's
  own attributes keeps the XML namespace consistent.

## Notes

- Prefer avoiding compound words entirely. `line` is better than `start_line`.
  Only introduce compounds when the meaning would be ambiguous without a qualifier.
- Environment variables follow the standard `UPPER_SNAKE_CASE` convention with
  a `TRACTOR_` prefix.
