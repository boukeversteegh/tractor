# Tractor

Write a rule once. Enforce it everywhere.

Tractor parses source code into a clean, inspectable tree and lets you query it with standard expressions. Find patterns, enforce conventions, and catch structural issues — across 20+ languages with one tool.

## Install

```bash
cargo install tractor
```

## Quick Start

```bash
# See the tree structure of any file
tractor Program.cs

# Find code patterns
tractor src/**/*.cs -x "method[async][returns/type='void']"

# Enforce conventions in CI
tractor src/**/*.cs -x "method[async][returns/type='void']" --expect none \
    --format gcc --message "async void is dangerous"
```

## How It Works

Tractor turns source code into a semantic tree you can inspect and query:

```bash
tractor file.cs          # See the tree
tractor file.cs -x "..." # Query it
```

No hidden structure. No guessing why a query didn't match. You see exactly what you're working with.

Queries use [XPath](https://devhints.io/xpath) — a standard syntax that works the same across all supported languages. AI tools can write queries without special documentation.

## Examples

```bash
# Find all method names
tractor src/**/*.cs -x "method/name"

# Find public async methods
tractor src/**/*.cs -x "method[public][async]"

# Find methods with more than 3 parameters
tractor src/**/*.cs -x "method[count(parameters/parameter) > 3]"

# Count classes per file
tractor src/**/*.cs -x "count(class)"

# Batch: run all rules from a config file
tractor run rules.yaml
```

## Convention Enforcement

Define rules and run them in CI:

```bash
# Fail the build if any async void methods exist
tractor check src/**/*.cs -x "method[async][returns/type='void']" \
    --expect none --format gcc --message "async void is dangerous"

# Assert expected match counts
tractor test src/**/*.cs -x "class" --expect ">0"
```

Output formats for every workflow:
```bash
-f lines    # Source code snippets (default)
-f xml      # XML fragments
-f json     # JSON array with file, line, value
-f gcc      # GCC-style for IDE integration
-f github   # GitHub Actions annotations
-f count    # Just the count
```

## Supported Languages

C#, TypeScript, JavaScript, Rust, Python, Go, Java, Ruby, C++, C, JSON, YAML, HTML, CSS, Bash, PHP, Scala, Lua, Haskell, and more.

## Web Playground

Try Tractor in your browser at the [online playground](https://tractor.fly.dev/playground).

## Development

```bash
# Build
cargo build --release

# Run tests
cargo test

# Web playground
cd web && npm install && npm run dev
```

## License

MIT
