---
title: Field-Backed Text Leaves Collapse to Strings
priority: 1
---

Field-backed text-only leaf elements collapse to plain string values on the parent.
`<name field="name">Foo</name>` gives the parent `"name": "Foo"` rather than a
nested object.
