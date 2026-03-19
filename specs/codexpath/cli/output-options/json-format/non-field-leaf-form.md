---
title: Non-Field Text Leaves Use Compact Object Form
priority: 1
---

Non-field text-only child elements are serialized as single-key objects with the
element name as key. `<accessor>get;</accessor>` becomes `{"accessor": "get;"}`.
No `$type` is emitted for these compact text nodes; only structural non-field
children receive `$type`.
