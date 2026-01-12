---
title: Location Attribute Format
priority: 1
---

Source location is stored in compact `start` and `end` attributes using `line:col` format.

```xml
<class start="1:1" end="14:2">
  <method start="4:5" end="7:6">
    <name>Execute</name>
  </method>
</class>
```

Format: `start="startLine:startCol" end="endLine:endCol"`

This is more compact than four separate attributes while remaining human-readable:
- Current: `startLine="1" startCol="1" endLine="14" endCol="2"` (48 chars)
- New: `start="1:1" end="14:2"` (23 chars)

XPath access (when needed):
- Start line: `substring-before(@start, ':')`
- Start column: `substring-after(@start, ':')`
- End line: `substring-before(@end, ':')`
- End column: `substring-after(@end, ':')`

For line-based queries (e.g., finding long methods):
```xpath
//method[
  number(substring-before(@end, ':')) -
  number(substring-before(@start, ':')) > 50
]
```

The location info enables:
- Displaying code snippets with highlighting
- Error messages with precise locations
- IDE integration (go to definition)
- Calculating method length in lines
