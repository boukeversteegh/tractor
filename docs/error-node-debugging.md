# Debugging Tree-sitter ERROR Nodes

## The Problem

When tree-sitter encounters syntax it cannot parse, it produces `ERROR` nodes in the AST. These nodes indicate parsing failures, but discovering *why* parsing failed can be surprisingly difficult.

## Case Study: The Null-Forgiving Operator Mystery

We spent significant time investigating what appeared to be a tree-sitter-c-sharp grammar bug. The C# null-forgiving operator (`!`) was being parsed as an `ERROR` node instead of `postfix_unary_expression`:

```xml
<member_access_expression>
  <identifier>name</identifier>
  <ERROR>\!</ERROR>           <!-- Why is ! an error? -->
  <identifier>Length</identifier>
</member_access_expression>
```

### The Investigation

1. Checked if tree-sitter-c-sharp supported the `!` operator → Yes, it was in `grammar.js`
2. Ran tree-sitter's own test suite → Tests passed
3. Compared WASM vs native builds → Both showed ERROR
4. Regenerated parser.c with different tree-sitter versions → Still ERROR
5. Examined lexer debug output → Found the culprit

### The Root Cause

The shell was escaping `!` (bash history expansion character) to `\!`:

```bash
# What we typed:
echo 'var x = name!.Length;' | tractor -l csharp

# What tractor actually received (hex dump):
00000000: 7661 7220 7820 3d20 6e61 6d65 5c21 2e4c  var x = name\!.L
#                                       ^^^^ backslash!
```

The grammar was always correct. The input was wrong.

### Why This Was Hard to Discover

1. **No visibility into actual input**: Tractor showed the ERROR but not what characters caused it
2. **Shell escaping is invisible**: The `\!` looked like `!` in terminal output
3. **Misdirection**: The ERROR node's content showed `\!` but we assumed that was XML escaping
4. **Test files worked**: Real `.cs` files parsed correctly, only stdin failed

## Proposed Improvements

### 1. Show Source Context for ERROR Nodes

When ERROR nodes are present, display the source text with the error highlighted:

```
Parse error at line 1, column 13-14:
  var x = name\!.Length;
              ^^
  Unexpected character sequence: 0x5C 0x21 (\!)
```

### 2. Hex Dump Mode for Debugging

Add `--debug-input` flag to show exactly what bytes tractor received:

```bash
$ echo 'var x = name!.Length;' | tractor -l csharp --debug-input
Input (23 bytes):
  00: 76 61 72 20 78 20 3d 20  var x =
  08: 6e 61 6d 65 5c 21 2e 4c  name\.L
  10: 65 6e 67 74 68 3b 0a     ength;.

Warning: Suspicious escape sequence at offset 12: \! (0x5C 0x21)
```

### 3. Common Escape Pattern Detection

Detect and warn about common shell escaping issues:

- `\!` → likely meant `!` (bash history)
- `\$` → likely meant `$` (variable expansion)
- `\\` → likely meant `\` (backslash)

```
Warning: Input contains '\!' which may be unintended shell escaping.
  Tip: Use a heredoc or file input to avoid shell interpretation:
    cat <<'EOF' | tractor -l csharp
    var x = name!.Length;
    EOF
```

### 4. Enhanced `--expect` Failure Messages

When `--expect none` fails due to ERROR nodes, provide actionable guidance:

```
✗ Expected no matches, but found 1 ERROR node

ERROR node at line 1, columns 13-14:
  Source: \!
  Bytes:  5c 21

This may indicate:
  1. A syntax error in your source code
  2. Shell escaping of special characters (!, $, \)
  3. An unsupported language feature

Run with --debug-input to see the exact bytes received.
```

## Workarounds (Current)

Until these improvements are implemented:

1. **Use files instead of stdin** for testing
2. **Use heredocs** with single-quoted delimiter:
   ```bash
   cat <<'EOF' | tractor -l csharp
   var x = name!.Length;
   EOF
   ```
3. **Check hex dump** when debugging mysterious ERRORs:
   ```bash
   echo 'code' | xxd | head
   ```
4. **Compare with file input**: If a `.cs` file works but stdin doesn't, suspect shell escaping

## Implementation Notes

The key insight is that ERROR nodes are symptoms, not diagnoses. The debugging tools should help users trace from the symptom (ERROR node) back to the cause (malformed input, unsupported syntax, or in our case, invisible shell escaping).
