//! IR coverage audit.
//!
//! Round-trip identity (`to_source(ir, source) == source`) proves no
//! source bytes were lost. But it doesn't catch the **silent
//! structural drop** case: a typed parent IR (e.g. `Ir::Class`)
//! lowers most of its CST children but forgets one (say,
//! `attribute_list`). The dropped child's bytes still appear inside
//! the parent's gap text, so round-trip passes — but no IR node
//! represents the dropped kind, so XPath structural queries can't
//! find it.
//!
//! The audit walks both trees in lockstep:
//!
//! - For each named CST node, classify it against the IR's coverage:
//!   - **Typed** — an IR node has *exactly* this byte range and is
//!     not `Ir::Unknown`. The kind is structurally represented.
//!   - **Unknown** — an `Ir::Unknown` node has *exactly* this byte
//!     range. The kind is explicitly punted.
//!   - **Under-typed** — a typed IR ancestor's range contains this
//!     CST node but no IR has its exact range. Common case:
//!     chain-folded structure (the inner `member_access_expression`
//!     for `a.b` of `a.b.c` is folded into `Ir::Access`'s segment
//!     list). Acceptable when intentional; suspicious when it's
//!     meaningful structure that got buried.
//!   - **Under-unknown** — under an `Ir::Unknown`'s range. The whole
//!     subtree is unhandled at a higher level.
//!   - **Dropped** — no IR range covers this CST node at all.
//!     Should *never* happen if round-trip identity holds; existence
//!     would indicate a renderer bug.
//!
//! Aggregate metrics:
//! - **Kind coverage** — fraction of distinct CST kinds with ≥1
//!   typed instance. The public-facing support level.
//! - **Node coverage** — fraction of named CST nodes (instances)
//!   that landed in Typed or Under-typed buckets. Real-world
//!   "fraction of code we can query."
//! - **Drop count** — should always be zero.

#![cfg(feature = "native")]

use std::collections::BTreeMap;
use tree_sitter::Node as TsNode;

use super::types::{AccessSegment, ByteRange, Ir};

/// Per-CST-node classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    /// An IR node has exactly this byte range and is not Unknown.
    Typed,
    /// An `Ir::Unknown` has exactly this byte range.
    Unknown,
    /// A typed IR ancestor's range contains this CST node, but no
    /// IR matches it exactly. E.g. inner CST nodes folded into an
    /// access chain.
    UnderTyped,
    /// An `Ir::Unknown` ancestor covers this CST node.
    UnderUnknown,
    /// No IR range covers this CST node. Should never happen if
    /// round-trip identity holds.
    Dropped,
}

/// Coverage report for one (CST, IR) pair.
#[derive(Debug, Default)]
pub struct CoverageReport {
    pub source_bytes: usize,
    pub total_named_cst_nodes: usize,

    /// Per-kind counts in each coverage bucket.
    pub by_kind: BTreeMap<String, KindStats>,

    /// Aggregate counts across all kinds.
    pub typed: usize,
    pub unknown: usize,
    pub under_typed: usize,
    pub under_unknown: usize,
    pub dropped: usize,
}

#[derive(Debug, Default, Clone)]
pub struct KindStats {
    pub typed: usize,
    pub unknown: usize,
    pub under_typed: usize,
    pub under_unknown: usize,
    pub dropped: usize,
}

impl KindStats {
    pub fn total(&self) -> usize {
        self.typed + self.unknown + self.under_typed + self.under_unknown + self.dropped
    }
}

impl CoverageReport {
    /// Fraction of distinct CST kinds with ≥1 typed instance. The
    /// public "language support level" metric.
    pub fn kind_coverage_pct(&self) -> f64 {
        if self.by_kind.is_empty() { return 0.0; }
        let supported = self.by_kind.values().filter(|k| k.typed > 0).count();
        100.0 * supported as f64 / self.by_kind.len() as f64
    }

    /// Fraction of named CST nodes (instances) with Typed or
    /// Under-typed coverage. Reflects actual usage in the corpus.
    pub fn node_coverage_pct(&self) -> f64 {
        if self.total_named_cst_nodes == 0 { return 0.0; }
        100.0 * (self.typed + self.under_typed) as f64 / self.total_named_cst_nodes as f64
    }

    /// Render a human-readable summary.
    pub fn summary(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "=== IR coverage report ===\n\
             source bytes:       {}\n\
             named CST nodes:    {}\n\
             kind coverage:      {:.1}% ({} of {} distinct kinds typed at least once)\n\
             node coverage:      {:.1}% ({}+{}={} of {} nodes typed or under-typed)\n\
             buckets: typed={} unknown={} under_typed={} under_unknown={} dropped={}\n",
            self.source_bytes,
            self.total_named_cst_nodes,
            self.kind_coverage_pct(),
            self.by_kind.values().filter(|k| k.typed > 0).count(),
            self.by_kind.len(),
            self.node_coverage_pct(),
            self.typed,
            self.under_typed,
            self.typed + self.under_typed,
            self.total_named_cst_nodes,
            self.typed, self.unknown, self.under_typed, self.under_unknown, self.dropped,
        ));
        if self.dropped > 0 {
            s.push_str(&format!("\n!!! {} CST nodes dropped (renderer bug)\n", self.dropped));
        }
        s.push_str("\n--- per-kind (sorted by total count, descending) ---\n");
        let mut kinds: Vec<(&String, &KindStats)> = self.by_kind.iter().collect();
        kinds.sort_by(|a, b| b.1.total().cmp(&a.1.total()));
        for (kind, stats) in kinds {
            let mark = if stats.typed > 0 { "✓" }
                       else if stats.under_typed > 0 { "~" }   // structurally folded
                       else if stats.unknown > 0 { "?" }       // explicitly punted
                       else if stats.under_unknown > 0 { "·" } // under unknown parent
                       else { "✗" };                           // dropped
            s.push_str(&format!(
                "  {} {:34} typed={:4} ut={:4} unk={:4} uu={:4} drop={:4}\n",
                mark, kind, stats.typed, stats.under_typed, stats.unknown,
                stats.under_unknown, stats.dropped,
            ));
        }
        s
    }
}

/// Walk the IR and collect every node's byte range with a flag for
/// whether it's Unknown.
fn collect_ir_ranges(ir: &Ir, out: &mut Vec<(ByteRange, bool /* is_unknown */)>) {
    let r = ir.range();
    let is_unknown = matches!(ir, Ir::Unknown { .. });
    out.push((r, is_unknown));
    match ir {
        Ir::Module { children, .. } | Ir::Inline { children, .. } => {
            for c in children { collect_ir_ranges(c, out); }
        }
        Ir::Body { children, .. } => {
            for c in children { collect_ir_ranges(c, out); }
        }
        Ir::Expression { inner, .. } => collect_ir_ranges(inner, out),
        Ir::Access { receiver, segments, .. } => {
            collect_ir_ranges(receiver, out);
            for s in segments {
                match s {
                    AccessSegment::Member { .. } => {}, // no IR children
                    AccessSegment::Index { indices, .. } => {
                        for i in indices { collect_ir_ranges(i, out); }
                    }
                    AccessSegment::Call { arguments, .. } => {
                        for a in arguments { collect_ir_ranges(a, out); }
                    }
                }
            }
        }
        Ir::Call { callee, arguments, .. } => {
            collect_ir_ranges(callee, out);
            for a in arguments { collect_ir_ranges(a, out); }
        }
        Ir::Binary { left, right, .. } | Ir::Comparison { left, right, .. } => {
            collect_ir_ranges(left, out);
            collect_ir_ranges(right, out);
        }
        Ir::Unary { operand, .. } => collect_ir_ranges(operand, out),
        Ir::If { condition, body, else_branch, .. }
        | Ir::ElseIf { condition, body, else_branch, .. } => {
            collect_ir_ranges(condition, out);
            collect_ir_ranges(body, out);
            if let Some(e) = else_branch { collect_ir_ranges(e, out); }
        }
        Ir::Else { body, .. } => collect_ir_ranges(body, out),
        Ir::For { targets, iterables, body, else_body, .. } => {
            for t in targets { collect_ir_ranges(t, out); }
            for i in iterables { collect_ir_ranges(i, out); }
            collect_ir_ranges(body, out);
            if let Some(e) = else_body { collect_ir_ranges(e, out); }
        }
        Ir::While { condition, body, else_body, .. } => {
            collect_ir_ranges(condition, out);
            collect_ir_ranges(body, out);
            if let Some(e) = else_body { collect_ir_ranges(e, out); }
        }
        Ir::Function { decorators, name, generics, parameters, returns, body, .. } => {
            for d in decorators { collect_ir_ranges(d, out); }
            collect_ir_ranges(name, out);
            if let Some(g) = generics { collect_ir_ranges(g, out); }
            for p in parameters { collect_ir_ranges(p, out); }
            if let Some(r) = returns { collect_ir_ranges(r, out); }
            collect_ir_ranges(body, out);
        }
        Ir::Class { decorators, name, generics, bases, body, modifiers: _, .. } => {
            for d in decorators { collect_ir_ranges(d, out); }
            collect_ir_ranges(name, out);
            if let Some(g) = generics { collect_ir_ranges(g, out); }
            for b in bases { collect_ir_ranges(b, out); }
            collect_ir_ranges(body, out);
        }
        Ir::Parameter { name, type_ann, default, .. } => {
            collect_ir_ranges(name, out);
            if let Some(t) = type_ann { collect_ir_ranges(t, out); }
            if let Some(d) = default { collect_ir_ranges(d, out); }
        }
        Ir::Decorator { inner, .. } => collect_ir_ranges(inner, out),
        Ir::Returns { type_ann, .. } => collect_ir_ranges(type_ann, out),
        Ir::Generic { items, .. } => {
            for i in items { collect_ir_ranges(i, out); }
        }
        Ir::TypeParameter { name, constraint, .. } => {
            collect_ir_ranges(name, out);
            if let Some(c) = constraint { collect_ir_ranges(c, out); }
        }
        Ir::Return { value, .. } => {
            if let Some(v) = value { collect_ir_ranges(v, out); }
        }
        Ir::Assign { targets, type_annotation, values, .. } => {
            for t in targets { collect_ir_ranges(t, out); }
            if let Some(ty) = type_annotation { collect_ir_ranges(ty, out); }
            for v in values { collect_ir_ranges(v, out); }
        }
        Ir::Import { children, .. } => {
            for c in children { collect_ir_ranges(c, out); }
        }
        Ir::From { path, imports, .. } => {
            if let Some(p) = path { collect_ir_ranges(p, out); }
            for i in imports { collect_ir_ranges(i, out); }
        }
        Ir::FromImport { name, alias, .. } => {
            collect_ir_ranges(name, out);
            if let Some(a) = alias { collect_ir_ranges(a, out); }
        }
        Ir::Path { segments, .. } => {
            for s in segments { collect_ir_ranges(s, out); }
        }
        Ir::Aliased { inner, .. } => collect_ir_ranges(inner, out),
        Ir::Tuple { children, .. } | Ir::List { children, .. } | Ir::Set { children, .. } => {
            for c in children { collect_ir_ranges(c, out); }
        }
        Ir::Dictionary { pairs, .. } => {
            for p in pairs { collect_ir_ranges(p, out); }
        }
        Ir::Pair { key, value, .. } => {
            collect_ir_ranges(key, out);
            collect_ir_ranges(value, out);
        }
        Ir::GenericType { name, params, .. } => {
            collect_ir_ranges(name, out);
            for p in params { collect_ir_ranges(p, out); }
        }
        Ir::Is { value, type_target, .. } => {
            collect_ir_ranges(value, out);
            collect_ir_ranges(type_target, out);
        }
        Ir::Cast { type_ann, value, .. } => {
            collect_ir_ranges(type_ann, out);
            collect_ir_ranges(value, out);
        }
        Ir::Namespace { name, children, .. } => {
            collect_ir_ranges(name, out);
            for c in children { collect_ir_ranges(c, out); }
        }
        Ir::Variable { type_ann, name, value, .. } => {
            if let Some(t) = type_ann { collect_ir_ranges(t, out); }
            collect_ir_ranges(name, out);
            if let Some(v) = value { collect_ir_ranges(v, out); }
        }
        // Leaves and markers — no further recursion.
        Ir::Name { .. } | Ir::Int { .. } | Ir::Float { .. } | Ir::String { .. }
        | Ir::True { .. } | Ir::False { .. } | Ir::None { .. } | Ir::Null { .. }
        | Ir::Comment { .. } | Ir::PositionalSeparator { .. } | Ir::KeywordSeparator { .. }
        | Ir::Break { .. } | Ir::Continue { .. } | Ir::Unknown { .. } => {}
    }
}

/// Walk the CST and call `visit` for each named node.
fn walk_cst<F: FnMut(TsNode)>(node: TsNode, visit: &mut F) {
    if node.is_named() { visit(node); }
    let mut c = node.walk();
    for child in node.children(&mut c) {
        walk_cst(child, visit);
    }
}

/// Run the audit. Returns a populated [`CoverageReport`].
pub fn audit_coverage(ts_root: TsNode, ir: &Ir, source: &str) -> CoverageReport {
    // Step 1: collect all IR ranges with their typed/unknown status.
    let mut ir_ranges: Vec<(ByteRange, bool)> = Vec::new();
    collect_ir_ranges(ir, &mut ir_ranges);

    // For exact-range lookups, build a map from range to is_unknown.
    // Multiple IR nodes can share a range (e.g. Ir::Module and a
    // single child whose range == module's). For exact-match, we
    // prefer the typed one.
    let mut exact: BTreeMap<(u32, u32), bool> = BTreeMap::new();
    for (r, unk) in &ir_ranges {
        let key = (r.start, r.end);
        match exact.get(&key) {
            None => { exact.insert(key, *unk); }
            // If we already have non-unknown at this range, keep it.
            Some(&existing) => {
                if existing && !*unk {
                    exact.insert(key, false); // upgrade to typed
                }
            }
        }
    }

    // Step 2: walk the CST and classify each named node.
    let mut report = CoverageReport {
        source_bytes: source.len(),
        ..Default::default()
    };

    walk_cst(ts_root, &mut |node| {
        // Skip the root itself if it has no parent context; counting
        // module-level nodes is fine but we want to report leaf-ish
        // structure. Actually, count it — "compilation_unit" is a
        // valid kind to track.
        report.total_named_cst_nodes += 1;
        let kind = node.kind().to_string();
        let r = node.byte_range();
        let key = (r.start as u32, r.end as u32);
        let stats = report.by_kind.entry(kind).or_default();

        let coverage = if let Some(&unk) = exact.get(&key) {
            if unk { Coverage::Unknown } else { Coverage::Typed }
        } else {
            // Find the smallest IR range that strictly contains us.
            let mut best: Option<(u32, bool)> = None;
            for (ir_r, unk) in &ir_ranges {
                if ir_r.start <= key.0 && ir_r.end >= key.1
                    && (ir_r.start, ir_r.end) != key
                {
                    let size = ir_r.end - ir_r.start;
                    match best {
                        None => best = Some((size, *unk)),
                        Some((bs, _)) if size < bs => best = Some((size, *unk)),
                        _ => {}
                    }
                }
            }
            match best {
                None => Coverage::Dropped,
                Some((_, true)) => Coverage::UnderUnknown,
                Some((_, false)) => Coverage::UnderTyped,
            }
        };

        match coverage {
            Coverage::Typed       => { stats.typed += 1; report.typed += 1; }
            Coverage::Unknown     => { stats.unknown += 1; report.unknown += 1; }
            Coverage::UnderTyped  => { stats.under_typed += 1; report.under_typed += 1; }
            Coverage::UnderUnknown=> { stats.under_unknown += 1; report.under_unknown += 1; }
            Coverage::Dropped     => { stats.dropped += 1; report.dropped += 1; }
        }
    });

    report
}
