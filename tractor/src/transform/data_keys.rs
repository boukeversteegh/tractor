//! Data-tree key naming transformer.
//!
//! Shared by the YAML/JSON/TOML/INI/dotenv data-branch transforms: turn
//! source key strings into XML-safe element names, surface the original
//! key in a `key` attribute when sanitization changed it, and walk the
//! ancestor chain to find the enclosing key wrapper for array items.

use xot::{Xot, Node as XotNode};

use super::helpers::{get_attr, get_element_name, get_kind, rename, set_attr};

/// Sanitize a string to be a valid XML element name.
/// Replaces invalid characters with underscores.
pub fn sanitize_xml_name(name: &str) -> String {
    if name.is_empty() {
        return "_".to_string();
    }

    let mut result = String::with_capacity(name.len());
    for (i, c) in name.chars().enumerate() {
        if i == 0 {
            if c.is_ascii_alphabetic() || c == '_' {
                result.push(c);
            } else {
                result.push('_');
                if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                    result.push(c);
                }
            }
        } else if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
            result.push(c);
        } else {
            result.push('_');
        }
    }
    result
}

// /specs/tractor-parse/dual-view/data-branch/key-sanitization.md: Key Name Sanitization
/// Rename an element to represent a data key.
///
/// Sanitizes the key for use as an XML element name, renames the node,
/// and stores the original key in a `key` attribute when sanitization
/// was needed. Returns the sanitized name.
pub fn rename_to_key(xot: &mut Xot, node: XotNode, key: &str) -> String {
    let safe_name = sanitize_xml_name(key);
    rename(xot, node, &safe_name);
    if safe_name != key {
        set_attr(xot, node, "key", key);
    }
    safe_name
}

/// Walk up the ancestor chain to find the nearest mapping pair that was
/// renamed to its key name. Returns the key name for use as array item
/// wrapper, enabling `//key[n]` instead of `//key/item[n]`.
///
/// Returns `None` for top-level arrays (no named pair ancestor) or when
/// the ancestor pair has a sanitized key (has `<key>` child).
pub fn find_ancestor_key_name(xot: &Xot, node: XotNode) -> Option<String> {
    let mut current = xot.parent(node)?;
    loop {
        if let Some(kind) = get_kind(xot, current) {
            match kind.as_str() {
                "block_mapping_pair" | "flow_pair" | "pair" => {
                    // Skip if key was sanitized (has key="..." attribute)
                    if get_attr(xot, current, "key").is_some() {
                        return None;
                    }
                    return get_element_name(xot, current);
                }
                _ => {}
            }
        }
        current = xot.parent(current)?;
    }
}

/// Check if a node has a sequence/array descendant (through wrapper nodes).
/// Used by pair transforms to decide whether to Flatten for array values.
pub fn has_sequence_child(xot: &Xot, node: XotNode) -> bool {
    for child in xot.children(node) {
        // Use kind attr for TreeSitter nodes, element name for TreeBuilder
        // wrappers (like <value>) which don't have a kind attr.
        let tag = get_kind(xot, child)
            .or_else(|| get_element_name(xot, child));
        if let Some(tag) = tag {
            match tag.as_str() {
                "block_sequence" | "flow_sequence" | "array" => return true,
                "block_node" | "flow_node" | "value" => {
                    if has_sequence_child(xot, child) { return true; }
                }
                _ => {}
            }
        }
    }
    false
}
