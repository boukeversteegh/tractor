---
title: Location Attribute Format
priority: 1
---

Source location is stored in separate numeric attributes: `line`, `column`, `end_line`, `end_column`.

```xml
<class line="1" column="1" end_line="14" end_column="2">
  <method line="4" column="5" end_line="7" end_column="6">
    <name>Execute</name>
  </method>
</class>
```

All values are 1-based integers.

XPath access is straightforward — no string parsing needed:
- Start line: `@line`
- Start column: `@column`
- End line: `@end_line`
- End column: `@end_column`

For line-based queries (e.g., finding long methods):
```xpath
//method[@end_line - @line > 50]
```

The location info enables:
- Displaying code snippets with highlighting
- Error messages with precise locations
- IDE integration (go to definition)
- Calculating method length in lines
