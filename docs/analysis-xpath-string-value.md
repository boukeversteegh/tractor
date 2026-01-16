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

## Resolution (Implemented)

**Option 1 was implemented** with the following changes:

### Changes Made

1. **`RenderOptions.pretty_print`** (`xml_renderer.rs`)
   - Added `pretty_print: bool` field (default: `true`)
   - When `false`, skips indentation and newlines between elements
   - Text content is always preserved exactly (not affected by pretty_print)

2. **`generate_xml_document(results, pretty_print)`** (`parser/mod.rs`)
   - Changed from no-arg to single function with `pretty_print: bool` parameter
   - XPath queries use `generate_xml_document(&results, false)` for compact XML

3. **`OutputOptions.pretty_print`** (`formatter.rs`)
   - Added to control `-o xml` output formatting

4. **`--no-pretty` CLI flag** (`cli.rs`, `main.rs`)
   - Allows users to see compact XML for debugging
   - Useful to verify what XPath engine actually sees

### Behavior

- **XPath queries**: Always use compact XML internally (no formatting whitespace)
- **Display output**: Pretty-printed by default for readability
- **`--no-pretty`**: Shows compact XML in all output modes

### Verification

```bash
# Exact match now works
echo 'class T { List<string> x; }' | tractor -l csharp -x "//generic[.='List<string>']" -o value
# Output: List<string>

# String content with spaces preserved
echo 'var s = "   hello   ";' | tractor -l csharp --raw -x "//string_literal_content[.='   hello   ']" -o value
# Output:    hello

# View compact XML for debugging
echo 'class T { List<string> x; }' | tractor -l csharp --no-pretty -o xml
# Output: <Files><File ...><unit><class>...</class></unit></File></Files>
```

### Inter-Token Whitespace Preservation

A second issue was discovered: the string-value `let mut batches` was rendered as
`letmutbatches` because inter-token whitespace wasn't captured.

**Additional changes:**

5. **`XotBuilder.build_raw_node`** (`xot_builder.rs`)
   - Now tracks byte positions while iterating children
   - When there's a gap between nodes containing whitespace, inserts a space text node
   - This preserves source whitespace in the XPath string-value

6. **`get_text_children`** (`xot_transform.rs`)
   - Now filters out whitespace-only text nodes
   - Trims text content before returning
   - This prevents whitespace nodes from being mistaken for operators/keywords

### Verification

```bash
# Whitespace preserved in string-value
echo 'let mut batches = Vec::new();' | tractor -l rust --raw -x "//let_declaration" -o value
# Output: let mut batches = Vec::new();

# XPath matching with whitespace works
echo 'let mut batches = Vec::new();' | tractor -l rust --raw -x "//let_declaration[contains(.,'let mut batches')]" -o value
# Output: let mut batches = Vec::new();
```

### Note on Source Whitespace

Tree-sitter normalizes whitespace between tokens. For example, `Dictionary<string, int>`
(with space after comma) becomes `Dictionary<string,int>` (no space) because the space
is inter-token whitespace that tree-sitter doesn't capture. This is expected parser
behavior, not related to the pretty_print fix.
