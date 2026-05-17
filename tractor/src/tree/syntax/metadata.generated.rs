// DO NOT EDIT — emitted by `tractor/build.rs` on every build.
// Source: SyntaxTree enum in tractor/src/tree/syntax/types.rs.
//
// Variant-blind reflection metadata that drives the XML and JSON
// renderers' mechanical walks (`to_xot.rs`, `to_json.rs`, `from_json.rs`).
// Rules are derived from field types only — no per-variant special
// cases. See `tractor/build_codegen.rs`.

#![cfg(feature = "native")]
#![allow(clippy::too_many_lines)]

#[allow(unused_imports)]
use super::types::{
    Access, AccessReceiver, AccessorKind, AccessSegment, ByteRange,
    Expression, Flag, LambdaBody, Marker, Modifiers, OperatorKind, ParamKind,
    QuoteStyle, SlotKind, Span, SyntaxTree,
};

#[allow(unused_imports)]
use crate::tree::walker::TreeField;

#[allow(unused_imports)]
use serde_json::Value;

// Per-variant element-name overrides declared via
// `@element_name = <fn>` on the variant's doc comment in `types.rs`.
#[allow(unused_imports)]
use super::types::{
    element_name_for_accessor, element_name_for_atom,
    element_name_for_field_wrap, element_name_for_generic_type,
    element_name_for_object_access, element_name_for_raw,
    element_name_for_simple_statement, element_name_for_slot,
    element_name_for_type_parameter,
};

/// The XML element name for this tree node, or `None` if the
/// node renders no wrapper (`Inline`, `Skip`).
///
/// Borrowed lifetime: most arms return `&'static str` literals or
/// `&'static str` field values, but `Unknown` / `Raw` carry `String`
/// kinds so the return type ties to the tree.
pub fn element_name_of(tree: &SyntaxTree) -> Option<&str> {
    match tree {
        SyntaxTree::Module { .. } => Some("module"),
        SyntaxTree::Expression { .. } => Some("expression"),
        SyntaxTree::Slot { .. } => Some(element_name_for_slot(tree)),
        SyntaxTree::ObjectAccess { .. } => Some(element_name_for_object_access(tree)),
        SyntaxTree::Binary { .. } => Some("binary"),
        SyntaxTree::Logical { .. } => Some("logical"),
        SyntaxTree::Operator { .. } => Some("operator"),
        SyntaxTree::Unary { .. } => Some("unary"),
        SyntaxTree::Tuple { .. } => Some("tuple"),
        SyntaxTree::List { .. } => Some("list"),
        SyntaxTree::Set { .. } => Some("set"),
        SyntaxTree::Dictionary { .. } => Some("dictionary"),
        SyntaxTree::Pair { .. } => Some("pair"),
        SyntaxTree::GenericType { .. } => Some(element_name_for_generic_type(tree)),
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
        SyntaxTree::FieldWrap { .. } => Some(element_name_for_field_wrap(tree)),
        SyntaxTree::SimpleStatement { .. } => Some(element_name_for_simple_statement(tree)),
        SyntaxTree::Try { .. } => Some("try"),
        SyntaxTree::Except { .. } => Some("except"),
        SyntaxTree::Catch { .. } => Some("catch"),
        SyntaxTree::TypeAlias { .. } => Some("type_alias"),
        SyntaxTree::KeywordArgument { .. } => Some("keyword_argument"),
        SyntaxTree::ListSplat { .. } => Some("list_splat"),
        SyntaxTree::DictSplat { .. } => Some("dict_splat"),
        SyntaxTree::Ternary { .. } => Some("ternary"),
        SyntaxTree::ObjectCreation { .. } => Some("object_creation"),
        SyntaxTree::Lambda { .. } => Some("lambda"),
        SyntaxTree::Function { .. } => Some("function"),
        SyntaxTree::Method { .. } => Some("method"),
        SyntaxTree::Class { .. } => Some("class"),
        SyntaxTree::Struct { .. } => Some("struct"),
        SyntaxTree::Interface { .. } => Some("interface"),
        SyntaxTree::Record { .. } => Some("record"),
        SyntaxTree::Body { .. } => Some("body"),
        SyntaxTree::Parameter { .. } => Some("parameter"),
        SyntaxTree::Skip { .. } => None,
        SyntaxTree::PositionalSeparator { .. } => Some("positional_separator"),
        SyntaxTree::KeywordSeparator { .. } => Some("keyword_separator"),
        SyntaxTree::Decorator { .. } => Some("decorator"),
        SyntaxTree::Returns { .. } => Some("returns"),
        SyntaxTree::Generic { .. } => Some("generic"),
        SyntaxTree::TypeParameter { .. } => Some(element_name_for_type_parameter(tree)),
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
        SyntaxTree::Atom { .. } => Some(element_name_for_atom(tree)),
        SyntaxTree::Enum { .. } => Some("enum"),
        SyntaxTree::EnumMember { .. } => Some("enum_member"),
        SyntaxTree::Property { .. } => Some("property"),
        SyntaxTree::Accessor { .. } => Some(element_name_for_accessor(tree)),
        SyntaxTree::Constructor { .. } => Some("constructor"),
        SyntaxTree::Using { .. } => Some("using"),
        SyntaxTree::Namespace { .. } => Some("namespace"),
        SyntaxTree::Variable { .. } => Some("variable"),
        SyntaxTree::Field { .. } => Some("field"),
        SyntaxTree::Event { .. } => Some("event"),
        SyntaxTree::Is { .. } => Some("is"),
        SyntaxTree::Cast { .. } => Some("cast"),
        SyntaxTree::Null { .. } => Some("null"),
        SyntaxTree::Inline { .. } => None,
        SyntaxTree::Unknown { .. } => Some("unknown"),
        SyntaxTree::Raw { .. } => Some(element_name_for_raw(tree)),
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
pub fn flags_of(tree: &SyntaxTree) -> Vec<Marker> {
    let mut out: Vec<Marker> = Vec::new();
    match tree {
        SyntaxTree::Module { .. } => {}
        SyntaxTree::Expression { marker, span, .. } => {
            if let Some(name) = marker {
                out.push(Marker {
                    name,
                    range: ByteRange::synthetic_empty(),
                    span: *span,
                });
            }

        }
        SyntaxTree::Slot { .. } => {}
        SyntaxTree::ObjectAccess { .. } => {}
        SyntaxTree::Binary { .. } => {}
        SyntaxTree::Logical { .. } => {}
        SyntaxTree::Operator { .. } => {}
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
        SyntaxTree::Except { .. } => {}
        SyntaxTree::Catch { .. } => {}
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
        SyntaxTree::Method { modifiers, span, .. } => {
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
        SyntaxTree::Struct { modifiers, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(Marker {
                    name: mname,
                    range: ByteRange::synthetic_empty(),
                    span: mspan.unwrap_or(*span),
                });
            }

        }
        SyntaxTree::Interface { modifiers, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(Marker {
                    name: mname,
                    range: ByteRange::synthetic_empty(),
                    span: mspan.unwrap_or(*span),
                });
            }

        }
        SyntaxTree::Record { modifiers, span, .. } => {
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
        SyntaxTree::Comment { leading, trailing, span, .. } => {
            if let Flag::On { range: frange, span: fspan } = leading {
                out.push(Marker { name: "leading", range: *frange, span: *fspan });
            }
            if let Flag::On { range: frange, span: fspan } = trailing {
                out.push(Marker { name: "trailing", range: *frange, span: *fspan });
            }

        }
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
        SyntaxTree::Variable { modifiers, extra_markers, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(Marker {
                    name: mname,
                    range: ByteRange::synthetic_empty(),
                    span: mspan.unwrap_or(*span),
                });
            }
            for m in extra_markers { out.push(*m); }

        }
        SyntaxTree::Field { modifiers, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(Marker {
                    name: mname,
                    range: ByteRange::synthetic_empty(),
                    span: mspan.unwrap_or(*span),
                });
            }

        }
        SyntaxTree::Event { modifiers, span, .. } => {
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

/// Source byte range of this node. Used for verbatim-source
/// recovery (`source[range]`) and for gap-text computation in the
/// renderer. Every variant carries a `range: ByteRange` field.
pub fn range_of(tree: &SyntaxTree) -> ByteRange {
    match tree {
        SyntaxTree::Module { range, .. } => *range,
        SyntaxTree::Expression { range, .. } => *range,
        SyntaxTree::Slot { range, .. } => *range,
        SyntaxTree::ObjectAccess { range, .. } => *range,
        SyntaxTree::Binary { range, .. } => *range,
        SyntaxTree::Logical { range, .. } => *range,
        SyntaxTree::Operator { range, .. } => *range,
        SyntaxTree::Unary { range, .. } => *range,
        SyntaxTree::Tuple { range, .. } => *range,
        SyntaxTree::List { range, .. } => *range,
        SyntaxTree::Set { range, .. } => *range,
        SyntaxTree::Dictionary { range, .. } => *range,
        SyntaxTree::Pair { range, .. } => *range,
        SyntaxTree::GenericType { range, .. } => *range,
        SyntaxTree::Comparison { range, .. } => *range,
        SyntaxTree::If { range, .. } => *range,
        SyntaxTree::ElseIf { range, .. } => *range,
        SyntaxTree::Else { range, .. } => *range,
        SyntaxTree::For { range, .. } => *range,
        SyntaxTree::While { range, .. } => *range,
        SyntaxTree::Foreach { range, .. } => *range,
        SyntaxTree::CFor { range, .. } => *range,
        SyntaxTree::DoWhile { range, .. } => *range,
        SyntaxTree::Break { range, .. } => *range,
        SyntaxTree::Continue { range, .. } => *range,
        SyntaxTree::FieldWrap { range, .. } => *range,
        SyntaxTree::SimpleStatement { range, .. } => *range,
        SyntaxTree::Try { range, .. } => *range,
        SyntaxTree::Except { range, .. } => *range,
        SyntaxTree::Catch { range, .. } => *range,
        SyntaxTree::TypeAlias { range, .. } => *range,
        SyntaxTree::KeywordArgument { range, .. } => *range,
        SyntaxTree::ListSplat { range, .. } => *range,
        SyntaxTree::DictSplat { range, .. } => *range,
        SyntaxTree::Ternary { range, .. } => *range,
        SyntaxTree::ObjectCreation { range, .. } => *range,
        SyntaxTree::Lambda { range, .. } => *range,
        SyntaxTree::Function { range, .. } => *range,
        SyntaxTree::Method { range, .. } => *range,
        SyntaxTree::Class { range, .. } => *range,
        SyntaxTree::Struct { range, .. } => *range,
        SyntaxTree::Interface { range, .. } => *range,
        SyntaxTree::Record { range, .. } => *range,
        SyntaxTree::Body { range, .. } => *range,
        SyntaxTree::Parameter { range, .. } => *range,
        SyntaxTree::Skip { range, .. } => *range,
        SyntaxTree::PositionalSeparator { range, .. } => *range,
        SyntaxTree::KeywordSeparator { range, .. } => *range,
        SyntaxTree::Decorator { range, .. } => *range,
        SyntaxTree::Returns { range, .. } => *range,
        SyntaxTree::Generic { range, .. } => *range,
        SyntaxTree::TypeParameter { range, .. } => *range,
        SyntaxTree::Return { range, .. } => *range,
        SyntaxTree::Comment { range, .. } => *range,
        SyntaxTree::Assign { range, .. } => *range,
        SyntaxTree::Import { range, .. } => *range,
        SyntaxTree::From { range, .. } => *range,
        SyntaxTree::FromImport { range, .. } => *range,
        SyntaxTree::Path { range, .. } => *range,
        SyntaxTree::Aliased { range, .. } => *range,
        SyntaxTree::Call { range, .. } => *range,
        SyntaxTree::Name { range, .. } => *range,
        SyntaxTree::Int { range, .. } => *range,
        SyntaxTree::Float { range, .. } => *range,
        SyntaxTree::String { range, .. } => *range,
        SyntaxTree::True { range, .. } => *range,
        SyntaxTree::False { range, .. } => *range,
        SyntaxTree::None { range, .. } => *range,
        SyntaxTree::Atom { range, .. } => *range,
        SyntaxTree::Enum { range, .. } => *range,
        SyntaxTree::EnumMember { range, .. } => *range,
        SyntaxTree::Property { range, .. } => *range,
        SyntaxTree::Accessor { range, .. } => *range,
        SyntaxTree::Constructor { range, .. } => *range,
        SyntaxTree::Using { range, .. } => *range,
        SyntaxTree::Namespace { range, .. } => *range,
        SyntaxTree::Variable { range, .. } => *range,
        SyntaxTree::Field { range, .. } => *range,
        SyntaxTree::Event { range, .. } => *range,
        SyntaxTree::Is { range, .. } => *range,
        SyntaxTree::Cast { range, .. } => *range,
        SyntaxTree::Null { range, .. } => *range,
        SyntaxTree::Inline { range, .. } => *range,
        SyntaxTree::Unknown { range, .. } => *range,
        SyntaxTree::Raw { range, .. } => *range,
    }
}

/// Source-location span of this node. Used for XML attribute
/// emission (`line` / `column` / `end_line` / `end_column` / `id`).
/// Every variant carries a `span: Span` field.
pub fn span_of(tree: &SyntaxTree) -> Span {
    match tree {
        SyntaxTree::Module { span, .. } => *span,
        SyntaxTree::Expression { span, .. } => *span,
        SyntaxTree::Slot { span, .. } => *span,
        SyntaxTree::ObjectAccess { span, .. } => *span,
        SyntaxTree::Binary { span, .. } => *span,
        SyntaxTree::Logical { span, .. } => *span,
        SyntaxTree::Operator { span, .. } => *span,
        SyntaxTree::Unary { span, .. } => *span,
        SyntaxTree::Tuple { span, .. } => *span,
        SyntaxTree::List { span, .. } => *span,
        SyntaxTree::Set { span, .. } => *span,
        SyntaxTree::Dictionary { span, .. } => *span,
        SyntaxTree::Pair { span, .. } => *span,
        SyntaxTree::GenericType { span, .. } => *span,
        SyntaxTree::Comparison { span, .. } => *span,
        SyntaxTree::If { span, .. } => *span,
        SyntaxTree::ElseIf { span, .. } => *span,
        SyntaxTree::Else { span, .. } => *span,
        SyntaxTree::For { span, .. } => *span,
        SyntaxTree::While { span, .. } => *span,
        SyntaxTree::Foreach { span, .. } => *span,
        SyntaxTree::CFor { span, .. } => *span,
        SyntaxTree::DoWhile { span, .. } => *span,
        SyntaxTree::Break { span, .. } => *span,
        SyntaxTree::Continue { span, .. } => *span,
        SyntaxTree::FieldWrap { span, .. } => *span,
        SyntaxTree::SimpleStatement { span, .. } => *span,
        SyntaxTree::Try { span, .. } => *span,
        SyntaxTree::Except { span, .. } => *span,
        SyntaxTree::Catch { span, .. } => *span,
        SyntaxTree::TypeAlias { span, .. } => *span,
        SyntaxTree::KeywordArgument { span, .. } => *span,
        SyntaxTree::ListSplat { span, .. } => *span,
        SyntaxTree::DictSplat { span, .. } => *span,
        SyntaxTree::Ternary { span, .. } => *span,
        SyntaxTree::ObjectCreation { span, .. } => *span,
        SyntaxTree::Lambda { span, .. } => *span,
        SyntaxTree::Function { span, .. } => *span,
        SyntaxTree::Method { span, .. } => *span,
        SyntaxTree::Class { span, .. } => *span,
        SyntaxTree::Struct { span, .. } => *span,
        SyntaxTree::Interface { span, .. } => *span,
        SyntaxTree::Record { span, .. } => *span,
        SyntaxTree::Body { span, .. } => *span,
        SyntaxTree::Parameter { span, .. } => *span,
        SyntaxTree::Skip { span, .. } => *span,
        SyntaxTree::PositionalSeparator { span, .. } => *span,
        SyntaxTree::KeywordSeparator { span, .. } => *span,
        SyntaxTree::Decorator { span, .. } => *span,
        SyntaxTree::Returns { span, .. } => *span,
        SyntaxTree::Generic { span, .. } => *span,
        SyntaxTree::TypeParameter { span, .. } => *span,
        SyntaxTree::Return { span, .. } => *span,
        SyntaxTree::Comment { span, .. } => *span,
        SyntaxTree::Assign { span, .. } => *span,
        SyntaxTree::Import { span, .. } => *span,
        SyntaxTree::From { span, .. } => *span,
        SyntaxTree::FromImport { span, .. } => *span,
        SyntaxTree::Path { span, .. } => *span,
        SyntaxTree::Aliased { span, .. } => *span,
        SyntaxTree::Call { span, .. } => *span,
        SyntaxTree::Name { span, .. } => *span,
        SyntaxTree::Int { span, .. } => *span,
        SyntaxTree::Float { span, .. } => *span,
        SyntaxTree::String { span, .. } => *span,
        SyntaxTree::True { span, .. } => *span,
        SyntaxTree::False { span, .. } => *span,
        SyntaxTree::None { span, .. } => *span,
        SyntaxTree::Atom { span, .. } => *span,
        SyntaxTree::Enum { span, .. } => *span,
        SyntaxTree::EnumMember { span, .. } => *span,
        SyntaxTree::Property { span, .. } => *span,
        SyntaxTree::Accessor { span, .. } => *span,
        SyntaxTree::Constructor { span, .. } => *span,
        SyntaxTree::Using { span, .. } => *span,
        SyntaxTree::Namespace { span, .. } => *span,
        SyntaxTree::Variable { span, .. } => *span,
        SyntaxTree::Field { span, .. } => *span,
        SyntaxTree::Event { span, .. } => *span,
        SyntaxTree::Is { span, .. } => *span,
        SyntaxTree::Cast { span, .. } => *span,
        SyntaxTree::Null { span, .. } => *span,
        SyntaxTree::Inline { span, .. } => *span,
        SyntaxTree::Unknown { span, .. } => *span,
        SyntaxTree::Raw { span, .. } => *span,
    }
}

/// Stored text for scalar-leaf variants (`Name`, `Atom`, `Int`,
/// `Float`, `String`, `True`, `False`, `None`, `Null`). Returns
/// `None` for compound variants. The renderer uses this to emit
/// leaf literals without consulting the source string (S13-Z1).
pub fn scalar_text_of(tree: &SyntaxTree) -> Option<&str> {
    match tree {
        SyntaxTree::Operator { text, .. } => Some(text.as_str()),
        SyntaxTree::Name { text, .. } => Some(text.as_str()),
        SyntaxTree::Int { text, .. } => Some(text.as_str()),
        SyntaxTree::Float { text, .. } => Some(text.as_str()),
        SyntaxTree::String { text, .. } => Some(text.as_str()),
        SyntaxTree::True { text, .. } => Some(text.as_str()),
        SyntaxTree::False { text, .. } => Some(text.as_str()),
        SyntaxTree::None { text, .. } => Some(text.as_str()),
        SyntaxTree::Atom { text, .. } => Some(text.as_str()),
        SyntaxTree::Null { text, .. } => Some(text.as_str()),
        _ => None,
    }
}

/// Direct tree children of this node, in source order. Excludes
/// synthetic render-time wrappers, modifier markers, and other shape
/// metadata. Used by every variant-blind walker (`to_xot.rs`,
/// `to_json.rs`, …) as the single source of truth for tree traversal.
pub fn children_of(tree: &SyntaxTree) -> Vec<&SyntaxTree> {
    let mut v: Vec<&SyntaxTree> = Vec::new();
    match tree {
        SyntaxTree::Module { children, .. } => {
            v.extend(children.iter());
        }
        SyntaxTree::Expression { inner, .. } => {
            v.push(inner);
        }
        SyntaxTree::Slot { children, .. } => {
            v.extend(children.iter());
        }
        SyntaxTree::ObjectAccess { receiver, segments, .. } => {
            if let AccessReceiver::Instance(__t) = receiver { v.push(__t); }
            for __s in segments {
                match __s {
                    AccessSegment::Member { .. } => {}
                    AccessSegment::Index { indices, .. } => v.extend(indices.iter()),
                    AccessSegment::Call { arguments, .. } => v.extend(arguments.iter()),
                }
            }
        }
        SyntaxTree::Binary { left, op, right, .. } => {
            v.push(left);
            v.push(op);
            v.push(right);
        }
        SyntaxTree::Logical { left, op, right, .. } => {
            v.push(left);
            v.push(op);
            v.push(right);
        }
        SyntaxTree::Operator { .. } => {}
        SyntaxTree::Unary { operand, .. } => {
            v.push(operand);
        }
        SyntaxTree::Tuple { children, .. } => {
            v.extend(children.iter());
        }
        SyntaxTree::List { children, .. } => {
            v.extend(children.iter());
        }
        SyntaxTree::Set { children, .. } => {
            v.extend(children.iter());
        }
        SyntaxTree::Dictionary { pairs, .. } => {
            v.extend(pairs.iter());
        }
        SyntaxTree::Pair { key, value, .. } => {
            v.push(key);
            v.push(value);
        }
        SyntaxTree::GenericType { name, params, .. } => {
            v.push(name);
            v.extend(params.iter());
        }
        SyntaxTree::Comparison { left, right, .. } => {
            v.push(left);
            v.push(right);
        }
        SyntaxTree::If { condition, body, else_branch, .. } => {
            v.push(condition);
            v.push(body);
            if let Some(__t) = else_branch { v.push(__t); }
        }
        SyntaxTree::ElseIf { condition, body, else_branch, .. } => {
            v.push(condition);
            v.push(body);
            if let Some(__t) = else_branch { v.push(__t); }
        }
        SyntaxTree::Else { body, .. } => {
            v.push(body);
        }
        SyntaxTree::For { targets, iterables, body, else_body, .. } => {
            v.extend(targets.iter());
            v.extend(iterables.iter());
            v.push(body);
            if let Some(__t) = else_body { v.push(__t); }
        }
        SyntaxTree::While { condition, body, else_body, .. } => {
            v.push(condition);
            v.push(body);
            if let Some(__t) = else_body { v.push(__t); }
        }
        SyntaxTree::Foreach { type_ann, target, iterable, body, .. } => {
            if let Some(__t) = type_ann { v.push(__t); }
            v.push(target);
            v.push(iterable);
            v.push(body);
        }
        SyntaxTree::CFor { initializer, condition, updates, body, .. } => {
            if let Some(__t) = initializer { v.push(__t); }
            if let Some(__t) = condition { v.push(__t); }
            v.extend(updates.iter());
            v.push(body);
        }
        SyntaxTree::DoWhile { body, condition, .. } => {
            v.push(body);
            v.push(condition);
        }
        SyntaxTree::Break { .. } => {}
        SyntaxTree::Continue { .. } => {}
        SyntaxTree::FieldWrap { inner, .. } => {
            v.push(inner);
        }
        SyntaxTree::SimpleStatement { children, .. } => {
            v.extend(children.iter());
        }
        SyntaxTree::Try { try_body, handlers, else_body, finally_body, .. } => {
            v.push(try_body);
            v.extend(handlers.iter());
            if let Some(__t) = else_body { v.push(__t); }
            if let Some(__t) = finally_body { v.push(__t); }
        }
        SyntaxTree::Except { type_target, binding, filter, body, .. } => {
            if let Some(__t) = type_target { v.push(__t); }
            if let Some(__t) = binding { v.push(__t); }
            if let Some(__t) = filter { v.push(__t); }
            v.push(body);
        }
        SyntaxTree::Catch { type_target, binding, filter, body, .. } => {
            if let Some(__t) = type_target { v.push(__t); }
            if let Some(__t) = binding { v.push(__t); }
            if let Some(__t) = filter { v.push(__t); }
            v.push(body);
        }
        SyntaxTree::TypeAlias { name, type_params, value, .. } => {
            v.push(name);
            if let Some(__t) = type_params { v.push(__t); }
            v.push(value);
        }
        SyntaxTree::KeywordArgument { name, value, .. } => {
            v.push(name);
            v.push(value);
        }
        SyntaxTree::ListSplat { inner, .. } => {
            v.push(inner);
        }
        SyntaxTree::DictSplat { inner, .. } => {
            v.push(inner);
        }
        SyntaxTree::Ternary { condition, if_true, if_false, .. } => {
            v.push(condition);
            v.push(if_true);
            v.push(if_false);
        }
        SyntaxTree::ObjectCreation { type_target, arguments, initializer, .. } => {
            if let Some(__t) = type_target { v.push(__t); }
            v.extend(arguments.iter());
            if let Some(__t) = initializer { v.push(__t); }
        }
        SyntaxTree::Lambda { parameters, body, .. } => {
            v.extend(parameters.iter());
            v.push(body.inner());
        }
        SyntaxTree::Function { decorators, name, generics, parameters, returns, throws, body, .. } => {
            v.extend(decorators.iter());
            v.push(name);
            v.extend(generics.iter());
            v.extend(parameters.iter());
            if let Some(__t) = returns { v.push(__t); }
            v.extend(throws.iter());
            if let Some(__t) = body { v.push(__t); }
        }
        SyntaxTree::Method { decorators, name, generics, parameters, returns, throws, body, .. } => {
            v.extend(decorators.iter());
            v.push(name);
            v.extend(generics.iter());
            v.extend(parameters.iter());
            if let Some(__t) = returns { v.push(__t); }
            v.extend(throws.iter());
            if let Some(__t) = body { v.push(__t); }
        }
        SyntaxTree::Class { decorators, name, generics, bases, where_clauses, body, .. } => {
            v.extend(decorators.iter());
            v.push(name);
            v.extend(generics.iter());
            v.extend(bases.iter());
            v.extend(where_clauses.iter());
            v.push(body);
        }
        SyntaxTree::Struct { decorators, name, generics, bases, where_clauses, body, .. } => {
            v.extend(decorators.iter());
            v.push(name);
            v.extend(generics.iter());
            v.extend(bases.iter());
            v.extend(where_clauses.iter());
            v.push(body);
        }
        SyntaxTree::Interface { decorators, name, generics, bases, where_clauses, body, .. } => {
            v.extend(decorators.iter());
            v.push(name);
            v.extend(generics.iter());
            v.extend(bases.iter());
            v.extend(where_clauses.iter());
            v.push(body);
        }
        SyntaxTree::Record { decorators, name, generics, bases, where_clauses, body, .. } => {
            v.extend(decorators.iter());
            v.push(name);
            v.extend(generics.iter());
            v.extend(bases.iter());
            v.extend(where_clauses.iter());
            v.push(body);
        }
        SyntaxTree::Body { children, .. } => {
            v.extend(children.iter());
        }
        SyntaxTree::Parameter { name, type_ann, default, .. } => {
            v.push(name);
            if let Some(__t) = type_ann { v.push(__t); }
            if let Some(__t) = default { v.push(__t); }
        }
        SyntaxTree::Skip { .. } => {}
        SyntaxTree::PositionalSeparator { .. } => {}
        SyntaxTree::KeywordSeparator { .. } => {}
        SyntaxTree::Decorator { inner, .. } => {
            v.push(inner);
        }
        SyntaxTree::Returns { type_ann, .. } => {
            v.push(type_ann);
        }
        SyntaxTree::Generic { items, .. } => {
            v.extend(items.iter());
        }
        SyntaxTree::TypeParameter { name, constraint, .. } => {
            v.push(name);
            if let Some(__t) = constraint { v.push(__t); }
        }
        SyntaxTree::Return { value, .. } => {
            if let Some(__t) = value { v.push(__t); }
        }
        SyntaxTree::Comment { .. } => {}
        SyntaxTree::Assign { targets, type_annotation, values, .. } => {
            v.extend(targets.iter());
            if let Some(__t) = type_annotation { v.push(__t); }
            v.extend(values.iter());
        }
        SyntaxTree::Import { children, .. } => {
            v.extend(children.iter());
        }
        SyntaxTree::From { path, imports, .. } => {
            if let Some(__t) = path { v.push(__t); }
            v.extend(imports.iter());
        }
        SyntaxTree::FromImport { name, alias, .. } => {
            v.push(name);
            if let Some(__t) = alias { v.push(__t); }
        }
        SyntaxTree::Path { segments, .. } => {
            v.extend(segments.iter());
        }
        SyntaxTree::Aliased { inner, .. } => {
            v.push(inner);
        }
        SyntaxTree::Call { callee, arguments, .. } => {
            v.push(callee);
            v.extend(arguments.iter());
        }
        SyntaxTree::Name { .. } => {}
        SyntaxTree::Int { .. } => {}
        SyntaxTree::Float { .. } => {}
        SyntaxTree::String { .. } => {}
        SyntaxTree::True { .. } => {}
        SyntaxTree::False { .. } => {}
        SyntaxTree::None { .. } => {}
        SyntaxTree::Atom { .. } => {}
        SyntaxTree::Enum { decorators, name, underlying_type, members, .. } => {
            v.extend(decorators.iter());
            v.push(name);
            if let Some(__t) = underlying_type { v.push(__t); }
            v.extend(members.iter());
        }
        SyntaxTree::EnumMember { decorators, name, value, .. } => {
            v.extend(decorators.iter());
            v.push(name);
            if let Some(__t) = value { v.push(__t); }
        }
        SyntaxTree::Property { decorators, type_ann, name, accessors, value, .. } => {
            v.extend(decorators.iter());
            if let Some(__t) = type_ann { v.push(__t); }
            v.push(name);
            v.extend(accessors.iter());
            if let Some(__t) = value { v.push(__t); }
        }
        SyntaxTree::Accessor { body, .. } => {
            if let Some(__t) = body { v.push(__t); }
        }
        SyntaxTree::Constructor { decorators, name, parameters, body, .. } => {
            v.extend(decorators.iter());
            v.push(name);
            v.extend(parameters.iter());
            v.push(body);
        }
        SyntaxTree::Using { alias, path, .. } => {
            if let Some(__t) = alias { v.push(__t); }
            v.push(path);
        }
        SyntaxTree::Namespace { name, children, .. } => {
            v.push(name);
            v.extend(children.iter());
        }
        SyntaxTree::Variable { decorators, type_ann, name, value, .. } => {
            v.extend(decorators.iter());
            if let Some(__t) = type_ann { v.push(__t); }
            v.push(name);
            if let Some(__e) = value { v.push(&__e.inner); }
        }
        SyntaxTree::Field { decorators, type_ann, name, value, .. } => {
            v.extend(decorators.iter());
            if let Some(__t) = type_ann { v.push(__t); }
            v.push(name);
            if let Some(__e) = value { v.push(&__e.inner); }
        }
        SyntaxTree::Event { decorators, type_ann, name, value, .. } => {
            v.extend(decorators.iter());
            if let Some(__t) = type_ann { v.push(__t); }
            v.push(name);
            if let Some(__e) = value { v.push(&__e.inner); }
        }
        SyntaxTree::Is { value, type_target, .. } => {
            v.push(value);
            v.push(type_target);
        }
        SyntaxTree::Cast { type_ann, value, .. } => {
            v.push(type_ann);
            v.push(value);
        }
        SyntaxTree::Null { .. } => {}
        SyntaxTree::Inline { children, .. } => {
            v.extend(children.iter());
        }
        SyntaxTree::Unknown { .. } => {}
        SyntaxTree::Raw { children, .. } => {
            v.extend(children.iter());
        }
    }
    v.sort_by_key(|c| range_of(c).start);
    v
}

/// Field-projection view of this node — one [`TreeField`] per
/// projected struct field. Only consulted when [`use_field_projection`]
/// returns true. See the walker for projection semantics.
pub fn fields_of(tree: &SyntaxTree) -> Vec<TreeField<'_, SyntaxTree>> {
    let mut out: Vec<TreeField<'_, SyntaxTree>> = Vec::new();
    match tree {
        SyntaxTree::Module { children, .. } => {
            out.push(TreeField::Many { name: "children", items: children.iter().collect() });
        }
        SyntaxTree::Expression { inner, marker, span, .. } => {
            out.push(TreeField::Single { name: "inner", value: inner });
            if let Some(name) = marker {
                out.push(TreeField::Flag {
                    name,
                    marker: Marker { name, range: ByteRange::synthetic_empty(), span: *span },
                });
            }
        }
        SyntaxTree::Slot { children, .. } => {
            out.push(TreeField::Many { name: "children", items: children.iter().collect() });
        }
        SyntaxTree::ObjectAccess { receiver, .. } => {
            if let AccessReceiver::Instance(__t) = receiver { out.push(TreeField::Single { name: "receiver", value: __t }); }
        }
        SyntaxTree::Binary { left, op, right, .. } => {
            out.push(TreeField::Single { name: "left", value: left });
            out.push(TreeField::Single { name: "op", value: op });
            out.push(TreeField::Single { name: "right", value: right });
        }
        SyntaxTree::Logical { left, op, right, .. } => {
            out.push(TreeField::Single { name: "left", value: left });
            out.push(TreeField::Single { name: "op", value: op });
            out.push(TreeField::Single { name: "right", value: right });
        }
        SyntaxTree::Operator { text, kind, span, .. } => {
            out.push(TreeField::Scalar { name: "text", value: text.as_str() });
            out.push(TreeField::Flag {
                name: kind.marker_name(),
                marker: Marker { name: kind.marker_name(), range: ByteRange::synthetic_empty(), span: *span },
            });
        }
        SyntaxTree::Unary { op_text, operand, extra_markers, .. } => {
            out.push(TreeField::Scalar { name: "op_text", value: op_text.as_str() });
            out.push(TreeField::Single { name: "operand", value: operand });
            for m in extra_markers { out.push(TreeField::Flag { name: m.name, marker: *m }); }
        }
        SyntaxTree::Tuple { children, .. } => {
            out.push(TreeField::Many { name: "children", items: children.iter().collect() });
        }
        SyntaxTree::List { children, .. } => {
            out.push(TreeField::Many { name: "children", items: children.iter().collect() });
        }
        SyntaxTree::Set { children, .. } => {
            out.push(TreeField::Many { name: "children", items: children.iter().collect() });
        }
        SyntaxTree::Dictionary { pairs, .. } => {
            out.push(TreeField::Many { name: "pairs", items: pairs.iter().collect() });
        }
        SyntaxTree::Pair { key, value, .. } => {
            out.push(TreeField::Single { name: "key", value: key });
            out.push(TreeField::Single { name: "value", value: value });
        }
        SyntaxTree::GenericType { name, params, .. } => {
            out.push(TreeField::Single { name: "name", value: name });
            out.push(TreeField::Many { name: "params", items: params.iter().collect() });
        }
        SyntaxTree::Comparison { left, op_text, right, .. } => {
            out.push(TreeField::Single { name: "left", value: left });
            out.push(TreeField::Scalar { name: "op_text", value: op_text.as_str() });
            out.push(TreeField::Single { name: "right", value: right });
        }
        SyntaxTree::If { condition, body, else_branch, .. } => {
            out.push(TreeField::Single { name: "condition", value: condition });
            out.push(TreeField::Single { name: "body", value: body });
            if let Some(__t) = else_branch { out.push(TreeField::Single { name: "else_branch", value: __t }); }
        }
        SyntaxTree::ElseIf { condition, body, else_branch, .. } => {
            out.push(TreeField::Single { name: "condition", value: condition });
            out.push(TreeField::Single { name: "body", value: body });
            if let Some(__t) = else_branch { out.push(TreeField::Single { name: "else_branch", value: __t }); }
        }
        SyntaxTree::Else { body, .. } => {
            out.push(TreeField::Single { name: "body", value: body });
        }
        SyntaxTree::For { targets, iterables, body, else_body, .. } => {
            out.push(TreeField::Many { name: "targets", items: targets.iter().collect() });
            out.push(TreeField::Many { name: "iterables", items: iterables.iter().collect() });
            out.push(TreeField::Single { name: "body", value: body });
            if let Some(__t) = else_body { out.push(TreeField::Single { name: "else_body", value: __t }); }
        }
        SyntaxTree::While { condition, body, else_body, .. } => {
            out.push(TreeField::Single { name: "condition", value: condition });
            out.push(TreeField::Single { name: "body", value: body });
            if let Some(__t) = else_body { out.push(TreeField::Single { name: "else_body", value: __t }); }
        }
        SyntaxTree::Foreach { type_ann, target, iterable, body, .. } => {
            if let Some(__t) = type_ann { out.push(TreeField::Single { name: "type_ann", value: __t }); }
            out.push(TreeField::Single { name: "target", value: target });
            out.push(TreeField::Single { name: "iterable", value: iterable });
            out.push(TreeField::Single { name: "body", value: body });
        }
        SyntaxTree::CFor { initializer, condition, updates, body, .. } => {
            if let Some(__t) = initializer { out.push(TreeField::Single { name: "initializer", value: __t }); }
            if let Some(__t) = condition { out.push(TreeField::Single { name: "condition", value: __t }); }
            out.push(TreeField::Many { name: "updates", items: updates.iter().collect() });
            out.push(TreeField::Single { name: "body", value: body });
        }
        SyntaxTree::DoWhile { body, condition, .. } => {
            out.push(TreeField::Single { name: "body", value: body });
            out.push(TreeField::Single { name: "condition", value: condition });
        }
        SyntaxTree::Break { .. } => {}
        SyntaxTree::Continue { .. } => {}
        SyntaxTree::FieldWrap { inner, .. } => {
            out.push(TreeField::Single { name: "inner", value: inner });
        }
        SyntaxTree::SimpleStatement { modifiers, extra_markers, children, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(TreeField::Flag {
                    name: mname,
                    marker: Marker {
                        name: mname,
                        range: ByteRange::synthetic_empty(),
                        span: mspan.unwrap_or(*span),
                    },
                });
            }
            for m in extra_markers { out.push(TreeField::Flag { name: m.name, marker: *m }); }
            out.push(TreeField::Many { name: "children", items: children.iter().collect() });
        }
        SyntaxTree::Try { try_body, handlers, else_body, finally_body, .. } => {
            out.push(TreeField::Single { name: "try_body", value: try_body });
            out.push(TreeField::Many { name: "handlers", items: handlers.iter().collect() });
            if let Some(__t) = else_body { out.push(TreeField::Single { name: "else_body", value: __t }); }
            if let Some(__t) = finally_body { out.push(TreeField::Single { name: "finally_body", value: __t }); }
        }
        SyntaxTree::Except { type_target, binding, filter, body, .. } => {
            if let Some(__t) = type_target { out.push(TreeField::Single { name: "type_target", value: __t }); }
            if let Some(__t) = binding { out.push(TreeField::Single { name: "binding", value: __t }); }
            if let Some(__t) = filter { out.push(TreeField::Single { name: "filter", value: __t }); }
            out.push(TreeField::Single { name: "body", value: body });
        }
        SyntaxTree::Catch { type_target, binding, filter, body, .. } => {
            if let Some(__t) = type_target { out.push(TreeField::Single { name: "type_target", value: __t }); }
            if let Some(__t) = binding { out.push(TreeField::Single { name: "binding", value: __t }); }
            if let Some(__t) = filter { out.push(TreeField::Single { name: "filter", value: __t }); }
            out.push(TreeField::Single { name: "body", value: body });
        }
        SyntaxTree::TypeAlias { name, type_params, value, .. } => {
            out.push(TreeField::Single { name: "name", value: name });
            if let Some(__t) = type_params { out.push(TreeField::Single { name: "type_params", value: __t }); }
            out.push(TreeField::Single { name: "value", value: value });
        }
        SyntaxTree::KeywordArgument { name, value, .. } => {
            out.push(TreeField::Single { name: "name", value: name });
            out.push(TreeField::Single { name: "value", value: value });
        }
        SyntaxTree::ListSplat { inner, .. } => {
            out.push(TreeField::Single { name: "inner", value: inner });
        }
        SyntaxTree::DictSplat { inner, .. } => {
            out.push(TreeField::Single { name: "inner", value: inner });
        }
        SyntaxTree::Ternary { condition, if_true, if_false, .. } => {
            out.push(TreeField::Single { name: "condition", value: condition });
            out.push(TreeField::Single { name: "if_true", value: if_true });
            out.push(TreeField::Single { name: "if_false", value: if_false });
        }
        SyntaxTree::ObjectCreation { type_target, arguments, initializer, .. } => {
            if let Some(__t) = type_target { out.push(TreeField::Single { name: "type_target", value: __t }); }
            out.push(TreeField::Many { name: "arguments", items: arguments.iter().collect() });
            if let Some(__t) = initializer { out.push(TreeField::Single { name: "initializer", value: __t }); }
        }
        SyntaxTree::Lambda { modifiers, parameters, body, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(TreeField::Flag {
                    name: mname,
                    marker: Marker {
                        name: mname,
                        range: ByteRange::synthetic_empty(),
                        span: mspan.unwrap_or(*span),
                    },
                });
            }
            out.push(TreeField::Many { name: "parameters", items: parameters.iter().collect() });
            out.push(TreeField::Single { name: "body", value: body.inner() });
        }
        SyntaxTree::Function { modifiers, decorators, name, generics, parameters, returns, throws, body, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(TreeField::Flag {
                    name: mname,
                    marker: Marker {
                        name: mname,
                        range: ByteRange::synthetic_empty(),
                        span: mspan.unwrap_or(*span),
                    },
                });
            }
            out.push(TreeField::Many { name: "decorators", items: decorators.iter().collect() });
            out.push(TreeField::Single { name: "name", value: name });
            out.push(TreeField::Many { name: "generics", items: generics.iter().collect() });
            out.push(TreeField::Many { name: "parameters", items: parameters.iter().collect() });
            if let Some(__t) = returns { out.push(TreeField::Single { name: "returns", value: __t }); }
            out.push(TreeField::Many { name: "throws", items: throws.iter().collect() });
            if let Some(__t) = body { out.push(TreeField::Single { name: "body", value: __t }); }
        }
        SyntaxTree::Method { modifiers, decorators, name, generics, parameters, returns, throws, body, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(TreeField::Flag {
                    name: mname,
                    marker: Marker {
                        name: mname,
                        range: ByteRange::synthetic_empty(),
                        span: mspan.unwrap_or(*span),
                    },
                });
            }
            out.push(TreeField::Many { name: "decorators", items: decorators.iter().collect() });
            out.push(TreeField::Single { name: "name", value: name });
            out.push(TreeField::Many { name: "generics", items: generics.iter().collect() });
            out.push(TreeField::Many { name: "parameters", items: parameters.iter().collect() });
            if let Some(__t) = returns { out.push(TreeField::Single { name: "returns", value: __t }); }
            out.push(TreeField::Many { name: "throws", items: throws.iter().collect() });
            if let Some(__t) = body { out.push(TreeField::Single { name: "body", value: __t }); }
        }
        SyntaxTree::Class { modifiers, decorators, name, generics, bases, where_clauses, body, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(TreeField::Flag {
                    name: mname,
                    marker: Marker {
                        name: mname,
                        range: ByteRange::synthetic_empty(),
                        span: mspan.unwrap_or(*span),
                    },
                });
            }
            out.push(TreeField::Many { name: "decorators", items: decorators.iter().collect() });
            out.push(TreeField::Single { name: "name", value: name });
            out.push(TreeField::Many { name: "generics", items: generics.iter().collect() });
            out.push(TreeField::Many { name: "bases", items: bases.iter().collect() });
            out.push(TreeField::Many { name: "where_clauses", items: where_clauses.iter().collect() });
            out.push(TreeField::Single { name: "body", value: body });
        }
        SyntaxTree::Struct { modifiers, decorators, name, generics, bases, where_clauses, body, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(TreeField::Flag {
                    name: mname,
                    marker: Marker {
                        name: mname,
                        range: ByteRange::synthetic_empty(),
                        span: mspan.unwrap_or(*span),
                    },
                });
            }
            out.push(TreeField::Many { name: "decorators", items: decorators.iter().collect() });
            out.push(TreeField::Single { name: "name", value: name });
            out.push(TreeField::Many { name: "generics", items: generics.iter().collect() });
            out.push(TreeField::Many { name: "bases", items: bases.iter().collect() });
            out.push(TreeField::Many { name: "where_clauses", items: where_clauses.iter().collect() });
            out.push(TreeField::Single { name: "body", value: body });
        }
        SyntaxTree::Interface { modifiers, decorators, name, generics, bases, where_clauses, body, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(TreeField::Flag {
                    name: mname,
                    marker: Marker {
                        name: mname,
                        range: ByteRange::synthetic_empty(),
                        span: mspan.unwrap_or(*span),
                    },
                });
            }
            out.push(TreeField::Many { name: "decorators", items: decorators.iter().collect() });
            out.push(TreeField::Single { name: "name", value: name });
            out.push(TreeField::Many { name: "generics", items: generics.iter().collect() });
            out.push(TreeField::Many { name: "bases", items: bases.iter().collect() });
            out.push(TreeField::Many { name: "where_clauses", items: where_clauses.iter().collect() });
            out.push(TreeField::Single { name: "body", value: body });
        }
        SyntaxTree::Record { modifiers, decorators, name, generics, bases, where_clauses, body, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(TreeField::Flag {
                    name: mname,
                    marker: Marker {
                        name: mname,
                        range: ByteRange::synthetic_empty(),
                        span: mspan.unwrap_or(*span),
                    },
                });
            }
            out.push(TreeField::Many { name: "decorators", items: decorators.iter().collect() });
            out.push(TreeField::Single { name: "name", value: name });
            out.push(TreeField::Many { name: "generics", items: generics.iter().collect() });
            out.push(TreeField::Many { name: "bases", items: bases.iter().collect() });
            out.push(TreeField::Many { name: "where_clauses", items: where_clauses.iter().collect() });
            out.push(TreeField::Single { name: "body", value: body });
        }
        SyntaxTree::Body { children, .. } => {
            out.push(TreeField::Many { name: "children", items: children.iter().collect() });
        }
        SyntaxTree::Parameter { extra_markers, modifiers, name, type_ann, default, span, .. } => {
            for m in extra_markers { out.push(TreeField::Flag { name: m.name, marker: *m }); }
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(TreeField::Flag {
                    name: mname,
                    marker: Marker {
                        name: mname,
                        range: ByteRange::synthetic_empty(),
                        span: mspan.unwrap_or(*span),
                    },
                });
            }
            out.push(TreeField::Single { name: "name", value: name });
            if let Some(__t) = type_ann { out.push(TreeField::Single { name: "type_ann", value: __t }); }
            if let Some(__t) = default { out.push(TreeField::Single { name: "default", value: __t }); }
        }
        SyntaxTree::Skip { .. } => {}
        SyntaxTree::PositionalSeparator { .. } => {}
        SyntaxTree::KeywordSeparator { .. } => {}
        SyntaxTree::Decorator { inner, .. } => {
            out.push(TreeField::Single { name: "inner", value: inner });
        }
        SyntaxTree::Returns { type_ann, .. } => {
            out.push(TreeField::Single { name: "type_ann", value: type_ann });
        }
        SyntaxTree::Generic { items, .. } => {
            out.push(TreeField::Many { name: "items", items: items.iter().collect() });
        }
        SyntaxTree::TypeParameter { name, constraint, .. } => {
            out.push(TreeField::Single { name: "name", value: name });
            if let Some(__t) = constraint { out.push(TreeField::Single { name: "constraint", value: __t }); }
        }
        SyntaxTree::Return { value, .. } => {
            if let Some(__t) = value { out.push(TreeField::Single { name: "value", value: __t }); }
        }
        SyntaxTree::Comment { leading, trailing, .. } => {
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
        SyntaxTree::Assign { targets, type_annotation, op_text, values, .. } => {
            out.push(TreeField::Many { name: "targets", items: targets.iter().collect() });
            if let Some(__t) = type_annotation { out.push(TreeField::Single { name: "type_annotation", value: __t }); }
            out.push(TreeField::Scalar { name: "op_text", value: op_text.as_str() });
            out.push(TreeField::Many { name: "values", items: values.iter().collect() });
        }
        SyntaxTree::Import { children, .. } => {
            out.push(TreeField::Many { name: "children", items: children.iter().collect() });
        }
        SyntaxTree::From { path, imports, .. } => {
            if let Some(__t) = path { out.push(TreeField::Single { name: "path", value: __t }); }
            out.push(TreeField::Many { name: "imports", items: imports.iter().collect() });
        }
        SyntaxTree::FromImport { name, alias, .. } => {
            out.push(TreeField::Single { name: "name", value: name });
            if let Some(__t) = alias { out.push(TreeField::Single { name: "alias", value: __t }); }
        }
        SyntaxTree::Path { segments, .. } => {
            out.push(TreeField::Many { name: "segments", items: segments.iter().collect() });
        }
        SyntaxTree::Aliased { inner, .. } => {
            out.push(TreeField::Single { name: "inner", value: inner });
        }
        SyntaxTree::Call { callee, arguments, .. } => {
            out.push(TreeField::Single { name: "callee", value: callee });
            out.push(TreeField::Many { name: "arguments", items: arguments.iter().collect() });
        }
        SyntaxTree::Name { text, .. } => {
            out.push(TreeField::Scalar { name: "text", value: text.as_str() });
        }
        SyntaxTree::Int { text, .. } => {
            out.push(TreeField::Scalar { name: "text", value: text.as_str() });
        }
        SyntaxTree::Float { text, .. } => {
            out.push(TreeField::Scalar { name: "text", value: text.as_str() });
        }
        SyntaxTree::String { text, .. } => {
            out.push(TreeField::Scalar { name: "text", value: text.as_str() });
        }
        SyntaxTree::True { text, .. } => {
            out.push(TreeField::Scalar { name: "text", value: text.as_str() });
        }
        SyntaxTree::False { text, .. } => {
            out.push(TreeField::Scalar { name: "text", value: text.as_str() });
        }
        SyntaxTree::None { text, .. } => {
            out.push(TreeField::Scalar { name: "text", value: text.as_str() });
        }
        SyntaxTree::Atom { text, .. } => {
            out.push(TreeField::Scalar { name: "text", value: text.as_str() });
        }
        SyntaxTree::Enum { modifiers, decorators, name, underlying_type, members, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(TreeField::Flag {
                    name: mname,
                    marker: Marker {
                        name: mname,
                        range: ByteRange::synthetic_empty(),
                        span: mspan.unwrap_or(*span),
                    },
                });
            }
            out.push(TreeField::Many { name: "decorators", items: decorators.iter().collect() });
            out.push(TreeField::Single { name: "name", value: name });
            if let Some(__t) = underlying_type { out.push(TreeField::Single { name: "underlying_type", value: __t }); }
            out.push(TreeField::Many { name: "members", items: members.iter().collect() });
        }
        SyntaxTree::EnumMember { decorators, name, value, .. } => {
            out.push(TreeField::Many { name: "decorators", items: decorators.iter().collect() });
            out.push(TreeField::Single { name: "name", value: name });
            if let Some(__t) = value { out.push(TreeField::Single { name: "value", value: __t }); }
        }
        SyntaxTree::Property { modifiers, decorators, type_ann, name, accessors, value, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(TreeField::Flag {
                    name: mname,
                    marker: Marker {
                        name: mname,
                        range: ByteRange::synthetic_empty(),
                        span: mspan.unwrap_or(*span),
                    },
                });
            }
            out.push(TreeField::Many { name: "decorators", items: decorators.iter().collect() });
            if let Some(__t) = type_ann { out.push(TreeField::Single { name: "type_ann", value: __t }); }
            out.push(TreeField::Single { name: "name", value: name });
            out.push(TreeField::Many { name: "accessors", items: accessors.iter().collect() });
            if let Some(__t) = value { out.push(TreeField::Single { name: "value", value: __t }); }
        }
        SyntaxTree::Accessor { modifiers, body, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(TreeField::Flag {
                    name: mname,
                    marker: Marker {
                        name: mname,
                        range: ByteRange::synthetic_empty(),
                        span: mspan.unwrap_or(*span),
                    },
                });
            }
            if let Some(__t) = body { out.push(TreeField::Single { name: "body", value: __t }); }
        }
        SyntaxTree::Constructor { modifiers, decorators, name, parameters, body, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(TreeField::Flag {
                    name: mname,
                    marker: Marker {
                        name: mname,
                        range: ByteRange::synthetic_empty(),
                        span: mspan.unwrap_or(*span),
                    },
                });
            }
            out.push(TreeField::Many { name: "decorators", items: decorators.iter().collect() });
            out.push(TreeField::Single { name: "name", value: name });
            out.push(TreeField::Many { name: "parameters", items: parameters.iter().collect() });
            out.push(TreeField::Single { name: "body", value: body });
        }
        SyntaxTree::Using { alias, path, .. } => {
            if let Some(__t) = alias { out.push(TreeField::Single { name: "alias", value: __t }); }
            out.push(TreeField::Single { name: "path", value: path });
        }
        SyntaxTree::Namespace { name, children, .. } => {
            out.push(TreeField::Single { name: "name", value: name });
            out.push(TreeField::Many { name: "children", items: children.iter().collect() });
        }
        SyntaxTree::Variable { modifiers, decorators, extra_markers, type_ann, name, value, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(TreeField::Flag {
                    name: mname,
                    marker: Marker {
                        name: mname,
                        range: ByteRange::synthetic_empty(),
                        span: mspan.unwrap_or(*span),
                    },
                });
            }
            out.push(TreeField::Many { name: "decorators", items: decorators.iter().collect() });
            for m in extra_markers { out.push(TreeField::Flag { name: m.name, marker: *m }); }
            if let Some(__t) = type_ann { out.push(TreeField::Single { name: "type_ann", value: __t }); }
            out.push(TreeField::Single { name: "name", value: name });
            if let Some(__e) = value { out.push(TreeField::Single { name: "value", value: &__e.inner }); }
        }
        SyntaxTree::Field { modifiers, decorators, type_ann, name, value, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(TreeField::Flag {
                    name: mname,
                    marker: Marker {
                        name: mname,
                        range: ByteRange::synthetic_empty(),
                        span: mspan.unwrap_or(*span),
                    },
                });
            }
            out.push(TreeField::Many { name: "decorators", items: decorators.iter().collect() });
            if let Some(__t) = type_ann { out.push(TreeField::Single { name: "type_ann", value: __t }); }
            out.push(TreeField::Single { name: "name", value: name });
            if let Some(__e) = value { out.push(TreeField::Single { name: "value", value: &__e.inner }); }
        }
        SyntaxTree::Event { modifiers, decorators, type_ann, name, value, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(TreeField::Flag {
                    name: mname,
                    marker: Marker {
                        name: mname,
                        range: ByteRange::synthetic_empty(),
                        span: mspan.unwrap_or(*span),
                    },
                });
            }
            out.push(TreeField::Many { name: "decorators", items: decorators.iter().collect() });
            if let Some(__t) = type_ann { out.push(TreeField::Single { name: "type_ann", value: __t }); }
            out.push(TreeField::Single { name: "name", value: name });
            if let Some(__e) = value { out.push(TreeField::Single { name: "value", value: &__e.inner }); }
        }
        SyntaxTree::Is { value, type_target, .. } => {
            out.push(TreeField::Single { name: "value", value: value });
            out.push(TreeField::Single { name: "type_target", value: type_target });
        }
        SyntaxTree::Cast { type_ann, value, .. } => {
            out.push(TreeField::Single { name: "type_ann", value: type_ann });
            out.push(TreeField::Single { name: "value", value: value });
        }
        SyntaxTree::Null { text, .. } => {
            out.push(TreeField::Scalar { name: "text", value: text.as_str() });
        }
        SyntaxTree::Inline { children, .. } => {
            out.push(TreeField::Many { name: "children", items: children.iter().collect() });
        }
        SyntaxTree::Unknown { kind, .. } => {
            out.push(TreeField::Scalar { name: "kind", value: kind.as_str() });
        }
        SyntaxTree::Raw { kind, children, .. } => {
            out.push(TreeField::Scalar { name: "kind", value: kind.as_str() });
            out.push(TreeField::Many { name: "children", items: children.iter().collect() });
        }
    }
    out
}

/// Per-variant opt-in flag for the field-projection walker
/// path. Returns true for variants whose doc comments carry
/// `@field_projection`; false otherwise.
pub fn use_field_projection(tree: &SyntaxTree) -> bool {
    match tree {
        SyntaxTree::Binary { .. } => true,
        SyntaxTree::Logical { .. } => true,
        SyntaxTree::Operator { .. } => true,
        SyntaxTree::Variable { .. } => true,
        _ => false,
    }
}

/// Mutable access to the source-location span. Used by
/// `assign_ids` to stamp `NodeId`s into existing spans without
/// rebuilding nodes. Delegates from `TreeNode::span_mut`.
pub fn span_mut_of(tree: &mut SyntaxTree) -> &mut Span {
    match tree {
        SyntaxTree::Module { span, .. } => span,
        SyntaxTree::Expression { span, .. } => span,
        SyntaxTree::Slot { span, .. } => span,
        SyntaxTree::ObjectAccess { span, .. } => span,
        SyntaxTree::Binary { span, .. } => span,
        SyntaxTree::Logical { span, .. } => span,
        SyntaxTree::Operator { span, .. } => span,
        SyntaxTree::Unary { span, .. } => span,
        SyntaxTree::Tuple { span, .. } => span,
        SyntaxTree::List { span, .. } => span,
        SyntaxTree::Set { span, .. } => span,
        SyntaxTree::Dictionary { span, .. } => span,
        SyntaxTree::Pair { span, .. } => span,
        SyntaxTree::GenericType { span, .. } => span,
        SyntaxTree::Comparison { span, .. } => span,
        SyntaxTree::If { span, .. } => span,
        SyntaxTree::ElseIf { span, .. } => span,
        SyntaxTree::Else { span, .. } => span,
        SyntaxTree::For { span, .. } => span,
        SyntaxTree::While { span, .. } => span,
        SyntaxTree::Foreach { span, .. } => span,
        SyntaxTree::CFor { span, .. } => span,
        SyntaxTree::DoWhile { span, .. } => span,
        SyntaxTree::Break { span, .. } => span,
        SyntaxTree::Continue { span, .. } => span,
        SyntaxTree::FieldWrap { span, .. } => span,
        SyntaxTree::SimpleStatement { span, .. } => span,
        SyntaxTree::Try { span, .. } => span,
        SyntaxTree::Except { span, .. } => span,
        SyntaxTree::Catch { span, .. } => span,
        SyntaxTree::TypeAlias { span, .. } => span,
        SyntaxTree::KeywordArgument { span, .. } => span,
        SyntaxTree::ListSplat { span, .. } => span,
        SyntaxTree::DictSplat { span, .. } => span,
        SyntaxTree::Ternary { span, .. } => span,
        SyntaxTree::ObjectCreation { span, .. } => span,
        SyntaxTree::Lambda { span, .. } => span,
        SyntaxTree::Function { span, .. } => span,
        SyntaxTree::Method { span, .. } => span,
        SyntaxTree::Class { span, .. } => span,
        SyntaxTree::Struct { span, .. } => span,
        SyntaxTree::Interface { span, .. } => span,
        SyntaxTree::Record { span, .. } => span,
        SyntaxTree::Body { span, .. } => span,
        SyntaxTree::Parameter { span, .. } => span,
        SyntaxTree::Skip { span, .. } => span,
        SyntaxTree::PositionalSeparator { span, .. } => span,
        SyntaxTree::KeywordSeparator { span, .. } => span,
        SyntaxTree::Decorator { span, .. } => span,
        SyntaxTree::Returns { span, .. } => span,
        SyntaxTree::Generic { span, .. } => span,
        SyntaxTree::TypeParameter { span, .. } => span,
        SyntaxTree::Return { span, .. } => span,
        SyntaxTree::Comment { span, .. } => span,
        SyntaxTree::Assign { span, .. } => span,
        SyntaxTree::Import { span, .. } => span,
        SyntaxTree::From { span, .. } => span,
        SyntaxTree::FromImport { span, .. } => span,
        SyntaxTree::Path { span, .. } => span,
        SyntaxTree::Aliased { span, .. } => span,
        SyntaxTree::Call { span, .. } => span,
        SyntaxTree::Name { span, .. } => span,
        SyntaxTree::Int { span, .. } => span,
        SyntaxTree::Float { span, .. } => span,
        SyntaxTree::String { span, .. } => span,
        SyntaxTree::True { span, .. } => span,
        SyntaxTree::False { span, .. } => span,
        SyntaxTree::None { span, .. } => span,
        SyntaxTree::Atom { span, .. } => span,
        SyntaxTree::Enum { span, .. } => span,
        SyntaxTree::EnumMember { span, .. } => span,
        SyntaxTree::Property { span, .. } => span,
        SyntaxTree::Accessor { span, .. } => span,
        SyntaxTree::Constructor { span, .. } => span,
        SyntaxTree::Using { span, .. } => span,
        SyntaxTree::Namespace { span, .. } => span,
        SyntaxTree::Variable { span, .. } => span,
        SyntaxTree::Field { span, .. } => span,
        SyntaxTree::Event { span, .. } => span,
        SyntaxTree::Is { span, .. } => span,
        SyntaxTree::Cast { span, .. } => span,
        SyntaxTree::Null { span, .. } => span,
        SyntaxTree::Inline { span, .. } => span,
        SyntaxTree::Unknown { span, .. } => span,
        SyntaxTree::Raw { span, .. } => span,
    }
}

/// Mutable mirror of `children_of`. Source-sorted
/// `Vec<&mut SyntaxTree>` covering every reachable sub-tree.
pub fn children_mut_of(tree: &mut SyntaxTree) -> Vec<&mut SyntaxTree> {
    let mut v: Vec<&mut SyntaxTree> = Vec::new();
    match tree {
        SyntaxTree::Module { children, .. } => {
            v.extend(children.iter_mut());
        }
        SyntaxTree::Expression { inner, .. } => {
            v.push(inner.as_mut());
        }
        SyntaxTree::Slot { children, .. } => {
            v.extend(children.iter_mut());
        }
        SyntaxTree::ObjectAccess { receiver, segments, .. } => {
            if let AccessReceiver::Instance(__t) = receiver { v.push(__t.as_mut()); }
            for __s in segments.iter_mut() {
                match __s {
                    AccessSegment::Member { .. } => {}
                    AccessSegment::Index { indices, .. } => v.extend(indices.iter_mut()),
                    AccessSegment::Call { arguments, .. } => v.extend(arguments.iter_mut()),
                }
            }
        }
        SyntaxTree::Binary { left, op, right, .. } => {
            v.push(left.as_mut());
            v.push(op.as_mut());
            v.push(right.as_mut());
        }
        SyntaxTree::Logical { left, op, right, .. } => {
            v.push(left.as_mut());
            v.push(op.as_mut());
            v.push(right.as_mut());
        }
        SyntaxTree::Operator { .. } => {}
        SyntaxTree::Unary { operand, .. } => {
            v.push(operand.as_mut());
        }
        SyntaxTree::Tuple { children, .. } => {
            v.extend(children.iter_mut());
        }
        SyntaxTree::List { children, .. } => {
            v.extend(children.iter_mut());
        }
        SyntaxTree::Set { children, .. } => {
            v.extend(children.iter_mut());
        }
        SyntaxTree::Dictionary { pairs, .. } => {
            v.extend(pairs.iter_mut());
        }
        SyntaxTree::Pair { key, value, .. } => {
            v.push(key.as_mut());
            v.push(value.as_mut());
        }
        SyntaxTree::GenericType { name, params, .. } => {
            v.push(name.as_mut());
            v.extend(params.iter_mut());
        }
        SyntaxTree::Comparison { left, right, .. } => {
            v.push(left.as_mut());
            v.push(right.as_mut());
        }
        SyntaxTree::If { condition, body, else_branch, .. } => {
            v.push(condition.as_mut());
            v.push(body.as_mut());
            if let Some(__t) = else_branch { v.push(__t.as_mut()); }
        }
        SyntaxTree::ElseIf { condition, body, else_branch, .. } => {
            v.push(condition.as_mut());
            v.push(body.as_mut());
            if let Some(__t) = else_branch { v.push(__t.as_mut()); }
        }
        SyntaxTree::Else { body, .. } => {
            v.push(body.as_mut());
        }
        SyntaxTree::For { targets, iterables, body, else_body, .. } => {
            v.extend(targets.iter_mut());
            v.extend(iterables.iter_mut());
            v.push(body.as_mut());
            if let Some(__t) = else_body { v.push(__t.as_mut()); }
        }
        SyntaxTree::While { condition, body, else_body, .. } => {
            v.push(condition.as_mut());
            v.push(body.as_mut());
            if let Some(__t) = else_body { v.push(__t.as_mut()); }
        }
        SyntaxTree::Foreach { type_ann, target, iterable, body, .. } => {
            if let Some(__t) = type_ann { v.push(__t.as_mut()); }
            v.push(target.as_mut());
            v.push(iterable.as_mut());
            v.push(body.as_mut());
        }
        SyntaxTree::CFor { initializer, condition, updates, body, .. } => {
            if let Some(__t) = initializer { v.push(__t.as_mut()); }
            if let Some(__t) = condition { v.push(__t.as_mut()); }
            v.extend(updates.iter_mut());
            v.push(body.as_mut());
        }
        SyntaxTree::DoWhile { body, condition, .. } => {
            v.push(body.as_mut());
            v.push(condition.as_mut());
        }
        SyntaxTree::Break { .. } => {}
        SyntaxTree::Continue { .. } => {}
        SyntaxTree::FieldWrap { inner, .. } => {
            v.push(inner.as_mut());
        }
        SyntaxTree::SimpleStatement { children, .. } => {
            v.extend(children.iter_mut());
        }
        SyntaxTree::Try { try_body, handlers, else_body, finally_body, .. } => {
            v.push(try_body.as_mut());
            v.extend(handlers.iter_mut());
            if let Some(__t) = else_body { v.push(__t.as_mut()); }
            if let Some(__t) = finally_body { v.push(__t.as_mut()); }
        }
        SyntaxTree::Except { type_target, binding, filter, body, .. } => {
            if let Some(__t) = type_target { v.push(__t.as_mut()); }
            if let Some(__t) = binding { v.push(__t.as_mut()); }
            if let Some(__t) = filter { v.push(__t.as_mut()); }
            v.push(body.as_mut());
        }
        SyntaxTree::Catch { type_target, binding, filter, body, .. } => {
            if let Some(__t) = type_target { v.push(__t.as_mut()); }
            if let Some(__t) = binding { v.push(__t.as_mut()); }
            if let Some(__t) = filter { v.push(__t.as_mut()); }
            v.push(body.as_mut());
        }
        SyntaxTree::TypeAlias { name, type_params, value, .. } => {
            v.push(name.as_mut());
            if let Some(__t) = type_params { v.push(__t.as_mut()); }
            v.push(value.as_mut());
        }
        SyntaxTree::KeywordArgument { name, value, .. } => {
            v.push(name.as_mut());
            v.push(value.as_mut());
        }
        SyntaxTree::ListSplat { inner, .. } => {
            v.push(inner.as_mut());
        }
        SyntaxTree::DictSplat { inner, .. } => {
            v.push(inner.as_mut());
        }
        SyntaxTree::Ternary { condition, if_true, if_false, .. } => {
            v.push(condition.as_mut());
            v.push(if_true.as_mut());
            v.push(if_false.as_mut());
        }
        SyntaxTree::ObjectCreation { type_target, arguments, initializer, .. } => {
            if let Some(__t) = type_target { v.push(__t.as_mut()); }
            v.extend(arguments.iter_mut());
            if let Some(__t) = initializer { v.push(__t.as_mut()); }
        }
        SyntaxTree::Lambda { parameters, body, .. } => {
            v.extend(parameters.iter_mut());
            v.push(body.inner_mut());
        }
        SyntaxTree::Function { decorators, name, generics, parameters, returns, throws, body, .. } => {
            v.extend(decorators.iter_mut());
            v.push(name.as_mut());
            v.extend(generics.iter_mut());
            v.extend(parameters.iter_mut());
            if let Some(__t) = returns { v.push(__t.as_mut()); }
            v.extend(throws.iter_mut());
            if let Some(__t) = body { v.push(__t.as_mut()); }
        }
        SyntaxTree::Method { decorators, name, generics, parameters, returns, throws, body, .. } => {
            v.extend(decorators.iter_mut());
            v.push(name.as_mut());
            v.extend(generics.iter_mut());
            v.extend(parameters.iter_mut());
            if let Some(__t) = returns { v.push(__t.as_mut()); }
            v.extend(throws.iter_mut());
            if let Some(__t) = body { v.push(__t.as_mut()); }
        }
        SyntaxTree::Class { decorators, name, generics, bases, where_clauses, body, .. } => {
            v.extend(decorators.iter_mut());
            v.push(name.as_mut());
            v.extend(generics.iter_mut());
            v.extend(bases.iter_mut());
            v.extend(where_clauses.iter_mut());
            v.push(body.as_mut());
        }
        SyntaxTree::Struct { decorators, name, generics, bases, where_clauses, body, .. } => {
            v.extend(decorators.iter_mut());
            v.push(name.as_mut());
            v.extend(generics.iter_mut());
            v.extend(bases.iter_mut());
            v.extend(where_clauses.iter_mut());
            v.push(body.as_mut());
        }
        SyntaxTree::Interface { decorators, name, generics, bases, where_clauses, body, .. } => {
            v.extend(decorators.iter_mut());
            v.push(name.as_mut());
            v.extend(generics.iter_mut());
            v.extend(bases.iter_mut());
            v.extend(where_clauses.iter_mut());
            v.push(body.as_mut());
        }
        SyntaxTree::Record { decorators, name, generics, bases, where_clauses, body, .. } => {
            v.extend(decorators.iter_mut());
            v.push(name.as_mut());
            v.extend(generics.iter_mut());
            v.extend(bases.iter_mut());
            v.extend(where_clauses.iter_mut());
            v.push(body.as_mut());
        }
        SyntaxTree::Body { children, .. } => {
            v.extend(children.iter_mut());
        }
        SyntaxTree::Parameter { name, type_ann, default, .. } => {
            v.push(name.as_mut());
            if let Some(__t) = type_ann { v.push(__t.as_mut()); }
            if let Some(__t) = default { v.push(__t.as_mut()); }
        }
        SyntaxTree::Skip { .. } => {}
        SyntaxTree::PositionalSeparator { .. } => {}
        SyntaxTree::KeywordSeparator { .. } => {}
        SyntaxTree::Decorator { inner, .. } => {
            v.push(inner.as_mut());
        }
        SyntaxTree::Returns { type_ann, .. } => {
            v.push(type_ann.as_mut());
        }
        SyntaxTree::Generic { items, .. } => {
            v.extend(items.iter_mut());
        }
        SyntaxTree::TypeParameter { name, constraint, .. } => {
            v.push(name.as_mut());
            if let Some(__t) = constraint { v.push(__t.as_mut()); }
        }
        SyntaxTree::Return { value, .. } => {
            if let Some(__t) = value { v.push(__t.as_mut()); }
        }
        SyntaxTree::Comment { .. } => {}
        SyntaxTree::Assign { targets, type_annotation, values, .. } => {
            v.extend(targets.iter_mut());
            if let Some(__t) = type_annotation { v.push(__t.as_mut()); }
            v.extend(values.iter_mut());
        }
        SyntaxTree::Import { children, .. } => {
            v.extend(children.iter_mut());
        }
        SyntaxTree::From { path, imports, .. } => {
            if let Some(__t) = path { v.push(__t.as_mut()); }
            v.extend(imports.iter_mut());
        }
        SyntaxTree::FromImport { name, alias, .. } => {
            v.push(name.as_mut());
            if let Some(__t) = alias { v.push(__t.as_mut()); }
        }
        SyntaxTree::Path { segments, .. } => {
            v.extend(segments.iter_mut());
        }
        SyntaxTree::Aliased { inner, .. } => {
            v.push(inner.as_mut());
        }
        SyntaxTree::Call { callee, arguments, .. } => {
            v.push(callee.as_mut());
            v.extend(arguments.iter_mut());
        }
        SyntaxTree::Name { .. } => {}
        SyntaxTree::Int { .. } => {}
        SyntaxTree::Float { .. } => {}
        SyntaxTree::String { .. } => {}
        SyntaxTree::True { .. } => {}
        SyntaxTree::False { .. } => {}
        SyntaxTree::None { .. } => {}
        SyntaxTree::Atom { .. } => {}
        SyntaxTree::Enum { decorators, name, underlying_type, members, .. } => {
            v.extend(decorators.iter_mut());
            v.push(name.as_mut());
            if let Some(__t) = underlying_type { v.push(__t.as_mut()); }
            v.extend(members.iter_mut());
        }
        SyntaxTree::EnumMember { decorators, name, value, .. } => {
            v.extend(decorators.iter_mut());
            v.push(name.as_mut());
            if let Some(__t) = value { v.push(__t.as_mut()); }
        }
        SyntaxTree::Property { decorators, type_ann, name, accessors, value, .. } => {
            v.extend(decorators.iter_mut());
            if let Some(__t) = type_ann { v.push(__t.as_mut()); }
            v.push(name.as_mut());
            v.extend(accessors.iter_mut());
            if let Some(__t) = value { v.push(__t.as_mut()); }
        }
        SyntaxTree::Accessor { body, .. } => {
            if let Some(__t) = body { v.push(__t.as_mut()); }
        }
        SyntaxTree::Constructor { decorators, name, parameters, body, .. } => {
            v.extend(decorators.iter_mut());
            v.push(name.as_mut());
            v.extend(parameters.iter_mut());
            v.push(body.as_mut());
        }
        SyntaxTree::Using { alias, path, .. } => {
            if let Some(__t) = alias { v.push(__t.as_mut()); }
            v.push(path.as_mut());
        }
        SyntaxTree::Namespace { name, children, .. } => {
            v.push(name.as_mut());
            v.extend(children.iter_mut());
        }
        SyntaxTree::Variable { decorators, type_ann, name, value, .. } => {
            v.extend(decorators.iter_mut());
            if let Some(__t) = type_ann { v.push(__t.as_mut()); }
            v.push(name.as_mut());
            if let Some(__e) = value { v.push(&mut __e.inner); }
        }
        SyntaxTree::Field { decorators, type_ann, name, value, .. } => {
            v.extend(decorators.iter_mut());
            if let Some(__t) = type_ann { v.push(__t.as_mut()); }
            v.push(name.as_mut());
            if let Some(__e) = value { v.push(&mut __e.inner); }
        }
        SyntaxTree::Event { decorators, type_ann, name, value, .. } => {
            v.extend(decorators.iter_mut());
            if let Some(__t) = type_ann { v.push(__t.as_mut()); }
            v.push(name.as_mut());
            if let Some(__e) = value { v.push(&mut __e.inner); }
        }
        SyntaxTree::Is { value, type_target, .. } => {
            v.push(value.as_mut());
            v.push(type_target.as_mut());
        }
        SyntaxTree::Cast { type_ann, value, .. } => {
            v.push(type_ann.as_mut());
            v.push(value.as_mut());
        }
        SyntaxTree::Null { .. } => {}
        SyntaxTree::Inline { children, .. } => {
            v.extend(children.iter_mut());
        }
        SyntaxTree::Unknown { .. } => {}
        SyntaxTree::Raw { children, .. } => {
            v.extend(children.iter_mut());
        }
    }
    v.sort_by_key(|c| range_of(c).start);
    v
}

/// Reconstruct a `SyntaxTree` from its JSON projection.
///
/// Inverse of `tree_to_json` (modulo lossy bits — source positions,
/// the `Vec<AccessSegment>` chain shape, ordering of duplicate-named
/// children). The reconstructed tree is suitable for re-rendering
/// via `render_source` to produce parseable source code; bit-identity
/// is **not** preserved.
///
/// Generated mechanically from `SyntaxTree` field types — no
/// per-variant special cases. See `build_codegen.rs::render_from_json`.
pub fn tree_from_json(value: &Value) -> SyntaxTree {
    tree_from_json_value(value)
}

/// Leak `s` into the static string pool. Used for `&'static str`
/// fields (`element_name`, `kind`, `wrapper`) where the JSON carries
/// a runtime string. One-time leak per distinct tag is acceptable for
/// a deserializer; the alternative would be a static interner map.
fn intern_static(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

/// Strip a common plural suffix so a Vec<SyntaxTree> field named
/// `decorators` falls back to looking up `decorator` (the singular
/// element name). Heuristic — covers `…s`, `…es`, `…_clauses`,
/// `…_branches`. Returns the input unchanged when no rule fires.
fn strip_plural(s: &str) -> &str {
    for suffix in ["_clauses", "_branches"] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            // `where_clauses` → `where`; matches the typical
            // SimpleStatement element_name pinned at lowering time.
            return stripped;
        }
    }
    for suffix in ["ies", "es", "s"] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            if suffix == "ies" {
                // Heuristic — return the stripped form; caller still
                // owns the lookup-fallback chain so a miss here just
                // means we drain from unclaimed_children.
                return stripped;
            }
            return stripped;
        }
    }
    s
}

fn tree_from_json_value(value: &Value) -> SyntaxTree {
    tree_from_json_with_type_hint(value, None)
}

/// Reconstruct with an optional `$type` hint. Used when recursing
/// into a child looked up by JSON key: the key implies the child's
/// type, and `to_json` strips `$type` from such children to avoid
/// duplication. The inverse restores it here.
fn tree_from_json_with_type_hint(value: &Value, type_hint: Option<&str>) -> SyntaxTree {
    match value {
        Value::Null => SyntaxTree::Null {
            text: String::new(),
            range: ByteRange::synthetic_empty(),
            span: Span::point(0, 0),
        },
        Value::Bool(true) => SyntaxTree::True {
            text: "true".to_string(),
            range: ByteRange::synthetic_empty(),
            span: Span::point(0, 0),
        },
        Value::Bool(false) => SyntaxTree::False {
            text: "false".to_string(),
            range: ByteRange::synthetic_empty(),
            span: Span::point(0, 0),
        },
        Value::Number(n) => SyntaxTree::Int {
            text: n.to_string(),
            range: ByteRange::synthetic_empty(),
            span: Span::point(0, 0),
        },
        Value::String(s) => {
            // A bare string under a typed key (e.g. `"name": "foo"`)
            // is the scalar form of a leaf variant — pick the variant
            // by the hint so `name`, `int`, `string`, etc. all
            // reconstruct correctly. Without a hint it falls back to
            // `Name` (the most common leaf shape).
            scalar_leaf_from_text(type_hint.unwrap_or("name"), s.clone())
        }
        Value::Array(arr) => {
            // Arrays under a typed key carry siblings of that type
            // (after the Z6 array-grouping fix). Each element inherits
            // the parent key as its type hint.
            let children: Vec<SyntaxTree> = arr
                .iter()
                .map(|v| tree_from_json_with_type_hint(v, type_hint))
                .collect();
            SyntaxTree::Inline {
                children,
                list_name: None,
                range: ByteRange::synthetic_empty(),
                span: Span::point(0, 0),
            }
        }
        Value::Object(map) => from_json_object_with_hint(map, type_hint),
    }
}

/// Construct the right scalar-leaf variant for a JSON string value,
/// keyed by the surrounding `$type` hint. Falls back to `Name` for
/// unknown hints — keeps the lossy inversion robust.
fn scalar_leaf_from_text(tag: &str, text: String) -> SyntaxTree {
    let range = ByteRange::synthetic_empty();
    let span = Span::point(0, 0);
    match tag {
        "int" => SyntaxTree::Int { text, range, span },
        "float" => SyntaxTree::Float { text, range, span },
        "string" => SyntaxTree::String {
            text,
            quote_style: QuoteStyle::Double,
            range,
            span,
        },
        "true" => SyntaxTree::True { text, range, span },
        "false" => SyntaxTree::False { text, range, span },
        "none" => SyntaxTree::None { text, range, span },
        "null" => SyntaxTree::Null { text, range, span },
        // Anything else: a bare string under an arbitrary key is most
        // likely an identifier-like leaf. Use Name so renderers see
        // text content without needing source bytes.
        _ => SyntaxTree::Name { text, range, span },
    }
}

fn from_json_object_with_hint(
    map: &serde_json::Map<String, Value>,
    type_hint: Option<&str>,
) -> SyntaxTree {
    // Resolve $type — prefer the explicit field; fall back to the
    // hint passed in by the parent context (key under which this
    // object was nested).
    let tag = map
        .get("$type")
        .and_then(|v| v.as_str())
        .or(type_hint)
        .unwrap_or("");
    dispatch_from_json_object(map, tag)
}

fn dispatch_from_json_object(map: &serde_json::Map<String, Value>, tag: &str) -> SyntaxTree {
    // Drain non-meta values into a flat list of children, in stable
    // key order. Each child carries the JSON key as a type hint so
    // nested objects with stripped `$type` reconstruct under the
    // correct variant. Booleans become marker names; numbers/null
    // are converted via the value-level dispatcher.
    let mut children: Vec<SyntaxTree> = Vec::new();
    let mut markers: Vec<&'static str> = Vec::new();
    for (key, val) in map.iter() {
        if key == "$type" { continue; }
        if key == "$children" {
            if let Value::Array(arr) = val {
                for item in arr { children.push(tree_from_json_value(item)); }
            }
            continue;
        }
        match val {
            Value::Bool(true) => markers.push(intern_static(key)),
            Value::Bool(false) => {}
            Value::Array(arr) => {
                for item in arr {
                    children.push(tree_from_json_with_type_hint(item, Some(key.as_str())));
                }
            }
            _ => children.push(tree_from_json_with_type_hint(val, Some(key.as_str()))),
        }
    }
    let marker_strs: Vec<&str> = markers.iter().copied().collect();
    let _ = &marker_strs;
    let leaf_text = map.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let _ = &leaf_text;
    let static_tag: &'static str = intern_static(tag);
    let _ = static_tag;
    match tag {
        "module" => {
            let claimed: &[&str] = &["children", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let children = {
                let singular = strip_plural("children");
                let alt_key = "children";
                if let Some(v) = map.get("children")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Module {
                children,
                range,
                span,
            }
        }
        "expression" => {
            let claimed: &[&str] = &["inner", "marker", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let inner = {
                let alt_key = "inner";
                let hint = if map.contains_key("inner") { "inner" } else { alt_key };
                if let Some(v) = map.get("inner").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let marker = None;
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Expression {
                inner,
                marker,
                range,
                span,
            }
        }
        "slot" => {
            let claimed: &[&str] = &["kind", "children", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let kind = {
                // The slot's kind is conveyed by the parent's JSON
                // key ($type after strip), passed in via `tag`.
                match tag {
                    "left" => crate::tree::syntax::types::SlotKind::Left,
                    "right" => crate::tree::syntax::types::SlotKind::Right,
                    "condition" => crate::tree::syntax::types::SlotKind::Condition,
                    "then" => crate::tree::syntax::types::SlotKind::Then,
                    "else" => crate::tree::syntax::types::SlotKind::Else,
                    "as" => crate::tree::syntax::types::SlotKind::As,
                    "filter" => crate::tree::syntax::types::SlotKind::Filter,
                    _ => crate::tree::syntax::types::SlotKind::Left,
                }
            };
            let children = {
                let singular = strip_plural("children");
                let alt_key = "children";
                if let Some(v) = map.get("children")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Slot {
                kind,
                children,
                range,
                span,
            }
        }
        "object_access" => {
            let claimed: &[&str] = &["receiver", "segments", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let receiver = {
                let inner = if let Some(v) = map.get("receiver") {
                    tree_from_json_with_type_hint(v, Some("receiver"))
                } else if !unclaimed_children.is_empty() {
                    unclaimed_children.remove(0)
                } else {
                    SyntaxTree::Unknown {
                        kind: "from_json:missing_receiver".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    }
                };
                AccessReceiver::from_tree(inner, &["this", "self", "super", "base"])
            };
            let segments = { let _ = &mut unclaimed_children; Vec::new() };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::ObjectAccess {
                receiver,
                segments,
                range,
                span,
            }
        }
        "binary" => {
            let claimed: &[&str] = &["left", "op", "right", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let left = {
                let alt_key = "left";
                let hint = if map.contains_key("left") { "left" } else { alt_key };
                if let Some(v) = map.get("left").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let op = {
                let alt_key = "op";
                let hint = if map.contains_key("op") { "op" } else { alt_key };
                if let Some(v) = map.get("op").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let right = {
                let alt_key = "right";
                let hint = if map.contains_key("right") { "right" } else { alt_key };
                if let Some(v) = map.get("right").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Binary {
                left,
                op,
                right,
                range,
                span,
            }
        }
        "logical" => {
            let claimed: &[&str] = &["left", "op", "right", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let left = {
                let alt_key = "left";
                let hint = if map.contains_key("left") { "left" } else { alt_key };
                if let Some(v) = map.get("left").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let op = {
                let alt_key = "op";
                let hint = if map.contains_key("op") { "op" } else { alt_key };
                if let Some(v) = map.get("op").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let right = {
                let alt_key = "right";
                let hint = if map.contains_key("right") { "right" } else { alt_key };
                if let Some(v) = map.get("right").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Logical {
                left,
                op,
                right,
                range,
                span,
            }
        }
        "operator" => {
            let claimed: &[&str] = &["text", "kind", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let text = map.get("text").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let kind = Default::default() /* TODO: from_json for kind: OperatorKind */;
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Operator {
                text,
                kind,
                range,
                span,
            }
        }
        "unary" => {
            let claimed: &[&str] = &["op_text", "op_marker", "op_range", "operand", "extra_markers", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let op_text = map.get("op_text").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let op_marker = static_tag;
            let op_range = ByteRange::synthetic_empty();
            let operand = {
                let alt_key = "operand";
                let hint = if map.contains_key("operand") { "operand" } else { alt_key };
                if let Some(v) = map.get("operand").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let extra_markers = Vec::new();
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Unary {
                op_text,
                op_marker,
                op_range,
                operand,
                extra_markers,
                range,
                span,
            }
        }
        "tuple" => {
            let claimed: &[&str] = &["children", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let children = {
                let singular = strip_plural("children");
                let alt_key = "children";
                if let Some(v) = map.get("children")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Tuple {
                children,
                range,
                span,
            }
        }
        "list" => {
            let claimed: &[&str] = &["children", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let children = {
                let singular = strip_plural("children");
                let alt_key = "children";
                if let Some(v) = map.get("children")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::List {
                children,
                range,
                span,
            }
        }
        "set" => {
            let claimed: &[&str] = &["children", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let children = {
                let singular = strip_plural("children");
                let alt_key = "children";
                if let Some(v) = map.get("children")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Set {
                children,
                range,
                span,
            }
        }
        "dictionary" => {
            let claimed: &[&str] = &["pairs", "pair", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let pairs = {
                let singular = strip_plural("pairs");
                let alt_key = "pairs";
                if let Some(v) = map.get("pairs")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Dictionary {
                pairs,
                range,
                span,
            }
        }
        "pair" => {
            let claimed: &[&str] = &["key", "value", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let key = {
                let alt_key = "key";
                let hint = if map.contains_key("key") { "key" } else { alt_key };
                if let Some(v) = map.get("key").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let value = {
                let alt_key = "value";
                let hint = if map.contains_key("value") { "value" } else { alt_key };
                if let Some(v) = map.get("value").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Pair {
                key,
                value,
                range,
                span,
            }
        }
        "generic_type" => {
            let claimed: &[&str] = &["name", "params", "param", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let params = {
                let singular = strip_plural("params");
                let alt_key = "params";
                if let Some(v) = map.get("params")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::GenericType {
                name,
                params,
                range,
                span,
            }
        }
        "comparison" => {
            let claimed: &[&str] = &["left", "op_text", "op_marker", "op_range", "right", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let left = {
                let alt_key = "left";
                let hint = if map.contains_key("left") { "left" } else { alt_key };
                if let Some(v) = map.get("left").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let op_text = map.get("op_text").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let op_marker = static_tag;
            let op_range = ByteRange::synthetic_empty();
            let right = {
                let alt_key = "right";
                let hint = if map.contains_key("right") { "right" } else { alt_key };
                if let Some(v) = map.get("right").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Comparison {
                left,
                op_text,
                op_marker,
                op_range,
                right,
                range,
                span,
            }
        }
        "if" => {
            let claimed: &[&str] = &["condition", "body", "else_branch", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let condition = {
                let alt_key = "condition";
                let hint = if map.contains_key("condition") { "condition" } else { alt_key };
                if let Some(v) = map.get("condition").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let body = {
                let alt_key = "body";
                let hint = if map.contains_key("body") { "body" } else { alt_key };
                if let Some(v) = map.get("body").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let else_branch = {
                let alt_key = "else_branch";
                let hint = if map.contains_key("else_branch") { "else_branch" } else { alt_key };
                if let Some(v) = map.get("else_branch").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::If {
                condition,
                body,
                else_branch,
                range,
                span,
            }
        }
        "else_if" => {
            let claimed: &[&str] = &["condition", "body", "else_branch", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let condition = {
                let alt_key = "condition";
                let hint = if map.contains_key("condition") { "condition" } else { alt_key };
                if let Some(v) = map.get("condition").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let body = {
                let alt_key = "body";
                let hint = if map.contains_key("body") { "body" } else { alt_key };
                if let Some(v) = map.get("body").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let else_branch = {
                let alt_key = "else_branch";
                let hint = if map.contains_key("else_branch") { "else_branch" } else { alt_key };
                if let Some(v) = map.get("else_branch").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::ElseIf {
                condition,
                body,
                else_branch,
                range,
                span,
            }
        }
        "else" => {
            let claimed: &[&str] = &["body", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let body = {
                let alt_key = "body";
                let hint = if map.contains_key("body") { "body" } else { alt_key };
                if let Some(v) = map.get("body").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Else {
                body,
                range,
                span,
            }
        }
        "for" => {
            let claimed: &[&str] = &["is_async", "targets", "target", "iterables", "iterabl", "body", "else_body", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let is_async = map.get("is_async").and_then(|v| v.as_bool()).unwrap_or(false);
            let targets = {
                let singular = strip_plural("targets");
                let alt_key = "targets";
                if let Some(v) = map.get("targets")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let iterables = {
                let singular = strip_plural("iterables");
                let alt_key = "iterables";
                if let Some(v) = map.get("iterables")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let body = {
                let alt_key = "body";
                let hint = if map.contains_key("body") { "body" } else { alt_key };
                if let Some(v) = map.get("body").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let else_body = {
                let alt_key = "else_body";
                let hint = if map.contains_key("else_body") { "else_body" } else { alt_key };
                if let Some(v) = map.get("else_body").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::For {
                is_async,
                targets,
                iterables,
                body,
                else_body,
                range,
                span,
            }
        }
        "while" => {
            let claimed: &[&str] = &["condition", "body", "else_body", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let condition = {
                let alt_key = "condition";
                let hint = if map.contains_key("condition") { "condition" } else { alt_key };
                if let Some(v) = map.get("condition").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let body = {
                let alt_key = "body";
                let hint = if map.contains_key("body") { "body" } else { alt_key };
                if let Some(v) = map.get("body").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let else_body = {
                let alt_key = "else_body";
                let hint = if map.contains_key("else_body") { "else_body" } else { alt_key };
                if let Some(v) = map.get("else_body").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::While {
                condition,
                body,
                else_body,
                range,
                span,
            }
        }
        "foreach" => {
            let claimed: &[&str] = &["type_ann", "type", "target", "iterable", "body", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let type_ann = {
                let alt_key = "type";
                let hint = if map.contains_key("type_ann") { "type_ann" } else { alt_key };
                if let Some(v) = map.get("type_ann").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let target = {
                let alt_key = "target";
                let hint = if map.contains_key("target") { "target" } else { alt_key };
                if let Some(v) = map.get("target").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let iterable = {
                let alt_key = "iterable";
                let hint = if map.contains_key("iterable") { "iterable" } else { alt_key };
                if let Some(v) = map.get("iterable").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let body = {
                let alt_key = "body";
                let hint = if map.contains_key("body") { "body" } else { alt_key };
                if let Some(v) = map.get("body").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Foreach {
                type_ann,
                target,
                iterable,
                body,
                range,
                span,
            }
        }
        "c_for" => {
            let claimed: &[&str] = &["initializer", "condition", "updates", "updat", "body", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let initializer = {
                let alt_key = "initializer";
                let hint = if map.contains_key("initializer") { "initializer" } else { alt_key };
                if let Some(v) = map.get("initializer").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let condition = {
                let alt_key = "condition";
                let hint = if map.contains_key("condition") { "condition" } else { alt_key };
                if let Some(v) = map.get("condition").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let updates = {
                let singular = strip_plural("updates");
                let alt_key = "updates";
                if let Some(v) = map.get("updates")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let body = {
                let alt_key = "body";
                let hint = if map.contains_key("body") { "body" } else { alt_key };
                if let Some(v) = map.get("body").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::CFor {
                initializer,
                condition,
                updates,
                body,
                range,
                span,
            }
        }
        "do_while" => {
            let claimed: &[&str] = &["body", "condition", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let body = {
                let alt_key = "body";
                let hint = if map.contains_key("body") { "body" } else { alt_key };
                if let Some(v) = map.get("body").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let condition = {
                let alt_key = "condition";
                let hint = if map.contains_key("condition") { "condition" } else { alt_key };
                if let Some(v) = map.get("condition").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::DoWhile {
                body,
                condition,
                range,
                span,
            }
        }
        "break" => {
            let claimed: &[&str] = &["range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Break {
                range,
                span,
            }
        }
        "continue" => {
            let claimed: &[&str] = &["range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Continue {
                range,
                span,
            }
        }
        "field_wrap" => {
            let claimed: &[&str] = &["wrapper", "inner", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let wrapper = static_tag;
            let inner = {
                let alt_key = "inner";
                let hint = if map.contains_key("inner") { "inner" } else { alt_key };
                if let Some(v) = map.get("inner").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::FieldWrap {
                wrapper,
                inner,
                range,
                span,
            }
        }
        "simple_statement" => {
            let claimed: &[&str] = &["element_name", "modifiers", "extra_markers", "children", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let element_name = static_tag;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let extra_markers = Vec::new();
            let children = {
                let singular = strip_plural("children");
                let alt_key = "children";
                if let Some(v) = map.get("children")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::SimpleStatement {
                element_name,
                modifiers,
                extra_markers,
                children,
                range,
                span,
            }
        }
        "try" => {
            let claimed: &[&str] = &["try_body", "handlers", "handler", "else_body", "finally_body", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let try_body = {
                let alt_key = "try_body";
                let hint = if map.contains_key("try_body") { "try_body" } else { alt_key };
                if let Some(v) = map.get("try_body").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let handlers = {
                let singular = strip_plural("handlers");
                let alt_key = "handlers";
                if let Some(v) = map.get("handlers")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let else_body = {
                let alt_key = "else_body";
                let hint = if map.contains_key("else_body") { "else_body" } else { alt_key };
                if let Some(v) = map.get("else_body").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let finally_body = {
                let alt_key = "finally_body";
                let hint = if map.contains_key("finally_body") { "finally_body" } else { alt_key };
                if let Some(v) = map.get("finally_body").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Try {
                try_body,
                handlers,
                else_body,
                finally_body,
                range,
                span,
            }
        }
        "except" => {
            let claimed: &[&str] = &["type_target", "type", "binding", "filter", "body", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let type_target = {
                let alt_key = "type";
                let hint = if map.contains_key("type_target") { "type_target" } else { alt_key };
                if let Some(v) = map.get("type_target").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let binding = {
                let alt_key = "binding";
                let hint = if map.contains_key("binding") { "binding" } else { alt_key };
                if let Some(v) = map.get("binding").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let filter = {
                let alt_key = "filter";
                let hint = if map.contains_key("filter") { "filter" } else { alt_key };
                if let Some(v) = map.get("filter").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let body = {
                let alt_key = "body";
                let hint = if map.contains_key("body") { "body" } else { alt_key };
                if let Some(v) = map.get("body").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Except {
                type_target,
                binding,
                filter,
                body,
                range,
                span,
            }
        }
        "catch" => {
            let claimed: &[&str] = &["type_target", "type", "binding", "filter", "body", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let type_target = {
                let alt_key = "type";
                let hint = if map.contains_key("type_target") { "type_target" } else { alt_key };
                if let Some(v) = map.get("type_target").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let binding = {
                let alt_key = "binding";
                let hint = if map.contains_key("binding") { "binding" } else { alt_key };
                if let Some(v) = map.get("binding").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let filter = {
                let alt_key = "filter";
                let hint = if map.contains_key("filter") { "filter" } else { alt_key };
                if let Some(v) = map.get("filter").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let body = {
                let alt_key = "body";
                let hint = if map.contains_key("body") { "body" } else { alt_key };
                if let Some(v) = map.get("body").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Catch {
                type_target,
                binding,
                filter,
                body,
                range,
                span,
            }
        }
        "type_alias" => {
            let claimed: &[&str] = &["name", "type_params", "value", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let type_params = {
                let alt_key = "type_params";
                let hint = if map.contains_key("type_params") { "type_params" } else { alt_key };
                if let Some(v) = map.get("type_params").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let value = {
                let alt_key = "value";
                let hint = if map.contains_key("value") { "value" } else { alt_key };
                if let Some(v) = map.get("value").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::TypeAlias {
                name,
                type_params,
                value,
                range,
                span,
            }
        }
        "keyword_argument" => {
            let claimed: &[&str] = &["name", "value", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let value = {
                let alt_key = "value";
                let hint = if map.contains_key("value") { "value" } else { alt_key };
                if let Some(v) = map.get("value").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::KeywordArgument {
                name,
                value,
                range,
                span,
            }
        }
        "list_splat" => {
            let claimed: &[&str] = &["inner", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let inner = {
                let alt_key = "inner";
                let hint = if map.contains_key("inner") { "inner" } else { alt_key };
                if let Some(v) = map.get("inner").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::ListSplat {
                inner,
                range,
                span,
            }
        }
        "dict_splat" => {
            let claimed: &[&str] = &["inner", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let inner = {
                let alt_key = "inner";
                let hint = if map.contains_key("inner") { "inner" } else { alt_key };
                if let Some(v) = map.get("inner").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::DictSplat {
                inner,
                range,
                span,
            }
        }
        "ternary" => {
            let claimed: &[&str] = &["condition", "if_true", "if_false", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let condition = {
                let alt_key = "condition";
                let hint = if map.contains_key("condition") { "condition" } else { alt_key };
                if let Some(v) = map.get("condition").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let if_true = {
                let alt_key = "if_true";
                let hint = if map.contains_key("if_true") { "if_true" } else { alt_key };
                if let Some(v) = map.get("if_true").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let if_false = {
                let alt_key = "if_false";
                let hint = if map.contains_key("if_false") { "if_false" } else { alt_key };
                if let Some(v) = map.get("if_false").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Ternary {
                condition,
                if_true,
                if_false,
                range,
                span,
            }
        }
        "object_creation" => {
            let claimed: &[&str] = &["type_target", "type", "arguments", "argument", "initializer", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let type_target = {
                let alt_key = "type";
                let hint = if map.contains_key("type_target") { "type_target" } else { alt_key };
                if let Some(v) = map.get("type_target").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let arguments = {
                let singular = strip_plural("arguments");
                let alt_key = "arguments";
                if let Some(v) = map.get("arguments")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let initializer = {
                let alt_key = "initializer";
                let hint = if map.contains_key("initializer") { "initializer" } else { alt_key };
                if let Some(v) = map.get("initializer").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::ObjectCreation {
                type_target,
                arguments,
                initializer,
                range,
                span,
            }
        }
        "lambda" => {
            let claimed: &[&str] = &["modifiers", "parameters", "parameter", "body", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let parameters = {
                let singular = strip_plural("parameters");
                let alt_key = "parameters";
                if let Some(v) = map.get("parameters")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let body = {
                let inner = if let Some(v) = map.get("body").or_else(|| map.get("body")) {
                    tree_from_json_with_type_hint(v, Some("body"))
                } else if !unclaimed_children.is_empty() {
                    unclaimed_children.remove(0)
                } else {
                    SyntaxTree::Body {
                        children: Vec::new(),
                        block_wrap: false,
                        pass_only: false,
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    }
                };
                LambdaBody::Expression(Box::new(inner))
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Lambda {
                modifiers,
                parameters,
                body,
                range,
                span,
            }
        }
        "function" => {
            let claimed: &[&str] = &["modifiers", "decorators", "decorator", "name", "generics", "generic", "parameters", "parameter", "returns", "throws", "throw", "body", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                let alt_key = "decorators";
                if let Some(v) = map.get("decorators")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let generics = {
                let singular = strip_plural("generics");
                let alt_key = "generics";
                if let Some(v) = map.get("generics")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let parameters = {
                let singular = strip_plural("parameters");
                let alt_key = "parameters";
                if let Some(v) = map.get("parameters")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let returns = {
                let alt_key = "returns";
                let hint = if map.contains_key("returns") { "returns" } else { alt_key };
                if let Some(v) = map.get("returns").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let throws = {
                let singular = strip_plural("throws");
                let alt_key = "throws";
                if let Some(v) = map.get("throws")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let body = {
                let alt_key = "body";
                let hint = if map.contains_key("body") { "body" } else { alt_key };
                if let Some(v) = map.get("body").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Function {
                modifiers,
                decorators,
                name,
                generics,
                parameters,
                returns,
                throws,
                body,
                range,
                span,
            }
        }
        "method" => {
            let claimed: &[&str] = &["modifiers", "decorators", "decorator", "name", "generics", "generic", "parameters", "parameter", "returns", "throws", "throw", "body", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                let alt_key = "decorators";
                if let Some(v) = map.get("decorators")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let generics = {
                let singular = strip_plural("generics");
                let alt_key = "generics";
                if let Some(v) = map.get("generics")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let parameters = {
                let singular = strip_plural("parameters");
                let alt_key = "parameters";
                if let Some(v) = map.get("parameters")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let returns = {
                let alt_key = "returns";
                let hint = if map.contains_key("returns") { "returns" } else { alt_key };
                if let Some(v) = map.get("returns").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let throws = {
                let singular = strip_plural("throws");
                let alt_key = "throws";
                if let Some(v) = map.get("throws")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let body = {
                let alt_key = "body";
                let hint = if map.contains_key("body") { "body" } else { alt_key };
                if let Some(v) = map.get("body").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Method {
                modifiers,
                decorators,
                name,
                generics,
                parameters,
                returns,
                throws,
                body,
                range,
                span,
            }
        }
        "class" => {
            let claimed: &[&str] = &["modifiers", "decorators", "decorator", "name", "generics", "generic", "bases", "bas", "where_clauses", "where", "body", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                let alt_key = "decorators";
                if let Some(v) = map.get("decorators")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let generics = {
                let singular = strip_plural("generics");
                let alt_key = "generics";
                if let Some(v) = map.get("generics")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let bases = {
                let singular = strip_plural("bases");
                let alt_key = "bases";
                if let Some(v) = map.get("bases")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let where_clauses = {
                let singular = strip_plural("where_clauses");
                let alt_key = "where_clauses";
                if let Some(v) = map.get("where_clauses")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let body = {
                let alt_key = "body";
                let hint = if map.contains_key("body") { "body" } else { alt_key };
                if let Some(v) = map.get("body").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Class {
                modifiers,
                decorators,
                name,
                generics,
                bases,
                where_clauses,
                body,
                range,
                span,
            }
        }
        "struct" => {
            let claimed: &[&str] = &["modifiers", "decorators", "decorator", "name", "generics", "generic", "bases", "bas", "where_clauses", "where", "body", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                let alt_key = "decorators";
                if let Some(v) = map.get("decorators")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let generics = {
                let singular = strip_plural("generics");
                let alt_key = "generics";
                if let Some(v) = map.get("generics")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let bases = {
                let singular = strip_plural("bases");
                let alt_key = "bases";
                if let Some(v) = map.get("bases")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let where_clauses = {
                let singular = strip_plural("where_clauses");
                let alt_key = "where_clauses";
                if let Some(v) = map.get("where_clauses")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let body = {
                let alt_key = "body";
                let hint = if map.contains_key("body") { "body" } else { alt_key };
                if let Some(v) = map.get("body").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Struct {
                modifiers,
                decorators,
                name,
                generics,
                bases,
                where_clauses,
                body,
                range,
                span,
            }
        }
        "interface" => {
            let claimed: &[&str] = &["modifiers", "decorators", "decorator", "name", "generics", "generic", "bases", "bas", "where_clauses", "where", "body", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                let alt_key = "decorators";
                if let Some(v) = map.get("decorators")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let generics = {
                let singular = strip_plural("generics");
                let alt_key = "generics";
                if let Some(v) = map.get("generics")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let bases = {
                let singular = strip_plural("bases");
                let alt_key = "bases";
                if let Some(v) = map.get("bases")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let where_clauses = {
                let singular = strip_plural("where_clauses");
                let alt_key = "where_clauses";
                if let Some(v) = map.get("where_clauses")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let body = {
                let alt_key = "body";
                let hint = if map.contains_key("body") { "body" } else { alt_key };
                if let Some(v) = map.get("body").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Interface {
                modifiers,
                decorators,
                name,
                generics,
                bases,
                where_clauses,
                body,
                range,
                span,
            }
        }
        "record" => {
            let claimed: &[&str] = &["modifiers", "decorators", "decorator", "name", "generics", "generic", "bases", "bas", "where_clauses", "where", "body", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                let alt_key = "decorators";
                if let Some(v) = map.get("decorators")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let generics = {
                let singular = strip_plural("generics");
                let alt_key = "generics";
                if let Some(v) = map.get("generics")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let bases = {
                let singular = strip_plural("bases");
                let alt_key = "bases";
                if let Some(v) = map.get("bases")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let where_clauses = {
                let singular = strip_plural("where_clauses");
                let alt_key = "where_clauses";
                if let Some(v) = map.get("where_clauses")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let body = {
                let alt_key = "body";
                let hint = if map.contains_key("body") { "body" } else { alt_key };
                if let Some(v) = map.get("body").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Record {
                modifiers,
                decorators,
                name,
                generics,
                bases,
                where_clauses,
                body,
                range,
                span,
            }
        }
        "body" => {
            let claimed: &[&str] = &["children", "pass_only", "block_wrap", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let children = {
                let singular = strip_plural("children");
                let alt_key = "children";
                if let Some(v) = map.get("children")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let pass_only = map.get("pass_only").and_then(|v| v.as_bool()).unwrap_or(false);
            let block_wrap = map.get("block_wrap").and_then(|v| v.as_bool()).unwrap_or(false);
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Body {
                children,
                pass_only,
                block_wrap,
                range,
                span,
            }
        }
        "parameter" => {
            let claimed: &[&str] = &["kind", "extra_markers", "modifiers", "name", "type_ann", "type", "default", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let kind = {
                if marker_strs.iter().any(|m| *m == "args") {
                    ParamKind::Args
                } else if marker_strs.iter().any(|m| *m == "kwargs") {
                    ParamKind::Kwargs
                } else {
                    ParamKind::Regular
                }
            };
            let extra_markers = Vec::new();
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let type_ann = {
                let alt_key = "type";
                let hint = if map.contains_key("type_ann") { "type_ann" } else { alt_key };
                if let Some(v) = map.get("type_ann").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let default = {
                let alt_key = "default";
                let hint = if map.contains_key("default") { "default" } else { alt_key };
                if let Some(v) = map.get("default").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Parameter {
                kind,
                extra_markers,
                modifiers,
                name,
                type_ann,
                default,
                range,
                span,
            }
        }
        "positional_separator" => {
            let claimed: &[&str] = &["range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::PositionalSeparator {
                range,
                span,
            }
        }
        "keyword_separator" => {
            let claimed: &[&str] = &["range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::KeywordSeparator {
                range,
                span,
            }
        }
        "decorator" => {
            let claimed: &[&str] = &["inner", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let inner = {
                let alt_key = "inner";
                let hint = if map.contains_key("inner") { "inner" } else { alt_key };
                if let Some(v) = map.get("inner").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Decorator {
                inner,
                range,
                span,
            }
        }
        "returns" => {
            let claimed: &[&str] = &["type_ann", "type", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let type_ann = {
                let alt_key = "type";
                let hint = if map.contains_key("type_ann") { "type_ann" } else { alt_key };
                if let Some(v) = map.get("type_ann").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Returns {
                type_ann,
                range,
                span,
            }
        }
        "generic" => {
            let claimed: &[&str] = &["items", "item", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let items = {
                let singular = strip_plural("items");
                let alt_key = "items";
                if let Some(v) = map.get("items")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Generic {
                items,
                range,
                span,
            }
        }
        "type_parameter" => {
            let claimed: &[&str] = &["name", "constraint", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let constraint = {
                let alt_key = "constraint";
                let hint = if map.contains_key("constraint") { "constraint" } else { alt_key };
                if let Some(v) = map.get("constraint").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::TypeParameter {
                name,
                constraint,
                range,
                span,
            }
        }
        "return" => {
            let claimed: &[&str] = &["value", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let value = {
                let alt_key = "value";
                let hint = if map.contains_key("value") { "value" } else { alt_key };
                if let Some(v) = map.get("value").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Return {
                value,
                range,
                span,
            }
        }
        "comment" => {
            let claimed: &[&str] = &["leading", "trailing", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let leading = { if map.get("leading").and_then(|v| v.as_bool()).unwrap_or(false) {
                    Flag::implicit_at(0, 0)
                } else { Flag::Off } };
            let trailing = { if map.get("trailing").and_then(|v| v.as_bool()).unwrap_or(false) {
                    Flag::implicit_at(0, 0)
                } else { Flag::Off } };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Comment {
                leading,
                trailing,
                range,
                span,
            }
        }
        "assign" => {
            let claimed: &[&str] = &["targets", "target", "type_annotation", "op_text", "op_range", "op_markers", "values", "valu", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let targets = {
                let singular = strip_plural("targets");
                let alt_key = "targets";
                if let Some(v) = map.get("targets")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let type_annotation = {
                let alt_key = "type_annotation";
                let hint = if map.contains_key("type_annotation") { "type_annotation" } else { alt_key };
                if let Some(v) = map.get("type_annotation").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let op_text = map.get("op_text").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let op_range = ByteRange::synthetic_empty();
            let op_markers = Vec::new();
            let values = {
                let singular = strip_plural("values");
                let alt_key = "values";
                if let Some(v) = map.get("values")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Assign {
                targets,
                type_annotation,
                op_text,
                op_range,
                op_markers,
                values,
                range,
                span,
            }
        }
        "import" => {
            let claimed: &[&str] = &["has_alias", "children", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let has_alias = map.get("has_alias").and_then(|v| v.as_bool()).unwrap_or(false);
            let children = {
                let singular = strip_plural("children");
                let alt_key = "children";
                if let Some(v) = map.get("children")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Import {
                has_alias,
                children,
                range,
                span,
            }
        }
        "from" => {
            let claimed: &[&str] = &["relative", "path", "imports", "import", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let relative = map.get("relative").and_then(|v| v.as_bool()).unwrap_or(false);
            let path = {
                let alt_key = "path";
                let hint = if map.contains_key("path") { "path" } else { alt_key };
                if let Some(v) = map.get("path").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let imports = {
                let singular = strip_plural("imports");
                let alt_key = "imports";
                if let Some(v) = map.get("imports")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::From {
                relative,
                path,
                imports,
                range,
                span,
            }
        }
        "from_import" => {
            let claimed: &[&str] = &["has_alias", "name", "alias", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let has_alias = map.get("has_alias").and_then(|v| v.as_bool()).unwrap_or(false);
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let alias = {
                let alt_key = "alias";
                let hint = if map.contains_key("alias") { "alias" } else { alt_key };
                if let Some(v) = map.get("alias").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::FromImport {
                has_alias,
                name,
                alias,
                range,
                span,
            }
        }
        "path" => {
            let claimed: &[&str] = &["segments", "segment", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let segments = {
                let singular = strip_plural("segments");
                let alt_key = "segments";
                if let Some(v) = map.get("segments")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Path {
                segments,
                range,
                span,
            }
        }
        "aliased" => {
            let claimed: &[&str] = &["inner", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let inner = {
                let alt_key = "inner";
                let hint = if map.contains_key("inner") { "inner" } else { alt_key };
                if let Some(v) = map.get("inner").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Aliased {
                inner,
                range,
                span,
            }
        }
        "call" => {
            let claimed: &[&str] = &["callee", "arguments", "argument", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let callee = {
                let alt_key = "callee";
                let hint = if map.contains_key("callee") { "callee" } else { alt_key };
                if let Some(v) = map.get("callee").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let arguments = {
                let singular = strip_plural("arguments");
                let alt_key = "arguments";
                if let Some(v) = map.get("arguments")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Call {
                callee,
                arguments,
                range,
                span,
            }
        }
        "name" => {
            let claimed: &[&str] = &["text", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let text = map.get("text").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Name {
                text,
                range,
                span,
            }
        }
        "int" => {
            let claimed: &[&str] = &["text", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let text = map.get("text").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Int {
                text,
                range,
                span,
            }
        }
        "float" => {
            let claimed: &[&str] = &["text", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let text = map.get("text").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Float {
                text,
                range,
                span,
            }
        }
        "string" => {
            let claimed: &[&str] = &["text", "quote_style", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let text = map.get("text").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let quote_style = QuoteStyle::Double;
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::String {
                text,
                quote_style,
                range,
                span,
            }
        }
        "true" => {
            let claimed: &[&str] = &["text", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let text = map.get("text").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::True {
                text,
                range,
                span,
            }
        }
        "false" => {
            let claimed: &[&str] = &["text", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let text = map.get("text").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::False {
                text,
                range,
                span,
            }
        }
        "none" => {
            let claimed: &[&str] = &["text", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let text = map.get("text").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::None {
                text,
                range,
                span,
            }
        }
        "atom" => {
            let claimed: &[&str] = &["element_name", "text", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let element_name = static_tag;
            let text = map.get("text").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Atom {
                element_name,
                text,
                range,
                span,
            }
        }
        "enum" => {
            let claimed: &[&str] = &["modifiers", "decorators", "decorator", "name", "underlying_type", "members", "member", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                let alt_key = "decorators";
                if let Some(v) = map.get("decorators")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let underlying_type = {
                let alt_key = "underlying_type";
                let hint = if map.contains_key("underlying_type") { "underlying_type" } else { alt_key };
                if let Some(v) = map.get("underlying_type").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let members = {
                let singular = strip_plural("members");
                let alt_key = "members";
                if let Some(v) = map.get("members")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Enum {
                modifiers,
                decorators,
                name,
                underlying_type,
                members,
                range,
                span,
            }
        }
        "enum_member" => {
            let claimed: &[&str] = &["decorators", "decorator", "name", "value", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let decorators = {
                let singular = strip_plural("decorators");
                let alt_key = "decorators";
                if let Some(v) = map.get("decorators")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let value = {
                let alt_key = "value";
                let hint = if map.contains_key("value") { "value" } else { alt_key };
                if let Some(v) = map.get("value").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::EnumMember {
                decorators,
                name,
                value,
                range,
                span,
            }
        }
        "property" => {
            let claimed: &[&str] = &["modifiers", "decorators", "decorator", "type_ann", "type", "name", "accessors", "accessor", "value", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                let alt_key = "decorators";
                if let Some(v) = map.get("decorators")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let type_ann = {
                let alt_key = "type";
                let hint = if map.contains_key("type_ann") { "type_ann" } else { alt_key };
                if let Some(v) = map.get("type_ann").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let accessors = {
                let singular = strip_plural("accessors");
                let alt_key = "accessors";
                if let Some(v) = map.get("accessors")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let value = {
                let alt_key = "value";
                let hint = if map.contains_key("value") { "value" } else { alt_key };
                if let Some(v) = map.get("value").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Property {
                modifiers,
                decorators,
                type_ann,
                name,
                accessors,
                value,
                range,
                span,
            }
        }
        "accessor" => {
            let claimed: &[&str] = &["modifiers", "kind", "body", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let kind = {
                map.get("kind").and_then(|v| v.as_str()).map(|s| match s {
                    "get" => AccessorKind::Get,
                    "set" => AccessorKind::Set,
                    "init" => AccessorKind::Init,
                    _ => AccessorKind::Get,
                }).unwrap_or(AccessorKind::Get)
            };
            let body = {
                let alt_key = "body";
                let hint = if map.contains_key("body") { "body" } else { alt_key };
                if let Some(v) = map.get("body").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Accessor {
                modifiers,
                kind,
                body,
                range,
                span,
            }
        }
        "constructor" => {
            let claimed: &[&str] = &["modifiers", "decorators", "decorator", "name", "parameters", "parameter", "body", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                let alt_key = "decorators";
                if let Some(v) = map.get("decorators")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let parameters = {
                let singular = strip_plural("parameters");
                let alt_key = "parameters";
                if let Some(v) = map.get("parameters")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let body = {
                let alt_key = "body";
                let hint = if map.contains_key("body") { "body" } else { alt_key };
                if let Some(v) = map.get("body").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Constructor {
                modifiers,
                decorators,
                name,
                parameters,
                body,
                range,
                span,
            }
        }
        "using" => {
            let claimed: &[&str] = &["is_static", "alias", "path", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let is_static = map.get("is_static").and_then(|v| v.as_bool()).unwrap_or(false);
            let alias = {
                let alt_key = "alias";
                let hint = if map.contains_key("alias") { "alias" } else { alt_key };
                if let Some(v) = map.get("alias").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let path = {
                let alt_key = "path";
                let hint = if map.contains_key("path") { "path" } else { alt_key };
                if let Some(v) = map.get("path").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Using {
                is_static,
                alias,
                path,
                range,
                span,
            }
        }
        "namespace" => {
            let claimed: &[&str] = &["name", "children", "file_scoped", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let children = {
                let singular = strip_plural("children");
                let alt_key = "children";
                if let Some(v) = map.get("children")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let file_scoped = map.get("file_scoped").and_then(|v| v.as_bool()).unwrap_or(false);
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Namespace {
                name,
                children,
                file_scoped,
                range,
                span,
            }
        }
        "variable" => {
            let claimed: &[&str] = &["modifiers", "decorators", "decorator", "extra_markers", "type_ann", "type", "name", "value", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                let alt_key = "decorators";
                if let Some(v) = map.get("decorators")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let extra_markers = Vec::new();
            let type_ann = {
                let alt_key = "type";
                let hint = if map.contains_key("type_ann") { "type_ann" } else { alt_key };
                if let Some(v) = map.get("type_ann").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let value = {
                if let Some(v) = map.get("value").or_else(|| map.get("expression")) {
                    Some(Expression::wrap(tree_from_json_with_type_hint(v, Some("expression"))))
                } else if !unclaimed_children.is_empty() {
                    Some(Expression::wrap(unclaimed_children.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Variable {
                modifiers,
                decorators,
                extra_markers,
                type_ann,
                name,
                value,
                range,
                span,
            }
        }
        "field" => {
            let claimed: &[&str] = &["modifiers", "decorators", "decorator", "type_ann", "type", "name", "value", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                let alt_key = "decorators";
                if let Some(v) = map.get("decorators")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let type_ann = {
                let alt_key = "type";
                let hint = if map.contains_key("type_ann") { "type_ann" } else { alt_key };
                if let Some(v) = map.get("type_ann").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let value = {
                if let Some(v) = map.get("value").or_else(|| map.get("expression")) {
                    Some(Expression::wrap(tree_from_json_with_type_hint(v, Some("expression"))))
                } else if !unclaimed_children.is_empty() {
                    Some(Expression::wrap(unclaimed_children.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Field {
                modifiers,
                decorators,
                type_ann,
                name,
                value,
                range,
                span,
            }
        }
        "event" => {
            let claimed: &[&str] = &["modifiers", "decorators", "decorator", "type_ann", "type", "name", "value", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                let alt_key = "decorators";
                if let Some(v) = map.get("decorators")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let type_ann = {
                let alt_key = "type";
                let hint = if map.contains_key("type_ann") { "type_ann" } else { alt_key };
                if let Some(v) = map.get("type_ann").or_else(|| map.get(alt_key)) {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some(hint))))
                } else if !unclaimed_children.is_empty() {
                    Some(Box::new(unclaimed_children.remove(0)))
                } else { None }
            };
            let name = {
                let alt_key = "name";
                let hint = if map.contains_key("name") { "name" } else { alt_key };
                if let Some(v) = map.get("name").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let value = {
                if let Some(v) = map.get("value").or_else(|| map.get("expression")) {
                    Some(Expression::wrap(tree_from_json_with_type_hint(v, Some("expression"))))
                } else if !unclaimed_children.is_empty() {
                    Some(Expression::wrap(unclaimed_children.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Event {
                modifiers,
                decorators,
                type_ann,
                name,
                value,
                range,
                span,
            }
        }
        "is" => {
            let claimed: &[&str] = &["value", "type_target", "type", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let value = {
                let alt_key = "value";
                let hint = if map.contains_key("value") { "value" } else { alt_key };
                if let Some(v) = map.get("value").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let type_target = {
                let alt_key = "type";
                let hint = if map.contains_key("type_target") { "type_target" } else { alt_key };
                if let Some(v) = map.get("type_target").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Is {
                value,
                type_target,
                range,
                span,
            }
        }
        "cast" => {
            let claimed: &[&str] = &["type_ann", "type", "value", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let type_ann = {
                let alt_key = "type";
                let hint = if map.contains_key("type_ann") { "type_ann" } else { alt_key };
                if let Some(v) = map.get("type_ann").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let value = {
                let alt_key = "value";
                let hint = if map.contains_key("value") { "value" } else { alt_key };
                if let Some(v) = map.get("value").or_else(|| map.get(alt_key)) {
                    Box::new(tree_from_json_with_type_hint(v, Some(hint)))
                } else if !unclaimed_children.is_empty() {
                    Box::new(unclaimed_children.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Cast {
                type_ann,
                value,
                range,
                span,
            }
        }
        "null" => {
            let claimed: &[&str] = &["text", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let text = map.get("text").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Null {
                text,
                range,
                span,
            }
        }
        "unknown" => {
            let claimed: &[&str] = &["kind", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let kind = map.get("kind").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Unknown {
                kind,
                range,
                span,
            }
        }
        "raw" => {
            let claimed: &[&str] = &["kind", "is_named", "children", "range", "span"];
            let mut unclaimed_children: Vec<SyntaxTree> = map.iter()
                .filter(|(k, _)| k.as_str() != "$type" && k.as_str() != "$children" && !claimed.contains(&k.as_str()))
                .flat_map(|(k, v)| match v {
                    Value::Bool(_) => Vec::new(),
                    Value::Array(arr) => arr.iter().map(|c| tree_from_json_with_type_hint(c, Some(k.as_str()))).collect(),
                    _ => vec![tree_from_json_with_type_hint(v, Some(k.as_str()))],
                })
                .collect();
            let _ = &children;
            let kind = map.get("kind").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let is_named = map.get("is_named").and_then(|v| v.as_bool()).unwrap_or(false);
            let children = {
                let singular = strip_plural("children");
                let alt_key = "children";
                if let Some(v) = map.get("children")
                    .or_else(|| map.get(singular))
                    .or_else(|| map.get(alt_key))
                {
                    let hint = if map.contains_key(singular) { singular } else { alt_key };
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(hint)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(hint))],
                    }
                } else {
                    std::mem::take(&mut unclaimed_children)
                }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Raw {
                kind,
                is_named,
                children,
                range,
                span,
            }
        }
        _ => {
            if tag_looks_like_slot_name(tag) {
                SyntaxTree::SimpleStatement {
                    element_name: static_tag,
                    modifiers: Modifiers::from_marker_names(&marker_strs),
                    extra_markers: Vec::new(),
                    children,
                    range: ByteRange::synthetic_empty(),
                    span: Span::point(0, 0),
                }
            } else {
                SyntaxTree::Unknown {
                    kind: format!("from_json:{}", tag),
                    range: ByteRange::synthetic_empty(),
                    span: Span::point(0, 0),
                }
            }
        }
    }
}

/// True iff `tag` looks like a valid SimpleStatement-style slot
/// name: starts with `[a-z_]`, continues with `[a-z0-9_]`. Avoids
/// silently rewriting genuine typos (`not-a-real-variant`) into
/// SimpleStatement when they really should surface as Unknown.
fn tag_looks_like_slot_name(tag: &str) -> bool {
    let mut chars = tag.chars();
    let Some(first) = chars.next() else { return false };
    if !(first.is_ascii_lowercase() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}
