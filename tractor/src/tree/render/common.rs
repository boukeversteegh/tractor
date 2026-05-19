//! Common rendering helpers shared across per-language emitters.
//!
//! Each per-language module supplies a [`Syntax`] config plus optional
//! override hooks; the shared [`write_ir`] engine handles the bulk of
//! tree variant dispatch.
//!
//! A small set of leaf-emit primitives ([`write_quoted_scalar`]) is
//! also lifted here so SyntaxTree, DataTree, and (future) SqlTree
//! agree on how a [`QuoteStyle`] decorates a stored text payload.


use crate::tree::types::{AccessReceiver, AccessSegment, QuoteStyle, SyntaxTree};

#[derive(Clone, Copy, Debug)]
pub struct Indent {
    pub level: usize,
    pub unit: &'static str,
}

impl Indent {
    pub const SPACES_4: Self = Self { level: 0, unit: "    " };
    pub const SPACES_2: Self = Self { level: 0, unit: "  " };
    pub const TAB: Self = Self { level: 0, unit: "\t" };
    pub fn deeper(self) -> Self { Self { level: self.level + 1, unit: self.unit } }
    pub fn write(self, out: &mut String) {
        for _ in 0..self.level { out.push_str(self.unit); }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Syntax {
    pub fn_keyword: &'static str,
    pub class_keyword: &'static str,
    pub interface_keyword: &'static str,
    pub return_keyword: &'static str,
    pub if_keyword: &'static str,
    pub elif_keyword: &'static str,
    pub else_keyword: &'static str,
    pub while_keyword: &'static str,
    pub for_keyword: &'static str,
    pub foreach_keyword: &'static str,
    pub foreach_in: &'static str,
    pub break_keyword: &'static str,
    pub continue_keyword: &'static str,
    pub null_keyword: &'static str,
    pub true_keyword: &'static str,
    pub false_keyword: &'static str,
    pub new_keyword: &'static str,
    pub block_open: &'static str,
    pub block_close: &'static str,
    pub statement_terminator: &'static str,
    pub block_intro: &'static str,
    pub paren_conditions: bool,
    pub indent_blocks: bool,
    pub typed_param_pre: bool,
    pub indent: Indent,
    pub comment_line: &'static str,
}

impl Default for Syntax {
    fn default() -> Self {
        Self {
            fn_keyword: "function", class_keyword: "class", interface_keyword: "interface",
            return_keyword: "return", if_keyword: "if", elif_keyword: "else if",
            else_keyword: "else", while_keyword: "while", for_keyword: "for",
            foreach_keyword: "foreach", foreach_in: " in ",
            break_keyword: "break", continue_keyword: "continue",
            null_keyword: "null", true_keyword: "true", false_keyword: "false",
            new_keyword: "new",
            block_open: " {", block_close: "}",
            statement_terminator: ";", block_intro: "",
            paren_conditions: true, indent_blocks: false, typed_param_pre: false,
            indent: Indent::SPACES_4, comment_line: "//",
        }
    }
}

/// Emit `text` decorated with `style`, applying `escape` to the inner
/// content for delimited variants. Single source of truth for
/// quote-style wrapping across SyntaxTree, DataTree, and future
/// SqlTree renderers.
///
/// `escape` is a per-format function (JSON escapes `\n`/`"`, Python
/// escapes differently, YAML's plain form typically doesn't escape at
/// all). Use [`identity_escape`] when no escaping is needed.
///
/// The recursive `Raw { prefix, inner }` arm prepends the lowercase
/// prefix (e.g. `r`, `b`) and then re-emits with the inner style — so
/// a `Raw { prefix: "r", inner: Double }` becomes `r"text"`. `Block`
/// and `Heredoc` are best-effort; the structural prefix is correct
/// but the renderer doesn't yet manage indentation for multi-line
/// payloads.
pub fn write_quoted_scalar(
    text: &str,
    style: &QuoteStyle,
    escape: impl Fn(&str) -> String,
    out: &mut String,
) {
    match style {
        QuoteStyle::Plain => out.push_str(text),
        QuoteStyle::Single => {
            out.push('\'');
            out.push_str(&escape(text));
            out.push('\'');
        }
        QuoteStyle::Double => {
            out.push('"');
            out.push_str(&escape(text));
            out.push('"');
        }
        QuoteStyle::TripleSingle => {
            out.push_str("'''");
            out.push_str(&escape(text));
            out.push_str("'''");
        }
        QuoteStyle::TripleDouble => {
            out.push_str("\"\"\"");
            out.push_str(&escape(text));
            out.push_str("\"\"\"");
        }
        QuoteStyle::Backtick => {
            out.push('`');
            out.push_str(&escape(text));
            out.push('`');
        }
        QuoteStyle::Brackets => {
            out.push('[');
            // T-SQL escapes `]` as `]]` inside bracketed identifiers;
            // caller's `escape` table handles that if it cares.
            out.push_str(&escape(text));
            out.push(']');
        }
        QuoteStyle::Raw { prefix, inner } => {
            out.push_str(prefix);
            // Raw means "the inner style chosen the framing; don't
            // double-escape". Identity-escape inside.
            write_quoted_scalar(text, inner, identity_escape, out);
        }
        QuoteStyle::Heredoc { delimiter } => {
            out.push_str("<<");
            out.push_str(delimiter);
            out.push('\n');
            out.push_str(text);
            if !text.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(delimiter);
        }
        QuoteStyle::Block { folded, chomp } => {
            out.push(if *folded { '>' } else { '|' });
            if *chomp == '-' || *chomp == '+' {
                out.push(*chomp);
            }
            out.push('\n');
            out.push_str(text);
        }
    }
}

/// No-op escape — pass text through unchanged. Use with
/// [`write_quoted_scalar`] for formats that don't escape (e.g. YAML
/// plain scalars, raw-string variants).
pub fn identity_escape(s: &str) -> String {
    s.to_string()
}

/// Source-aware tree walker (S13-Z6/Z7). Identical to [`write_ir`]
/// except that when `source` is `Some(s)` and an anchored subtree is
/// encountered, the renderer prefers slicing `s[node.range()]` over
/// canonical re-rendering. This lets the same engine serve anchored,
/// canonical, and mixed-mode rendering.
///
/// Per-subtree byte-slice shortcut: today this kicks in when the
/// current node *and all of its direct children* are anchored. Going
/// deeper than one level still works correctly via recursion; the
/// shallow shortcut catches the common "edit a leaf, keep the
/// surrounding parsed source" pattern.
///
/// The gap-context fallback between anchored and synthetic siblings
/// (S13-Z6, per-language defaults via [`Syntax`]) is a near-term
/// extension; today the canonical walker handles those gaps.
pub fn write_ir_with_source(
    tree: &SyntaxTree,
    source: Option<&str>,
    out: &mut String,
    indent: Indent,
    sx: &Syntax,
) {
    if let Some(s) = source {
        if tree.is_anchored() && tree.children().iter().all(|c| c.is_anchored()) {
            out.push_str(tree.range().slice(s));
            return;
        }
    }
    write_ir(tree, out, indent, sx);
}

/// Shared tree walker. Renders scalar leaves from their stored `text`
/// field (S13-Z1) and compound nodes from the per-language [`Syntax`]
/// config. Whitespace between siblings uses canonical defaults; for
/// source-anchored gap preservation see [`write_ir_with_source`].
pub fn write_ir(tree: &SyntaxTree, out: &mut String, indent: Indent, sx: &Syntax) {
    match tree {
        SyntaxTree::Module { children, .. } => {
            for c in children {
                indent.write(out);
                write_ir(c, out, indent, sx);
                if needs_terminator(c, sx) { out.push_str(sx.statement_terminator); }
                out.push('\n');
            }
        }
        SyntaxTree::Namespace { name, children, .. } => {
            out.push_str("namespace ");
            write_ir(name, out, indent, sx);
            out.push_str(sx.block_open);
            out.push('\n');
            for c in children {
                indent.deeper().write(out);
                write_ir(c, out, indent.deeper(), sx);
                if needs_terminator(c, sx) { out.push_str(sx.statement_terminator); }
                out.push('\n');
            }
            indent.write(out);
            out.push_str(sx.block_close);
        }
        SyntaxTree::Class { name, generics, bases, body, .. }
        | SyntaxTree::Struct { name, generics, bases, body, .. }
        | SyntaxTree::Interface { name, generics, bases, body, .. }
        | SyntaxTree::Record { name, generics, bases, body, .. } => {
            // Element name comes from the variant tag; per-language
            // keyword still needs a string, derived from the variant.
            let kind: &str = match tree {
                SyntaxTree::Struct { .. } => "struct",
                SyntaxTree::Interface { .. } => "interface",
                SyntaxTree::Record { .. } => "record",
                _ => "class",
            };
            out.push_str(kind);
            out.push(' ');
            write_ir(name, out, indent, sx);
            if !generics.is_empty() {
                out.push('[');
                for (i, g) in generics.iter().enumerate() {
                    if i > 0 { out.push_str(", "); }
                    write_ir(g, out, indent, sx);
                }
                out.push(']');
            }
            if !bases.is_empty() {
                out.push_str(" : ");
                for (i, b) in bases.iter().enumerate() {
                    if i > 0 { out.push_str(", "); }
                    write_ir(b, out, indent, sx);
                }
            }
            if sx.indent_blocks {
                out.push_str(sx.block_intro);
                out.push('\n');
                write_ir(body, out, indent.deeper(), sx);
            } else {
                out.push_str(sx.block_open);
                out.push('\n');
                write_ir(body, out, indent.deeper(), sx);
                indent.write(out);
                out.push_str(sx.block_close);
            }
        }
        SyntaxTree::Function { name, generics, parameters, returns, body, .. }
        | SyntaxTree::Method { name, generics, parameters, returns, body, .. } => {
            if !sx.typed_param_pre || returns.is_none() {
                out.push_str(sx.fn_keyword);
                out.push(' ');
            } else if let Some(r) = returns {
                write_ir(r, out, indent, sx);
                out.push(' ');
            }
            write_ir(name, out, indent, sx);
            if !generics.is_empty() {
                out.push('[');
                for (i, g) in generics.iter().enumerate() {
                    if i > 0 { out.push_str(", "); }
                    write_ir(g, out, indent, sx);
                }
                out.push(']');
            }
            out.push('(');
            for (i, p) in parameters.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                write_ir(p, out, indent, sx);
            }
            out.push(')');
            if !sx.typed_param_pre {
                if let Some(r) = returns {
                    out.push_str(" -> ");
                    write_ir(r, out, indent, sx);
                }
            }
            if let Some(b) = body {
                if sx.indent_blocks {
                    out.push_str(sx.block_intro);
                    out.push('\n');
                    write_ir(b, out, indent.deeper(), sx);
                } else {
                    out.push_str(sx.block_open);
                    out.push('\n');
                    write_ir(b, out, indent.deeper(), sx);
                    indent.write(out);
                    out.push_str(sx.block_close);
                }
            } else {
                out.push_str(sx.statement_terminator);
            }
        }
        SyntaxTree::Body { children, .. } => {
            for c in children {
                indent.write(out);
                write_ir(c, out, indent, sx);
                if needs_terminator(c, sx) { out.push_str(sx.statement_terminator); }
                out.push('\n');
            }
        }
        SyntaxTree::Parameter { name, type_ann, default, .. } => {
            if sx.typed_param_pre {
                if let Some(t) = type_ann { write_ir(t, out, indent, sx); out.push(' '); }
                write_ir(name, out, indent, sx);
            } else {
                write_ir(name, out, indent, sx);
                if let Some(t) = type_ann { out.push_str(": "); write_ir(t, out, indent, sx); }
            }
            if let Some(d) = default { out.push_str(" = "); write_ir(d, out, indent, sx); }
        }
        SyntaxTree::Returns { type_ann, .. } => write_ir(type_ann, out, indent, sx),
        SyntaxTree::Return { value, .. } => {
            out.push_str(sx.return_keyword);
            if let Some(v) = value { out.push(' '); write_ir(v, out, indent, sx); }
        }
        SyntaxTree::If { condition, body, else_branch, .. } => {
            out.push_str(sx.if_keyword);
            cond_inline(condition, out, indent, sx);
            emit_block(body, out, indent, sx);
            if let Some(branch) = else_branch { out.push(' '); write_ir(branch, out, indent, sx); }
        }
        SyntaxTree::ElseIf { condition, body, else_branch, .. } => {
            out.push_str(sx.elif_keyword);
            cond_inline(condition, out, indent, sx);
            emit_block(body, out, indent, sx);
            if let Some(branch) = else_branch { out.push(' '); write_ir(branch, out, indent, sx); }
        }
        SyntaxTree::Else { body, .. } => {
            out.push_str(sx.else_keyword);
            emit_block(body, out, indent, sx);
        }
        SyntaxTree::While { condition, body, .. } => {
            out.push_str(sx.while_keyword);
            cond_inline(condition, out, indent, sx);
            emit_block(body, out, indent, sx);
        }
        SyntaxTree::Foreach { type_ann, target, iterable, body, .. } => {
            out.push_str(sx.foreach_keyword);
            let inner = |out: &mut String| {
                if let Some(t) = type_ann { write_ir(t, out, indent, sx); out.push(' '); }
                write_ir(target, out, indent, sx);
                out.push_str(sx.foreach_in);
                write_ir(iterable, out, indent, sx);
            };
            if sx.paren_conditions {
                out.push_str(" (");
                inner(out);
                out.push(')');
            } else {
                out.push(' ');
                inner(out);
            }
            emit_block(body, out, indent, sx);
        }
        SyntaxTree::For { targets, iterables, body, .. } => {
            out.push_str(sx.for_keyword);
            out.push(' ');
            for (i, t) in targets.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                write_ir(t, out, indent, sx);
            }
            out.push_str(sx.foreach_in);
            for (i, it) in iterables.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                write_ir(it, out, indent, sx);
            }
            emit_block(body, out, indent, sx);
        }
        SyntaxTree::CFor { initializer, condition, updates, body, .. } => {
            out.push_str(sx.for_keyword);
            out.push_str(" (");
            if let Some(init) = initializer { write_ir(init, out, indent, sx); }
            out.push_str("; ");
            if let Some(cond) = condition { write_ir(cond, out, indent, sx); }
            out.push_str("; ");
            for (i, u) in updates.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                write_ir(u, out, indent, sx);
            }
            out.push(')');
            emit_block(body, out, indent, sx);
        }
        SyntaxTree::Break { .. } => out.push_str(sx.break_keyword),
        SyntaxTree::Continue { .. } => out.push_str(sx.continue_keyword),
        SyntaxTree::Binary { left, op, right, .. }
        | SyntaxTree::Logical { left, op, right, .. } => {
            write_ir(left, out, indent, sx);
            out.push(' ');
            write_ir(op, out, indent, sx);
            out.push(' ');
            write_ir(right, out, indent, sx);
        }
        SyntaxTree::Operator { text, .. } => out.push_str(text),
        SyntaxTree::Unary { op_text, operand, .. } => {
            out.push_str(op_text); write_ir(operand, out, indent, sx);
        }
        SyntaxTree::Comparison { left, op_text, right, .. } => {
            write_ir(left, out, indent, sx); out.push(' ');
            out.push_str(op_text); out.push(' ');
            write_ir(right, out, indent, sx);
        }
        SyntaxTree::Assign { targets, op_text, values, .. } => {
            for (i, t) in targets.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                write_ir(t, out, indent, sx);
            }
            out.push(' '); out.push_str(op_text); out.push(' ');
            for (i, v) in values.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                write_ir(v, out, indent, sx);
            }
        }
        SyntaxTree::Call { callee, arguments, .. } => {
            write_ir(callee, out, indent, sx);
            out.push('(');
            for (i, a) in arguments.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                write_ir(a, out, indent, sx);
            }
            out.push(')');
        }
        SyntaxTree::ObjectAccess { receiver, segments, .. } => {
            match receiver {
                AccessReceiver::Base { .. } => out.push_str("base"),
                AccessReceiver::This { .. } => out.push_str("this"),
                AccessReceiver::Super { .. } => out.push_str("super"),
                AccessReceiver::Self_ { .. } => out.push_str("self"),
                AccessReceiver::Instance(t) => write_ir(t, out, indent, sx),
            }
            for seg in segments { write_segment(seg, out, indent, sx); }
        }
        SyntaxTree::ObjectCreation { type_target, arguments, initializer, .. } => {
            out.push_str(sx.new_keyword);
            if let Some(t) = type_target { out.push(' '); write_ir(t, out, indent, sx); }
            out.push('(');
            for (i, a) in arguments.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                write_ir(a, out, indent, sx);
            }
            out.push(')');
            if let Some(init) = initializer {
                out.push_str(" { ");
                write_ir(init, out, indent, sx);
                out.push_str(" }");
            }
        }
        SyntaxTree::List { children, .. } => list_like('[', ']', children, out, indent, sx),
        SyntaxTree::Tuple { children, .. } => {
            out.push('(');
            for (i, c) in children.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                write_ir(c, out, indent, sx);
            }
            if children.len() == 1 { out.push(','); }
            out.push(')');
        }
        SyntaxTree::Dictionary { pairs, .. } => list_like('{', '}', pairs, out, indent, sx),
        SyntaxTree::Set { children, .. } => list_like('{', '}', children, out, indent, sx),
        SyntaxTree::Pair { key, value, .. } => {
            write_ir(key, out, indent, sx);
            out.push_str(": ");
            write_ir(value, out, indent, sx);
        }
        SyntaxTree::Ternary { condition, if_true, if_false, .. } => {
            write_ir(condition, out, indent, sx); out.push_str(" ? ");
            write_ir(if_true, out, indent, sx); out.push_str(" : ");
            write_ir(if_false, out, indent, sx);
        }
        SyntaxTree::Lambda { parameters, body, .. } => {
            out.push('(');
            for (i, p) in parameters.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                write_ir(p, out, indent, sx);
            }
            out.push_str(") => ");
            write_ir(body.inner(), out, indent, sx);
        }
        SyntaxTree::Inline { children, .. } => {
            for c in children { write_ir(c, out, indent, sx); }
        }
        SyntaxTree::Expression { inner, .. } => write_ir(inner, out, indent, sx),
        SyntaxTree::Comment { .. } => { out.push_str(sx.comment_line); out.push_str(" (comment)"); }
        // S13-Z1: scalar variants now carry their decoded text. When
        // the stored text is non-empty (parsed or programmatically set)
        // we emit it verbatim; otherwise fall back to the per-language
        // keyword / canonical placeholder so empty synthetic trees
        // still produce something readable.
        SyntaxTree::Null { text, .. } | SyntaxTree::None { text, .. } => {
            if text.is_empty() { out.push_str(sx.null_keyword); } else { out.push_str(text); }
        }
        SyntaxTree::True { text, .. } => {
            if text.is_empty() { out.push_str(sx.true_keyword); } else { out.push_str(text); }
        }
        SyntaxTree::False { text, .. } => {
            if text.is_empty() { out.push_str(sx.false_keyword); } else { out.push_str(text); }
        }
        SyntaxTree::Name { text, .. } => {
            if text.is_empty() { out.push_str("«name»"); } else { out.push_str(text); }
        }
        SyntaxTree::Atom { text, .. } => out.push_str(text),
        SyntaxTree::Int { text, .. } => {
            if text.is_empty() { out.push('0'); } else { out.push_str(text); }
        }
        SyntaxTree::Float { text, .. } => {
            if text.is_empty() { out.push_str("0.0"); } else { out.push_str(text); }
        }
        SyntaxTree::String { text, quote_style, .. } => {
            write_quoted_scalar(text, quote_style, identity_escape, out);
        }
        SyntaxTree::SimpleStatement { children, .. } => {
            for (i, c) in children.iter().enumerate() {
                if i > 0 { out.push(' '); }
                write_ir(c, out, indent, sx);
            }
        }
        SyntaxTree::Variable { name, type_ann, value, .. }
        | SyntaxTree::Field { name, type_ann, value, .. }
        | SyntaxTree::Event { name, type_ann, value, .. } => {
            // Cross-language canonical form: `name[: type][ = value]`.
            // Per-language render_source can override later if a
            // specific keyword (let / const / var) is needed.
            write_ir(name, out, indent, sx);
            if let Some(t) = type_ann {
                out.push_str(": ");
                write_ir(t, out, indent, sx);
            }
            if let Some(v) = value {
                out.push_str(" = ");
                write_ir(&v.inner, out, indent, sx);
            }
        }
        _ => out.push_str("«?»"),
    }
}

fn list_like(open: char, close: char, items: &[SyntaxTree], out: &mut String, indent: Indent, sx: &Syntax) {
    out.push(open);
    for (i, c) in items.iter().enumerate() {
        if i > 0 { out.push_str(", "); }
        write_ir(c, out, indent, sx);
    }
    out.push(close);
}

fn cond_inline(condition: &SyntaxTree, out: &mut String, indent: Indent, sx: &Syntax) {
    if sx.paren_conditions {
        out.push_str(" (");
        write_ir(condition, out, indent, sx);
        out.push(')');
    } else {
        out.push(' ');
        write_ir(condition, out, indent, sx);
    }
}

fn emit_block(body: &SyntaxTree, out: &mut String, indent: Indent, sx: &Syntax) {
    if sx.indent_blocks {
        out.push_str(sx.block_intro);
        out.push('\n');
        write_ir(body, out, indent.deeper(), sx);
    } else {
        out.push_str(sx.block_open);
        out.push('\n');
        write_ir(body, out, indent.deeper(), sx);
        indent.write(out);
        out.push_str(sx.block_close);
    }
}

fn needs_terminator(tree: &SyntaxTree, sx: &Syntax) -> bool {
    if sx.statement_terminator.is_empty() { return false; }
    !matches!(
        tree,
        SyntaxTree::If { .. } | SyntaxTree::While { .. } | SyntaxTree::Foreach { .. } | SyntaxTree::CFor { .. }
            | SyntaxTree::For { .. } | SyntaxTree::Function { .. } | SyntaxTree::Method { .. }
            | SyntaxTree::Class { .. } | SyntaxTree::Struct { .. }
            | SyntaxTree::Interface { .. } | SyntaxTree::Record { .. }
            | SyntaxTree::Namespace { .. } | SyntaxTree::Try { .. } | SyntaxTree::Comment { .. }
    )
}

fn write_segment(seg: &AccessSegment, out: &mut String, indent: Indent, sx: &Syntax) {
    match seg {
        AccessSegment::Member { optional, .. } => {
            if *optional { out.push_str("?."); } else { out.push('.'); }
            out.push_str("«property»");
        }
        AccessSegment::Index { indices, .. } => {
            out.push('[');
            for (i, idx) in indices.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                write_ir(idx, out, indent, sx);
            }
            out.push(']');
        }
        AccessSegment::Call { name, arguments, .. } => {
            if name.is_some() { out.push('.'); out.push_str("«method»"); }
            out.push('(');
            for (i, a) in arguments.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                write_ir(a, out, indent, sx);
            }
            out.push(')');
        }
    }
}
