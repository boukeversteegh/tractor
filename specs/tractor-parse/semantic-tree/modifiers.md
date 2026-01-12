---
title: Modifiers as Empty Elements
priority: 1
---

Modifiers (access levels, keywords like static/async) are represented as empty
child elements rather than attributes or text nodes.

This enables intuitive XPath predicates using element existence:

```xml
<method>
  <public/>
  <static/>
  <async/>
  <name>FetchDataAsync</name>
</method>
```

XPath queries become natural language:
- `//method[public]` - find public methods
- `//method[static]` - find static methods
- `//method[async]` - find async methods
- `//method[public][static]` - find public static methods
- `//class[not(public)]` - find non-public classes
- `//param[this]` - find extension method parameters

Supported modifiers:

**Access modifiers:**
- `public`, `private`, `protected`, `internal`

**Other modifiers:**
- `static`, `async`, `abstract`, `virtual`, `override`
- `sealed`, `readonly`, `const`, `partial`
- `this` (for C# extension method first parameter)

**Python-specific:**
- `async` (for async def)

The empty element approach is chosen over attributes because:
1. `//method[public]` is more readable than `//method[@public='true']`
2. No need to remember attribute value formats
3. Naturally supports `not()` for negation
