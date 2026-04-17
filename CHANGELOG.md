# Changelog

## Unreleased

- `-v count` now follows the normal report renderer instead of bypassing it.
  In text output this means the default query output is the report-style
  summary (`N matches`) rather than a bare scalar. Use `-p count` to request
  the old scalar shape explicitly.
