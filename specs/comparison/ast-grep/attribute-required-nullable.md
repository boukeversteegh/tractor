# attribute-required-nullable

type: note

## Purpose

Find non-nullable value types (int, Guid, bool, etc.) with `[Required]` attribute, which has no effect since value types always have a default value.

## AST-Grep Rule (50 lines)

```yaml
id: attribute-required-nullable
message: |
  [Required]-attribute on a non-nullable field or property has no effect.
  Non-nullable fields always have a value (the default), so the validation logic won't work.
severity: error
language: CSharp
ignores:
  - "**/*ClientLibrary/*.cs"
  - "**/Client.cs"
rule:
  any:
    - kind: property_declaration
    - kind: field_declaration
  all:
    - has:
        kind: attribute_list
        regex: \WRequired\W
    - has:
        kind: modifier
        regex: public
  has:
    field: type
    regex: "^int|long|float|double|decimal|short|ushort|uint|ulong|bool|char|byte|sbyte|Guid|DateOnly|TimeOnly|DateTimeOffset|TimeSpan$"
  not:
    has:
      field: type
      kind: nullable_type
```

## Tractor XPath (1 line)

For Guid specifically:
```xpath
//prop[public][attrs[contains(., 'Required')]][name[.='Guid'] or contains(., ' Guid ')][not(nullable)]/name
```

Or using a regex-like approach for multiple value types:
```xpath
//prop[public][attrs[contains(., 'Required')]][type[contains('|int|long|bool|Guid|', concat('|', ., '|'))]][not(nullable)]/name
```

## Breakdown

| Predicate | Purpose |
|-----------|---------|
| `//prop[public]` | Public properties |
| `[attrs[contains(., 'Required')]]` | Has [Required] attribute |
| `[type[...]]` | Type is a value type (int, Guid, etc.) |
| `[not(nullable)]` | Type is NOT nullable |
| `/name` | Return property name |

## Test

```bash
cat << 'EOF' | tractor-parse -l csharp | tractor-xpath -x "//prop[public][attrs[contains(., 'Required')]][contains(., 'Guid')][not(nullable)]/name"
public class UserRecord
{
    [Required]
    public Guid UserId { get; set; }  // ERROR - Required on non-nullable Guid

    [Required]
    public Guid? NullableId { get; set; }  // OK - nullable Guid

    [Required]
    public string Name { get; set; }  // OK - string is reference type
}
EOF
```

Output: `UserId`

## Notes

XPath 2.0's `matches()` function could be used for regex matching if the XPath processor supports it:
```xpath
//prop[public][attrs[contains(., 'Required')]][matches(type, '^(int|long|bool|Guid)$')][not(nullable)]/name
```
