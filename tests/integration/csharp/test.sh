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
    -x "//prop[attrs[contains(., 'MaxLength')]][not(attrs[contains(., 'AutoTruncate')])]/name" \
    --expect 1 -m "find props with MaxLength but missing AutoTruncate"

# MaxLength on boolean (attribute-maxlength-boolean.md)
run_test tractor attribute-maxlength-boolean.cs \
    -x "//prop[type='bool'][attrs[contains(., 'MaxLength')]]/name" \
    --expect 1 -m "find bool props with MaxLength (invalid)"

# Extension method detection (mapper-extension-method.md)
run_test tractor mapper-extension-method.cs \
    -x "//class[static][contains(name, 'Mapper')]//method[public][static][count(params/param)=1][not(params/param/this)]/name" \
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

report
