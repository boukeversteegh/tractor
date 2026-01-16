---
title: XPath String Value
priority: 2
---

The XPath string-value of an element is the concatenation of all text content within that element
and its descendants. This is used for exact matching with `[.='value']` predicates.

## Whitespace Handling

### Inter-token Whitespace

When tree-sitter parses source code, whitespace between tokens (like spaces in `let mut x`)
is preserved in the XPath string-value:

```bash
tractor code.rs -x "//let_declaration[contains(.,'let mut')]" --expect 1
```

The xot builder tracks byte positions and inserts space text nodes where the source has
inter-token whitespace. This enables matching against readable code patterns.

### Line Endings

Tree-sitter normalizes CRLF (`\r\n`) to LF (`\n`) during parsing. XPath queries should
use `\n` for newline matching regardless of the source file's line endings:

```bash
# Both LF and CRLF source files match with \n
tractor file.py -x $'//string_content[.="hello\n\n"]'
```

### Multiline Strings

Newlines within string literals are preserved exactly in the XPath string-value:

```python
msg = """hello

"""
```

Can be matched with:
```bash
tractor file.py -x $'//string_content[.="hello\n\n"]' --expect 1
```

## Internal Architecture

1. **Build phase**: Source text nodes preserve exact content from tree-sitter byte ranges
2. **Render phase**: XML output may show simplified content (for readability)
3. **Query phase**: `xot.string_value()` returns the preserved original content

The rendered XML display and the XPath string-value may differ because rendering is
optimized for human readability while XPath queries operate on the internal xot tree.

## References

- `tractor-core/src/xot_builder.rs` - Inter-token whitespace insertion
- `tractor-core/src/xpath/engine.rs:64` - `xot.string_value(node)` for match values
