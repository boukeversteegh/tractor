# repository-getall-orderby

type: note

## Purpose

Find `GetAll` methods in Repository classes that don't use `OrderBy`.

## AST-Grep Rule (22 lines)

```yaml
id: repository-getall-orderby
message: GetAll methods in repositories should specify ordering using OrderBy
severity: warning
language: CSharp
rule:
  kind: method_declaration
  has:
    kind: identifier
    field: name
    regex: GetAll
  inside:
    kind: declaration_list
    inside:
      kind: class_declaration
      has:
        kind: identifier
        field: name
        regex: Repository
        not:
          regex: Mock
  not:
    regex: OrderBy
```

## Tractor XPath (1 line)

```xpath
//class[name[contains(., 'Repository')]][not(name[contains(., 'Mock')])]/method[name[contains(., 'GetAll')]][not(contains(., 'OrderBy'))]/name
```

## Breakdown

| Predicate | Purpose |
|-----------|---------|
| `//class` | Find any class |
| `[name[contains(., 'Repository')]]` | Class name contains 'Repository' |
| `[not(name[contains(., 'Mock')])]` | Exclude Mock classes |
| `/method` | Direct child methods |
| `[name[contains(., 'GetAll')]]` | Method name contains 'GetAll' |
| `[not(contains(., 'OrderBy'))]` | Method body doesn't contain OrderBy |
| `/name` | Return the method name |

## Test

```bash
cat << 'EOF' | tractor-parse -l csharp | tractor-xpath -x "//class[name[contains(., 'Repository')]][not(name[contains(., 'Mock')])]/method[name[contains(., 'GetAll')]][not(contains(., 'OrderBy'))]/name"
public class UserRepository
{
    public List<User> GetAll() { return _context.Users.ToList(); }  // MATCH
}

public class MockUserRepository
{
    public List<User> GetAll() { return mock.ToList(); }  // Skip - Mock
}
EOF
```

Output: `GetAll`
