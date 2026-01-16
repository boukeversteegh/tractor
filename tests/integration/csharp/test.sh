#!/usr/bin/env bash
# C# integration tests
source "$(dirname "$0")/../common.sh"

echo "C#:"

# Basic transforms
run_test tractor sample.cs -x "method" --expect 2 -m "method declarations become method elements"
run_test tractor sample.cs -x "method[name='Add']" --expect 1 -m "method names are directly queryable"
run_test tractor sample.cs -x "class[name='Sample']" --expect 1 -m "class names are directly queryable"
run_test tractor sample.cs -x "unit" --expect 1 -m "compilation_unit renamed to unit"
run_test tractor sample.cs -x "static" --expect 2 -m "static modifier extracted"
run_test tractor sample.cs -x "binary[op='+']" --expect 1 -m "operators extracted to op element"
run_test tractor sample.cs -x "call" --expect 2 -m "invocation expressions renamed to call"
run_test tractor sample.cs -x "int" --expect 2 -m "integer literals renamed to int"

# -------------------------------------------------------------------------
# AST-Grep comparison tests
# These demonstrate real-world CI/linting use cases from specs/comparison/ast-grep/
# -------------------------------------------------------------------------

echo ""
echo "  AST-Grep Comparisons:"

# MaxLength without AutoTruncate (attribute-maxlength-autotruncate.md)
run_test tractor attribute-maxlength-autotruncate.cs \
    -x "//property[attributes[contains(., 'MaxLength')]][not(attributes[contains(., 'AutoTruncate')])]/name" \
    --expect 1 -m "find properties with MaxLength but missing AutoTruncate"

# MaxLength on boolean (attribute-maxlength-boolean.md)
run_test tractor attribute-maxlength-boolean.cs \
    -x "//property[type='bool'][attributes[contains(., 'MaxLength')]]/name" \
    --expect 1 -m "find bool properties with MaxLength (invalid)"

# Extension method detection (mapper-extension-method.md)
run_test tractor mapper-extension-method.cs \
    -x "//class[static][contains(name, 'Mapper')]//method[public][static][count(parameters/parameter)=1][not(parameters/parameter/this)]/name" \
    --expect 1 -m "find static methods that should be extension methods"

# Block-scoped namespaces (namespaces-file-scoped.md)
run_test tractor namespaces-file-scoped.cs \
    -x "//namespace[body]" \
    --expect 1 -m "find block-scoped namespaces (not file-scoped)"

# Repository GetAll without OrderBy (repository-getall-orderby.md)
run_test tractor repository-getall-orderby.cs \
    -x "//class[contains(name, 'Repository')][not(contains(name, 'Mock'))]//method[contains(name, 'GetAll')][not(contains(., 'OrderBy'))]/name" \
    --expect 1 -m "find GetAll methods missing OrderBy"

# Query without AsNoTracking (query-asnotracking.md)
run_test tractor query-asnotracking.cs \
    -x "//method[contains(name, 'Get')][contains(., '_context')][contains(., 'Map')][not(contains(., 'AsNoTracking'))]/name" \
    --expect 1 -m "find Get methods using _context without AsNoTracking"

# -------------------------------------------------------------------------
# Generic Type Matching tests
# Test exact string-value matching on generic types
# -------------------------------------------------------------------------

echo ""
echo "  Generic Type Matching:"

# Basic generic type matching (2 each: return type + new expression)
run_test tractor generic-type-match.cs \
    -x "//type[.='List<string>']" \
    --expect 2 -m "exact match List<string>"

run_test tractor generic-type-match.cs \
    -x "//type[.='Dictionary<string, int>']" \
    --expect 2 -m "exact match Dictionary<string, int>"

# Find all generic types (2 per method × 3 methods + 2 nested = 8)
run_test tractor generic-type-match.cs \
    -x "//type[generic]" \
    --expect 8 -m "find all generic types (including nested)"

# Nested generic matching (2: return type + new expression)
run_test tractor generic-type-match.cs \
    -x "//type[.='List<Dictionary<string, User>>']" \
    --expect 2 -m "exact match nested generic"

# Query type arguments (List<string>×2 + Dictionary<string,int>×2 + Dictionary<string,User>×2 = 6)
run_test tractor generic-type-match.cs \
    -x "//type[generic]/arguments/type[.='string']" \
    --expect 6 -m "find string type arguments"

report
