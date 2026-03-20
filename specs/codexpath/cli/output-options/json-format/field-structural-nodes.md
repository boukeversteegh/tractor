---
title: Field-Backed Structural Nodes Become Objects Without $type
priority: 1
---

Field-backed structural (non-leaf) elements become objects on the parent without
a `$type` key. `<body field="body">...</body>` gives the parent
`"body": { "children": [...] }`.
