// DO NOT EDIT — regenerate via `task gen:walker`.
// Source: SyntaxTree enum in tractor/src/tree/types.rs.
//
// Variant-blind accessors that drive the XML renderer's mechanical
// walk in `to_xot.rs`. Rules are derived from field types only — no
// per-variant special cases. See `tractor/src/bin/gen_walker.rs`.

#![cfg(feature = "native")]

#[allow(unused_imports)]
use super::types::{ByteRange, Flag, Marker, Span, SyntaxTree};

/// The XML element name for this tree node, or `None` if the
/// node renders no wrapper (`Inline`, `Skip`).
///
/// Borrowed lifetime: most arms return `&'static str` literals or
/// `&'static str` field values, but `Unknown` / `Raw` carry `String`
/// kinds so the return type ties to the tree.
pub fn element_name_of(tree: &SyntaxTree) -> Option<&str> {
    match tree {
        SyntaxTree::Module { element_name, .. } => Some(*element_name),
        SyntaxTree::Expression { .. } => Some("expression"),
        SyntaxTree::Access { .. } => Some("access"),
        SyntaxTree::Binary { element_name, .. } => Some(*element_name),
        SyntaxTree::Unary { .. } => Some("unary"),
        SyntaxTree::Tuple { .. } => Some("tuple"),
        SyntaxTree::List { .. } => Some("list"),
        SyntaxTree::Set { .. } => Some("set"),
        SyntaxTree::Dictionary { .. } => Some("dictionary"),
        SyntaxTree::Pair { .. } => Some("pair"),
        SyntaxTree::GenericType { .. } => Some("generic_type"),
        SyntaxTree::Comparison { .. } => Some("comparison"),
        SyntaxTree::If { .. } => Some("if"),
        SyntaxTree::ElseIf { .. } => Some("else_if"),
        SyntaxTree::Else { .. } => Some("else"),
        SyntaxTree::For { .. } => Some("for"),
        SyntaxTree::While { .. } => Some("while"),
        SyntaxTree::Foreach { .. } => Some("foreach"),
        SyntaxTree::CFor { .. } => Some("c_for"),
        SyntaxTree::DoWhile { .. } => Some("do_while"),
        SyntaxTree::Break { .. } => Some("break"),
        SyntaxTree::Continue { .. } => Some("continue"),
        SyntaxTree::FieldWrap { wrapper, .. } => Some(*wrapper),
        SyntaxTree::SimpleStatement { element_name, .. } => Some(*element_name),
        SyntaxTree::Try { .. } => Some("try"),
        SyntaxTree::ExceptHandler { kind, .. } => Some(*kind),
        SyntaxTree::TypeAlias { .. } => Some("type_alias"),
        SyntaxTree::KeywordArgument { .. } => Some("keyword_argument"),
        SyntaxTree::ListSplat { .. } => Some("list_splat"),
        SyntaxTree::DictSplat { .. } => Some("dict_splat"),
        SyntaxTree::Ternary { .. } => Some("ternary"),
        SyntaxTree::ObjectCreation { .. } => Some("object_creation"),
        SyntaxTree::Lambda { .. } => Some("lambda"),
        SyntaxTree::Function { element_name, .. } => Some(*element_name),
        SyntaxTree::Class { kind, .. } => Some(*kind),
        SyntaxTree::Body { .. } => Some("body"),
        SyntaxTree::Parameter { .. } => Some("parameter"),
        SyntaxTree::Skip { .. } => None,
        SyntaxTree::PositionalSeparator { .. } => Some("positional_separator"),
        SyntaxTree::KeywordSeparator { .. } => Some("keyword_separator"),
        SyntaxTree::Decorator { .. } => Some("decorator"),
        SyntaxTree::Returns { .. } => Some("returns"),
        SyntaxTree::Generic { .. } => Some("generic"),
        SyntaxTree::TypeParameter { .. } => Some("type_parameter"),
        SyntaxTree::Return { .. } => Some("return"),
        SyntaxTree::Comment { .. } => Some("comment"),
        SyntaxTree::Assign { .. } => Some("assign"),
        SyntaxTree::Import { .. } => Some("import"),
        SyntaxTree::From { .. } => Some("from"),
        SyntaxTree::FromImport { .. } => Some("from_import"),
        SyntaxTree::Path { .. } => Some("path"),
        SyntaxTree::Aliased { .. } => Some("aliased"),
        SyntaxTree::Call { .. } => Some("call"),
        SyntaxTree::Name { .. } => Some("name"),
        SyntaxTree::Int { .. } => Some("int"),
        SyntaxTree::Float { .. } => Some("float"),
        SyntaxTree::String { .. } => Some("string"),
        SyntaxTree::True { .. } => Some("true"),
        SyntaxTree::False { .. } => Some("false"),
        SyntaxTree::None { .. } => Some("none"),
        SyntaxTree::Atom { element_name, .. } => Some(*element_name),
        SyntaxTree::Enum { .. } => Some("enum"),
        SyntaxTree::EnumMember { .. } => Some("enum_member"),
        SyntaxTree::Property { .. } => Some("property"),
        SyntaxTree::Accessor { kind, .. } => Some(*kind),
        SyntaxTree::Constructor { .. } => Some("constructor"),
        SyntaxTree::Using { .. } => Some("using"),
        SyntaxTree::Namespace { .. } => Some("namespace"),
        SyntaxTree::Variable { element_name, .. } => Some(*element_name),
        SyntaxTree::Is { .. } => Some("is"),
        SyntaxTree::Cast { .. } => Some("cast"),
        SyntaxTree::Null { .. } => Some("null"),
        SyntaxTree::Inline { .. } => None,
        SyntaxTree::Unknown { kind, .. } => Some(kind.as_str()),
        SyntaxTree::Raw { kind, .. } => Some(kind.as_str()),
    }
}

/// Empty-element marker children for this tree node. Drawn from
/// `Modifiers::markers_with_spans()`, any `Flag` field (named after
/// the field with trailing `_` stripped), and any `Vec<Marker>` field.
///
/// Each entry's `span` lets the XML renderer emit
/// `line` / `column` attributes at the keyword position when the flag
/// is anchored, falling back to a default for implicit flags.
pub fn flags_of(tree: &SyntaxTree) -> Vec<Marker> {
    let mut out: Vec<Marker> = Vec::new();
    match tree {
        SyntaxTree::Module { .. } => {}
        SyntaxTree::Expression { .. } => {}
        SyntaxTree::Access { .. } => {}
        SyntaxTree::Binary { .. } => {}
        SyntaxTree::Unary { extra_markers, .. } => {
            for m in extra_markers { out.push(*m); }

        }
        SyntaxTree::Tuple { .. } => {}
        SyntaxTree::List { .. } => {}
        SyntaxTree::Set { .. } => {}
        SyntaxTree::Dictionary { .. } => {}
        SyntaxTree::Pair { .. } => {}
        SyntaxTree::GenericType { .. } => {}
        SyntaxTree::Comparison { .. } => {}
        SyntaxTree::If { .. } => {}
        SyntaxTree::ElseIf { .. } => {}
        SyntaxTree::Else { .. } => {}
        SyntaxTree::For { .. } => {}
        SyntaxTree::While { .. } => {}
        SyntaxTree::Foreach { .. } => {}
        SyntaxTree::CFor { .. } => {}
        SyntaxTree::DoWhile { .. } => {}
        SyntaxTree::Break { .. } => {}
        SyntaxTree::Continue { .. } => {}
        SyntaxTree::FieldWrap { .. } => {}
        SyntaxTree::SimpleStatement { modifiers, extra_markers, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(Marker {
                    name: mname,
                    range: ByteRange::synthetic_empty(),
                    span: mspan.unwrap_or(*span),
                });
            }
            for m in extra_markers { out.push(*m); }

        }
        SyntaxTree::Try { .. } => {}
        SyntaxTree::ExceptHandler { .. } => {}
        SyntaxTree::TypeAlias { .. } => {}
        SyntaxTree::KeywordArgument { .. } => {}
        SyntaxTree::ListSplat { .. } => {}
        SyntaxTree::DictSplat { .. } => {}
        SyntaxTree::Ternary { .. } => {}
        SyntaxTree::ObjectCreation { .. } => {}
        SyntaxTree::Lambda { modifiers, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(Marker {
                    name: mname,
                    range: ByteRange::synthetic_empty(),
                    span: mspan.unwrap_or(*span),
                });
            }

        }
        SyntaxTree::Function { modifiers, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(Marker {
                    name: mname,
                    range: ByteRange::synthetic_empty(),
                    span: mspan.unwrap_or(*span),
                });
            }

        }
        SyntaxTree::Class { modifiers, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(Marker {
                    name: mname,
                    range: ByteRange::synthetic_empty(),
                    span: mspan.unwrap_or(*span),
                });
            }

        }
        SyntaxTree::Body { .. } => {}
        SyntaxTree::Parameter { extra_markers, modifiers, span, .. } => {
            for m in extra_markers { out.push(*m); }
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(Marker {
                    name: mname,
                    range: ByteRange::synthetic_empty(),
                    span: mspan.unwrap_or(*span),
                });
            }

        }
        SyntaxTree::Skip { .. } => {}
        SyntaxTree::PositionalSeparator { .. } => {}
        SyntaxTree::KeywordSeparator { .. } => {}
        SyntaxTree::Decorator { .. } => {}
        SyntaxTree::Returns { .. } => {}
        SyntaxTree::Generic { .. } => {}
        SyntaxTree::TypeParameter { .. } => {}
        SyntaxTree::Return { .. } => {}
        SyntaxTree::Comment { .. } => {}
        SyntaxTree::Assign { .. } => {}
        SyntaxTree::Import { .. } => {}
        SyntaxTree::From { .. } => {}
        SyntaxTree::FromImport { .. } => {}
        SyntaxTree::Path { .. } => {}
        SyntaxTree::Aliased { .. } => {}
        SyntaxTree::Call { .. } => {}
        SyntaxTree::Name { .. } => {}
        SyntaxTree::Int { .. } => {}
        SyntaxTree::Float { .. } => {}
        SyntaxTree::String { .. } => {}
        SyntaxTree::True { .. } => {}
        SyntaxTree::False { .. } => {}
        SyntaxTree::None { .. } => {}
        SyntaxTree::Atom { .. } => {}
        SyntaxTree::Enum { modifiers, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(Marker {
                    name: mname,
                    range: ByteRange::synthetic_empty(),
                    span: mspan.unwrap_or(*span),
                });
            }

        }
        SyntaxTree::EnumMember { .. } => {}
        SyntaxTree::Property { modifiers, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(Marker {
                    name: mname,
                    range: ByteRange::synthetic_empty(),
                    span: mspan.unwrap_or(*span),
                });
            }

        }
        SyntaxTree::Accessor { modifiers, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(Marker {
                    name: mname,
                    range: ByteRange::synthetic_empty(),
                    span: mspan.unwrap_or(*span),
                });
            }

        }
        SyntaxTree::Constructor { modifiers, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(Marker {
                    name: mname,
                    range: ByteRange::synthetic_empty(),
                    span: mspan.unwrap_or(*span),
                });
            }

        }
        SyntaxTree::Using { .. } => {}
        SyntaxTree::Namespace { .. } => {}
        SyntaxTree::Variable { modifiers, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(Marker {
                    name: mname,
                    range: ByteRange::synthetic_empty(),
                    span: mspan.unwrap_or(*span),
                });
            }

        }
        SyntaxTree::Is { .. } => {}
        SyntaxTree::Cast { .. } => {}
        SyntaxTree::Null { .. } => {}
        SyntaxTree::Inline { .. } => {}
        SyntaxTree::Unknown { .. } => {}
        SyntaxTree::Raw { .. } => {}
    }
    out
}
