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

use super::element_naming::element_name_for_lang;
use super::metadata_generated::{element_name_of, flags_of};
use super::types::{SyntaxTree, TreeNode};

const KEY_TYPE: &str = "$type";
const KEY_CHILDREN: &str = "$children";

/// Entry point. Render `tree` (with its `source`) to a JSON Value.
/// `lang` selects per-language element naming; `None` uses universal
/// variant tags throughout (useful for invertible JSON round-trips).
pub fn tree_to_json(tree: &SyntaxTree, source: &str, lang: Option<&str>) -> Value {
    render(tree, source, lang, /*strip_type=*/ false)
}

/// Variant-blind render. `strip_type` is set by the parent when the
/// child sits under a key that already names its type — then the
/// child's `$type` field is omitted to avoid duplication.
fn render(tree: &SyntaxTree, source: &str, lang: Option<&str>, strip_type: bool) -> Value {
    let tag = match element_name_of(tree) {
        Some(t) => t,
        None => return render_inline(tree, source, lang),
    };
    let display = element_name_for_lang(tag, lang);

    let flags = flags_of(tree);
    let children = tree.children();

    if children.is_empty() && flags.is_empty() {
        if let Some(text) = leaf_text(tree, source) {
            return Value::String(text);
        }
    }

    let mut obj: Map<String, Value> = Map::new();
    if !strip_type {
        obj.insert(KEY_TYPE.to_string(), Value::String(display.to_string()));
    }

    for marker in &flags {
        obj.insert(marker.name.to_string(), Value::Bool(true));
    }

    let mut groups: BTreeMap<String, Vec<&SyntaxTree>> = BTreeMap::new();
    let mut inline_overflow: Vec<&SyntaxTree> = Vec::new();
    for child in &children {
        push_child(child, lang, &mut groups, &mut inline_overflow);
    }

    for (key, items) in groups {
        if items.len() == 1 {
            obj.insert(key, render(items[0], source, lang, /*strip_type=*/ true));
        } else {
            // Multiple same-named children render as an array under the
            // key (`imports: [...]`, `classes: [...]`). Restores the
            // pre-IR `data_to_json` grouping behaviour (S16-Z6) using
            // only the variant-blind walker — no per-language render
            // logic. Each item's `$type` is stripped because the
            // surrounding key already names the type.
            let arr: Vec<Value> = items
                .iter()
                .map(|item| render(item, source, lang, /*strip_type=*/ true))
                .collect();
            obj.insert(key, Value::Array(arr));
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
            existing.push(render(item, source, lang, /*strip_type=*/ false));
        }
        obj.insert(KEY_CHILDREN.to_string(), Value::Array(existing));
    }

    Value::Object(obj)
}

fn render_inline(tree: &SyntaxTree, source: &str, lang: Option<&str>) -> Value {
    let children = tree.children();
    match children.len() {
        0 => Value::Null,
        1 => render(children[0], source, lang, /*strip_type=*/ false),
        _ => Value::Array(
            children
                .iter()
                .map(|c| render(c, source, lang, /*strip_type=*/ false))
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
    lang: Option<&str>,
    groups: &mut BTreeMap<String, Vec<&'a SyntaxTree>>,
    inline_overflow: &mut Vec<&'a SyntaxTree>,
) {
    match element_name_of(child) {
        Some(tag) => {
            let display = element_name_for_lang(tag, lang);
            groups.entry(display.to_string()).or_default().push(child);
        }
        None => {
            for grand in child.children() {
                push_child(grand, lang, groups, inline_overflow);
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
