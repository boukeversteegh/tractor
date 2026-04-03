# Nest Rust attributes as children of their annotated item

## Context

While adding a self-lint rule to enforce `#[serde(deny_unknown_fields)]`
on all `Deserialize` structs (issue #68), the XPath query had to use
`preceding-sibling::*[position()<=5]` hacks because Rust attributes are
siblings of the struct in tree-sitter's AST, not children. This makes it
impossible to reliably scope "which attributes belong to which struct"
without fragile position bounds.

## Problem

In Rust's tree-sitter grammar, `attribute_item` nodes are siblings of the
`struct`/`enum`/`function` they annotate:

```xml
<attribute_item>#[derive(Deserialize)]</attribute_item>
<attribute_item>#[serde(deny_unknown_fields)]</attribute_item>
<struct>
  <name>Foo</name>
  ...
</struct>
```

This means XPath queries like `//struct[attribute[contains(.,"X")]]` don't
work. Instead you need positional sibling checks that break when structs
have varying numbers of attributes.

C# gets this right — attributes are children of the declaration node, so
`//class[attribute[...]]` works naturally.

## Desired state

Tractor transforms the Rust XML so consecutive `attribute_item` nodes
immediately preceding an item are nested inside it:

```xml
<struct>
  <attribute_item>#[derive(Deserialize)]</attribute_item>
  <attribute_item>#[serde(deny_unknown_fields)]</attribute_item>
  <name>Foo</name>
  ...
</struct>
```

This enables natural queries like:
- `//struct[attribute_item[contains(.,"Deserialize")]]`
- `//function[attribute_item[contains(.,"test")]]`

The `tractor-lint.yaml` rule for `deserialize-deny-unknown-fields` can
then be simplified to remove the `position()<=5` workaround.

## Notes

- This is a semantic transform similar to what's already done for other
  languages — see `semantic-transform-rewrite` (todo 13).
- Applies to structs, enums, functions, impl blocks, traits, etc.
- Should only re-parent consecutive `attribute_item` siblings immediately
  before the annotated item — not all attributes in the file.
- Related: issue #68, `tractor/tractor-lint.yaml`
