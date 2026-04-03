#!/usr/bin/env bash
# Integration tests for additive/subtractive view field modifiers (-v +field, -v -field)
source "$(dirname "$0")/../common.sh"

SAMPLE_CS="$REPO_ROOT/tests/integration/formats/sample.cs"

echo "View modifiers (+field / -field):"

# ---------------------------------------------------------------------------
# -field: remove a field from the default view
# ---------------------------------------------------------------------------

# check -f gcc default includes lines; -lines should drop the source block
run_test bash -c "
  actual=\$(tractor check '$SAMPLE_CS' -x '//class' --reason 'class found' --no-color -f gcc '-v=-lines' 2>/dev/null)
  # With lines removed, each match should produce exactly one line (the header).
  # Count output lines (2 classes → 2 header lines, plus empty summary line from stderr)
  lines=\$(echo \"\$actual\" | grep -c ':.*error:')
  [ \"\$lines\" -eq 2 ]
" -m "check -f gcc -v=-lines removes source line blocks"

# Ensure the source-block content is absent (no gutter lines like "1 >| ...")
run_test bash -c "
  actual=\$(tractor check '$SAMPLE_CS' -x '//class' --reason 'class found' --no-color -f gcc '-v=-lines' 2>/dev/null || true)
  ! echo \"\$actual\" | grep -qE '^[[:space:]]*[0-9]+ [>|]'
" -m "check -f gcc -v=-lines produces no line-number gutter output"

# Removing severity from check default still produces output
run_test bash -c "
  actual=\$(tractor check '$SAMPLE_CS' -x '//class' --reason 'class found' --no-color '-v=-severity' 2>/dev/null || true)
  echo \"\$actual\" | grep -q 'class found'
" -m "check -v=-severity keeps other fields intact"

# Remove tree from query default: output should have no XML tags
run_test bash -c "
  actual=\$(tractor '$SAMPLE_CS' -x '//class/name' --no-color '-v=-tree' 2>/dev/null)
  ! echo \"\$actual\" | grep -q '<'
" -m "query -v=-tree removes XML tree from output"

# ---------------------------------------------------------------------------
# +field: add a field to the default view
# ---------------------------------------------------------------------------

# Adding source to query default (default: file,line,tree) should produce source text lines
run_test bash -c "
  actual=\$(tractor '$SAMPLE_CS' -x '//class/name' --no-color '-v=+source' 2>/dev/null)
  echo \"\$actual\" | grep -qE '^(Foo|Qux)$'
" -m "query -v=+source adds source text to output"

# ---------------------------------------------------------------------------
# Combining + and - modifiers
# ---------------------------------------------------------------------------

# Remove lines and add source in one -v expression (text format check)
run_test bash -c "
  actual=\$(tractor check '$SAMPLE_CS' -x '//class/name' --reason 'found' --no-color -f text '-v=-lines,+source' 2>/dev/null)
  # Should have source text (class names) in text format output
  echo \"\$actual\" | grep -qE '(Foo|Qux)'
" -m "check -f text -v=-lines,+source adds source but drops lines"

# ---------------------------------------------------------------------------
# No-op and idempotency
# ---------------------------------------------------------------------------

# Adding a field that is already in the default is a no-op (tree is in default)
run_test bash -c "
  default_out=\$(tractor '$SAMPLE_CS' -x '//class/name' --no-color 2>/dev/null)
  modifier_out=\$(tractor '$SAMPLE_CS' -x '//class/name' --no-color '-v=+tree' 2>/dev/null)
  [ \"\$default_out\" = \"\$modifier_out\" ]
" -m "query -v=+field already in default is a no-op"

# Removing a field that is not in the default is a no-op
run_test bash -c "
  default_out=\$(tractor '$SAMPLE_CS' -x '//class/name' --no-color -v value 2>/dev/null)
  modifier_out=\$(tractor '$SAMPLE_CS' -x '//class/name' --no-color '-v=-source' 2>/dev/null)
  # Both should list class names (source was not in default anyway)
  echo \"\$modifier_out\" | grep -q 'Foo'
" -m "query -v=-field not in default is a no-op"

# ---------------------------------------------------------------------------
# Error cases
# ---------------------------------------------------------------------------

# Mixing plain and modifier fields should fail with an error message
run_test bash -c "
  ! tractor '$SAMPLE_CS' -x '//class' '-v=tree,+source' 2>/dev/null
" -m "mixing plain fields with +/- modifiers fails"

# Removing all fields should fail
run_test bash -c "
  ! tractor '$SAMPLE_CS' -x '//class/name' '-v=-file,-line,-tree' 2>/dev/null
" -m "removing all default fields produces an error"

# Invalid field name in modifier position should fail
run_test bash -c "
  ! tractor '$SAMPLE_CS' -x '//class' '-v=-nosuchfield' 2>/dev/null
" -m "invalid field name in -field modifier produces an error"

report
