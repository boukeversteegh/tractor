---
title: Cardinality-Independent JSON Shape
type: note
priority: 1
---

The JSON shape of a node must not depend on how many children it has at runtime.
A class with one method and a class with three methods must produce the same
structure for the `body` field — always a `children` array. This ensures jq
queries and JSON schemas work uniformly regardless of input.

Singleton wrapper lifting (`lift_singleton_children` in `xot_transform.rs`,
controlled by `DEFAULT_SINGLETON_WRAPPERS`) must only apply to wrappers whose
child is grammatically guaranteed to be a singleton (e.g. `value`, `returns`,
`condition`). Wrappers like `body` whose children can vary in count after
transforms must not participate in singleton child lifting.
