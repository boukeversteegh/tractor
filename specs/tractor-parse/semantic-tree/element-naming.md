---
title: Element Naming Convention
priority: 1
refs:
  - design.md#2-full-names-over-abbreviations
  - design.md#1-use-language-keywords
  - design.md#3-always-lowercase
---

Element names are lowercase, use full words (not abbreviations), and mirror
language keywords where possible.

## Principles Applied

- **Full names**: `property` not `prop`, `parameter` not `param`
- **Language keywords**: `class`, `method`, `if`, `return`, `for`
- **Lowercase always**: Never use capitals

## C# Examples

| TreeSitter Node | Element Name |
|-----------------|--------------|
| `class_declaration` | `class` |
| `method_declaration` | `method` |
| `property_declaration` | `property` |
| `field_declaration` | `field` |
| `constructor_declaration` | `constructor` |
| `parameter` | `parameter` |
| `argument` | `argument` |
| `return_statement` | `return` |
| `if_statement` | `if` |

## Language Keywords Preserved

Some short names are kept because they are C# keywords that developers recognize:

- `int` (integer literal) - C# type keyword
- `bool` (boolean literal) - C# type keyword
- `string` (string literal) - C# type keyword
- `var` (implicit type) - C# keyword

These are NOT abbreviations - they are the actual language syntax.

## Python Examples

| TreeSitter Node | Element Name |
|-----------------|--------------|
| `function_definition` | `def` |
| `class_definition` | `class` |
| `return_statement` | `return` |
| `import_statement` | `import` |

Python uses `def` because that's the keyword in the language.
