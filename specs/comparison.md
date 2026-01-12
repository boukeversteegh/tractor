# AST-Grep Comparison

Comparison of AST-Grep rules to Tractor XPath equivalents.

## Summary

Tractor's semantic XML tree enables concise XPath queries that replace verbose AST-Grep YAML rules.

| AST-Grep Rule | Lines | Tractor XPath | Length |
|--------------|-------|---------------|--------|
| repository-getall-orderby | 22 | 1 query | ~100 chars |
| mapper-extension-method | 57 | 1 query | ~80 chars |
| query-asnotracking | 23 | 1 query | ~90 chars |
| attribute-maxlength-autotruncate | 23 | 1 query | ~60 chars |
