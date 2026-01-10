---
title: Message Option (-m, --message)
priority: 1
---

Custom error message for --expect failures. Supports placeholders:
- {value}: matched value (truncated to 50 chars)
- {line}: line number
- {col}: column number
- {file}: file path
- {ancestor::ClassDecl/@name}: any relative XPath expression

Example: "SQL must end with semicolon in {ancestor::ClassDecl/@name}"
