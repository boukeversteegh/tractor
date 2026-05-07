//! Go: IR → source code (canonical, no-anchor).
#![allow(dead_code)]
use super::common::{write_ir, Indent, Syntax};
use crate::ir::types::Ir;

pub fn render(ir: &Ir) -> String {
    let mut out = String::new();
    write_ir(ir, &mut out, Indent::TAB, &go_syntax());
    out
}

fn go_syntax() -> Syntax {
    Syntax {
        fn_keyword: "func", class_keyword: "type", interface_keyword: "type",
        return_keyword: "return", if_keyword: "if", elif_keyword: "else if",
        else_keyword: "else", while_keyword: "for", for_keyword: "for",
        foreach_keyword: "for", foreach_in: " range ",
        break_keyword: "break", continue_keyword: "continue",
        null_keyword: "nil", true_keyword: "true", false_keyword: "false",
        new_keyword: "new",
        block_open: " {", block_close: "}",
        statement_terminator: "", block_intro: "",
        paren_conditions: false, indent_blocks: false, typed_param_pre: false,
        indent: Indent::TAB, comment_line: "//",
    }
}
