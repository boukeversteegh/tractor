---
title: LCA Target Algorithm
priority: 1
---

Lowest Common Ancestor algorithm for auto-detecting the query target when no explicit target is set.

## Algorithm Steps

1. Filter to leaf selections (nodes without selected descendants)
2. Find common prefix (LCA) of all leaf paths
3. Determine selection topology:
   - **Linear**: All paths form a chain → deepest leaf is target
   - **Siblings**: Same depth, same parent → prefer node WITHOUT condition
   - **Branching**: Different subtrees → LCA becomes implicit target

## Topology Detection

### Linear Paths
```
class → method → body → return
```
All nodes on same path. Target = deepest leaf (return).

### Sibling Paths
```
method/name  +  method/body
```
Same parent (method), same depth. Target = node without condition (body if name has condition).

### Branching Paths
```
method/parameter  +  method/body/return
```
Different subtrees under method. Target = LCA (method), all leaves become predicates.

## Implicit Target Handling

When LCA becomes the implicit target:
- LCA node may not be explicitly selected
- All original leaf selections become predicates on the LCA
- Query structure: `//lca[pred1][pred2]`
