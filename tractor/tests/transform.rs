//! Semantic-tree shape tests, organised by transformation category.
//!
//! Each test pins down one design-principle invariant with an
//! explicit XPath assertion. When an assertion fails, consult the
//! cited principle and the invariant description before touching the
//! test. The goal is that a failing assertion names the violated
//! principle clearly enough that a reviewer (or a coding agent)
//! cannot "fix" it by simply flipping the expected value.
//!
//! See `specs/tractor-parse/semantic-tree/design.md` for the
//! principle catalogue referenced in the comments below.
//!
//! Each test owns a minimal inline source and a handful of
//! assertions; no shared fixture files. The helpers (`parse_src`,
//! `query`, `claim`, `assert_count`, `assert_value`, `multi_xpath`)
//! live in `tests/support/semantic.rs`.
//!
//! Layout:
//!   - Cross-language categories live at the top of the tree
//!     (`comments`, `if_else`, `types`, ...).
//!   - Language-specific quirks live under
//!     `transform/<language>/<construct>.rs`.
//!
//! Each `mod` declaration uses an explicit `#[path]` so submodules
//! resolve to `tests/transform/<name>.rs` rather than the default
//! `tests/<name>.rs` integration-test placement.
//!
//! Style notes for shape claims (carried over from the previous
//! single-file layout):
//!
//!   - Source code uses raw strings, indented to fit the test.
//!   - **Be compact.** A shape claim should fit on one line whenever
//!     the path is short and the predicates fit. Only break across
//!     lines when the path is genuinely deep, or when several
//!     sibling structural conditions need their own line for clarity.
//!
//!   - When breaking, indent so the path mirrors the tree. Two
//!     equivalent styles — pick whichever reads better:
//!
//!     **Path** — counts the leaf:
//!     ```text
//!     //class
//!         /body
//!             /method[public][returns/type[name='int']]
//!     ```
//!
//!     **Bracket-predicate** — counts the root; nesting via `[…]`:
//!     ```text
//!     //class[
//!         body/method[public][returns/type[name='int']]
//!     ]
//!     ```
//!
//!   - Combine sibling predicates on the same node with `and`:
//!     `comment[not(leading) and not(trailing)]` — not separate `[…]`
//!     blocks. Bracket nesting is for HIERARCHY only.
//!
//!   - Don't mention things you don't care about. If the test is
//!     about trailing comments, write `//comment[trailing]`, not
//!     `//class/body/comment[trailing]` — unless the position
//!     matters.

mod support;

// ----- Cross-language categories ------------------------------------------

#[path = "transform/accessors.rs"]   mod accessors;
#[path = "transform/collections.rs"] mod collections;
#[path = "transform/comments.rs"]    mod comments;
#[path = "transform/decorators.rs"]  mod decorators;
#[path = "transform/flat_lists.rs"]  mod flat_lists;
#[path = "transform/functions.rs"]   mod functions;
#[path = "transform/if_else.rs"]     mod if_else;
#[path = "transform/modifiers.rs"]   mod modifiers;
#[path = "transform/patterns.rs"]    mod patterns;
#[path = "transform/strings.rs"]     mod strings;
#[path = "transform/types.rs"]       mod types;
#[path = "transform/variables.rs"]   mod variables;
#[path = "transform/visibility.rs"]  mod visibility;

// ----- Language-specific quirks -------------------------------------------

#[path = "transform/go"]
mod go {
    pub mod spec_flattening;
    pub mod struct_interface_hoist;
    pub mod switch_markers;
    pub mod type_declaration;
}

#[path = "transform/python"]
mod python {
    pub mod expression_list;
}

#[path = "transform/ruby"]
mod ruby {
    pub mod name_inlining;
}

#[path = "transform/rust"]
mod rust {
    pub mod match_expression;
    pub mod struct_expression;
}
