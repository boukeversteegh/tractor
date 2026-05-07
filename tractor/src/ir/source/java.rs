//! Java: IR → source code (canonical, no-anchor).
#![allow(dead_code)]
use super::common::{write_ir, Indent, Syntax};
use crate::ir::types::Ir;

pub fn render(ir: &Ir) -> String {
    let mut out = String::new();
    write_ir(ir, &mut out, Indent::SPACES_4, &java_syntax());
    out
}

fn java_syntax() -> Syntax {
    Syntax {
        fn_keyword: "void", class_keyword: "class", interface_keyword: "interface",
        return_keyword: "return", if_keyword: "if", elif_keyword: "else if",
        else_keyword: "else", while_keyword: "while", for_keyword: "for",
        foreach_keyword: "for", foreach_in: " : ",
        break_keyword: "break", continue_keyword: "continue",
        null_keyword: "null", true_keyword: "true", false_keyword: "false",
        new_keyword: "new",
        block_open: " {", block_close: "}",
        statement_terminator: ";", block_intro: "",
        paren_conditions: true, indent_blocks: false, typed_param_pre: true,
        indent: Indent::SPACES_4, comment_line: "//",
    }
}
