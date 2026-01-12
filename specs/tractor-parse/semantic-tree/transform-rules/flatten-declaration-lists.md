---
title: Flatten Declaration Lists
priority: 1
---

Remove wrapper nodes like `declaration_list` that add nesting without meaning:

```xml
<!-- Before -->
<class_declaration>
  <declaration_list>
    <method_declaration>...</method_declaration>
  </declaration_list>
</class_declaration>

<!-- After -->
<class>
  <method>...</method>
</class>
```
