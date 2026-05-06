//! IR → JSON renderer. Skips the XML intermediate.
//!
//! Walks the typed `Ir` tree directly and produces a `serde_json::Value`
//! whose shape matches what the XML→JSON projection (`xml_to_json.rs`)
//! would produce, but without going through Xot or `XmlNode`. The IR
//! is the source of truth: list-cardinality decisions come from
//! `Vec<Ir>` vs `Box<Ir>` field shapes, marker flags come from
//! `Modifiers::marker_names()` and per-variant `extra_markers`.
//!
//! ## Output shape (kept compatible with `xml_to_json.rs` for snapshot
//! parity)
//!
//! - `$type`: element name (omitted when the parent's chosen JSON key
//!   already conveys the type — list entries under a plural key, or
//!   singleton entries under their own name).
//! - Self-closing markers / modifier flags → `"name": true`.
//! - Multiple same-keyed children → array under the plural key
//!   (`methods: [...]`).
//! - Singleton structural children → keyed by element name
//!   (`body: {...}`).
//! - Text-only leaves → scalar string under the parent's key.
//! - Same-name siblings without a list discriminator promote to
//!   `$children: [...]` (Principle #19 escape hatch).
//!
//! ## Why skip XML
//!
//! The IR already encodes every projection decision (Vec → array, Box
//! → singleton, modifiers → flags). Routing through XML adds an
//! intermediate `list="X"` attribute step that's purely a serializer
//! affordance for the XML-driven JSON projector — and it costs us
//! flexibility (the XML attribute namespace clutters queries and
//! pre-supposes a particular plural-name spelling). Going direct lets
//! the IR define the JSON contract without that detour.

use serde_json::{Map, Value};

use crate::ir::types::{AccessSegment, Ir, Modifiers, ParamKind};
use crate::transform::helpers::pluralize_list_name;

const KEY_TYPE: &str = "$type";
const KEY_CHILDREN: &str = "$children";
const KEY_TEXT: &str = "text";

/// Top-level entry: convert an IR tree to a JSON value. The root is
/// emitted with its `$type` (no parent context to strip it).
pub fn ir_to_json(ir: &Ir, source: &str) -> Value {
    Renderer::new(source).render_root(ir)
}

struct Renderer<'a> {
    source: &'a str,
}

impl<'a> Renderer<'a> {
    fn new(source: &'a str) -> Self {
        Self { source }
    }

    fn render_root(&self, ir: &Ir) -> Value {
        // Roots keep their $type — nothing above them sets a key.
        self.render(ir, /*strip_type=*/ false)
    }

    /// Render an IR node. `strip_type` is true when the parent's
    /// chosen key already conveys the type (list entry under plural
    /// key, or singleton under its own element name) — matches the
    /// XML→JSON `strip_top_level_type` behaviour.
    fn render(&self, ir: &Ir, strip_type: bool) -> Value {
        match self.try_render_scalar(ir) {
            Some(scalar) => scalar,
            None => {
                let mut shape = Shape::new(self.element_name(ir));
                self.populate(ir, &mut shape);
                shape.into_value(strip_type)
            }
        }
    }

    /// Some IR nodes naturally render as scalars (Name → string,
    /// integer-literal → number, true/false → boolean, null → null).
    /// `xml_to_json.rs` collapses text-only-leaf elements to strings;
    /// we do the same here at render time.
    fn try_render_scalar(&self, ir: &Ir) -> Option<Value> {
        let text_scalar = |ir: &Ir| {
            let text = ir.range().slice(self.source);
            Value::String(text.to_string())
        };
        match ir {
            Ir::Name { .. }
            | Ir::Int { .. }
            | Ir::Float { .. }
            | Ir::String { .. }
            | Ir::None { .. }
            | Ir::Null { .. }
            | Ir::True { .. }
            | Ir::False { .. } => Some(text_scalar(ir)),
            Ir::PositionalSeparator { .. } | Ir::KeywordSeparator { .. } => {
                // Markers; rendered as flags by parents. Render as null
                // when reached as a value.
                Some(Value::Null)
            }
            _ => None,
        }
    }

    /// Element name (matches the XML element name for the same IR node).
    /// Used as the JSON `$type` and as the key when this node sits in
    /// its parent as a singleton or list entry.
    fn element_name(&self, ir: &Ir) -> &'static str {
        match ir {
            Ir::Module { element_name, .. } => element_name,
            Ir::Expression { .. } => "expression",
            Ir::Access { .. } => "object",
            Ir::Binary { element_name, .. } => element_name,
            Ir::Unary { .. } => "unary",
            Ir::Tuple { .. } => "tuple",
            Ir::List { .. } => "list",
            Ir::Set { .. } => "set",
            Ir::Dictionary { .. } => "dict",
            Ir::Pair { .. } => "pair",
            Ir::GenericType { .. } => "type",
            Ir::Comparison { .. } => "compare",
            Ir::If { .. } => "if",
            Ir::ElseIf { .. } => "else_if",
            Ir::Else { .. } => "else",
            Ir::For { .. } => "for",
            Ir::While { .. } => "while",
            Ir::Foreach { .. } => "foreach",
            Ir::CFor { .. } => "for",
            Ir::DoWhile { .. } => "do",
            Ir::Break { .. } => "break",
            Ir::Continue { .. } => "continue",
            Ir::Lambda { .. } => "lambda",
            Ir::ObjectCreation { .. } => "new",
            Ir::Ternary { .. } => "ternary",
            Ir::FieldWrap { wrapper, .. } => wrapper,
            Ir::SimpleStatement { element_name, .. } => element_name,
            Ir::Try { .. } => "try",
            Ir::ExceptHandler { .. } => "catch",
            Ir::TypeAlias { .. } => "type_alias",
            Ir::KeywordArgument { .. } => "keyword_argument",
            Ir::ListSplat { .. } => "spread",
            Ir::DictSplat { .. } => "spread",
            Ir::Function { element_name, .. } => element_name,
            Ir::Class { kind, .. } => kind,
            Ir::Body { .. } => "body",
            Ir::Parameter { .. } => "parameter",
            Ir::PositionalSeparator { .. } => "positional",
            Ir::KeywordSeparator { .. } => "keyword",
            Ir::Decorator { .. } => "decorator",
            Ir::Returns { .. } => "returns",
            Ir::Generic { .. } => "generic",
            Ir::TypeParameter { .. } => "type",
            Ir::Return { .. } => "return",
            Ir::Comment { .. } => "comment",
            Ir::Assign { .. } => "assign",
            Ir::Import { .. } => "import",
            Ir::From { .. } => "from",
            Ir::FromImport { .. } => "import",
            Ir::Path { .. } => "path",
            Ir::Aliased { .. } => "aliased",
            Ir::Name { .. } => "name",
            Ir::Int { .. } => "int",
            Ir::Float { .. } => "float",
            Ir::String { .. } => "string",
            Ir::True { .. } => "true",
            Ir::False { .. } => "false",
            Ir::None { .. } => "none",
            Ir::Enum { .. } => "enum",
            Ir::EnumMember { .. } => "constant",
            Ir::Property { .. } => "property",
            Ir::Accessor { kind, .. } => kind,
            Ir::Constructor { .. } => "constructor",
            Ir::Using { .. } => "using",
            Ir::Namespace { .. } => "namespace",
            Ir::Variable { element_name, .. } => element_name,
            Ir::Is { .. } => "is",
            Ir::Cast { .. } => "cast",
            Ir::Null { .. } => "null",
            Ir::Inline { .. } => "$inline",
            Ir::Unknown { .. } => "unknown",
            Ir::Call { .. } => "call",
        }
    }

    /// Populate the shape with the IR's flags + child entries.
    fn populate(&self, ir: &Ir, shape: &mut Shape) {
        match ir {
            Ir::Module { children, .. } => {
                self.add_children(shape, children);
            }
            Ir::Expression { inner, marker, .. } => {
                if let Some(m) = marker {
                    shape.flag(m);
                }
                self.add_singleton_or_text(shape, inner);
            }
            Ir::Access { receiver, segments, .. } => {
                self.add_access_chain(shape, receiver, segments);
            }
            Ir::Binary { left, op_text, op_marker, right, .. } => {
                shape.singleton("left", self.wrap_expression_host(left));
                shape.singleton("op", self.op_value(op_text, op_marker));
                shape.singleton("right", self.wrap_expression_host(right));
            }
            Ir::Unary { op_text, op_marker, operand, extra_markers, .. } => {
                for m in *extra_markers {
                    shape.flag(m);
                }
                shape.singleton("op", self.op_value(op_text, op_marker));
                self.add_singleton_or_text(shape, operand);
            }
            Ir::Tuple { children, .. }
            | Ir::List { children, .. }
            | Ir::Set { children, .. } => {
                self.add_children(shape, children);
            }
            Ir::Dictionary { pairs, .. } => {
                self.add_children(shape, pairs);
            }
            Ir::Pair { key, value, .. } => {
                shape.singleton("key", self.render(key, true));
                shape.singleton("value", self.render(value, true));
            }
            Ir::GenericType { name, params, .. } => {
                shape.flag("generic");
                self.add_singleton_or_text(shape, name);
                for p in params {
                    shape.list_with("type", self.render(p, true));
                }
            }
            Ir::Comparison { left, op_text, op_marker, right, .. } => {
                shape.singleton("left", self.wrap_expression_host(left));
                shape.singleton("op", self.op_value(op_text, op_marker));
                shape.singleton("right", self.wrap_expression_host(right));
            }
            Ir::If { condition, body, else_branch, .. } => {
                shape.singleton("condition", self.wrap_expression_host(condition));
                shape.singleton("body", self.render(body, true));
                if let Some(e) = else_branch {
                    self.add_else_chain(shape, e);
                }
            }
            Ir::ElseIf { condition, body, else_branch, .. } => {
                shape.singleton("condition", self.wrap_expression_host(condition));
                shape.singleton("body", self.render(body, true));
                if let Some(e) = else_branch {
                    self.add_else_chain(shape, e);
                }
            }
            Ir::Else { body, .. } => {
                shape.singleton("body", self.render(body, true));
            }
            Ir::For { is_async, targets, iterables, body, else_body, .. } => {
                if *is_async {
                    shape.flag("async");
                }
                if targets.len() == 1 {
                    shape.singleton("left", self.wrap_expression_host(&targets[0]));
                } else {
                    let arr: Vec<Value> = targets.iter().map(|t| self.wrap_expression_host(t)).collect();
                    shape.put("lefts", Value::Array(arr));
                }
                if iterables.len() == 1 {
                    shape.singleton("right", self.wrap_expression_host(&iterables[0]));
                } else {
                    let arr: Vec<Value> = iterables.iter().map(|i| self.wrap_expression_host(i)).collect();
                    shape.put("rights", Value::Array(arr));
                }
                shape.singleton("body", self.render(body, true));
                if let Some(e) = else_body {
                    shape.singleton("else", self.render(e, true));
                }
            }
            Ir::While { condition, body, else_body, .. } => {
                shape.singleton("condition", self.wrap_expression_host(condition));
                shape.singleton("body", self.render(body, true));
                if let Some(e) = else_body {
                    shape.singleton("else", self.render(e, true));
                }
            }
            Ir::Foreach { type_ann, target, iterable, body, .. } => {
                shape.flag("in");
                if let Some(t) = type_ann {
                    shape.singleton("type", self.render_as_type(t));
                }
                shape.singleton("left", self.wrap_expression_host(target));
                shape.singleton("right", self.wrap_expression_host(iterable));
                shape.singleton("body", self.render(body, true));
            }
            Ir::CFor { initializer, condition, updates, body, .. } => {
                if let Some(i) = initializer {
                    shape.list_with(self.element_name(i), self.render(i, true));
                }
                if let Some(c) = condition {
                    shape.singleton("condition", self.wrap_expression_host(c));
                }
                for u in updates {
                    shape.list_with(self.element_name(u), self.render(u, true));
                }
                shape.singleton("body", self.render(body, true));
            }
            Ir::DoWhile { body, condition, .. } => {
                shape.singleton("body", self.render(body, true));
                shape.singleton("condition", self.wrap_expression_host(condition));
            }
            Ir::Break { .. } | Ir::Continue { .. } => {}
            Ir::Lambda { parameters, body, .. } => {
                for p in parameters {
                    shape.list_with("parameter", self.render(p, true));
                }
                shape.singleton("body", self.render(body, true));
            }
            Ir::ObjectCreation { type_target, arguments, initializer, .. } => {
                if let Some(t) = type_target {
                    self.add_singleton_or_text(shape, t);
                }
                for a in arguments {
                    shape.list_with(self.element_name(a), self.render(a, true));
                }
                if let Some(i) = initializer {
                    shape.singleton("literal", self.render(i, true));
                }
            }
            Ir::Ternary { condition, if_true, if_false, .. } => {
                shape.singleton("condition", self.wrap_expression_host(condition));
                shape.singleton("then", self.wrap_expression_host(if_true));
                shape.singleton("else", self.wrap_expression_host(if_false));
            }
            Ir::FieldWrap { inner, .. } => {
                self.add_singleton_or_text(shape, inner);
            }
            Ir::SimpleStatement { children, modifiers, extra_markers, .. } => {
                self.add_modifier_flags(shape, modifiers);
                for m in *extra_markers {
                    shape.flag(m);
                }
                self.add_children(shape, children);
            }
            Ir::Try { try_body, handlers, else_body, finally_body, .. } => {
                shape.singleton("body", self.render(try_body, true));
                for h in handlers {
                    shape.list_with("catch", self.render(h, true));
                }
                if let Some(e) = else_body {
                    shape.singleton("else", self.render(e, true));
                }
                if let Some(f) = finally_body {
                    shape.singleton("finally", self.render(f, true));
                }
            }
            Ir::ExceptHandler { type_target, binding, filter, body, .. } => {
                if let Some(t) = type_target {
                    shape.singleton("type", self.render_as_type(t));
                }
                if let Some(b) = binding {
                    self.add_singleton_or_text(shape, b);
                }
                if let Some(f) = filter {
                    shape.singleton("when", self.wrap_expression_host(f));
                }
                shape.singleton("body", self.render(body, true));
            }
            Ir::TypeAlias { name, type_params, value, .. } => {
                self.add_singleton_or_text(shape, name);
                if let Some(p) = type_params {
                    self.add_singleton_or_text(shape, p);
                }
                shape.singleton("value", self.wrap_expression_host(value));
            }
            Ir::KeywordArgument { name, value, .. } => {
                self.add_singleton_or_text(shape, name);
                shape.singleton("value", self.wrap_expression_host(value));
            }
            Ir::ListSplat { inner, .. } | Ir::DictSplat { inner, .. } => {
                self.add_singleton_or_text(shape, inner);
            }
            Ir::Function {
                modifiers, decorators, name, generics, parameters, returns, body, ..
            } => {
                for d in decorators {
                    shape.list_with("attribute", self.render(d, true));
                }
                self.add_modifier_flags(shape, modifiers);
                self.add_singleton_or_text(shape, name);
                if let Some(g) = generics {
                    self.add_generics(shape, g);
                }
                for p in parameters {
                    shape.list_with("parameter", self.render(p, true));
                }
                if let Some(r) = returns {
                    shape.singleton("returns", self.render(r, true));
                }
                if let Some(b) = body {
                    shape.singleton("body", self.render(b, true));
                }
            }
            Ir::Class {
                modifiers, decorators, name, generics, bases, where_clauses: _, body, ..
            } => {
                for d in decorators {
                    shape.list_with("attribute", self.render(d, true));
                }
                self.add_modifier_flags(shape, modifiers);
                self.add_singleton_or_text(shape, name);
                if let Some(g) = generics {
                    self.add_generics(shape, g);
                }
                for b in bases {
                    let inner = self.render_as_type(b);
                    let mut wrap = Map::new();
                    wrap.insert("type".into(), inner);
                    shape.list_with("extends", Value::Object(wrap));
                }
                shape.singleton("body", self.render(body, true));
            }
            Ir::Body { children, .. } => {
                self.add_children(shape, children);
            }
            Ir::Parameter { kind, extra_markers, name, type_ann, default, .. } => {
                match kind {
                    ParamKind::Args => shape.flag("args"),
                    ParamKind::Kwargs => shape.flag("kwargs"),
                    _ => {}
                }
                for m in *extra_markers {
                    shape.flag(m);
                }
                if let Some(t) = type_ann {
                    shape.singleton("type", self.render_as_type(t));
                }
                self.add_singleton_or_text(shape, name);
                if let Some(d) = default {
                    shape.singleton("value", self.wrap_expression_host(d));
                }
            }
            Ir::PositionalSeparator { .. } | Ir::KeywordSeparator { .. } => {}
            Ir::Decorator { inner, .. } => {
                self.add_singleton_or_text(shape, inner);
            }
            Ir::Returns { type_ann, .. } => {
                shape.singleton("type", self.render_as_type(type_ann));
            }
            Ir::Generic { items, .. } => {
                for it in items {
                    shape.list_with(self.element_name(it), self.render(it, true));
                }
            }
            Ir::TypeParameter { name, constraint, .. } => {
                self.add_singleton_or_text(shape, name);
                if let Some(c) = constraint {
                    self.add_singleton_or_text(shape, c);
                }
            }
            Ir::Return { value, .. } => {
                if let Some(v) = value {
                    shape.singleton("expression", self.wrap_expression_host(v));
                }
            }
            Ir::Comment { leading, trailing, range, .. } => {
                if *leading {
                    shape.flag("leading");
                }
                if *trailing {
                    shape.flag("trailing");
                }
                let text = range.slice(self.source).to_string();
                shape.text(text);
            }
            Ir::Assign { targets, type_annotation, op_text, op_markers, values, .. } => {
                let _ = op_text;
                for marker in op_markers.iter() {
                    shape.flag(marker);
                }
                let mut left_arr: Vec<Value> = Vec::new();
                for t in targets {
                    left_arr.push(self.wrap_expression_host(t));
                }
                if left_arr.len() == 1 {
                    shape.singleton("left", left_arr.into_iter().next().unwrap());
                } else if !left_arr.is_empty() {
                    let mut left_obj = Map::new();
                    left_obj.insert("$type".into(), Value::String("left".into()));
                    left_obj.insert("expressions".into(), Value::Array(left_arr));
                    shape.singleton("left", Value::Object(left_obj));
                }
                if let Some(ty) = type_annotation {
                    shape.singleton("type", self.render_as_type(ty));
                }
                let mut right_arr: Vec<Value> = Vec::new();
                for v in values {
                    right_arr.push(self.wrap_expression_host(v));
                }
                if right_arr.len() == 1 {
                    shape.singleton("right", right_arr.into_iter().next().unwrap());
                } else if !right_arr.is_empty() {
                    let mut right_obj = Map::new();
                    right_obj.insert("$type".into(), Value::String("right".into()));
                    right_obj.insert("expressions".into(), Value::Array(right_arr));
                    shape.singleton("right", Value::Object(right_obj));
                }
            }
            Ir::Import { children, .. } => {
                self.add_children(shape, children);
            }
            Ir::From { relative, path, imports, .. } => {
                if *relative {
                    shape.flag("relative");
                }
                if let Some(p) = path {
                    shape.singleton("path", self.render(p, true));
                }
                for it in imports {
                    shape.list_with(self.element_name(it), self.render(it, true));
                }
            }
            Ir::FromImport { has_alias, name, alias, .. } => {
                if *has_alias {
                    shape.flag("alias");
                }
                self.add_singleton_or_text(shape, name);
                if let Some(a) = alias {
                    shape.singleton("alias", self.render(a, true));
                }
            }
            Ir::Path { segments, .. } => {
                let arr: Vec<Value> = segments
                    .iter()
                    .map(|s| Value::String(s.range().slice(self.source).to_string()))
                    .collect();
                shape.put("names", Value::Array(arr));
            }
            Ir::Aliased { inner, .. } => {
                self.add_singleton_or_text(shape, inner);
            }
            Ir::Enum { modifiers, decorators, name, underlying_type, members, .. } => {
                for d in decorators {
                    shape.list_with("attribute", self.render(d, true));
                }
                self.add_modifier_flags(shape, modifiers);
                self.add_singleton_or_text(shape, name);
                if let Some(t) = underlying_type {
                    shape.singleton("type", self.render_as_type(t));
                    shape.flag("underlying");
                }
                for m in members {
                    shape.list_with("constant", self.render(m, true));
                }
            }
            Ir::EnumMember { name, value, .. } => {
                self.add_singleton_or_text(shape, name);
                if let Some(v) = value {
                    shape.singleton("value", self.wrap_expression_host(v));
                }
            }
            Ir::Property { modifiers, decorators, type_ann, name, accessors, value, .. } => {
                for d in decorators {
                    shape.list_with("attribute", self.render(d, true));
                }
                self.add_modifier_flags(shape, modifiers);
                if let Some(t) = type_ann {
                    shape.singleton("type", self.render_as_type(t));
                }
                self.add_singleton_or_text(shape, name);
                for a in accessors {
                    shape.list_with(self.element_name(a), self.render(a, true));
                }
                if let Some(v) = value {
                    shape.singleton("value", self.wrap_expression_host(v));
                }
            }
            Ir::Accessor { modifiers, body, .. } => {
                self.add_modifier_flags(shape, modifiers);
                if let Some(b) = body {
                    shape.singleton("body", self.render(b, true));
                }
            }
            Ir::Constructor { modifiers, decorators, name, parameters, body, .. } => {
                for d in decorators {
                    shape.list_with("attribute", self.render(d, true));
                }
                self.add_modifier_flags(shape, modifiers);
                self.add_singleton_or_text(shape, name);
                for p in parameters {
                    shape.list_with("parameter", self.render(p, true));
                }
                shape.singleton("body", self.render(body, true));
            }
            Ir::Using { is_static, alias, path, .. } => {
                if *is_static {
                    shape.flag("static");
                }
                if let Some(a) = alias {
                    shape.singleton("alias", self.render(a, true));
                }
                shape.singleton("path", self.render(path, true));
            }
            Ir::Namespace { file_scoped, name, children, .. } => {
                if *file_scoped {
                    shape.flag("file");
                }
                self.add_singleton_or_text(shape, name);
                self.add_children(shape, children);
            }
            Ir::Variable { modifiers, decorators, type_ann, name, value, .. } => {
                for d in decorators {
                    shape.list_with("attribute", self.render(d, true));
                }
                self.add_modifier_flags(shape, modifiers);
                if let Some(t) = type_ann {
                    shape.singleton("type", self.render_as_type(t));
                }
                self.add_singleton_or_text(shape, name);
                if let Some(v) = value {
                    shape.singleton("value", self.wrap_expression_host(v));
                }
            }
            Ir::Is { value, type_target, .. } => {
                shape.singleton("left", self.wrap_expression_host(value));
                let mut right_inner = Map::new();
                right_inner.insert("$type".into(), Value::String("expression".into()));
                right_inner.insert("type".into(), self.render_as_type(type_target));
                shape.singleton("right", Value::Object(right_inner));
            }
            Ir::Cast { type_ann, value, .. } => {
                shape.singleton("type", self.render_as_type(type_ann));
                shape.singleton("value", self.wrap_expression_host(value));
            }
            Ir::Inline { children, list_name, .. } => {
                if let Some(list) = list_name {
                    let arr: Vec<Value> = children
                        .iter()
                        .map(|c| self.render(c, true))
                        .collect();
                    shape.put(list, Value::Array(arr));
                } else {
                    self.add_children(shape, children);
                }
            }
            Ir::Unknown { .. } => {
                // Unknown is opaque; carry the source text so consumers
                // see what fell through.
                let text = ir.range().slice(self.source).to_string();
                if !text.is_empty() {
                    shape.text(text);
                }
            }
            Ir::Call { callee, arguments, .. } => {
                self.add_singleton_or_text(shape, callee);
                for a in arguments {
                    shape.list_with(self.element_name(a), self.render(a, true));
                }
            }
            // Scalar leaves are short-circuited in `try_render_scalar`
            // before reaching `populate` — guard the match exhaustively.
            Ir::Name { .. }
            | Ir::Int { .. }
            | Ir::Float { .. }
            | Ir::String { .. }
            | Ir::True { .. }
            | Ir::False { .. }
            | Ir::None { .. }
            | Ir::Null { .. } => {}
        }
    }

    /// `Ir::Access` chain rendering — preserves the right-nested
    /// `<object>` shape that `xml_to_json.rs` projects via list= /
    /// singleton rules. Each segment becomes a key on the previous
    /// segment's JSON object.
    fn add_access_chain(&self, shape: &mut Shape, receiver: &Ir, segments: &[AccessSegment]) {
        shape.flag("access");
        // Rendered right-nested in XML; the IR walks segments in
        // source order. For JSON we emit the receiver at the
        // outermost level, then each segment as a child key on the
        // accumulated object.
        let receiver_val = self.render(receiver, true);
        // Drop the wrapper object's $type when scalar
        match receiver_val {
            Value::String(s) => shape.text(s),
            Value::Object(map) => {
                for (k, v) in map {
                    shape.put(&k, v);
                }
            }
            other => shape.put("receiver", other),
        }
        for seg in segments {
            match seg {
                AccessSegment::Member { property_range, optional, .. } => {
                    let mut m = Map::new();
                    m.insert(KEY_TYPE.into(), Value::String("member".into()));
                    if *optional { m.insert("optional".into(), Value::Bool(true)); }
                    m.insert("name".into(), Value::String(property_range.slice(self.source).to_string()));
                    shape.list_with("member", Value::Object(m));
                }
                AccessSegment::Index { indices, .. } => {
                    let mut m = Map::new();
                    m.insert(KEY_TYPE.into(), Value::String("index".into()));
                    if indices.len() == 1 {
                        let v = self.render(&indices[0], true);
                        match v {
                            Value::String(s) => { m.insert("text".into(), Value::String(s)); }
                            Value::Object(inner) => { for (k, vv) in inner { m.insert(k, vv); } }
                            other => { m.insert("$children".into(), Value::Array(vec![other])); }
                        }
                    } else {
                        let arr: Vec<Value> = indices.iter().map(|i| self.render(i, true)).collect();
                        m.insert("arguments".into(), Value::Array(arr));
                    }
                    shape.list_with("index", Value::Object(m));
                }
                AccessSegment::Call { name, arguments, .. } => {
                    let mut m = Map::new();
                    m.insert(KEY_TYPE.into(), Value::String("call".into()));
                    if let Some(n) = name {
                        m.insert("name".into(), Value::String(n.slice(self.source).to_string()));
                    }
                    if !arguments.is_empty() {
                        let arr: Vec<Value> = arguments.iter().map(|a| self.render(a, true)).collect();
                        if arr.len() == 1 {
                            m.insert("argument".into(), arr.into_iter().next().unwrap());
                        } else {
                            m.insert("arguments".into(), Value::Array(arr));
                        }
                    }
                    shape.list_with("call", Value::Object(m));
                }
            }
        }
    }

    fn add_else_chain(&self, shape: &mut Shape, ir: &Ir) {
        match ir {
            Ir::ElseIf { .. } => {
                shape.list_with("else_if", self.render(ir, true));
            }
            Ir::Else { .. } => {
                shape.singleton("else", self.render(ir, true));
            }
            _ => {
                shape.singleton("else", self.render(ir, true));
            }
        }
    }

    fn add_generics(&self, shape: &mut Shape, generics: &Ir) {
        if let Ir::Generic { items, .. } = generics {
            for it in items {
                shape.list_with(self.element_name(it), self.render(it, true));
            }
        } else {
            shape.singleton("generic", self.render(generics, true));
        }
    }

    /// Add a single child as either a singleton key-by-element-name or
    /// a scalar text value, depending on whether the rendering is an
    /// object or a string. Mirrors `xml_to_json`'s behaviour where
    /// text-only-leaves collapse to scalars under their parent's chosen
    /// key.
    fn add_singleton_or_text(&self, shape: &mut Shape, ir: &Ir) {
        let key = self.element_name(ir);
        let val = self.render(ir, true);
        shape.singleton(key, val);
    }

    /// Render an IR as the value-side of a `<type>` slot. If the IR
    /// already produces a `<type>`-shaped value (GenericType,
    /// SimpleStatement::type), unwrap so the parent doesn't double-wrap.
    fn render_as_type(&self, ir: &Ir) -> Value {
        let element = self.element_name(ir);
        if element == "type" {
            // Already type-shaped — return the inner so the parent's
            // explicit "type" key holds it directly.
            self.render(ir, true)
        } else {
            // Not type-shaped — wrap in a `{ "$type": "type", inner }`
            // object. Use a scalar for the inner if it renders as text.
            let inner = self.render(ir, true);
            match inner {
                Value::String(s) => {
                    let mut obj = Map::new();
                    obj.insert("name".into(), Value::String(s));
                    Value::Object(obj)
                }
                Value::Object(m) => Value::Object(m),
                other => other,
            }
        }
    }

    fn wrap_expression_host(&self, ir: &Ir) -> Value {
        // Mirror the XML render's <expression> host wrapping. Skip the
        // wrapper when the inner already produces an `<expression>`-
        // shaped value, to avoid `expression > expression` nesting.
        let inner_kind = self.element_name(ir);
        if matches!(inner_kind, "expression") {
            return self.render(ir, true);
        }
        let inner = self.render(ir, true);
        match inner {
            Value::Object(map) if !map.is_empty() => {
                // Lift inner into expression host without `$type`
                // duplication: the JSON shape emits `{ "<inner_key>":
                // {...} }` on the parent's key.
                let mut wrap = Map::new();
                for (k, v) in map { wrap.insert(k, v); }
                Value::Object(wrap)
            }
            other => other,
        }
    }

    fn op_value(&self, op_text: &str, op_marker: &str) -> Value {
        let mut obj = Map::new();
        obj.insert("text".into(), Value::String(op_text.to_string()));
        obj.insert(op_marker.to_string(), Value::Bool(true));
        Value::Object(obj)
    }

    fn add_modifier_flags(&self, shape: &mut Shape, modifiers: &Modifiers) {
        for marker in modifiers.marker_names() {
            shape.flag(marker);
        }
    }

    /// Add a heterogeneous Vec of children to the shape, grouping by
    /// their JSON key name (= element name). Leaves without children
    /// become scalar text under the same key. `Ir::Inline` is
    /// transparent — its children flatten into the parent (matching
    /// the XML render behaviour).
    fn add_children(&self, shape: &mut Shape, children: &[Ir]) {
        for c in children {
            // Markers (Break/Continue) collapse to flags when bare.
            if matches!(c, Ir::Break { .. } | Ir::Continue { .. }) {
                shape.flag(self.element_name(c));
                continue;
            }
            // Inline: transparent — recurse with its children. If
            // `list_name` is set, treat it as a flat list under that
            // key (matching the XML render's `list="X"` distribution).
            if let Ir::Inline { children: inner, list_name, .. } = c {
                if let Some(list) = list_name {
                    for ic in inner {
                        let val = self.render(ic, true);
                        shape.put_in_list(list, val);
                    }
                } else {
                    self.add_children(shape, inner);
                }
                continue;
            }
            let key = self.element_name(c);
            let val = self.render(c, true);
            shape.list_with(key, val);
        }
    }
}

/// Builder for an object shape. Tracks both single-keyed slots (with
/// collision promotion to arrays) and explicit array slots. Mirrors
/// the XML→JSON projection rules for `list=` / singleton-element-
/// name / collision-overflow.
struct Shape {
    type_name: &'static str,
    obj: Map<String, Value>,
    /// Tracks how many entries we've put under each key, so the
    /// second occurrence promotes the existing singleton to an array.
    counts: std::collections::HashMap<String, usize>,
    /// Anonymous overflow array for collisions that can't promote to
    /// a list (e.g. role-mixed shapes). `xml_to_json` calls this
    /// `$children`.
    overflow: Vec<Value>,
    /// Optional text content (for text-only leaves with extra flags).
    text: Option<String>,
}

impl Shape {
    fn new(type_name: &'static str) -> Self {
        Self {
            type_name,
            obj: Map::new(),
            counts: std::collections::HashMap::new(),
            overflow: Vec::new(),
            text: None,
        }
    }

    fn flag(&mut self, name: &str) {
        // Only set if not already present (avoid clobbering an actual
        // child with a same-named flag).
        if !self.obj.contains_key(name) {
            self.obj.insert(name.to_string(), Value::Bool(true));
        }
    }

    fn put(&mut self, key: &str, value: Value) {
        self.obj.insert(key.to_string(), value);
    }

    /// Append a value under a fixed list key, creating the array on
    /// first call and reusing it on subsequent calls. Used for
    /// `Ir::Inline { list_name }` flattening.
    fn put_in_list(&mut self, list_key: &str, value: Value) {
        let value = strip_top_level_type(value);
        match self.obj.remove(list_key) {
            Some(Value::Array(mut arr)) => {
                arr.push(value);
                self.obj.insert(list_key.into(), Value::Array(arr));
            }
            Some(other) => {
                self.obj.insert(list_key.into(), Value::Array(vec![other, value]));
            }
            None => {
                self.obj.insert(list_key.into(), Value::Array(vec![value]));
            }
        }
    }

    fn text(&mut self, text: String) {
        self.text = Some(text);
    }

    /// Insert a singleton-keyed value. On collision, promote to an
    /// array (wraps existing + new). Reserved sigil keys never collide.
    fn singleton(&mut self, key: &str, value: Value) {
        let count = self.counts.entry(key.to_string()).or_insert(0);
        *count += 1;
        if *count == 1 {
            self.obj.insert(key.to_string(), value);
        } else if *count == 2 {
            // Promote to array.
            let existing = self.obj.remove(key).unwrap_or(Value::Null);
            self.obj.insert(key.to_string(), Value::Array(vec![existing, value]));
        } else {
            // Already an array — append.
            if let Some(Value::Array(arr)) = self.obj.get_mut(key) {
                arr.push(value);
            }
        }
    }

    /// Insert into a list-keyed slot. The key is pluralised English
    /// (matches the existing `list="X"` convention). First entry becomes
    /// a singleton-then-array on the second insert, just like
    /// `xml_to_json.rs` projects.
    fn list_with(&mut self, element_name: &str, value: Value) {
        let plural = pluralize_list_name(element_name);
        let count = self.counts.entry(plural.clone()).or_insert(0);
        *count += 1;
        if *count == 1 {
            // First occurrence — use singular key (the singleton form).
            // Singleton entries DROP their $type since the key already
            // conveys it.
            let value = strip_top_level_type(value);
            self.obj.insert(element_name.to_string(), value);
        } else if *count == 2 {
            // Second occurrence — promote to plural array.
            let existing = self.obj.remove(element_name).unwrap_or(Value::Null);
            let value = strip_top_level_type(value);
            self.obj.insert(plural, Value::Array(vec![existing, value]));
        } else {
            // Already an array — append.
            let value = strip_top_level_type(value);
            if let Some(Value::Array(arr)) = self.obj.get_mut(&plural) {
                arr.push(value);
            }
        }
    }

    fn into_value(mut self, strip_type: bool) -> Value {
        // Text-only leaf with no other content: emit as scalar string.
        let no_content = self.obj.is_empty() && self.overflow.is_empty();
        if no_content {
            if let Some(text) = self.text.take() {
                if strip_type {
                    return Value::String(text);
                } else {
                    let mut obj = Map::new();
                    obj.insert(KEY_TYPE.into(), Value::String(self.type_name.into()));
                    obj.insert(KEY_TEXT.into(), Value::String(text));
                    return Value::Object(obj);
                }
            }
        }
        // Otherwise build an object.
        let mut obj = self.obj;
        if let Some(text) = self.text {
            obj.insert(KEY_TEXT.into(), Value::String(text));
        }
        if !self.overflow.is_empty() {
            obj.insert(KEY_CHILDREN.into(), Value::Array(self.overflow));
        }
        if !strip_type {
            // Insert $type at front for readability.
            let mut with_type = Map::new();
            with_type.insert(KEY_TYPE.into(), Value::String(self.type_name.into()));
            for (k, v) in obj {
                with_type.insert(k, v);
            }
            Value::Object(with_type)
        } else {
            Value::Object(obj)
        }
    }
}

fn strip_top_level_type(value: Value) -> Value {
    match value {
        Value::Object(mut m) => {
            m.remove(KEY_TYPE);
            Value::Object(m)
        }
        other => other,
    }
}
