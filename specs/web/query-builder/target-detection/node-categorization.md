---
title: Node Categorization
priority: 2
---

Classification of selected nodes relative to the target for XPath generation.

## Categories

### Ancestors
- Nodes that are prefixes of the target path
- Appear before the target in the XPath path expression
- Example: `class` is ancestor of `class/method`

### Descendants
- Nodes that extend the target path
- Become predicates on the target: `//target[descendant]`
- Example: `class/method/body` is descendant of `class/method`

### Uncles
- Nodes that share a common ancestor but diverge before the target
- Attached as predicates on their common ancestor with the target
- Example: `class/field` is uncle of `class/method/body` (both under `class`)

## XPath Generation

```
//ancestor[uncle-predicate]/target[descendant-predicate]
```

Ancestors form the path, uncles become predicates at their attachment point, descendants become predicates on the target.
