# query-asnotracking

type: note

## Purpose

Find `Get*` methods that query `_context`, use `Map`, but don't use `AsNoTracking` (and aren't saving changes).

## AST-Grep Rule (23 lines)

```yaml
id: query-asnotracking
message: "The Query should probably contain AsNoTracking since the result is altered before returning it"
severity: warning
language: CSharp
rule:
  kind: method_declaration
  has:
    field: body
    all:
      - regex: _context
      - regex: Map
      - regex: return
      - not:
          regex: AsNoTracking
      - not:
          regex: SaveChanges
    any:
      - not:
          regex: _repository
  all:
    - has:
        kind: identifier
        regex: Get
```

## Tractor XPath (1 line)

```xpath
//method[name[starts-with(., 'Get')]][contains(., '_context')][contains(., 'Map')][not(contains(., 'AsNoTracking'))][not(contains(., 'SaveChanges'))][not(contains(., '_repository'))]/name
```

## Breakdown

| Predicate | Purpose |
|-----------|---------|
| `//method` | Find any method |
| `[name[starts-with(., 'Get')]]` | Method name starts with 'Get' |
| `[contains(., '_context')]` | Uses _context (EF DbContext) |
| `[contains(., 'Map')]` | Uses mapping (transforms result) |
| `[not(contains(., 'AsNoTracking'))]` | Missing AsNoTracking |
| `[not(contains(., 'SaveChanges'))]` | Not a write operation |
| `[not(contains(., '_repository'))]` | Not using repository pattern |
| `/name` | Return method name |

## Test

```bash
cat << 'EOF' | tractor-parse -l csharp | tractor-xpath -x "//method[name[starts-with(., 'Get')]][contains(., '_context')][contains(., 'Map')][not(contains(., 'AsNoTracking'))][not(contains(., 'SaveChanges'))]/name"
public class UserService
{
    public UserDto GetUser(int id)
    {
        var user = _context.Users.First(u => u.Id == id);  // BAD
        return Map(user);
    }

    public UserDto GetUserTracked(int id)
    {
        var user = _context.Users.AsNoTracking().First(u => u.Id == id);  // OK
        return Map(user);
    }
}
EOF
```

Output: `GetUser`
