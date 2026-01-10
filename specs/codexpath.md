---
title: CodeXPath
---

CLI tool that converts C# source code to XML using Roslyn and allows querying with XPath expressions.

Parses C# syntax trees and generates XML representations where element names are in PascalCase
(e.g., ClassDecl, MethodDecl, InvocationExpr) and attributes capture location and metadata.
