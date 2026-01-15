# Analysis: XPath String Value and Whitespace

## Problem Statement

When querying with exact string match like `//type[.='Dictionary<string,int>']`, the query
fails even though the structure looks correct. Using `contains()` or `normalize-space()`
works, but exact `=` matching does not.

## Investigation

### Test with XML passthrough
```bash
echo '<type>List&lt;<arguments><type>string</type></arguments>&gt;</type>' \
  | tractor -l xml -x "//type[.='List<string>']"
# WORKS - returns the type
```

### Test with C# parsing
```bash
echo 'class T { List<string> x; }' \
  | tractor -l csharp -x "//generic[.='List<string>']"
# FAILS - no match
```

### String value output
```bash
echo 'class T { List<string> x; }' \
  | tractor -l csharp -x "//generic" -o value
# Output has lots of whitespace/indentation
```

## Root Cause

The XPath code path does NOT query the xot tree directly. Instead:

1. **Build phase**: TreeSitter AST → xot tree (correct, no whitespace)
2. **Serialize phase**: `render_document()` → XML string WITH formatting (indentation, newlines)
3. **Query phase**: `documents.add_string(xml)` → xee_xpath parses the formatted string
4. **Result**: Formatting whitespace becomes text nodes in xee_xpath's tree

### Code trace

```
tractor-core/src/parser/mod.rs:
  parse_string_to_xot() → builds xot tree

tractor-core/src/output/xml_renderer.rs:
  render_document() → adds indentation between elements

tractor-core/src/xpath/engine.rs:25-41:
  pub fn query(&self, xml: &str, ...) {
      let mut documents = Documents::new();
      let doc = documents.add_string("file:///query", xml)  // <-- parses XML STRING
```

The `xml` parameter is the formatted string from `render_document()`, not the xot tree.

## Why XML passthrough works

XML passthrough (`-l xml`) passes the input string directly without going through
render_document(). The input has no formatting whitespace, so no extra text nodes.

## Solutions

### Option 1: Add compact render mode (easiest)
Add `compact: bool` to `RenderOptions` that skips all formatting whitespace.
The XML string would be `<a><b>x</b></a>` instead of `<a>\n  <b>x</b>\n</a>`.

### Option 2: Direct xot→xee path (ideal, bigger refactor)
Skip string serialization entirely. Use xot tree directly with xee_xpath.
This requires xee_xpath to accept an existing xot tree instead of parsing XML.

### Option 3: Strip whitespace-only text nodes
Post-process the xee_xpath tree to remove whitespace-only text nodes.
Hacky but would work.

## Recommendation

Option 1 (compact render) for immediate fix.
Option 2 for long-term improvement.

## Impact

This affects ALL string-value based queries, not just generic types.
Any query using `[.='...']` or `[text()='...']` on elements with children
will fail due to formatting whitespace.
