//! Tree → JSON projection — **variant-blind walk**.
//!
//! Direct from the typed `SyntaxTree` to `serde_json::Value`, with no
//! XML intermediate. Uses the same generated metadata as the XML
//! walker:
//!
//! - [`element_name_of`](super::metadata_generated::element_name_of)
//!   produces the JSON key / `$type` value for the node.
//! - [`flags_of`](super::metadata_generated::flags_of) produces the
//!   boolean flag fields.
//! - [`SyntaxTree::children`] gives the variant-blind child list.
//!
//! ## Output shape (mechanical rules)
//!
//! - **Scalar leaves** (no children, no flags, has stored text) →
//!   bare JSON string.
//! - **Inline / Skip** (no element name) → render children inline at
//!   parent.
//! - **Structural nodes** → JSON object with:
//!   - `$type`: element name (omitted when the parent's key already
//!     conveys the type).
//!   - One boolean field per flag (`"async": true`).
//!   - One child group per distinct child element name; singleton
//!     children are keyed directly (`"body": { … }`); multiple
//!     same-named children fall back to `$children: [ … ]` so JSON
//!     keys stay unique.
//!
//! Zero `match SyntaxTree::…` arms — every per-variant decision lives
//! in the generated metadata layer. YAML re-encodes this Value via
//! `serde_yaml`.

#![cfg(feature = "native")]

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::metadata_generated::{element_name_of, flags_of};
use super::types::{SyntaxTree, TreeNode};

const KEY_TYPE: &str = "$type";
const KEY_CHILDREN: &str = "$children";

/// Entry point. Render `tree` (with its `source`) to a JSON Value.
/// The root keeps its `$type` (no parent key conveys it).
pub fn tree_to_json(tree: &SyntaxTree, source: &str) -> Value {
    render(tree, source, /*strip_type=*/ false)
}

/// Variant-blind render. `strip_type` is set by the parent when the
/// child sits under a key that already names its type — then the
/// child's `$type` field is omitted to avoid `{"body": {"$type":
/// "body", …}}` duplication.
fn render(tree: &SyntaxTree, source: &str, strip_type: bool) -> Value {
    // 1. Inline / Skip: no wrapper. Inline children at parent — but
    //    a value can't carry siblings, so collapse to:
    //      • the single child rendered, if exactly one,
    //      • a $children array otherwise.
    let elem_name = match element_name_of(tree) {
        Some(n) => n,
        None => return render_inline(tree, source),
    };

    let flags = flags_of(tree);
    let children = tree.children();

    // 2. Scalar leaf: no children, no flags, has stored text. Emit
    //    the text as a bare JSON string. The parent keys it under
    //    this node's element name.
    if children.is_empty() && flags.is_empty() {
        if let Some(text) = leaf_text(tree, source) {
            return Value::String(text);
        }
    }

    // 3. Structural node: build an object.
    let mut obj: Map<String, Value> = Map::new();
    if !strip_type {
        obj.insert(KEY_TYPE.to_string(), Value::String(elem_name.to_string()));
    }

    // Flags as boolean fields.
    for marker in &flags {
        obj.insert(marker.name.to_string(), Value::Bool(true));
    }

    // Group children by their element name so JSON keys stay unique.
    // Inline children flatten their grandchildren into the parent's
    // group set, so a `<expression>` wrapping a `<name>` contributes
    // a `name` entry rather than a `$children` overflow.
    let mut groups: BTreeMap<String, Vec<&SyntaxTree>> = BTreeMap::new();
    let mut inline_overflow: Vec<&SyntaxTree> = Vec::new();
    for child in &children {
        push_child(child, &mut groups, &mut inline_overflow);
    }

    for (key, items) in groups {
        if items.len() == 1 {
            // Singleton child → keyed by element name, $type stripped.
            obj.insert(key, render(items[0], source, /*strip_type=*/ true));
        } else {
            // Multiple same-named siblings: overflow into $children.
            // Each entry keeps its $type so callers can tell them
            // apart by name without inspecting JSON shape.
            for item in items {
                inline_overflow.push(item);
            }
        }
    }

    if !inline_overflow.is_empty() {
        let mut existing = obj
            .remove(KEY_CHILDREN)
            .and_then(|v| match v {
                Value::Array(a) => Some(a),
                _ => None,
            })
            .unwrap_or_default();
        for item in inline_overflow {
            existing.push(render(item, source, /*strip_type=*/ false));
        }
        obj.insert(KEY_CHILDREN.to_string(), Value::Array(existing));
    }

    Value::Object(obj)
}

/// Inline render: a node whose `element_name_of` is `None` (Inline,
/// Skip) carries children but no wrapper of its own. As a JSON value
/// it has no natural shape — fall through to either a single child or
/// a $children array.
fn render_inline(tree: &SyntaxTree, source: &str) -> Value {
    let children = tree.children();
    match children.len() {
        0 => Value::Null,
        1 => render(children[0], source, /*strip_type=*/ false),
        _ => Value::Array(
            children
                .iter()
                .map(|c| render(c, source, /*strip_type=*/ false))
                .collect(),
        ),
    }
}

/// Push one child into the parent's grouping. Inline children
/// dissolve: their grandchildren flow into the parent's groups so
/// `<inline><name>x</name><name>y</name></inline>` contributes two
/// `name` entries to the parent (which then promote to `$children`).
fn push_child<'a>(
    child: &'a SyntaxTree,
    groups: &mut BTreeMap<String, Vec<&'a SyntaxTree>>,
    inline_overflow: &mut Vec<&'a SyntaxTree>,
) {
    match element_name_of(child) {
        Some(name) => {
            groups.entry(name.to_string()).or_default().push(child);
        }
        None => {
            for grand in child.children() {
                push_child(grand, groups, inline_overflow);
            }
        }
    }
}

/// Text content of a leaf node: stored scalar text if anchored=false,
/// else the source slice. Returns `None` when the node has no text
/// content (markers / synthetic wrappers).
fn leaf_text(tree: &SyntaxTree, source: &str) -> Option<String> {
    if let Some(text) = tree.scalar_text() {
        return Some(text.to_string());
    }
    let range = tree.range();
    if range.is_anchored() && !range.is_empty() {
        return Some(range.slice(source).to_string());
    }
    None
}
