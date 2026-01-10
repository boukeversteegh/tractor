---
title: Element Name Transformation
priority: 1
---

Transform Roslyn syntax node kinds to abbreviated PascalCase element names:
- Remove "Syntax" suffix
- Replace "Declaration" with "Decl"
- Replace "Statement" with "Stmt"
- Replace "Expression" with "Expr"

Example: ClassDeclarationSyntax becomes ClassDecl
