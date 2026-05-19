// DO NOT EDIT — emitted by `tractor/build.rs` on every build.
// Source: DataTree enum in tractor/src/tree/data/types.rs.
//
// Variant-blind reflection metadata for the data-language tree
// (JSON / YAML / TOML / INI / Markdown / ...). Mirrors the
// `tree/syntax/metadata.generated.rs` shape one-for-one — same
// codegen functions in `tractor/build_codegen.rs`, same accessor
// signatures (modulo `from_json` which is `SyntaxTree`-only).

#![cfg(feature = "native")]
#![allow(clippy::too_many_lines)]

#[allow(unused_imports)]
use super::types::{DataTree, element_name_for_data_element};
#[allow(unused_imports)]
use crate::tree::types::{ByteRange, Flag, Marker, Span};
#[allow(unused_imports)]
use crate::tree::walker::TreeField;

/// The XML element name for this tree node, or `None` if the
/// node renders no wrapper (`Inline`, `Skip`).
///
/// Borrowed lifetime: most arms return `&'static str` literals or
/// `&'static str` field values, but `Unknown` / `Raw` carry `String`
/// kinds so the return type ties to the tree.
pub fn element_name_of(tree: &DataTree) -> Option<&str> {
    match tree {
        DataTree::Document { .. } => Some("document"),
        DataTree::Mapping { .. } => Some("mapping"),
        DataTree::Sequence { .. } => Some("sequence"),
        DataTree::Pair { .. } => Some("pair"),
        DataTree::Section { .. } => Some("section"),
        DataTree::String { .. } => Some("string"),
        DataTree::Number { .. } => Some("number"),
        DataTree::Bool { .. } => Some("bool"),
        DataTree::Null { .. } => Some("null"),
        DataTree::Comment { .. } => Some("comment"),
        DataTree::Directive { .. } => Some("directive"),
        DataTree::Element { .. } => Some(element_name_for_data_element(tree)),
        DataTree::Unknown { .. } => Some("unknown"),
    }
}

/// Empty-element marker children for this tree node. Drawn from
/// `Modifiers::markers_with_spans()`, any `Flag` field (named after
/// the field with trailing `_` stripped), `Vec<Marker>` and
/// `Vec<&'static str>` marker-name fields.
///
/// Each entry's `span` lets the XML renderer emit
/// `line` / `column` attributes at the keyword position when the flag
/// is anchored, falling back to a default for implicit flags.
pub fn flags_of(tree: &DataTree) -> Vec<Marker> {
    let mut out: Vec<Marker> = Vec::new();
    match tree {
        DataTree::Document { .. } => {}
        DataTree::Mapping { .. } => {}
        DataTree::Sequence { .. } => {}
        DataTree::Pair { .. } => {}
        DataTree::Section { .. } => {}
        DataTree::String { .. } => {}
        DataTree::Number { .. } => {}
        DataTree::Bool { .. } => {}
        DataTree::Null { .. } => {}
        DataTree::Comment { leading, trailing, span, .. } => {
            if let Flag::On { range: frange, span: fspan } = leading {
                out.push(Marker { name: "leading", range: *frange, span: *fspan });
            }
            if let Flag::On { range: frange, span: fspan } = trailing {
                out.push(Marker { name: "trailing", range: *frange, span: *fspan });
            }

        }
        DataTree::Directive { extra_markers, .. } => {
            for m in extra_markers { out.push(*m); }

        }
        DataTree::Element { markers, span, .. } => {
            for m in markers {
                out.push(Marker {
                    name: m,
                    range: ByteRange::synthetic_empty(),
                    span: *span,
                });
            }

        }
        DataTree::Unknown { .. } => {}
    }
    out
}

/// Source byte range of this node. Used for verbatim-source
/// recovery (`source[range]`) and for gap-text computation in the
/// renderer. Every variant carries a `range: ByteRange` field.
pub fn range_of(tree: &DataTree) -> ByteRange {
    match tree {
        DataTree::Document { range, .. } => *range,
        DataTree::Mapping { range, .. } => *range,
        DataTree::Sequence { range, .. } => *range,
        DataTree::Pair { range, .. } => *range,
        DataTree::Section { range, .. } => *range,
        DataTree::String { range, .. } => *range,
        DataTree::Number { range, .. } => *range,
        DataTree::Bool { range, .. } => *range,
        DataTree::Null { range, .. } => *range,
        DataTree::Comment { range, .. } => *range,
        DataTree::Directive { range, .. } => *range,
        DataTree::Element { range, .. } => *range,
        DataTree::Unknown { range, .. } => *range,
    }
}

/// Source-location span of this node. Used for XML attribute
/// emission (`line` / `column` / `end_line` / `end_column` / `id`).
/// Every variant carries a `span: Span` field.
pub fn span_of(tree: &DataTree) -> Span {
    match tree {
        DataTree::Document { span, .. } => *span,
        DataTree::Mapping { span, .. } => *span,
        DataTree::Sequence { span, .. } => *span,
        DataTree::Pair { span, .. } => *span,
        DataTree::Section { span, .. } => *span,
        DataTree::String { span, .. } => *span,
        DataTree::Number { span, .. } => *span,
        DataTree::Bool { span, .. } => *span,
        DataTree::Null { span, .. } => *span,
        DataTree::Comment { span, .. } => *span,
        DataTree::Directive { span, .. } => *span,
        DataTree::Element { span, .. } => *span,
        DataTree::Unknown { span, .. } => *span,
    }
}

/// Stored text for scalar-leaf variants (`Name`, `Atom`, `Int`,
/// `Float`, `String`, `True`, `False`, `None`, `Null`). Returns
/// `None` for compound variants. The renderer uses this to emit
/// leaf literals without consulting the source string (S13-Z1).
pub fn scalar_text_of(tree: &DataTree) -> Option<&str> {
    match tree {
        DataTree::String { value, .. } => Some(value.as_str()),
        DataTree::Number { text, .. } => Some(text.as_str()),
        DataTree::Bool { text, .. } => Some(text.as_str()),
        DataTree::Null { text, .. } => Some(text.as_str()),
        DataTree::Comment { text, .. } => Some(text.as_str()),
        _ => None,
    }
}

/// Direct tree children of this node, in source order. Excludes
/// synthetic render-time wrappers, modifier markers, and other shape
/// metadata. Used by every variant-blind walker (`to_xot.rs`,
/// `to_json.rs`, …) as the single source of truth for tree traversal.
pub fn children_of(tree: &DataTree) -> Vec<&DataTree> {
    let mut v: Vec<&DataTree> = Vec::new();
    match tree {
        DataTree::Document { children, .. } => {
            v.extend(children.iter());
        }
        DataTree::Mapping { pairs, .. } => {
            v.extend(pairs.iter());
        }
        DataTree::Sequence { items, .. } => {
            v.extend(items.iter());
        }
        DataTree::Pair { key, value, .. } => {
            v.push(key);
            v.push(value);
        }
        DataTree::Section { name, children, .. } => {
            v.push(name);
            v.extend(children.iter());
        }
        DataTree::String { .. } => {}
        DataTree::Number { .. } => {}
        DataTree::Bool { .. } => {}
        DataTree::Null { .. } => {}
        DataTree::Comment { .. } => {}
        DataTree::Directive { children, .. } => {
            v.extend(children.iter());
        }
        DataTree::Element { children, .. } => {
            v.extend(children.iter());
        }
        DataTree::Unknown { .. } => {}
    }
    v.sort_by_key(|c| range_of(c).start);
    v
}

/// Field-projection view of this node — one [`TreeField`] per
/// projected struct field. Only consulted when [`use_field_projection`]
/// returns true. See the walker for projection semantics.
pub fn fields_of(tree: &DataTree) -> Vec<TreeField<'_, DataTree>> {
    let mut out: Vec<TreeField<'_, DataTree>> = Vec::new();
    match tree {
        DataTree::Document { children, .. } => {
            out.push(TreeField::Many { name: "children", items: children.iter().collect() });
        }
        DataTree::Mapping { pairs, .. } => {
            out.push(TreeField::Many { name: "pairs", items: pairs.iter().collect() });
        }
        DataTree::Sequence { items, .. } => {
            out.push(TreeField::Many { name: "items", items: items.iter().collect() });
        }
        DataTree::Pair { key, value, .. } => {
            out.push(TreeField::Single { name: "key", value: key });
            out.push(TreeField::Single { name: "value", value: value });
        }
        DataTree::Section { name, children, .. } => {
            out.push(TreeField::Single { name: "name", value: name });
            out.push(TreeField::Many { name: "children", items: children.iter().collect() });
        }
        DataTree::String { value, .. } => {
            out.push(TreeField::Scalar { name: "value", value: value.as_str() });
        }
        DataTree::Number { text, .. } => {
            out.push(TreeField::Scalar { name: "text", value: text.as_str() });
        }
        DataTree::Bool { text, .. } => {
            out.push(TreeField::Scalar { name: "text", value: text.as_str() });
        }
        DataTree::Null { text, .. } => {
            out.push(TreeField::Scalar { name: "text", value: text.as_str() });
        }
        DataTree::Comment { text, leading, trailing, .. } => {
            out.push(TreeField::Scalar { name: "text", value: text.as_str() });
            if let Flag::On { range: frange, span: fspan } = leading {
                out.push(TreeField::Flag {
                    name: "leading",
                    marker: Marker { name: "leading", range: *frange, span: *fspan },
                });
            }
            if let Flag::On { range: frange, span: fspan } = trailing {
                out.push(TreeField::Flag {
                    name: "trailing",
                    marker: Marker { name: "trailing", range: *frange, span: *fspan },
                });
            }
        }
        DataTree::Directive { extra_markers, children, .. } => {
            for m in extra_markers { out.push(TreeField::Flag { name: m.name, marker: *m }); }
            out.push(TreeField::Many { name: "children", items: children.iter().collect() });
        }
        DataTree::Element { markers, children, span, .. } => {
            for m in markers {
                out.push(TreeField::Flag {
                    name: m,
                    marker: Marker { name: m, range: ByteRange::synthetic_empty(), span: *span },
                });
            }
            out.push(TreeField::Many { name: "children", items: children.iter().collect() });
        }
        DataTree::Unknown { kind, .. } => {
            out.push(TreeField::Scalar { name: "kind", value: kind.as_str() });
        }
    }
    out
}

/// Per-variant opt-in flag for the field-projection walker
/// path. Returns true for variants whose doc comments carry
/// `@field_projection`; false otherwise.
pub fn use_field_projection(tree: &DataTree) -> bool {
    match tree {
        _ => false,
    }
}

/// Mutable access to the source-location span. Used by
/// `assign_ids` to stamp `NodeId`s into existing spans without
/// rebuilding nodes. Delegates from `TreeNode::span_mut`.
pub fn span_mut_of(tree: &mut DataTree) -> &mut Span {
    match tree {
        DataTree::Document { span, .. } => span,
        DataTree::Mapping { span, .. } => span,
        DataTree::Sequence { span, .. } => span,
        DataTree::Pair { span, .. } => span,
        DataTree::Section { span, .. } => span,
        DataTree::String { span, .. } => span,
        DataTree::Number { span, .. } => span,
        DataTree::Bool { span, .. } => span,
        DataTree::Null { span, .. } => span,
        DataTree::Comment { span, .. } => span,
        DataTree::Directive { span, .. } => span,
        DataTree::Element { span, .. } => span,
        DataTree::Unknown { span, .. } => span,
    }
}

/// Mutable mirror of `children_of`. Source-sorted
/// `Vec<&mut DataTree>` covering every reachable sub-tree.
pub fn children_mut_of(tree: &mut DataTree) -> Vec<&mut DataTree> {
    let mut v: Vec<&mut DataTree> = Vec::new();
    match tree {
        DataTree::Document { children, .. } => {
            v.extend(children.iter_mut());
        }
        DataTree::Mapping { pairs, .. } => {
            v.extend(pairs.iter_mut());
        }
        DataTree::Sequence { items, .. } => {
            v.extend(items.iter_mut());
        }
        DataTree::Pair { key, value, .. } => {
            v.push(key.as_mut());
            v.push(value.as_mut());
        }
        DataTree::Section { name, children, .. } => {
            v.push(name.as_mut());
            v.extend(children.iter_mut());
        }
        DataTree::String { .. } => {}
        DataTree::Number { .. } => {}
        DataTree::Bool { .. } => {}
        DataTree::Null { .. } => {}
        DataTree::Comment { .. } => {}
        DataTree::Directive { children, .. } => {
            v.extend(children.iter_mut());
        }
        DataTree::Element { children, .. } => {
            v.extend(children.iter_mut());
        }
        DataTree::Unknown { .. } => {}
    }
    v.sort_by_key(|c| range_of(c).start);
    v
}
