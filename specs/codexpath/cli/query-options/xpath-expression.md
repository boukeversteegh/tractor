---
title: XPath Expression (-x, --xpath)
priority: 1
---

Specify XPath query to execute against the XML AST. If omitted, outputs the full XML representation.

Examples:
- //MethodDecl/@name (all method names)
- //InvocationExpr[@name='Execute'] (specific invocations)
- //LiteralExpr[@missingSemicolon='true'] (strings without semicolons)
