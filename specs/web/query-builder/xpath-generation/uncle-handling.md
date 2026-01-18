---
title: Uncle Node Handling
priority: 2
---

Attachment of uncle nodes (siblings of ancestors) as predicates.

## Uncle Definition

An uncle is a selected node that:
- Is NOT an ancestor of the target (not a prefix)
- Is NOT a descendant of the target (not an extension)
- Shares a common ancestor with the target path

## Attachment Point

Uncles attach as predicates on their common ancestor with the target path:

```
target: class/method/body
uncle:  class/field

Common ancestor: class (depth 1)
Uncle attaches at: class
```

## Rendering

```xpath
//class[field]/method/body
```

The uncle becomes a predicate on the node at the common prefix depth.

## Multiple Uncles

Multiple uncles at the same attachment point become separate predicates:

```xpath
//class[field1][field2]/method/body
```

## Intermediate Nodes

If uncles attach between explicitly selected ancestors, intermediate path
nodes are output to provide the attachment point:

```xpath
//class/method[sibling-of-body]/body
```
