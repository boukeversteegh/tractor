//! Markdown semantic shape: headings, lists, blockquotes, fenced
//! code blocks, and horizontal rules.
//!
//! The semantic transform exposes the structural intent of the
//! document — `<heading[h1]>`, `<list[ordered]>`, `<codeblock>`
//! with a `<language>` child — rather than the raw tree-sitter
//! kinds. Inline emphasis (bold/italic) is intentionally not
//! pinned by these tests; only the block-level structure is.
//!
//! One focused source per construct.

use crate::support::semantic::*;

#[test]
fn markdown_heading_levels() {
    let mut tree = parse_src("markdown", "# Top\n\n## Sub\n");

    claim("`# Top` renders as <heading[h1]>",
        &mut tree, "//heading[h1]", 1);

    claim("`## Sub` renders as <heading[h2]>",
        &mut tree, "//heading[h2]", 1);
}

#[test]
fn markdown_unordered_list_with_items() {
    claim("`- a / - b / - c` produces a <list[unordered]> with three <item>s",
        &mut parse_src("markdown", "- a\n- b\n- c\n"),
        &multi_xpath(r#"
            //list[unordered]
                [count(item)=3]
        "#),
        1);
}

#[test]
fn markdown_ordered_list_with_items() {
    claim("`1. a / 2. b` produces a <list[ordered]> with two <item>s",
        &mut parse_src("markdown", "1. a\n2. b\n"),
        &multi_xpath(r#"
            //list[ordered]
                [count(item)=2]
        "#),
        1);
}

#[test]
fn markdown_blockquote() {
    claim("`> quoted` renders as <blockquote>",
        &mut parse_src("markdown", "> quoted\n"),
        "//blockquote",
        1);
}

#[test]
fn markdown_fenced_code_block_with_language() {
    claim("a ```python``` fence carries <language>python</language>",
        &mut parse_src("markdown", "```python\nprint('hi')\n```\n"),
        "//codeblock[language='python']",
        1);
}

#[test]
fn markdown_fenced_code_block_without_language() {
    claim("an unlabeled fence produces a <codeblock> without a <language> child",
        &mut parse_src("markdown", "```\nplain\n```\n"),
        "//codeblock[not(language)]",
        1);
}

#[test]
fn markdown_horizontal_rule() {
    claim("`---` renders as <hr>",
        &mut parse_src("markdown", "before\n\n---\n\nafter\n"),
        "//hr",
        1);
}
