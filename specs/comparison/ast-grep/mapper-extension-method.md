# mapper-extension-method

type: note

## Purpose

Find static mapping methods in Mapper classes that should be extension methods (missing `this` modifier on first parameter).

## AST-Grep Rule (57 lines)

```yaml
id: mapper-extension-method
message: Static mapping method should be an extension method
severity: warning
language: CSharp
ignores:
  - '*ClientLibrary/*'
  - '*Tests/*'
rule:
  nthChild: 1
  pattern:
    selector: parameter
    context: class $$$ { $RETURN_TYPE Map($$$PARAMS) }
  regex: '^[A-Z][a-zA-Z0-9]*'
  inside:
    kind: parameter_list
    inside:
      kind: method_declaration
      regex: ^public static
      not:
        regex: "^public static [a-z]+"
      inside:
        kind: declaration_list
        inside:
          kind: class_declaration
          regex: ^public static class .*Mapper
    not:
      has:
        kind: parameter
        nthChild: 2
  not:
    has:
      kind: modifier
      regex: this
fix: this $PARAMS
```

## Tractor XPath (1 line)

```xpath
//class[static][name[contains(., 'Mapper')]]/method[public][static][count(params/param)=1][not(params/param[this])]/name
```

## Breakdown

| Predicate | Purpose |
|-----------|---------|
| `//class[static]` | Static class |
| `[name[contains(., 'Mapper')]]` | Class name contains 'Mapper' |
| `/method[public][static]` | Public static methods |
| `[count(params/param)=1]` | Single parameter only |
| `[not(params/param[this])]` | First param doesn't have `this` modifier |
| `/name` | Return method name |

## Test

```bash
cat << 'EOF' | tractor-parse -l csharp | tractor-xpath -x "//class[static][name[contains(., 'Mapper')]]/method[public][static][count(params/param)=1][not(params/param[this])]/name"
public static class UserMapper
{
    public static UserDto Map(User user) { return new UserDto(); }  // MATCH - missing 'this'
    public static UserDto MapExt(this User user) { return new UserDto(); }  // Skip - is extension
    public static UserDto MapTwo(User a, User b) { return new UserDto(); }  // Skip - 2 params
}
EOF
```

Output: `Map`
