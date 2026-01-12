# attribute-maxlength-autotruncate

type: note

## Purpose

Find string properties with `MaxLength` attribute that are missing `AutoTruncate` attribute.

## AST-Grep Rule (23 lines)

```yaml
id: attribute-maxlength-autotruncate
message: When using the MaxLength attribute, consider using the AutoTruncate attribute
severity: warning
language: CSharp
files:
  - '*Record.cs'
ignores:
  - '*ClientLibrary/*'
  - '*Tests/*'
rule:
  any:
    - kind: property_declaration
    - kind: field_declaration
  all:
    - has:
        kind: attribute_list
        all:
          - regex: MaxLength
          - not:
              regex: AutoTruncate
    - has:
        field: type
        regex: string
```

## Tractor XPath (1 line)

```xpath
//prop[type='string'][attrs[attr[name='MaxLength']]][not(attrs[attr[name='AutoTruncate']])]/name
```

Or with contains for partial matching:

```xpath
//prop[contains(., 'string')][attrs[contains(., 'MaxLength')]][not(attrs[contains(., 'AutoTruncate')])]/name
```

## Breakdown

| Predicate | Purpose |
|-----------|---------|
| `//prop` | Find any property |
| `[type='string']` | Property type is string |
| `[attrs[attr[name='MaxLength']]]` | Has MaxLength attribute |
| `[not(attrs[attr[name='AutoTruncate']])]` | Missing AutoTruncate |
| `/name` | Return property name |

## Test

```bash
cat << 'EOF' | tractor-parse -l csharp | tractor-xpath -x "//prop[contains(., 'string')][attrs[contains(., 'MaxLength')]][not(attrs[contains(., 'AutoTruncate')])]/name"
public class UserRecord
{
    [MaxLength(100)]
    public string Name { get; set; }  // MATCH - missing AutoTruncate

    [MaxLength(50)]
    [AutoTruncate]
    public string Email { get; set; }  // OK - has AutoTruncate

    public string Bio { get; set; }  // OK - no MaxLength
}
EOF
```

Output: `Name`
