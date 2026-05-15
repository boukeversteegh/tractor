// DO NOT EDIT — emitted by `tractor/build.rs` on every build.
// Source: SyntaxTree enum in tractor/src/tree/syntax/types.rs.
//
// Variant-blind reflection metadata that drives the XML and JSON
// renderers' mechanical walks (`to_xot.rs`, `to_json.rs`). Rules
// are derived from field types only — no per-variant special cases.
// See `tractor/build_codegen.rs`.

#![cfg(feature = "native")]

#[allow(unused_imports)]
use super::types::{
    AccessReceiver, AccessSegment, ByteRange, Flag, Marker, Span, SyntaxTree,
};

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

/// Source byte range of this node. Used for verbatim-source
/// recovery (`source[range]`) and for gap-text computation in the
/// renderer. Every variant carries a `range: ByteRange` field.
pub fn range_of(tree: &SyntaxTree) -> ByteRange {
    match tree {
        SyntaxTree::Module { range, .. } => *range,
        SyntaxTree::Expression { range, .. } => *range,
        SyntaxTree::Access { range, .. } => *range,
        SyntaxTree::Binary { range, .. } => *range,
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
        SyntaxTree::ExceptHandler { range, .. } => *range,
        SyntaxTree::TypeAlias { range, .. } => *range,
        SyntaxTree::KeywordArgument { range, .. } => *range,
        SyntaxTree::ListSplat { range, .. } => *range,
        SyntaxTree::DictSplat { range, .. } => *range,
        SyntaxTree::Ternary { range, .. } => *range,
        SyntaxTree::ObjectCreation { range, .. } => *range,
        SyntaxTree::Lambda { range, .. } => *range,
        SyntaxTree::Function { range, .. } => *range,
        SyntaxTree::Class { range, .. } => *range,
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
        SyntaxTree::Access { span, .. } => *span,
        SyntaxTree::Binary { span, .. } => *span,
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
        SyntaxTree::ExceptHandler { span, .. } => *span,
        SyntaxTree::TypeAlias { span, .. } => *span,
        SyntaxTree::KeywordArgument { span, .. } => *span,
        SyntaxTree::ListSplat { span, .. } => *span,
        SyntaxTree::DictSplat { span, .. } => *span,
        SyntaxTree::Ternary { span, .. } => *span,
        SyntaxTree::ObjectCreation { span, .. } => *span,
        SyntaxTree::Lambda { span, .. } => *span,
        SyntaxTree::Function { span, .. } => *span,
        SyntaxTree::Class { span, .. } => *span,
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
        SyntaxTree::Access { receiver, segments, .. } => {
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
        SyntaxTree::ExceptHandler { type_target, binding, filter, body, .. } => {
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
        SyntaxTree::Class { decorators, name, generics, bases, where_clauses, body, .. } => {
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
