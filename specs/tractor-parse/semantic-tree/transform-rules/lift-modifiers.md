---
title: Lift Modifiers
priority: 1
---

When a node has `<modifier>text</modifier>` children, convert to empty elements:

```xml
<!-- Before -->
<method_declaration>
  <modifier>public</modifier>
  <modifier>static</modifier>
</method_declaration>

<!-- After -->
<method>
  <public/>
  <static/>
</method>
```
