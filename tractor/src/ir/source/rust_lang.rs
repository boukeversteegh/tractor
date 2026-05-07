//! Rust: IR → source code (canonical, no-anchor).
#![allow(dead_code)]
use super::common::{write_ir, Indent, Syntax};
use crate::ir::types::Ir;

pub fn render(ir: &Ir) -> String {
    let mut out = String::new();
    write_ir(ir, &mut out, Indent::SPACES_4, &rust_syntax());
    out
}

fn rust_syntax() -> Syntax {
    Syntax {
        fn_keyword: "fn", class_keyword: "struct", interface_keyword: "trait",
        return_keyword: "return", if_keyword: "if", elif_keyword: "else if",
        else_keyword: "else", while_keyword: "while", for_keyword: "for",
        foreach_keyword: "for", foreach_in: " in ",
        break_keyword: "break", continue_keyword: "continue",
        null_keyword: "None", true_keyword: "true", false_keyword: "false",
        new_keyword: "",
        block_open: " {", block_close: "}",
        statement_terminator: ";", block_intro: "",
        paren_conditions: false, indent_blocks: false, typed_param_pre: false,
        indent: Indent::SPACES_4, comment_line: "//",
    }
}
