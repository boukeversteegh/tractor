---
title: Transformation Rules
type: group
priority: 1
---

Systematic rules for transforming TreeSitter syntax tree to semantic tree.

The transformation is applied recursively to each node. Rules are evaluated
in order for each node type, producing a cleaner, more queryable XML structure.

Transform rules must follow the guiding principles in [design.md](design.md),
in particular:
- **#8 Renderability** — transforms must not lose information needed for rendering
- **#9 Exhaustive markers** — mutually exclusive modifiers always include one marker
- **#10 Marker source locations** — keyword-based markers carry source locations

See `poc/unparse/transform-issues.md` for known violations.
