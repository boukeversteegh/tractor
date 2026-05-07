//! Ruby: IR → source code (canonical, no-anchor).
//!
//! Ruby uses `def`/`class`/`module`, `end`-terminated blocks (no
//! braces), no `;` between statements, no parens around conditions.
//! Modeled via empty `block_open` and `end` as `block_close`.
#![allow(dead_code)]
use super::common::{write_ir, Indent, Syntax};
use crate::ir::types::Ir;

pub fn render(ir: &Ir) -> String {
    let mut out = String::new();
    write_ir(ir, &mut out, Indent::SPACES_2, &ruby_syntax());
    out
}

fn ruby_syntax() -> Syntax {
    Syntax {
        fn_keyword: "def", class_keyword: "class", interface_keyword: "module",
        return_keyword: "return", if_keyword: "if", elif_keyword: "elsif",
        else_keyword: "else", while_keyword: "while", for_keyword: "for",
        foreach_keyword: "for", foreach_in: " in ",
        break_keyword: "break", continue_keyword: "next",
        null_keyword: "nil", true_keyword: "true", false_keyword: "false",
        new_keyword: ".new",
        block_open: "", block_close: "end",
        statement_terminator: "", block_intro: "",
        paren_conditions: false, indent_blocks: false, typed_param_pre: false,
        indent: Indent::SPACES_2, comment_line: "#",
    }
}
