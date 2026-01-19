# Tractor

`grep` for code structure, not text.

Tractor lets you query source code using XPath. It parses code into a semantic XML tree, then lets you search it with standard XPath expressions.

## Install

```bash
cargo install tractor
```

## Quick Start

```bash
# See the tree structure
tractor Program.cs

# Query with XPath
tractor src/**/*.cs -x "method[async][type='void']"
```

## Why XPath?

Other code search tools use custom query languages. When your query doesn't match, you're left guessing why.

With Tractor, you can *see* the tree you're querying:

```bash
tractor file.cs          # See the XML
tractor file.cs -x "..." # Query it
```

XPath is a W3C standard with extensive documentation and tooling:

- [XPath Tutorial (W3Schools)](https://www.w3schools.com/xml/xpath_intro.asp) - Quick intro
- [XPath Cheatsheet (DevHints)](https://devhints.io/xpath) - Handy reference
- [XPath 3.1 Spec (W3C)](https://www.w3.org/TR/xpath-31/) - Full specification

## Examples

```bash
# Find all method names
tractor src/**/*.cs -x "method/name"

# Find public async methods
tractor src/**/*.cs -x "method[public][async]"

# Find methods with more than 3 parameters
tractor src/**/*.cs -x "method[count(params/param) > 3]"

# Count classes per file
tractor src/**/*.cs -x "count(class)"

# CI: fail on async void methods
tractor src/**/*.cs -x "method[async][type='void']" --expect none \
    --format gcc --message "async void is dangerous"
```

## Output Formats

```bash
-f lines    # Source code snippets (default)
-f xml      # XML fragments
-f json     # JSON array with file, line, value
-f gcc      # GCC-style for IDE integration
-f count    # Just the count
```

## Supported Languages

C#, TypeScript, JavaScript, Rust, Python, Go, Java, Ruby, C++, C, JSON, HTML, CSS, Bash, PHP, and more.

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
