//! T-SQL tree-sitter CST → IR lowering.
//!
//! T-SQL has 552 distinct CST kinds, most of which are reserved-
//! keyword leaves (`keyword_*`). Those are detached uniformly
//! (no semantic), leaving only ~80 structural kinds to type.
//!
//! **Status: under construction.** Production parser does NOT yet
//! route T-SQL through this lowering.

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::lower_helpers::{range_of, span_of};
use super::types::{Ir, Modifiers};

pub fn lower_tsql_root(root: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(root);
    let range = range_of(root);
    match root.kind() {
        "program" => Ir::Module {
            element_name: "file",
            // Walk all children to swallow trailing `;` separators
            // between statements via `Ir::Skip`.
            children: {
                let mut cur = root.walk();
                root.children(&mut cur)
                    .map(|c| {
                        if c.is_named() {
                            lower_node(c, source)
                        } else {
                            Ir::Skip { range: range_of(c), span: span_of(c) }
                        }
                    })
                    .collect()
            },
            range, span,
        },
        other => Ir::Unknown { kind: other.to_string(), range, span },
    }
}

pub fn lower_tsql_node(node: TsNode<'_>, source: &str) -> Ir {
    lower_node(node, source)
}

fn lower_node(node: TsNode<'_>, source: &str) -> Ir {
    let span = span_of(node);
    let range = range_of(node);
    let kind = node.kind();

    // All `keyword_*` and the `op_*` operator leaves are detached —
    // their text is syntax (Principle #2), and the parent element
    // name conveys the role. Emit `Ir::Skip` so the parent's
    // `render_with_gaps` advances its cursor past the keyword
    // bytes without leaking them as text content.
    if kind.starts_with("keyword_") || kind.starts_with("op_") {
        return Ir::Skip { range, span };
    }

    match kind {
        // ----- Atoms ---------------------------------------------------
        "identifier" => {
            // T-SQL classifies identifier text by sigil:
            //   `@StartDate` → <var>      (T-SQL variable)
            //   `[bracketed]` → emit `<name>bracketed</name>` and
            //                   `Ir::Skip` the `[`/`]` delimiters so
            //                   they don't leak as gap text under
            //                   the parent (Principle #2 — the
            //                   brackets are quoting syntax, not
            //                   part of the identifier name).
            // The plain identifier renders as <name>; the role-based
            // alias / schema rename happens in the `term` and
            // `object_reference` arms below (they re-lower the
            // child as Atom("alias") / Atom("schema")).
            let text = range.slice(source);
            if text.starts_with('@') {
                Ir::Atom { element_name: "var", range, span }
            } else {
                bracket_stripped_name(range, span, source, "name")
            }
        }
        "object_reference" => lower_object_reference(node, source),
        "column_reference" | "field" => {
            // `field` and `column_reference` can be either a bare
            // identifier (`Name`) or a dot-chain of qualifiers
            // (`[dbo].[Users].[Name]`). For the bare case, lower
            // as a single Name (with `@var` sigil-detection); for
            // the chain case, recurse into children so each
            // `identifier` / `object_reference` segment goes
            // through its own bracket-stripping path.
            let text = range.slice(source);
            if text.starts_with('@') {
                return Ir::Atom { element_name: "var", range, span };
            }
            let mut cursor = node.walk();
            let named: Vec<TsNode<'_>> = node.named_children(&mut cursor).collect();
            if named.len() == 1 && named[0].kind() == "identifier" {
                bracket_stripped_name(range, span, source, "name")
            } else {
                // Walk all children to swallow anonymous `.` tokens
                // between segments via `Ir::Skip` (otherwise they
                // leak as gap text).
                let mut cur = node.walk();
                let kids: Vec<Ir> = node
                    .children(&mut cur)
                    .map(|c| {
                        if c.is_named() {
                            lower_node(c, source)
                        } else {
                            Ir::Skip { range: range_of(c), span: span_of(c) }
                        }
                    })
                    .collect();
                Ir::Inline {
                    children: kids,
                    list_name: None,
                    range, span,
                }
            }
        }
        "int" => Ir::Int { range, span },
        "literal" => simple_statement(node, "literal", source),
        "string" | "national_string" => Ir::String { range, span },
        "comment" | "line_comment" | "block_comment" => {
            Ir::Comment { leading: false, trailing: false, range, span }
        }

        // ----- Statements ----------------------------------------------
        "statement" => simple_statement(node, "statement", source),
        "go_statement" => simple_statement(node, "go", source),
        "execute_statement" => simple_statement(node, "exec", source),
        "set_statement" => simple_statement(node, "set", source),
        "select_expression" => {
            // Walk all children to swallow anonymous `,` separators
            // between columns via `Ir::Skip`.
            let mut cur = node.walk();
            let kids: Vec<Ir> = node
                .children(&mut cur)
                .map(|c| {
                    if c.is_named() {
                        lower_node(c, source)
                    } else {
                        Ir::Skip { range: range_of(c), span: span_of(c) }
                    }
                })
                .collect();
            Ir::Inline { children: kids, list_name: None, range, span }
        },
        "select" => simple_statement(node, "select", source),
        "from" => simple_statement(node, "from", source),
        "where" => simple_statement(node, "where", source),
        "group_by" => simple_statement(node, "group", source),
        "having" => simple_statement(node, "having", source),
        "order_by" => simple_statement(node, "order", source),
        "order_target" => simple_statement(node, "target", source),
        "partition_by" => simple_statement(node, "partition", source),
        "join" => lower_tsql_join(node, source),
        "insert" => lower_tsql_insert(node, source),
        "update" => simple_statement(node, "update", source),
        "delete" => simple_statement(node, "delete", source),
        "transaction" => simple_statement(node, "transaction", source),
        "subquery" => simple_statement(node, "subquery", source),
        "set_operation" => simple_statement(node, "union", source),
        "cte" => simple_statement(node, "cte", source),

        // ----- DDL -----------------------------------------------------
        "create_table" => simple_statement(node, "create", source),
        "create_index" => simple_statement(node, "create", source),
        "create_view" => simple_statement(node, "create", source),
        "create_function" => simple_statement(node, "function", source),
        "create_query" => Ir::Inline {
            // `CREATE VIEW … AS <query>` — the query CST wraps
            // select/from/where, but we want them as direct
            // children of `<create>` (no opaque wrapper). Inline
            // its children up.
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "drop_table" => simple_statement(node, "drop", source),
        "drop_index" => simple_statement(node, "drop", source),
        "alter_table" => simple_statement(node, "alter", source),
        "add_column" => simple_statement(node, "column", source),
        "add_constraint" => simple_statement(node, "constraint", source),
        // The grammar nests `constraint` inside `add_constraint`;
        // inline the inner so its body (FOREIGN KEY ..., REFERENCES
        // ...) surfaces as direct children of the outer
        // `<constraint>` instead of `<constraint>/<constraint>/...`.
        // Walk all children (not just named) so anonymous parens
        // around the referenced PK column get `Ir::Skip`'d
        // (otherwise they leak as gap text under the parent).
        "constraint" => {
            let mut cur = node.walk();
            let kids: Vec<Ir> = node.children(&mut cur)
                .map(|c| {
                    if c.is_named() {
                        lower_node(c, source)
                    } else {
                        Ir::Skip { range: range_of(c), span: span_of(c) }
                    }
                })
                .collect();
            Ir::Inline { children: kids, list_name: None, range, span }
        },
        "ordered_columns" => simple_statement(node, "columns", source),
        "column_definition" => simple_statement(node, "definition", source),
        "column_definitions" => simple_statement(node, "columns", source),
        "column" => simple_statement(node, "column", source),
        "index_fields" => simple_statement(node, "columns", source),
        "function_body" => simple_statement(node, "body", source),

        // ----- Expressions ---------------------------------------------
        "binary_expression" => lower_binary_expression(node, source),
        "unary_expression" => lower_unary_expression(node, source),
        "assignment" => lower_tsql_assignment(node, source),
        "between_expression" => lower_tsql_between(node, source),
        "exists" => simple_statement(node, "exists", source),
        "case" => lower_tsql_case(node, source),
        "when_clause" => simple_statement(node, "when", source),
        "cast" => simple_statement(node, "cast", source),
        "invocation" => simple_statement(node, "call", source),
        "function_argument" => simple_statement(node, "arg", source),
        "function_arguments" => Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        },
        "all_fields" => simple_statement(node, "star", source),
        "list" => simple_statement(node, "list", source),
        "term" => Ir::SimpleStatement {
            element_name: "column",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: lower_term_children(node, source),
            range, span,
        },
        "relation" => Ir::SimpleStatement {
            element_name: "relation",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: lower_term_children(node, source),
            range, span,
        },
        "direction" => simple_statement(node, "direction", source),
        "window_function" => simple_statement(node, "window", source),
        "window_specification" => simple_statement(node, "over", source),

        // ----- Type keywords (datatypes) -------------------------------
        "varchar" => simple_statement(node, "varchar", source),
        "nvarchar" => simple_statement(node, "nvarchar", source),

        other => Ir::Unknown { kind: other.to_string(), range, span },
    }
}

/// Lower a bracket-quoted T-SQL identifier (`[dbo]` /
/// `[Users]`) as `Ir::Inline { Skip("["), Atom(<name>dbo), Skip("]") }`
/// — the inner Atom carries the bare identifier text, the
/// flanking Skips consume the bracket bytes so the parent's gap
/// rendering doesn't leak them as text. Plain identifiers (no
/// brackets) skip the wrap and lower to a bare Atom.
///
/// `element_name` controls the inner Atom's tag (`name` for plain
/// identifiers, `schema` / `alias` for role-classified ones — see
/// the call sites in `lower_object_reference` and
/// `lower_term_children`).
fn bracket_stripped_name(
    range: super::types::ByteRange,
    span: super::types::Span,
    source: &str,
    element_name: &'static str,
) -> Ir {
    let bytes = source.as_bytes();
    let len = (range.end - range.start) as usize;
    let is_bracketed = len >= 2
        && bytes.get(range.start as usize) == Some(&b'[')
        && bytes.get((range.end - 1) as usize) == Some(&b']');
    if !is_bracketed {
        return if element_name == "name" {
            Ir::Name { range, span }
        } else {
            Ir::Atom { element_name, range, span }
        };
    }
    let inner = super::types::ByteRange::new(range.start + 1, range.end - 1);
    let lbracket = super::types::ByteRange::new(range.start, range.start + 1);
    let rbracket = super::types::ByteRange::new(range.end - 1, range.end);
    let inner_atom = if element_name == "name" {
        Ir::Name { range: inner, span }
    } else {
        Ir::Atom { element_name, range: inner, span }
    };
    Ir::Inline {
        children: vec![
            Ir::Skip { range: lbracket, span },
            inner_atom,
            Ir::Skip { range: rbracket, span },
        ],
        list_name: None,
        range,
        span,
    }
}

fn simple_statement(node: TsNode<'_>, element_name: &'static str, source: &str) -> Ir {
    // Walk *all* children (named + anonymous). Anonymous tokens —
    // commas, parens, dots, etc. — lower to `Ir::Skip` so their
    // bytes don't leak as gap text under the parent. Named
    // children lower normally; the early `keyword_*` / `op_*`
    // detection in `lower_node` already produces `Ir::Skip` for
    // those.
    let mut cursor = node.walk();
    let children: Vec<Ir> = node
        .children(&mut cursor)
        .map(|c| {
            if c.is_named() {
                lower_node(c, source)
            } else {
                Ir::Skip { range: range_of(c), span: span_of(c) }
            }
        })
        .collect();
    Ir::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range: range_of(node),
        span: span_of(node),
    }
}

fn lower_children(node: TsNode<'_>, source: &str) -> Vec<Ir> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).map(|c| lower_node(c, source)).collect()
}

/// `object_reference` is one or more dot-separated identifiers —
/// `dbo.Users`, `srv.db.dbo.Users`. The leading qualifiers are
/// schemas/databases/servers; the *last* identifier is the object
/// itself. Per the imperative pipeline's behavior, mark the leading
/// qualifier(s) as `<schema>`. The single-identifier case stays
/// as a plain `<name>`.
fn lower_object_reference(node: TsNode<'_>, source: &str) -> Ir {
    let range = range_of(node);
    let span = span_of(node);
    let mut cursor = node.walk();
    let id_children: Vec<TsNode<'_>> = node
        .named_children(&mut cursor)
        .filter(|c| c.kind() == "identifier")
        .collect();
    if id_children.len() < 2 {
        // Plain `Users` — re-lower children normally.
        return Ir::Inline {
            children: lower_children(node, source),
            list_name: None,
            range, span,
        };
    }
    // Qualified — leading identifiers become <schema>, last one
    // stays as <name>. Walk *all* children (named + anonymous) so
    // we can `Ir::Skip` the inter-segment `.` punctuation tokens
    // — without that, the dots leak as gap text between segments.
    let last_idx = id_children.len() - 1;
    let last_id_byte = id_children[last_idx].start_byte();
    let mut cursor2 = node.walk();
    let children: Vec<Ir> = node
        .children(&mut cursor2)
        .map(|c| {
            if c.is_named() {
                if c.kind() == "identifier" && c.start_byte() != last_id_byte {
                    bracket_stripped_name(range_of(c), span_of(c), source, "schema")
                } else {
                    lower_node(c, source)
                }
            } else {
                // Anonymous tokens (`.`, etc.) — consume their bytes
                // so they don't leak as gap text.
                Ir::Skip { range: range_of(c), span: span_of(c) }
            }
        })
        .collect();
    Ir::Inline {
        children,
        list_name: None,
        range, span,
    }
}

/// In a `term`, an optional alias appears as the trailing `identifier`
/// after the value-bearing first child (`field` / `invocation` /
/// literal / etc.). The alias may be preceded by `keyword_as` (an
/// anonymous keyword child); we only inspect the named-child sequence.
fn lower_term_children(node: TsNode<'_>, source: &str) -> Vec<Ir> {
    let mut cursor = node.walk();
    let named: Vec<TsNode<'_>> = node.named_children(&mut cursor).collect();
    // An alias only exists when there's MORE than one named child *and*
    // the last one is a bare `identifier`. Single-child terms are just
    // their value (`*`, `field/...`, ...).
    let alias_id = if named.len() >= 2 && named.last().map(|n| n.kind()) == Some("identifier") {
        named.last().map(|n| n.start_byte())
    } else {
        None
    };
    named
        .into_iter()
        .map(|c| match alias_id {
            Some(a) if c.kind() == "identifier" && c.start_byte() == a => {
                bracket_stripped_name(range_of(c), span_of(c), source, "alias")
            }
            _ => lower_node(c, source),
        })
        .collect()
}

/// `between_expression` (`x BETWEEN low AND high`) — wrap each
/// operand in a named slot: `<value>` for the test expression,
/// `<lower>` and `<upper>` for the bounds. Without slots, the
/// three values surfaced as flat siblings (`name`, `literal`,
/// `literal`) and queries couldn't tell which was the test
/// expression vs the bounds.
fn lower_tsql_between(node: TsNode<'_>, source: &str) -> Ir {
    let range = range_of(node);
    let span = span_of(node);
    // Collect operand-bearing children (skipping keyword_*/op_*).
    let mut operand_cursor = node.walk();
    let operand_nodes: Vec<TsNode<'_>> = node.named_children(&mut operand_cursor)
        .filter(|c| {
            let kind = c.kind();
            !kind.starts_with("keyword_") && !kind.starts_with("op_")
        })
        .collect();
    let value_id = operand_nodes.first().map(|n| n.id());
    let low_id = operand_nodes.get(1).map(|n| n.id());
    let high_id = operand_nodes.get(2).map(|n| n.id());
    // Walk *all* children so keyword/op/anonymous tokens get
    // `Ir::Skip`'d (otherwise BETWEEN / AND / parens leak as
    // gap text). Operand-bearing identifiers/literals get
    // wrapped in their respective slots.
    let mut cursor = node.walk();
    let children: Vec<Ir> = node.children(&mut cursor)
        .map(|c| {
            let cid = c.id();
            let slot = if Some(cid) == value_id { Some("value") }
                       else if Some(cid) == low_id { Some("low") }
                       else if Some(cid) == high_id { Some("high") }
                       else { None };
            if let Some(slot_name) = slot {
                return Ir::SimpleStatement {
                    element_name: slot_name,
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: vec![lower_node(c, source)],
                    range: range_of(c),
                    span: span_of(c),
                };
            }
            // Anonymous OR keyword/op children → Skip.
            let kind = c.kind();
            if !c.is_named() || kind.starts_with("keyword_") || kind.starts_with("op_") {
                Ir::Skip { range: range_of(c), span: span_of(c) }
            } else {
                lower_node(c, source)
            }
        })
        .collect();
    Ir::SimpleStatement {
        element_name: "between",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range,
        span,
    }
}

/// `insert` — differentiate the column list `(c1, c2)` from the
/// `VALUES (v1, v2)` list. Both surfaced as `<list>` siblings,
/// indistinguishable in queries. Rename the column list to
/// `<columns>` and the values list to `<values>`.
///
/// State machine: track whether we've seen `keyword_values`. The
/// list before VALUES is the column list; the list after is the
/// values list.
fn lower_tsql_insert(node: TsNode<'_>, source: &str) -> Ir {
    let range = range_of(node);
    let span = span_of(node);
    let mut seen_values_kw = false;
    let mut cursor = node.walk();
    let children: Vec<Ir> = node
        .children(&mut cursor)
        .map(|c| {
            let kind = c.kind();
            if kind == "keyword_values" {
                seen_values_kw = true;
                return Ir::Skip { range: range_of(c), span: span_of(c) };
            }
            if !c.is_named() {
                return Ir::Skip { range: range_of(c), span: span_of(c) };
            }
            if kind == "list" {
                // Lower the `list` CST inline as the slot's
                // children — drop the redundant `<list>` wrapper
                // since `<columns>` / `<values>` already conveys
                // the role. Walk children, Skipping `(`/`)`/`,`.
                let slot = if seen_values_kw { "values" } else { "columns" };
                let mut sub_cur = c.walk();
                let inner: Vec<Ir> = c.children(&mut sub_cur)
                    .map(|gc| {
                        if gc.is_named() {
                            lower_node(gc, source)
                        } else {
                            Ir::Skip { range: range_of(gc), span: span_of(gc) }
                        }
                    })
                    .collect();
                return Ir::SimpleStatement {
                    element_name: slot,
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: inner,
                    range: range_of(c),
                    span: span_of(c),
                };
            }
            lower_node(c, source)
        })
        .collect();
    Ir::SimpleStatement {
        element_name: "insert",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range,
        span,
    }
}

/// `case` — group `WHEN cond THEN val` clauses into `<when>` slot
/// wrappers and the `ELSE val` clause into an `<else>` slot.
///
/// The CST is flat: `keyword_case`, `keyword_when`, condition_expr,
/// `keyword_then`, then_value, (repeated), `keyword_else`,
/// else_value, `keyword_end`. Without grouping, the IR rendered
/// `case/{compare, literal, literal}` and queries couldn't tell the
/// condition from the THEN/ELSE values, nor pair THENs to their
/// WHEN conditions.
fn lower_tsql_case(node: TsNode<'_>, source: &str) -> Ir {
    let range = range_of(node);
    let span = span_of(node);
    let span_default = span;

    #[derive(Copy, Clone, PartialEq)]
    enum State { Before, AfterWhen, AfterThen, AfterElse, Done }
    let mut state = State::Before;
    let mut when_pending: Vec<Ir> = Vec::new();
    let mut when_start: u32 = 0;
    let mut else_pending: Vec<Ir> = Vec::new();
    let mut else_start: u32 = 0;
    let mut children: Vec<Ir> = Vec::new();

    let flush_when = |children: &mut Vec<Ir>, pending: &mut Vec<Ir>, start: u32| {
        if pending.is_empty() { return; }
        let end = pending.last().map(|ir| ir.range().end).unwrap_or(start);
        let drained: Vec<Ir> = pending.drain(..).collect();
        children.push(Ir::SimpleStatement {
            element_name: "when",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: drained,
            range: super::types::ByteRange::new(start, end),
            span: span_default,
        });
    };
    let flush_else = |children: &mut Vec<Ir>, pending: &mut Vec<Ir>, start: u32| {
        if pending.is_empty() { return; }
        let end = pending.last().map(|ir| ir.range().end).unwrap_or(start);
        let drained: Vec<Ir> = pending.drain(..).collect();
        children.push(Ir::SimpleStatement {
            element_name: "else",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: drained,
            range: super::types::ByteRange::new(start, end),
            span: span_default,
        });
    };

    let mut cursor = node.walk();
    for c in node.children(&mut cursor) {
        let c_range = range_of(c);
        let c_span = span_of(c);
        let kind = c.kind();
        let is_kw_when = kind == "keyword_when";
        let is_kw_then = kind == "keyword_then";
        let is_kw_else = kind == "keyword_else";
        let is_kw_end  = kind == "keyword_end";

        if is_kw_when {
            flush_when(&mut children, &mut when_pending, when_start);
            when_start = c_range.start;
            when_pending.push(Ir::Skip { range: c_range, span: c_span });
            state = State::AfterWhen;
            continue;
        }
        if is_kw_then {
            when_pending.push(Ir::Skip { range: c_range, span: c_span });
            state = State::AfterThen;
            continue;
        }
        if is_kw_else {
            flush_when(&mut children, &mut when_pending, when_start);
            else_start = c_range.start;
            else_pending.push(Ir::Skip { range: c_range, span: c_span });
            state = State::AfterElse;
            continue;
        }
        if is_kw_end {
            flush_when(&mut children, &mut when_pending, when_start);
            flush_else(&mut children, &mut else_pending, else_start);
            children.push(Ir::Skip { range: c_range, span: c_span });
            state = State::Done;
            continue;
        }

        let skip_kind = !c.is_named()
            || kind.starts_with("keyword_")
            || kind.starts_with("op_");
        let ir = if skip_kind {
            Ir::Skip { range: c_range, span: c_span }
        } else {
            lower_node(c, source)
        };
        match state {
            State::AfterWhen | State::AfterThen => when_pending.push(ir),
            State::AfterElse => else_pending.push(ir),
            _ => children.push(ir),
        }
    }
    // END keyword can be absent in some malformed parses — still flush.
    flush_when(&mut children, &mut when_pending, when_start);
    flush_else(&mut children, &mut else_pending, else_start);
    let _ = state;

    Ir::SimpleStatement {
        element_name: "case",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range,
        span,
    }
}

/// `join` — emit direction markers (`[left]`, `[right]`, `[full]`,
/// `[outer]`, `[cross]`) for the LEFT / RIGHT / FULL / OUTER /
/// CROSS keywords in the source. INNER is the default and emits
/// no marker. Without these, `LEFT JOIN` rendered identically to
/// plain `JOIN`.
fn lower_tsql_join(node: TsNode<'_>, source: &str) -> Ir {
    let range = range_of(node);
    let span = span_of(node);
    let mut cursor = node.walk();
    let mut children: Vec<Ir> = Vec::new();
    for c in node.children(&mut cursor) {
        if c.is_named() {
            let kind = c.kind();
            let marker_name = match kind {
                "keyword_left"  => Some("left"),
                "keyword_right" => Some("right"),
                "keyword_full"  => Some("full"),
                "keyword_outer" => Some("outer"),
                "keyword_cross" => Some("cross"),
                _ => None,
            };
            if let Some(m) = marker_name {
                // Two children for one keyword: a Skip that consumes
                // the keyword's source bytes (so they don't leak as
                // gap text), and an empty SimpleStatement at the
                // keyword's end position that renders as `<{m}/>`
                // — the empty-element pass folds it into the
                // parent join's `[…]` marker chip.
                let kw_range = range_of(c);
                children.push(Ir::Skip { range: kw_range, span: span_of(c) });
                let zero = super::types::ByteRange::empty_at(kw_range.end);
                children.push(Ir::SimpleStatement {
                    element_name: m,
                    modifiers: Modifiers::default(),
                    extra_markers: &[],
                    children: Vec::new(),
                    range: zero,
                    span: span_of(c),
                });
                continue;
            }
            // Other keyword/op kinds → Skip (consume their bytes).
            if kind.starts_with("keyword_") || kind.starts_with("op_") {
                children.push(Ir::Skip { range: range_of(c), span: span_of(c) });
                continue;
            }
            children.push(lower_node(c, source));
        } else {
            children.push(Ir::Skip { range: range_of(c), span: span_of(c) });
        }
    }
    Ir::SimpleStatement {
        element_name: "join",
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range,
        span,
    }
}

/// `assignment` (`x = expr`) — lower as `Ir::Binary` with
/// element_name="assign" so the LHS / RHS land in `<left>` /
/// `<right>` slots and the `=` carries an `<equals/>` marker
/// (per the cross-language assign shape).
fn lower_tsql_assignment(node: TsNode<'_>, source: &str) -> Ir {
    let range = range_of(node);
    let span = span_of(node);
    let mut cursor = node.walk();
    let mut left: Option<Ir> = None;
    let mut right: Option<Ir> = None;
    let mut op_text = String::new();
    let mut op_range: Option<super::types::ByteRange> = None;
    for c in node.children(&mut cursor) {
        if c.is_named() {
            let kind = c.kind();
            if kind.starts_with("keyword_") || kind.starts_with("op_") {
                if op_range.is_none() {
                    if let Ok(text) = c.utf8_text(source.as_bytes()) {
                        op_text = text.trim().to_string();
                        op_range = Some(range_of(c));
                    }
                }
                continue;
            }
            let ir = lower_node(c, source);
            if left.is_none() {
                left = Some(ir);
            } else if right.is_none() {
                right = Some(ir);
            }
        } else {
            if let Ok(text) = c.utf8_text(source.as_bytes()) {
                let trimmed = text.trim();
                if !trimmed.is_empty()
                    && !trimmed.chars().all(|ch| ch.is_alphanumeric() || ch == '_')
                    && op_range.is_none()
                {
                    op_text = trimmed.to_string();
                    op_range = Some(range_of(c));
                }
            }
        }
    }
    let (Some(l), Some(r), Some(opr)) = (left, right, op_range) else {
        return Ir::SimpleStatement {
            element_name: "assign",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: Vec::new(),
            range, span,
        };
    };
    Ir::Binary {
        element_name: "assign",
        op_text,
        op_marker: "",
        op_range: opr,
        left: Box::new(l),
        right: Box::new(r),
        range,
        span,
    }
}

/// `binary_expression` — lower as `Ir::Binary` so the rendered shape
/// is `<compare><left><expression>{lhs}</expression></left><op>{op}<{marker}/>…</op><right><expression>{rhs}</expression></right></compare>`,
/// matching the cross-language compare/binary shape (and getting
/// the `OPERATOR_MARKERS` table's marker structure for free —
/// `<compare/><greater/>`, `<compare/><less and equal/>`, etc.).
fn lower_binary_expression(node: TsNode<'_>, source: &str) -> Ir {
    let range = range_of(node);
    let span = span_of(node);
    let mut cursor = node.walk();
    let mut left: Option<Ir> = None;
    let mut right: Option<Ir> = None;
    let mut op_text = String::new();
    let mut op_range: Option<super::types::ByteRange> = None;
    for c in node.children(&mut cursor) {
        if c.is_named() {
            // Treat named keyword/op children (`keyword_in`, etc.)
            // as operators, not operands. They lower to `Ir::Skip`
            // through `lower_node`, but we want to capture their
            // text and range here for the `Ir::Binary` op slot.
            let kind = c.kind();
            if kind.starts_with("keyword_") || kind.starts_with("op_") {
                if op_range.is_none() {
                    if let Ok(text) = c.utf8_text(source.as_bytes()) {
                        op_text = text.trim().to_string();
                        op_range = Some(range_of(c));
                    }
                }
                continue;
            }
            let ir = lower_node(c, source);
            if left.is_none() {
                left = Some(ir);
            } else if right.is_none() {
                right = Some(ir);
            }
        } else {
            // Anonymous child — typically the operator (`>`, `=`, …).
            if let Ok(text) = c.utf8_text(source.as_bytes()) {
                let trimmed = text.trim();
                if !trimmed.is_empty()
                    && !trimmed.chars().all(|ch| ch.is_alphanumeric() || ch == '_')
                    && op_range.is_none()
                {
                    op_text = trimmed.to_string();
                    op_range = Some(range_of(c));
                }
            }
        }
    }
    let (Some(l), Some(r), Some(opr)) = (left, right, op_range) else {
        // Fall back to flat shape if we couldn't extract operands —
        // shouldn't happen for well-formed CSTs but keeps the
        // function total.
        return Ir::SimpleStatement {
            element_name: "compare",
            modifiers: Modifiers::default(),
            extra_markers: &[],
            children: Vec::new(),
            range, span,
        };
    };
    Ir::Binary {
        element_name: "compare",
        op_text,
        op_marker: "",
        op_range: opr,
        left: Box::new(l),
        right: Box::new(r),
        range,
        span,
    }
}

/// `unary_expression` — when the operator is `#`, this is a T-SQL
/// local-temp-table reference (`#TempUsers`). Render as a `<temp>`
/// element rather than the generic `<unary>`.
fn lower_unary_expression(node: TsNode<'_>, source: &str) -> Ir {
    let range = range_of(node);
    let span = span_of(node);
    let mut cursor = node.walk();
    let mut is_temp = false;
    for c in node.named_children(&mut cursor) {
        if c.kind() == "op_unary_other" {
            if let Ok(text) = c.utf8_text(source.as_bytes()) {
                if text.trim() == "#" {
                    is_temp = true;
                    break;
                }
            }
        }
    }
    let element_name = if is_temp { "temp" } else { "unary" };
    let mut cursor2 = node.walk();
    let children: Vec<Ir> = node.named_children(&mut cursor2).map(|c| lower_node(c, source)).collect();
    Ir::SimpleStatement {
        element_name,
        modifiers: Modifiers::default(),
        extra_markers: &[],
        children,
        range, span,
    }
}

