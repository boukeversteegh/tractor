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
    Expression, Flag, LambdaBody, Marker, Modifiers, ParamKind, QuoteStyle,
    Span, SyntaxTree,
};

#[allow(unused_imports)]
use serde_json::Value;

// Per-variant element-name overrides declared via
// `@element_name = <fn>` on the variant's doc comment in `types.rs`.
#[allow(unused_imports)]
use super::types::{
    element_name_for_accessor, element_name_for_atom,
    element_name_for_field_wrap, element_name_for_generic_type,
    element_name_for_object_access, element_name_for_raw,
    element_name_for_simple_statement, element_name_for_type_parameter,
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
        SyntaxTree::ObjectAccess { .. } => Some(element_name_for_object_access(tree)),
        SyntaxTree::Binary { .. } => Some("binary"),
        SyntaxTree::Logical { .. } => Some("logical"),
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
/// the field with trailing `_` stripped), and any `Vec<Marker>` field.
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
        SyntaxTree::ObjectAccess { .. } => {}
        SyntaxTree::Binary { .. } => {}
        SyntaxTree::Logical { .. } => {}
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
        SyntaxTree::Variable { modifiers, span, .. } => {
            for (mname, mspan) in modifiers.markers_with_spans() {
                out.push(Marker {
                    name: mname,
                    range: ByteRange::synthetic_empty(),
                    span: mspan.unwrap_or(*span),
                });
            }

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
        SyntaxTree::ObjectAccess { range, .. } => *range,
        SyntaxTree::Binary { range, .. } => *range,
        SyntaxTree::Logical { range, .. } => *range,
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
        SyntaxTree::ObjectAccess { span, .. } => *span,
        SyntaxTree::Binary { span, .. } => *span,
        SyntaxTree::Logical { span, .. } => *span,
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
        SyntaxTree::Binary { left, right, .. } => {
            v.push(left);
            v.push(right);
        }
        SyntaxTree::Logical { left, right, .. } => {
            v.push(left);
            v.push(right);
        }
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

/// Mutable access to the source-location span. Used by
/// `assign_ids` to stamp `NodeId`s into existing spans without
/// rebuilding nodes. Delegates from `TreeNode::span_mut`.
pub fn span_mut_of(tree: &mut SyntaxTree) -> &mut Span {
    match tree {
        SyntaxTree::Module { span, .. } => span,
        SyntaxTree::Expression { span, .. } => span,
        SyntaxTree::ObjectAccess { span, .. } => span,
        SyntaxTree::Binary { span, .. } => span,
        SyntaxTree::Logical { span, .. } => span,
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
        SyntaxTree::Binary { left, right, .. } => {
            v.push(left.as_mut());
            v.push(right.as_mut());
        }
        SyntaxTree::Logical { left, right, .. } => {
            v.push(left.as_mut());
            v.push(right.as_mut());
        }
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
                // means we drain from __kids.
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
            let mut __kids = children;
            let children = {
                let singular = strip_plural("children");
                if let Some(v) = map.get("children").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
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
            let mut __kids = children;
            let inner = {
                if let Some(v) = map.get("inner") {
                    Box::new(tree_from_json_with_type_hint(v, Some("inner")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
        "object_access" => {
            let mut __kids = children;
            let receiver = {
                let inner = if let Some(v) = map.get("receiver") {
                    tree_from_json_with_type_hint(v, Some("receiver"))
                } else if !__kids.is_empty() {
                    __kids.remove(0)
                } else {
                    SyntaxTree::Unknown {
                        kind: "from_json:missing_receiver".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    }
                };
                AccessReceiver::from_tree(inner, &["this", "self", "super", "base"])
            };
            let segments = { let _ = &mut __kids; Vec::new() };
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
            let mut __kids = children;
            let op_text = map.get("op_text").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let op_marker = static_tag;
            let op_range = ByteRange::synthetic_empty();
            let left = {
                if let Some(v) = map.get("left") {
                    Box::new(tree_from_json_with_type_hint(v, Some("left")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let right = {
                if let Some(v) = map.get("right") {
                    Box::new(tree_from_json_with_type_hint(v, Some("right")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
                op_text,
                op_marker,
                op_range,
                left,
                right,
                range,
                span,
            }
        }
        "logical" => {
            let mut __kids = children;
            let op_text = map.get("op_text").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let op_marker = static_tag;
            let op_range = ByteRange::synthetic_empty();
            let left = {
                if let Some(v) = map.get("left") {
                    Box::new(tree_from_json_with_type_hint(v, Some("left")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let right = {
                if let Some(v) = map.get("right") {
                    Box::new(tree_from_json_with_type_hint(v, Some("right")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
                op_text,
                op_marker,
                op_range,
                left,
                right,
                range,
                span,
            }
        }
        "unary" => {
            let mut __kids = children;
            let op_text = map.get("op_text").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let op_marker = static_tag;
            let op_range = ByteRange::synthetic_empty();
            let operand = {
                if let Some(v) = map.get("operand") {
                    Box::new(tree_from_json_with_type_hint(v, Some("operand")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let children = {
                let singular = strip_plural("children");
                if let Some(v) = map.get("children").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
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
            let mut __kids = children;
            let children = {
                let singular = strip_plural("children");
                if let Some(v) = map.get("children").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
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
            let mut __kids = children;
            let children = {
                let singular = strip_plural("children");
                if let Some(v) = map.get("children").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
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
            let mut __kids = children;
            let pairs = {
                let singular = strip_plural("pairs");
                if let Some(v) = map.get("pairs").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
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
            let mut __kids = children;
            let key = {
                if let Some(v) = map.get("key") {
                    Box::new(tree_from_json_with_type_hint(v, Some("key")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let value = {
                if let Some(v) = map.get("value") {
                    Box::new(tree_from_json_with_type_hint(v, Some("value")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
                if let Some(v) = map.get("params").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
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
            let mut __kids = children;
            let left = {
                if let Some(v) = map.get("left") {
                    Box::new(tree_from_json_with_type_hint(v, Some("left")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
                if let Some(v) = map.get("right") {
                    Box::new(tree_from_json_with_type_hint(v, Some("right")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let condition = {
                if let Some(v) = map.get("condition") {
                    Box::new(tree_from_json_with_type_hint(v, Some("condition")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let body = {
                if let Some(v) = map.get("body") {
                    Box::new(tree_from_json_with_type_hint(v, Some("body")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let else_branch = {
                if let Some(v) = map.get("else_branch") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("else_branch"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
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
            let mut __kids = children;
            let condition = {
                if let Some(v) = map.get("condition") {
                    Box::new(tree_from_json_with_type_hint(v, Some("condition")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let body = {
                if let Some(v) = map.get("body") {
                    Box::new(tree_from_json_with_type_hint(v, Some("body")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let else_branch = {
                if let Some(v) = map.get("else_branch") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("else_branch"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
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
            let mut __kids = children;
            let body = {
                if let Some(v) = map.get("body") {
                    Box::new(tree_from_json_with_type_hint(v, Some("body")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let is_async = map.get("is_async").and_then(|v| v.as_bool()).unwrap_or(false);
            let targets = {
                let singular = strip_plural("targets");
                if let Some(v) = map.get("targets").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let iterables = {
                let singular = strip_plural("iterables");
                if let Some(v) = map.get("iterables").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let body = {
                if let Some(v) = map.get("body") {
                    Box::new(tree_from_json_with_type_hint(v, Some("body")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let else_body = {
                if let Some(v) = map.get("else_body") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("else_body"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
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
            let mut __kids = children;
            let condition = {
                if let Some(v) = map.get("condition") {
                    Box::new(tree_from_json_with_type_hint(v, Some("condition")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let body = {
                if let Some(v) = map.get("body") {
                    Box::new(tree_from_json_with_type_hint(v, Some("body")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let else_body = {
                if let Some(v) = map.get("else_body") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("else_body"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
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
            let mut __kids = children;
            let type_ann = {
                if let Some(v) = map.get("type_ann") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("type_ann"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let target = {
                if let Some(v) = map.get("target") {
                    Box::new(tree_from_json_with_type_hint(v, Some("target")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let iterable = {
                if let Some(v) = map.get("iterable") {
                    Box::new(tree_from_json_with_type_hint(v, Some("iterable")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let body = {
                if let Some(v) = map.get("body") {
                    Box::new(tree_from_json_with_type_hint(v, Some("body")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let initializer = {
                if let Some(v) = map.get("initializer") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("initializer"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let condition = {
                if let Some(v) = map.get("condition") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("condition"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let updates = {
                let singular = strip_plural("updates");
                if let Some(v) = map.get("updates").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let body = {
                if let Some(v) = map.get("body") {
                    Box::new(tree_from_json_with_type_hint(v, Some("body")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let body = {
                if let Some(v) = map.get("body") {
                    Box::new(tree_from_json_with_type_hint(v, Some("body")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let condition = {
                if let Some(v) = map.get("condition") {
                    Box::new(tree_from_json_with_type_hint(v, Some("condition")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Break {
                range,
                span,
            }
        }
        "continue" => {
            let mut __kids = children;
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Continue {
                range,
                span,
            }
        }
        "field_wrap" => {
            let mut __kids = children;
            let wrapper = static_tag;
            let inner = {
                if let Some(v) = map.get("inner") {
                    Box::new(tree_from_json_with_type_hint(v, Some("inner")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let element_name = static_tag;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let extra_markers = Vec::new();
            let children = {
                let singular = strip_plural("children");
                if let Some(v) = map.get("children").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
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
            let mut __kids = children;
            let try_body = {
                if let Some(v) = map.get("try_body") {
                    Box::new(tree_from_json_with_type_hint(v, Some("try_body")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
                if let Some(v) = map.get("handlers").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let else_body = {
                if let Some(v) = map.get("else_body") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("else_body"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let finally_body = {
                if let Some(v) = map.get("finally_body") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("finally_body"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
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
            let mut __kids = children;
            let type_target = {
                if let Some(v) = map.get("type_target") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("type_target"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let binding = {
                if let Some(v) = map.get("binding") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("binding"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let filter = {
                if let Some(v) = map.get("filter") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("filter"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let body = {
                if let Some(v) = map.get("body") {
                    Box::new(tree_from_json_with_type_hint(v, Some("body")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let type_target = {
                if let Some(v) = map.get("type_target") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("type_target"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let binding = {
                if let Some(v) = map.get("binding") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("binding"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let filter = {
                if let Some(v) = map.get("filter") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("filter"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let body = {
                if let Some(v) = map.get("body") {
                    Box::new(tree_from_json_with_type_hint(v, Some("body")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let type_params = {
                if let Some(v) = map.get("type_params") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("type_params"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let value = {
                if let Some(v) = map.get("value") {
                    Box::new(tree_from_json_with_type_hint(v, Some("value")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let value = {
                if let Some(v) = map.get("value") {
                    Box::new(tree_from_json_with_type_hint(v, Some("value")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let inner = {
                if let Some(v) = map.get("inner") {
                    Box::new(tree_from_json_with_type_hint(v, Some("inner")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let inner = {
                if let Some(v) = map.get("inner") {
                    Box::new(tree_from_json_with_type_hint(v, Some("inner")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let condition = {
                if let Some(v) = map.get("condition") {
                    Box::new(tree_from_json_with_type_hint(v, Some("condition")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let if_true = {
                if let Some(v) = map.get("if_true") {
                    Box::new(tree_from_json_with_type_hint(v, Some("if_true")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let if_false = {
                if let Some(v) = map.get("if_false") {
                    Box::new(tree_from_json_with_type_hint(v, Some("if_false")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let type_target = {
                if let Some(v) = map.get("type_target") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("type_target"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let arguments = {
                let singular = strip_plural("arguments");
                if let Some(v) = map.get("arguments").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let initializer = {
                if let Some(v) = map.get("initializer") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("initializer"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
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
            let mut __kids = children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let parameters = {
                let singular = strip_plural("parameters");
                if let Some(v) = map.get("parameters").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let body = {
                let inner = if let Some(v) = map.get("body").or_else(|| map.get("body")) {
                    tree_from_json_with_type_hint(v, Some("body"))
                } else if !__kids.is_empty() {
                    __kids.remove(0)
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
            let mut __kids = children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                if let Some(v) = map.get("decorators").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
                if let Some(v) = map.get("generics").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let parameters = {
                let singular = strip_plural("parameters");
                if let Some(v) = map.get("parameters").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let returns = {
                if let Some(v) = map.get("returns") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("returns"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let throws = {
                let singular = strip_plural("throws");
                if let Some(v) = map.get("throws").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let body = {
                if let Some(v) = map.get("body") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("body"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
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
            let mut __kids = children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                if let Some(v) = map.get("decorators").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
                if let Some(v) = map.get("generics").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let parameters = {
                let singular = strip_plural("parameters");
                if let Some(v) = map.get("parameters").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let returns = {
                if let Some(v) = map.get("returns") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("returns"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let throws = {
                let singular = strip_plural("throws");
                if let Some(v) = map.get("throws").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let body = {
                if let Some(v) = map.get("body") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("body"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
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
            let mut __kids = children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                if let Some(v) = map.get("decorators").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
                if let Some(v) = map.get("generics").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let bases = {
                let singular = strip_plural("bases");
                if let Some(v) = map.get("bases").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let where_clauses = {
                let singular = strip_plural("where_clauses");
                if let Some(v) = map.get("where_clauses").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let body = {
                if let Some(v) = map.get("body") {
                    Box::new(tree_from_json_with_type_hint(v, Some("body")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                if let Some(v) = map.get("decorators").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
                if let Some(v) = map.get("generics").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let bases = {
                let singular = strip_plural("bases");
                if let Some(v) = map.get("bases").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let where_clauses = {
                let singular = strip_plural("where_clauses");
                if let Some(v) = map.get("where_clauses").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let body = {
                if let Some(v) = map.get("body") {
                    Box::new(tree_from_json_with_type_hint(v, Some("body")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                if let Some(v) = map.get("decorators").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
                if let Some(v) = map.get("generics").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let bases = {
                let singular = strip_plural("bases");
                if let Some(v) = map.get("bases").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let where_clauses = {
                let singular = strip_plural("where_clauses");
                if let Some(v) = map.get("where_clauses").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let body = {
                if let Some(v) = map.get("body") {
                    Box::new(tree_from_json_with_type_hint(v, Some("body")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                if let Some(v) = map.get("decorators").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
                if let Some(v) = map.get("generics").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let bases = {
                let singular = strip_plural("bases");
                if let Some(v) = map.get("bases").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let where_clauses = {
                let singular = strip_plural("where_clauses");
                if let Some(v) = map.get("where_clauses").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let body = {
                if let Some(v) = map.get("body") {
                    Box::new(tree_from_json_with_type_hint(v, Some("body")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let children = {
                let singular = strip_plural("children");
                if let Some(v) = map.get("children").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
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
            let mut __kids = children;
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
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let type_ann = {
                if let Some(v) = map.get("type_ann") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("type_ann"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let default = {
                if let Some(v) = map.get("default") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("default"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
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
            let mut __kids = children;
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::PositionalSeparator {
                range,
                span,
            }
        }
        "keyword_separator" => {
            let mut __kids = children;
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::KeywordSeparator {
                range,
                span,
            }
        }
        "decorator" => {
            let mut __kids = children;
            let inner = {
                if let Some(v) = map.get("inner") {
                    Box::new(tree_from_json_with_type_hint(v, Some("inner")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let type_ann = {
                if let Some(v) = map.get("type_ann") {
                    Box::new(tree_from_json_with_type_hint(v, Some("type_ann")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let items = {
                let singular = strip_plural("items");
                if let Some(v) = map.get("items").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
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
            let mut __kids = children;
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let constraint = {
                if let Some(v) = map.get("constraint") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("constraint"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
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
            let mut __kids = children;
            let value = {
                if let Some(v) = map.get("value") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("value"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
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
            let mut __kids = children;
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
            let mut __kids = children;
            let targets = {
                let singular = strip_plural("targets");
                if let Some(v) = map.get("targets").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let type_annotation = {
                if let Some(v) = map.get("type_annotation") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("type_annotation"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let op_text = map.get("op_text").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let op_range = ByteRange::synthetic_empty();
            let op_markers = Vec::new();
            let values = {
                let singular = strip_plural("values");
                if let Some(v) = map.get("values").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
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
            let mut __kids = children;
            let has_alias = map.get("has_alias").and_then(|v| v.as_bool()).unwrap_or(false);
            let children = {
                let singular = strip_plural("children");
                if let Some(v) = map.get("children").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
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
            let mut __kids = children;
            let relative = map.get("relative").and_then(|v| v.as_bool()).unwrap_or(false);
            let path = {
                if let Some(v) = map.get("path") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("path"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let imports = {
                let singular = strip_plural("imports");
                if let Some(v) = map.get("imports").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
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
            let mut __kids = children;
            let has_alias = map.get("has_alias").and_then(|v| v.as_bool()).unwrap_or(false);
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let alias = {
                if let Some(v) = map.get("alias") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("alias"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
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
            let mut __kids = children;
            let segments = {
                let singular = strip_plural("segments");
                if let Some(v) = map.get("segments").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
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
            let mut __kids = children;
            let inner = {
                if let Some(v) = map.get("inner") {
                    Box::new(tree_from_json_with_type_hint(v, Some("inner")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let callee = {
                if let Some(v) = map.get("callee") {
                    Box::new(tree_from_json_with_type_hint(v, Some("callee")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
                if let Some(v) = map.get("arguments").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
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
            let mut __kids = children;
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
            let mut __kids = children;
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
            let mut __kids = children;
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
            let mut __kids = children;
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
            let mut __kids = children;
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
            let mut __kids = children;
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
            let mut __kids = children;
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
            let mut __kids = children;
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
            let mut __kids = children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                if let Some(v) = map.get("decorators").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let underlying_type = {
                if let Some(v) = map.get("underlying_type") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("underlying_type"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let members = {
                let singular = strip_plural("members");
                if let Some(v) = map.get("members").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
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
            let mut __kids = children;
            let decorators = {
                let singular = strip_plural("decorators");
                if let Some(v) = map.get("decorators").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let value = {
                if let Some(v) = map.get("value") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("value"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
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
            let mut __kids = children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                if let Some(v) = map.get("decorators").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let type_ann = {
                if let Some(v) = map.get("type_ann") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("type_ann"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
                if let Some(v) = map.get("accessors").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let value = {
                if let Some(v) = map.get("value") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("value"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
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
            let mut __kids = children;
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
                if let Some(v) = map.get("body") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("body"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
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
            let mut __kids = children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                if let Some(v) = map.get("decorators").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
                if let Some(v) = map.get("parameters").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let body = {
                if let Some(v) = map.get("body") {
                    Box::new(tree_from_json_with_type_hint(v, Some("body")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let is_static = map.get("is_static").and_then(|v| v.as_bool()).unwrap_or(false);
            let alias = {
                if let Some(v) = map.get("alias") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("alias"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let path = {
                if let Some(v) = map.get("path") {
                    Box::new(tree_from_json_with_type_hint(v, Some("path")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
                if let Some(v) = map.get("children").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
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
            let mut __kids = children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                if let Some(v) = map.get("decorators").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let type_ann = {
                if let Some(v) = map.get("type_ann") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("type_ann"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
                } else if !__kids.is_empty() {
                    Some(Expression::wrap(__kids.remove(0)))
                } else { None }
            };
            let range = ByteRange::synthetic_empty();
            let span = Span::point(0, 0);
            SyntaxTree::Variable {
                modifiers,
                decorators,
                type_ann,
                name,
                value,
                range,
                span,
            }
        }
        "field" => {
            let mut __kids = children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                if let Some(v) = map.get("decorators").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let type_ann = {
                if let Some(v) = map.get("type_ann") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("type_ann"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
                } else if !__kids.is_empty() {
                    Some(Expression::wrap(__kids.remove(0)))
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
            let mut __kids = children;
            let modifiers = Modifiers::from_marker_names(&marker_strs);
            let decorators = {
                let singular = strip_plural("decorators");
                if let Some(v) = map.get("decorators").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
                }
            };
            let type_ann = {
                if let Some(v) = map.get("type_ann") {
                    Some(Box::new(tree_from_json_with_type_hint(v, Some("type_ann"))))
                } else if !__kids.is_empty() {
                    Some(Box::new(__kids.remove(0)))
                } else { None }
            };
            let name = {
                if let Some(v) = map.get("name") {
                    Box::new(tree_from_json_with_type_hint(v, Some("name")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
                } else if !__kids.is_empty() {
                    Some(Expression::wrap(__kids.remove(0)))
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
            let mut __kids = children;
            let value = {
                if let Some(v) = map.get("value") {
                    Box::new(tree_from_json_with_type_hint(v, Some("value")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let type_target = {
                if let Some(v) = map.get("type_target") {
                    Box::new(tree_from_json_with_type_hint(v, Some("type_target")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
            let type_ann = {
                if let Some(v) = map.get("type_ann") {
                    Box::new(tree_from_json_with_type_hint(v, Some("type_ann")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
                } else {
                    Box::new(SyntaxTree::Unknown {
                        kind: "from_json:missing_child".into(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(0, 0),
                    })
                }
            };
            let value = {
                if let Some(v) = map.get("value") {
                    Box::new(tree_from_json_with_type_hint(v, Some("value")))
                } else if !__kids.is_empty() {
                    Box::new(__kids.remove(0))
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
            let mut __kids = children;
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
            let mut __kids = children;
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
            let mut __kids = children;
            let kind = map.get("kind").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_default();
            let is_named = map.get("is_named").and_then(|v| v.as_bool()).unwrap_or(false);
            let children = {
                let singular = strip_plural("children");
                if let Some(v) = map.get("children").or_else(|| map.get(singular)) {
                    match v {
                        Value::Array(arr) => arr.iter()
                            .map(|c| tree_from_json_with_type_hint(c, Some(singular)))
                            .collect(),
                        _ => vec![tree_from_json_with_type_hint(v, Some(singular))],
                    }
                } else {
                    std::mem::take(&mut __kids)
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
        _ => SyntaxTree::Unknown {
            kind: format!("from_json:{}", tag),
            range: ByteRange::synthetic_empty(),
            span: Span::point(0, 0),
        },
    }
}
