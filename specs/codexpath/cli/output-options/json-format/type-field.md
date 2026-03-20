---
title: $type Field for Node Identification
priority: 1
---

Node type is encoded as `$type` instead of `type`. This frees the `type` key for
semantic use (e.g. property type annotations in C#). `$type` is only emitted on
`children` array items, not on field-lifted properties.
