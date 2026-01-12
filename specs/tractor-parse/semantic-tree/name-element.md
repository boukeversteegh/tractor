---
title: Name Element for Identifiers
priority: 1
---

Primary identifiers are placed in a `<name>` child element rather than an attribute.

TreeSitter represents identifiers as `<identifier>` child nodes. The semantic tree
uses `<name>` for brevity and clarity:

```xml
<!-- TreeSitter raw -->
<class_declaration>
  <identifier>QueryHelpers</identifier>
</class_declaration>

<!-- Semantic tree -->
<class>
  <name>QueryHelpers</name>
</class>
```

XPath queries:
- `//class[name='QueryHelpers']` - find class by name
- `//method/name` - get all method names
- `//class[name='Foo']/method/name` - get method names in class Foo

Why `<name>` over `@name` attribute:
- Consistency with how TreeSitter represents identifiers (as nodes)
- Works naturally with XPath text comparison: `[name='Foo']`
- Allows complex names (generics) to have structure if needed

The `<name>` element contains only the text of the identifier. For generic types
like `List<T>`, the generic structure is represented separately in `<type>`.
