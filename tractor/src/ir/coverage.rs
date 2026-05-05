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
//! - **Kind coverage** — fraction of *grammar-known* CST kinds with
//!   ≥1 typed instance. The public-facing support level. Denominator
//!   is the full grammar, not just kinds the corpus exercises, so a
//!   thin blueprint cannot inflate the score.
//! - **Blueprint completeness** — fraction of grammar-known kinds the
//!   corpus exercises at all (typed-or-not). A corpus-quality metric:
//!   when this is well below 100% the IR's coverage of unsampled
//!   kinds is *unknown*, not implicitly supported.
//! - **Node coverage** — fraction of named CST nodes (instances)
//!   that landed in Typed or Under-typed buckets. Real-world
//!   "fraction of code we can query."
//! - **Drop count** — should always be zero.

#![cfg(feature = "native")]

use std::collections::BTreeMap;
use tree_sitter::Node as TsNode;

use super::types::{ByteRange, Ir};

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

    /// Per-kind counts in each coverage bucket. When the audit was
    /// given the language's full kind list, this includes zero-stat
    /// entries for kinds the corpus never exercised.
    pub by_kind: BTreeMap<String, KindStats>,

    /// Total kinds the grammar declares (blueprint-absent included).
    /// `0` if the audit was run without a known-kinds list.
    pub known_kinds: usize,

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
    /// Denominator for the kind-based metrics: full grammar size when
    /// known, otherwise just the kinds we observed.
    fn kind_denominator(&self) -> usize {
        if self.known_kinds > 0 { self.known_kinds } else { self.by_kind.len() }
    }

    /// Fraction of grammar-known CST kinds with ≥1 typed instance.
    /// The public "language support level" metric. When the audit
    /// was run without a known-kinds list, denominator falls back to
    /// observed kinds only.
    pub fn kind_coverage_pct(&self) -> f64 {
        let denom = self.kind_denominator();
        if denom == 0 { return 0.0; }
        let supported = self.by_kind.values().filter(|k| k.typed > 0).count();
        100.0 * supported as f64 / denom as f64
    }

    /// Fraction of grammar-known kinds the corpus exercises at all
    /// (any bucket). Below 100% means the blueprint doesn't sample
    /// every grammar kind — the IR's coverage of those kinds is
    /// `unknown`, not "supported by absence."
    pub fn blueprint_completeness_pct(&self) -> f64 {
        let denom = self.kind_denominator();
        if denom == 0 { return 0.0; }
        let exercised = self.by_kind.values().filter(|k| k.total() > 0).count();
        100.0 * exercised as f64 / denom as f64
    }

    /// Number of kinds known to the grammar but absent from the
    /// corpus. `0` when the audit was run without a known-kinds list.
    pub fn absent_kinds(&self) -> usize {
        self.by_kind.values().filter(|k| k.total() == 0).count()
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
        let denom = self.kind_denominator();
        let typed_kinds = self.by_kind.values().filter(|k| k.typed > 0).count();
        let exercised_kinds = self.by_kind.values().filter(|k| k.total() > 0).count();
        s.push_str(&format!(
            "=== IR coverage report ===\n\
             source bytes:          {}\n\
             named CST nodes:       {}\n\
             kind coverage:         {:.1}% ({} of {} grammar kinds typed at least once)\n\
             blueprint completeness: {:.1}% ({} of {} grammar kinds exercised by corpus; {} absent)\n\
             node coverage:         {:.1}% ({}+{}={} of {} nodes typed or under-typed)\n\
             buckets: typed={} unknown={} under_typed={} under_unknown={} dropped={}\n",
            self.source_bytes,
            self.total_named_cst_nodes,
            self.kind_coverage_pct(),
            typed_kinds, denom,
            self.blueprint_completeness_pct(),
            exercised_kinds, denom, self.absent_kinds(),
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
        // Sort: corpus-exercised kinds by total desc, then absent kinds
        // alphabetically at the end (so absences are easy to scan).
        kinds.sort_by(|a, b| {
            let a_absent = a.1.total() == 0;
            let b_absent = b.1.total() == 0;
            match (a_absent, b_absent) {
                (false, true) => std::cmp::Ordering::Less,
                (true, false) => std::cmp::Ordering::Greater,
                (true, true) => a.0.cmp(b.0),
                (false, false) => b.1.total().cmp(&a.1.total()),
            }
        });
        for (kind, stats) in kinds {
            let mark = if stats.total() == 0 { "∅" }            // absent from corpus
                       else if stats.typed > 0 { "✓" }
                       else if stats.under_typed > 0 { "~" }    // structurally folded
                       else if stats.unknown > 0 { "?" }        // explicitly punted
                       else if stats.under_unknown > 0 { "·" }  // under unknown parent
                       else { "✗" };                            // dropped
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
/// whether it's `Ir::Unknown`. Powered by `Ir::children()` — adding a
/// new variant requires no change here as long as the variant declares
/// its children correctly.
fn collect_ir_ranges(ir: &Ir, out: &mut Vec<(ByteRange, bool /* is_unknown */)>) {
    out.push((ir.range(), matches!(ir, Ir::Unknown { .. })));
    for c in ir.children() {
        collect_ir_ranges(c, out);
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

/// Run the audit. `known_kinds` is the language's full set of named
/// CST kinds (typically derived by iterating the language's
/// `XKind` enum). When supplied, kinds the corpus never exercises
/// surface as zero-stat entries — making blueprint gaps visible
/// rather than implicitly "supported." Pass `&[]` to opt out.
pub fn audit_coverage(
    ts_root: TsNode,
    ir: &Ir,
    source: &str,
    known_kinds: &[&str],
) -> CoverageReport {
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
        known_kinds: known_kinds.len(),
        ..Default::default()
    };

    // Pre-populate by_kind with zero-stat entries for every grammar-
    // known kind, so kinds the corpus never exercises surface in the
    // report.
    for k in known_kinds {
        report.by_kind.insert((*k).to_string(), KindStats::default());
    }

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
