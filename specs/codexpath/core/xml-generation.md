---
title: XML AST Generation
priority: 1
---

Convert Roslyn syntax trees to XML representation with:
- PascalCase element names (ClassDecl, MethodDecl, InvocationExpr, etc.)
- Location attributes (line, column, end_line, end_column) with 1-based indexing
- Kind attribute for debugging
- Node-specific attributes (name, modifiers, etc.)
- CDATA for text containing special characters
