//! T-SQL: IR → source code (canonical, no-anchor).
//!
//! T-SQL is a query language with shape very different from
//! procedural languages. The IR currently models procedural-style
//! constructs (DECLARE/IF/WHILE/etc.); SELECT/FROM/WHERE clauses are
//! emitted via the SimpleStatement fallback for now. SQL-specific
//! rendering can be added as the IR's SQL coverage grows.
#![allow(dead_code)]
use super::common::{write_ir, Indent, Syntax};
use crate::ir::types::Ir;

pub fn render(ir: &Ir) -> String {
    let mut out = String::new();
    write_ir(ir, &mut out, Indent::SPACES_4, &tsql_syntax());
    out
}

fn tsql_syntax() -> Syntax {
    Syntax {
        fn_keyword: "FUNCTION", class_keyword: "TABLE", interface_keyword: "VIEW",
        return_keyword: "RETURN", if_keyword: "IF", elif_keyword: "ELSE IF",
        else_keyword: "ELSE", while_keyword: "WHILE", for_keyword: "FOR",
        foreach_keyword: "FOR", foreach_in: " IN ",
        break_keyword: "BREAK", continue_keyword: "CONTINUE",
        null_keyword: "NULL", true_keyword: "1", false_keyword: "0",
        new_keyword: "",
        block_open: "\nBEGIN", block_close: "END",
        statement_terminator: ";", block_intro: "",
        paren_conditions: false, indent_blocks: false, typed_param_pre: false,
        indent: Indent::SPACES_4, comment_line: "--",
    }
}
