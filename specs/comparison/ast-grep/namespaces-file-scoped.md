# namespaces-file-scoped

type: note

## Purpose

Find block-scoped namespaces that should be converted to file-scoped namespaces (C# 10+).

## AST-Grep Rule (18 lines)

```yaml
id: namespaces-file-scoped
message: Please use file scoped namespaces
severity: error
language: CSharp
rule:
  pattern: "namespace $$$NAMESPACE { $$$BODY }"
  all:
    - not:
        regex: Migrations|ServiceClient
    - not:
        inside:
          regex: auto-generated
fix: |
  namespace $$$NAMESPACE;

  $$$BODY
```

## Tractor XPath (1 line)

```xpath
//namespace[block][not(contains(name, 'Migrations'))][not(contains(name, 'ServiceClient'))]/name
```

## Breakdown

| Predicate | Purpose |
|-----------|---------|
| `//namespace` | Find namespace declarations |
| `[block]` | Has a block `{ }` (not file-scoped) |
| `[not(contains(name, 'Migrations'))]` | Exclude Migrations namespace |
| `[not(contains(name, 'ServiceClient'))]` | Exclude ServiceClient namespace |
| `/name` | Return namespace name |

## Test

```bash
cat << 'EOF' | tractor-parse -l csharp | tractor-xpath -x "//namespace[block]/name"
namespace MyApp.Services
{
    public class UserService { }
}
EOF
```

Output: `MyApp.Services`

## Notes

File-scoped namespaces (C# 10+) don't have a block:
```csharp
namespace MyApp.Services;

public class UserService { }
```

The TreeSitter tree for file-scoped vs block-scoped differs:
- Block-scoped: `namespace` → `name` + `block`
- File-scoped: `namespace` → `name` (no block child)

Query for file-scoped (compliant):
```xpath
//namespace[not(block)]/name
```
