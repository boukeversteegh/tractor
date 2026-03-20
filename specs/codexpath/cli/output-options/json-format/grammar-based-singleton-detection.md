---
title: Grammar-Based Singleton Detection
type: note
priority: 1
---

The decision to lift a child element as a direct JSON property is based on the
`field` attribute on XML elements — a grammar-level signal that a child slot
appears at most once per parent. Elements with `field` become direct JSON
properties; elements without `field` go into a `children` array.

This is purely grammar-based: no content-based heuristics, no runtime child
counting, no reliance on `node-types.json`. The `field` attribute is set by
`xot_builder.rs`.
