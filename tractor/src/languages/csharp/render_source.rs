//! C#: tree → source code (canonical, no-anchor).
//!
//! C# uses braced blocks with `;` terminators and typed-pre method
//! signatures (`int Foo()` not `function Foo() -> int`).
//!
//! The per-language [`syntax`] config is consumed by the unified
//! [`crate::tree::render::render`] walker (S13-Z7); the legacy
//! [`render`] wrapper remains for callers that want the canonical-
//! only entry point without a source anchor.


use crate::tree::render::common::{write_ir, Indent, Syntax};
use crate::tree::types::SyntaxTree;

pub fn render(tree: &SyntaxTree) -> String {
    let mut out = String::new();
    write_ir(tree, &mut out, Indent::SPACES_4, &syntax());
    out
}

/// Per-language Syntax config — exposed for use by the unified
/// renderer at [`crate::tree::render::render`].
pub fn syntax() -> Syntax {
    Syntax {
        fn_keyword: "void", class_keyword: "class", interface_keyword: "interface",
        return_keyword: "return", if_keyword: "if", elif_keyword: "else if",
        else_keyword: "else", while_keyword: "while", for_keyword: "for",
        foreach_keyword: "foreach", foreach_in: " in ",
        break_keyword: "break", continue_keyword: "continue",
        null_keyword: "null", true_keyword: "true", false_keyword: "false",
        new_keyword: "new",
        block_open: " {", block_close: "}",
        statement_terminator: ";", block_intro: "",
        paren_conditions: true, indent_blocks: false, typed_param_pre: true,
        indent: Indent::SPACES_4, comment_line: "//",
    }
}
