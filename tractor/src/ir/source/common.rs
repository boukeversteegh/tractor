//! Common rendering helpers shared across per-language emitters.
//!
//! Each per-language module supplies a [`Syntax`] config plus optional
//! override hooks; the shared [`write_ir`] engine handles the bulk of
//! IR variant dispatch.

#![allow(dead_code)]

use crate::ir::types::{AccessSegment, Ir};

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

pub fn render_generic(ir: &Ir) -> String {
    let mut out = String::new();
    write_ir(ir, &mut out, Indent::SPACES_4, &Syntax::default());
    out
}

/// Shared IR walker used by every per-language emitter. Atom rendering
/// emits placeholders (`«name»` / `0` / `""`) because the IR atoms
/// only carry byte ranges; the source-anchor path is the way to
/// reconstruct atom text. For from-scratch canonical rendering, atom
/// text would need to ride alongside the IR — out of scope for this
/// scaffold.
pub fn write_ir(ir: &Ir, out: &mut String, indent: Indent, sx: &Syntax) {
    match ir {
        Ir::Module { children, .. } => {
            for c in children {
                indent.write(out);
                write_ir(c, out, indent, sx);
                if needs_terminator(c, sx) { out.push_str(sx.statement_terminator); }
                out.push('\n');
            }
        }
        Ir::Namespace { name, children, .. } => {
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
        Ir::Class { kind, name, generics, bases, body, .. } => {
            out.push_str(kind);
            out.push(' ');
            write_ir(name, out, indent, sx);
            if let Some(g) = generics { write_ir(g, out, indent, sx); }
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
        Ir::Function { name, generics, parameters, returns, body, .. } => {
            if !sx.typed_param_pre || returns.is_none() {
                out.push_str(sx.fn_keyword);
                out.push(' ');
            } else if let Some(r) = returns {
                write_ir(r, out, indent, sx);
                out.push(' ');
            }
            write_ir(name, out, indent, sx);
            if let Some(g) = generics { write_ir(g, out, indent, sx); }
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
        Ir::Body { children, .. } => {
            for c in children {
                indent.write(out);
                write_ir(c, out, indent, sx);
                if needs_terminator(c, sx) { out.push_str(sx.statement_terminator); }
                out.push('\n');
            }
        }
        Ir::Parameter { name, type_ann, default, .. } => {
            if sx.typed_param_pre {
                if let Some(t) = type_ann { write_ir(t, out, indent, sx); out.push(' '); }
                write_ir(name, out, indent, sx);
            } else {
                write_ir(name, out, indent, sx);
                if let Some(t) = type_ann { out.push_str(": "); write_ir(t, out, indent, sx); }
            }
            if let Some(d) = default { out.push_str(" = "); write_ir(d, out, indent, sx); }
        }
        Ir::Returns { type_ann, .. } => write_ir(type_ann, out, indent, sx),
        Ir::Return { value, .. } => {
            out.push_str(sx.return_keyword);
            if let Some(v) = value { out.push(' '); write_ir(v, out, indent, sx); }
        }
        Ir::If { condition, body, else_branch, .. } => {
            out.push_str(sx.if_keyword);
            cond_inline(condition, out, indent, sx);
            emit_block(body, out, indent, sx);
            if let Some(branch) = else_branch { out.push(' '); write_ir(branch, out, indent, sx); }
        }
        Ir::ElseIf { condition, body, else_branch, .. } => {
            out.push_str(sx.elif_keyword);
            cond_inline(condition, out, indent, sx);
            emit_block(body, out, indent, sx);
            if let Some(branch) = else_branch { out.push(' '); write_ir(branch, out, indent, sx); }
        }
        Ir::Else { body, .. } => {
            out.push_str(sx.else_keyword);
            emit_block(body, out, indent, sx);
        }
        Ir::While { condition, body, .. } => {
            out.push_str(sx.while_keyword);
            cond_inline(condition, out, indent, sx);
            emit_block(body, out, indent, sx);
        }
        Ir::Foreach { type_ann, target, iterable, body, .. } => {
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
        Ir::For { targets, iterables, body, .. } => {
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
        Ir::CFor { initializer, condition, updates, body, .. } => {
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
        Ir::Break { .. } => out.push_str(sx.break_keyword),
        Ir::Continue { .. } => out.push_str(sx.continue_keyword),
        Ir::Binary { op_text, left, right, .. } => {
            write_ir(left, out, indent, sx); out.push(' ');
            out.push_str(op_text); out.push(' ');
            write_ir(right, out, indent, sx);
        }
        Ir::Unary { op_text, operand, .. } => {
            out.push_str(op_text); write_ir(operand, out, indent, sx);
        }
        Ir::Comparison { left, op_text, right, .. } => {
            write_ir(left, out, indent, sx); out.push(' ');
            out.push_str(op_text); out.push(' ');
            write_ir(right, out, indent, sx);
        }
        Ir::Assign { targets, op_text, values, .. } => {
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
        Ir::Call { callee, arguments, .. } => {
            write_ir(callee, out, indent, sx);
            out.push('(');
            for (i, a) in arguments.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                write_ir(a, out, indent, sx);
            }
            out.push(')');
        }
        Ir::Access { receiver, segments, .. } => {
            write_ir(receiver, out, indent, sx);
            for seg in segments { write_segment(seg, out, indent, sx); }
        }
        Ir::ObjectCreation { type_target, arguments, initializer, .. } => {
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
        Ir::List { children, .. } => list_like('[', ']', children, out, indent, sx),
        Ir::Tuple { children, .. } => {
            out.push('(');
            for (i, c) in children.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                write_ir(c, out, indent, sx);
            }
            if children.len() == 1 { out.push(','); }
            out.push(')');
        }
        Ir::Dictionary { pairs, .. } => list_like('{', '}', pairs, out, indent, sx),
        Ir::Set { children, .. } => list_like('{', '}', children, out, indent, sx),
        Ir::Pair { key, value, .. } => {
            write_ir(key, out, indent, sx);
            out.push_str(": ");
            write_ir(value, out, indent, sx);
        }
        Ir::Ternary { condition, if_true, if_false, .. } => {
            write_ir(condition, out, indent, sx); out.push_str(" ? ");
            write_ir(if_true, out, indent, sx); out.push_str(" : ");
            write_ir(if_false, out, indent, sx);
        }
        Ir::Lambda { parameters, body, .. } => {
            out.push('(');
            for (i, p) in parameters.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                write_ir(p, out, indent, sx);
            }
            out.push_str(") => ");
            write_ir(body, out, indent, sx);
        }
        Ir::Inline { children, .. } => {
            for c in children { write_ir(c, out, indent, sx); }
        }
        Ir::Expression { inner, .. } => write_ir(inner, out, indent, sx),
        Ir::Comment { .. } => { out.push_str(sx.comment_line); out.push_str(" (comment)"); }
        Ir::Null { .. } | Ir::None { .. } => out.push_str(sx.null_keyword),
        Ir::True { .. } => out.push_str(sx.true_keyword),
        Ir::False { .. } => out.push_str(sx.false_keyword),
        Ir::Name { .. } => out.push_str("«name»"),
        Ir::Int { .. } => out.push('0'),
        Ir::Float { .. } => out.push_str("0.0"),
        Ir::String { .. } => out.push_str("\"\""),
        Ir::SimpleStatement { children, .. } => {
            for (i, c) in children.iter().enumerate() {
                if i > 0 { out.push(' '); }
                write_ir(c, out, indent, sx);
            }
        }
        _ => out.push_str("«?»"),
    }
}

fn list_like(open: char, close: char, items: &[Ir], out: &mut String, indent: Indent, sx: &Syntax) {
    out.push(open);
    for (i, c) in items.iter().enumerate() {
        if i > 0 { out.push_str(", "); }
        write_ir(c, out, indent, sx);
    }
    out.push(close);
}

fn cond_inline(condition: &Ir, out: &mut String, indent: Indent, sx: &Syntax) {
    if sx.paren_conditions {
        out.push_str(" (");
        write_ir(condition, out, indent, sx);
        out.push(')');
    } else {
        out.push(' ');
        write_ir(condition, out, indent, sx);
    }
}

fn emit_block(body: &Ir, out: &mut String, indent: Indent, sx: &Syntax) {
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

fn needs_terminator(ir: &Ir, sx: &Syntax) -> bool {
    if sx.statement_terminator.is_empty() { return false; }
    !matches!(
        ir,
        Ir::If { .. } | Ir::While { .. } | Ir::Foreach { .. } | Ir::CFor { .. }
            | Ir::For { .. } | Ir::Function { .. } | Ir::Class { .. }
            | Ir::Namespace { .. } | Ir::Try { .. } | Ir::Comment { .. }
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
