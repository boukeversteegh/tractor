---
title: Rename Identifier to Name
priority: 1
---

Direct `<identifier>` children become `<name>`:

```xml
<!-- Before -->
<class_declaration>
  <identifier>Foo</identifier>
</class_declaration>

<!-- After -->
<class>
  <name>Foo</name>
</class>
```
