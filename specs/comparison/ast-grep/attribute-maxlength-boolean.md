# attribute-maxlength-boolean

type: note

## Purpose

Find boolean properties with `MaxLength` attribute (which makes no sense for booleans).

## AST-Grep Rule (15 lines)

```yaml
id: attribute-maxlength-boolean
message: MaxLength on a boolean field will never pass validation
severity: error
language: CSharp
rule:
  any:
    - kind: property_declaration
    - kind: field_declaration
  all:
    - has:
        kind: attribute_list
        regex: MaxLength
    - has:
        field: type
        regex: bool
```

## Tractor XPath (1 line)

```xpath
//prop[type='bool'][attrs[contains(., 'MaxLength')]]/name
```

## Breakdown

| Predicate | Purpose |
|-----------|---------|
| `//prop` | Find any property |
| `[type='bool']` | Property type is bool |
| `[attrs[contains(., 'MaxLength')]]` | Has MaxLength attribute |
| `/name` | Return property name |

## Test

```bash
cat << 'EOF' | tractor-parse -l csharp | tractor-xpath -x "//prop[contains(., 'bool')][attrs[contains(., 'MaxLength')]]/name"
public class UserRecord
{
    [MaxLength(1)]
    public bool IsActive { get; set; }  // ERROR - MaxLength on bool

    [MaxLength(100)]
    public string Name { get; set; }  // OK - MaxLength on string
}
EOF
```

Output: `IsActive`
