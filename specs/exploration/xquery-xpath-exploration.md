# XQuery / XPath 3.1 Exploration

## Summary

**xee does NOT support XQuery** - it only supports XPath 3.1 (and XSLT 3.0).
"Expand XPath to XQuery" is listed as a future "Challenging" idea in their `ideas.md`.

However, **XPath 3.1 already provides most XQuery-like constructs** needed for
map transforms. The `-x` flag can already handle complex transformations.

## What Works (XPath 3.1)

### Core Constructs

| Construct | XPath 3.1 Syntax | Status |
|-----------|-------------------|--------|
| for/return | `for $f in //function return string($f/name)` | Works |
| let/return | `let $fns := //function return count($fns)` | Works |
| if/then/else | `if (expr) then a else b` | Works |
| some/every | `some $f in //function satisfies ...` | Works |
| string concat | `"fn:" \|\| string($f/name)` | Works |
| arrow operator | `//function/name => count()` | Works |
| nested for | `for $c in //class, $m in $c//function return ...` | Works |

### Map Constructs

| Pattern | Syntax | Status |
|---------|--------|--------|
| Map literal | `map { "key": "value" }` | Works (Debug output) |
| Map + serialize | `serialize(map{...}, map{"method":"json"})` | Works (clean JSON) |
| for + map | `for $f in //X return map { "k": string($f/name) }` | Works |
| map:merge | `map:merge(for $f in //X return map { ... })` | Works |
| map:keys | `map:keys($m)` | Works |
| Array literal | `array { ... }` | Works (Debug output) |
| JSON array of maps | `serialize(array{for ... return map{...}}, map{"method":"json"})` | Works |

### What Does NOT Work (XQuery-only)

| Construct | Syntax | Workaround |
|-----------|--------|------------|
| where | `for $f in X where cond return ...` | Use predicate: `for $f in X[cond] return ...` |
| order by | `for $f in X order by ... return ...` | Use `sort()`: `sort(for $f in X return ...)` |
| element constructors | `<result>{$f/name}</result>` | Use `map{}` + `serialize()` |
| user-defined functions | `declare function f() { ... }` | Use inline functions: `function($x) { ... }` |
| modules | `import module namespace ...` | Not available |

## Practical Examples

### Map transform (the user's `/ map` use case)
```bash
# Extract function names as JSON objects
tractor file.py -x 'for $f in //function return serialize(
  map { "name": string($f/name), "params": string($f/params) },
  map{"method":"json"}
)' -v value

# Output:
# '{"name":"hello","params":"(name)"}'
# '{"name":"add","params":"(a, b)"}'
```

### JSON array of maps
```bash
tractor file.py -x 'serialize(
  array { for $f in //function return map { "name": string($f/name) } },
  map{"method":"json"}
)' -v value

# Output: '[{"name":"hello"},{"name":"add"},{"name":"greet"}]'
```

### Filtering (where equivalent)
```bash
# XPath predicate instead of XQuery 'where'
tractor file.py -x 'for $f in //function[count(.//params/type) > 1]
  return string($f/name)' -v value
```

### Nested class.method projection
```bash
tractor file.py -x 'for $c in //class, $m in $c//function
  return serialize(
    map { "class": string($c/name), "method": string($m/name) },
    map{"method":"json"}
  )' -v value
```

## Known Issues

### 1. Duplicate results for aggregate/constant expressions
Expressions that produce a single aggregate result (like `serialize(array{...})`)
get duplicated ~96x per file. The `for/return` pattern does NOT have this issue.

**Affected**: `sort()`, `for-each()`, `serialize(array{...})`, `true()`, `map{...}`
**Not affected**: `for/return`, `count()`, `let/return`, `(true())`

Wrapping in parens sometimes helps: `(true())` returns 1 result vs `true()` returns 96.

### 2. Map/Array rendering without serialize()
Maps and arrays render as Rust Debug format by default (ugly). Use
`serialize(..., map{"method":"json"})` wrapper for clean JSON output.

### 3. Atomic string values show XPath quotes
String atomics show as `"hello"` (with quotes) in `-v value` mode because we use
`xpath_representation()`. Integer/boolean atomics render correctly as `3`, `true()`.

## Code Changes Made

### `tractor-core/src/xpath/engine.rs`
1. **Fixed atomic rendering**: Changed `atomic.to_string().unwrap_or_default()` to
   `atomic.xpath_representation()`. The old code only worked for `Atomic::String`
   variants and returned empty strings for integers, booleans, etc.

2. **Added function/map/array rendering**: Changed `Item::Function(_) => {}` (silently
   dropped) to output the Debug representation.

## Recommendation

XPath 3.1 is **sufficient for map transforms**. The `for/return + map + serialize`
pattern covers the `/ map` use case. No XQuery support needed for this.

Key remaining work:
- Fix duplicate results for aggregate expressions (investigate xee SequenceQuery behavior)
- Improve map/array default rendering (ideally auto-serialize as JSON)
- Consider a shorthand syntax for the verbose `serialize(..., map{"method":"json"})` pattern
