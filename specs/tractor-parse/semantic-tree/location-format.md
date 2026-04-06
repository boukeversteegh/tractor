---
title: Location Attribute Format
priority: 1
---

Source location is stored in separate numeric attributes: `startLine`, `startCol`, `endLine`, `endCol`.

```xml
<class startLine="1" startCol="1" endLine="14" endCol="2">
  <method startLine="4" startCol="5" endLine="7" endCol="6">
    <name>Execute</name>
  </method>
</class>
```

All values are 1-based integers.

XPath access is straightforward — no string parsing needed:
- Start line: `@startLine`
- Start column: `@startCol`
- End line: `@endLine`
- End column: `@endCol`

For line-based queries (e.g., finding long methods):
```xpath
//method[@endLine - @startLine > 50]
```

The location info enables:
- Displaying code snippets with highlighting
- Error messages with precise locations
- IDE integration (go to definition)
- Calculating method length in lines
