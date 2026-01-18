---
title: XPath Generation
priority: 1
---

Algorithm for converting selection state into an XPath query string.

## Query Structure

```
//ancestor1[uncle1]/ancestor2[uncle2]//target[condition][descendant]
```

## Path Separators

- `/` - Direct parent-child relationship (depth differs by 1)
- `//` - Descendant relationship (depth differs by more than 1)

## Predicate Format

### Direct Child Predicate
```xpath
[child-name]
```

### Descendant Predicate
```xpath
[.//descendant-name]
```

### With Condition
```xpath
[name[.='value']]
```

## Example

Selection:
- `class` (ancestor)
- `class/method` (target)
- `class/method/body/return` (descendant)
- Condition on return: `.='result'`

Generated XPath:
```xpath
//class/method[.//return[.='result']]
```
