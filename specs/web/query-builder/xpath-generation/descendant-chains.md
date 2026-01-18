---
title: Descendant Chains
priority: 2
---

Grouping and rendering of descendant predicates that form ancestor-descendant chains.

## Chain Detection

Descendants that are ancestors of each other form a single predicate chain:

```
target/body          → chain start
target/body/return   → extends chain
```

Result: `[body/return]` instead of `[body][.//return]`

## Algorithm

1. Sort descendants by path depth
2. For each unprocessed descendant:
   - Start a new chain
   - Find all descendants that extend this path
   - Add them to the chain in order
3. Render each chain as a single predicate

## Rendering

Chain elements use `/` or `//` based on depth relationship:

```xpath
[body/return]           // body is direct child, return is direct child of body
[body//deep/nested]     // gaps in the path use //
```
