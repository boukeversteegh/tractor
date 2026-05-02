# Post-cycle review backlog

Items flagged by post-cycle subagent reviews of self-improvement loop
iterations. Sources tagged with the iter range that surfaced them.
Items move to "Addressed" when an iter closes them; un-acted items
collect here so they aren't lost between cycles.

This file is the canonical place to land reviewer findings. When a
subagent flags issues that aren't fixed in the same loop cycle,
they get written here.

## Open

### From iters 138-140 review

- [ ] **`json/yaml/transformations.rs` still set `field=`** on data-pair
  elements. JSON serializer ignores `field=` post-iter-139, but the
  attribute is dead weight. Verify and drop, or document why retained.
  Files: `tractor/src/languages/json/transformations.rs:118`,
  `tractor/src/languages/yaml/transformations.rs:126`.
- [ ] **Three internal helpers still set `field=`** on synthesized
  wrappers — `promote_field_to_wrapper`, `replace_identifier_with_name_child`,
  `wrap_text_in_name`. Likely dead post-iter-139.
  File: `tractor/src/transform/mod.rs` (~lines 1070, 1137, 1184).
- [ ] **`field=` claim about `--meta` debug output is misleading**.
  iter 139's commit message claimed tree-sitter `field=` survives for
  `--meta` debug, but the renderer filters them by element-name during
  inlining. Either fix renderer to show them, or drop the rationale.
  File: `tractor/src/output/xml_renderer.rs:443-455`.
- [ ] **Rust closure trailing-expression `body` single-name** —
  `|x| x` body is a single `<name>`. iter 141 stopped tagging
  self-closing markers but `<name>` here is text-only and gets
  `list="name"` even though it's the body's value, not a list.
  File: `tests/integration/languages/rust/blueprint.rs.snapshot.txt:315,569`.

### From iters 143-148 review

- [ ] **`Rule::Flatten { distribute_field: ... }` enum field name**
  is misleading after iter 145's helper rename. Rename the variant's
  `distribute_field` field to `distribute_list`. Touches the rule
  table dispatch + ~10 callsites. Deferred per iter-145 commit.
  File: `tractor/src/languages/rule.rs:57-59`.
- [ ] **Java method-call vs Python/Go method-call shape divergence**.
  Java emits `<call><object/><name="method"/>...</call>` (flat,
  bare method-name); Python/Go emit `<call><member><object/><property/></member>...</call>`
  (nested member). TS uses `<call><callee><member>...</member></callee>...</call>`.
  Three different call shapes across four PLs. Per
  `feedback_principle5_scope.md` this is OK (within-language unification,
  not across), but worth a deliberate decision. Design call.

### From iters 149-153 review

- [ ] **Stale `field=X list="true"` doc comments** — iter 154 swept
  many; remaining sites use the same string in different
  formattings the bulk-replace missed. ~6 sites still mention the
  old form in comment prose. Mechanical sweep TBD.
  Run: `grep -rn 'field=".*" list="true"\|field="extends"\|field="implements"\|field="parameters"\|field="arguments"\|field="generics"\|field="throws"' tractor/src/languages/ | grep -v -F '.rs:.*field=".*"' | head`.
- [ ] **Single-segment paths render as 1-element arrays** — Python
  `import os` → `path: {name: ["os"]}`. Defensible (consistency with
  multi-segment), but no spec note pins the choice. Document in
  design.md or revisit.
- [ ] **C# `<argument>` vocabulary mismatch** (long-pending) —
  C#/PHP wrap call args in `<argument>`; Java/Python/Rust/Go/TS use
  bare children with `list="arguments"`. `//argument` cross-language
  query has Java/Python/Rust/Go/TS holes. Design call (large scope).

### Standing items (re-flag every cycle)

- [ ] Snapshot cold-read pass every ~5 cycles — fresh eyes on every
  blueprint, surface anything suspicious that familiarity has hidden.
- [ ] When a transform test needs `(A or B)` disjunction or
  descendant-axis fallback, log as a candidate for re-shaping the
  underlying tree.

## Addressed

(Most-recent first. Older addressed items may be pruned periodically.)

- iter 154: C# import path-segment tagging + stale-doc sweep batch 1
  + drop unused `append_marker` import.
- iter 153: PHP + Go wired into `flatten_nested_paths`.
- iter 152: Rust wired into `flatten_nested_paths`.
- iter 151: shared `flatten_nested_paths` post-pass; Java + Python wired.
- iter 150: Go `qualified_type` package-wrap.
- iter 149: stale-doc sweep for iter 146 operator renames; fix
  `distribute_list_to_children` doc.
- iter 148: Java method_invocation receiver wraps in `<object>`.
- iter 147: member-access role-wrap (Java/Python/Go to match TS).
- iter 146: `no_dash_in_node_names` invariant + dashed operator
  marker renames (`not-equals` → `inequality`, `or-equal` → `equal`,
  `nullish-coalescing` → `nullish`, `floor-divide` → `floor`).
- iter 145: rename `distribute_field_to_children` → `distribute_list_to_children`.
- iter 144: design.md path-segments example matches actual emit.
- iter 143: Go composite-literal `keyed_element` → `<pair>`.
- iter 142: drop dead `is_anon_text_entry` stub.
- iter 141: tighten `distribute_member_list_attrs` — skip self-closing
  markers.
