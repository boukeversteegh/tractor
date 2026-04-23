# Unified `<string>` AST shape across languages

## Context

Reviewing the v1 Go transforms, the current `<string>"hello"</string>`
shape keeps the source quotes as part of the string's XPath string-value
— so a user who wants to match the content `hello` can't write
`//string='hello'`, they have to write `//string='"hello"'` or use
`contains()`. That's the opposite of what a query-oriented AST
should feel like.

The shape also diverges across languages:

- Go interpreted string: `<string>"hello\n"</string>` — quotes
  included, escapes intact.
- Go raw string: `<string><raw/>` + the backtick-wrapped body
  (`` `hello` ``) as text content.
- Python f-string after our flatten: `<string>f"hello <interpolation>...`
  — the `f"` prefix and quotes end up as raw text siblings.
- TS template literal: similar pattern.
- C# interpolated string: similar.

A user who wants "find every string containing the word 'error'"
today has to write language-specific queries that account for
quotes, prefixes, escape sequences, and concatenation.

## Problem

We need a unified design so that:

1. **Content queries are intuitive**: `//string='hello'` matches
   a source literal `"hello"` (or `'hello'` or `` `hello` ``) without
   the user thinking about quote characters.
2. **Formatting details are still queryable**: raw/interpreted,
   interpolated, multi-line, byte-string, etc. are markers the user
   can predicate on when they care.
3. **Concatenations are optionally merged**: `"hello" + "world"` in
   Python, or C#'s adjacent-string concatenation, can be treated
   as one logical string when the query is about content. Same
   principle applies to multi-line tokens we already merge in C#
   comments.

## Design goals

For v1, these are the goals we want to move toward even if the
full implementation is deferred:

- `//string='hello'` and `//string[.='hello']` match a string
  literal whose content is `hello`, regardless of quoting style.
- The source punctuation (`"`, `` ` ``, `f"`, `b"`, `'`, heredoc
  markers) should not pollute the string's XPath string-value.
- Formatting variants stay queryable as markers: `//string[raw]`,
  `//string[interpolated]`, `//string[multiline]`, `//string[byte]`.
- Interpolation expressions remain structurally visible so
  `//string/interpolation/name='age'` keeps working.
- Concatenation handling is an open question. Candidates:
  - Transform concatenation into a single `<string>` node with
    merged content when all parts are literals.
  - Leave `+` expressions as-is, but introduce a `string-content()`
    function or similar so queries can ask "does this expression
    produce a string containing X".
  - Do nothing for v1; cite it as a known limitation.

## Candidate shapes

Three candidate shapes we considered:

| Shape | Example | Pros | Cons |
|---|---|---|---|
| **A. Bare content** | `<string>hello</string>` | `//string='hello'` works. | Lossy — source quotes gone. Round-trip back to source needs a lookup. |
| **B. Quoted + content child** | `<string>"<literal>hello</literal>"</string>` | Content addressable via `//string/literal='hello'`; source quotes preserved as siblings. | Two-level predicate; less "natural" for simple content queries. |
| **C. Content + punctuation markers** | `<string><open>"</open>hello<close>"</close></string>` | Both content and punctuation queryable by element. | Very verbose; breaks the "string is text" mental model. |

**Lean**: shape A (bare content) for the common case. A user who
needs the exact source snippet can use `-v source` (which pulls
from the file). The punctuation / prefix / quote-kind goes into
markers: `<string><double/>hello</string>`, `<string><backtick/>hello</string>`,
etc. Multi-part f-strings keep their `<interpolation>` children
with literal text fragments as siblings.

## Desired state

- Every language emits `<string>CONTENT</string>` where CONTENT
  is the decoded body of the literal, without surrounding quotes
  or language-specific prefixes.
- Quote kind / raw-ness / interpolation / multi-line-ness are
  empty markers on `<string>`.
- f-strings / template literals / interpolated strings render as
  `<string><interpolation>...</interpolation></string>` with
  literal text portions as plain text children.
- Concatenation merging deferred — document the limitation,
  revisit when users actually hit it.

## Notes

- This is a cross-cutting change touching every language's string
  handler (`.rs` files under `tractor/src/languages/`).
- Existing fixtures will churn significantly; plan as its own
  commit series.
- Raw string content currently includes the surrounding backticks /
  heredoc markers in several languages; those need to go too.
- Python f-string internals (`string_start` / `string_content` /
  `string_end`) already got flattened, so the scaffolding is partly
  there — just not the quote-stripping.
- Escape-sequence decoding is a separate wrinkle: do we render
  `"hello\n"` as `hello\n` (literal backslash-n) or `hello<newline>`?
  Probably the former for v1 (preserve what the user wrote); decoded
  form can be an opt-in.
