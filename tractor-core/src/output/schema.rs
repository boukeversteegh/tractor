//! Schema output format - displays merged tree of unique element paths
//!
//! This aggregates XML structure across multiple files/matches to show
//! what elements exist and how often they appear.
//!
//! # Design Decision: Re-parsing xml_fragment strings
//!
//! When used with XPath queries (`-x`), this module re-parses the `xml_fragment`
//! strings stored in Match structs rather than keeping Xot nodes alive.
//!
//! **Why not store Xot nodes in Match?**
//! - `Node` is just an index into the `Xot` arena - it can't exist without its parent Xot
//! - Keeping Xot alive for all files would require holding all parsed documents in memory
//! - For 1000 files, that's 1000 full ASTs vs. just the matched fragment strings
//!
//! **Why this is acceptable:**
//! - Schema is an exploration tool, typically run once interactively
//! - The xml_fragment strings are small (just matched subtrees, not full files)
//! - Re-parsing with quick-xml is fast for small fragments
//!
//! **Alternative considered:**
//! - Store `Arc<Xot>` + `Node` in Match - rejected due to memory overhead
//! - Clone subtree into new Xot per match - rejected due to complexity
//!
//! For the no-XPath case, we use the Xot tree directly (no re-parsing needed).

use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use xot::{Node, Value, Xot};

/// A path with its text values and occurrence count
struct PathInfo {
    values: Vec<String>,
    count: usize,
}

/// Collector for aggregating schema paths across multiple sources
pub struct SchemaCollector {
    paths: HashMap<Vec<String>, PathInfo>,
}

impl SchemaCollector {
    /// Create a new empty collector
    pub fn new() -> Self {
        Self {
            paths: HashMap::new(),
        }
    }

    /// Collect paths from a Xot tree (used when we have direct access, e.g., no XPath)
    pub fn collect_from_xot(&mut self, xot: &Xot, node: Node) {
        let mut stack = Vec::new();
        self.collect_from_xot_recursive(xot, node, &mut stack);
    }

    fn collect_from_xot_recursive(&mut self, xot: &Xot, node: Node, stack: &mut Vec<String>) {
        match xot.value(node) {
            Value::Document => {
                for child in xot.children(node) {
                    self.collect_from_xot_recursive(xot, child, stack);
                }
            }
            Value::Element(element) => {
                let tag_name = xot.local_name_str(element.name()).to_string();
                stack.push(tag_name);

                // Record this path
                let entry = self.paths.entry(stack.clone()).or_insert(PathInfo {
                    values: Vec::new(),
                    count: 0,
                });
                entry.count += 1;

                // Collect text content from direct text children
                for child in xot.children(node) {
                    if let Value::Text(text) = xot.value(child) {
                        let trimmed = text.get().trim();
                        if !trimmed.is_empty() {
                            if let Some(info) = self.paths.get_mut(stack) {
                                if !info.values.contains(&trimmed.to_string()) {
                                    info.values.push(trimmed.to_string());
                                }
                            }
                        }
                    }
                }

                // Recurse into children
                for child in xot.children(node) {
                    self.collect_from_xot_recursive(xot, child, stack);
                }

                stack.pop();
            }
            _ => {}
        }
    }

    /// Collect paths from an XML string (used for Match xml_fragments with XPath)
    ///
    /// This re-parses the XML string. See module documentation for why this
    /// tradeoff was chosen over storing Xot nodes in Match structs.
    pub fn collect_from_xml_string(&mut self, xml: &str) {
        let mut stack: Vec<String> = Vec::new();
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    stack.push(tag_name);

                    let entry = self.paths.entry(stack.clone()).or_insert(PathInfo {
                        values: Vec::new(),
                        count: 0,
                    });
                    entry.count += 1;
                }
                Ok(Event::Empty(e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    stack.push(tag_name);

                    let entry = self.paths.entry(stack.clone()).or_insert(PathInfo {
                        values: Vec::new(),
                        count: 0,
                    });
                    entry.count += 1;

                    stack.pop();
                }
                Ok(Event::End(_)) => {
                    stack.pop();
                }
                Ok(Event::Text(e)) => {
                    let text = e.unescape().unwrap_or_default().trim().to_string();
                    if !text.is_empty() && !stack.is_empty() {
                        if let Some(info) = self.paths.get_mut(&stack) {
                            if !info.values.contains(&text) {
                                info.values.push(text);
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
        }
    }

    /// Format the collected paths as a tree
    pub fn format(&self, max_depth: Option<usize>, use_color: bool) -> String {
        let tree = self.build_tree();
        let mut output = String::new();
        let mut truncated = false;
        format_node(&tree, "", true, 0, max_depth, use_color, &mut output, &mut truncated);

        // Add helpful note if truncation occurred
        if truncated {
            output.push('\n');
            if use_color {
                output.push_str("\x1b[2m"); // dim
            }
            output.push_str("(use -d to increase depth, or -x to query specific elements)\n");
            if use_color {
                output.push_str("\x1b[0m"); // reset
            }
        }

        output
    }

    fn build_tree(&self) -> TreeNode {
        let mut root = TreeNode::default();
        root.count = 1;

        // Sort paths by length to ensure parents are created before children
        let mut sorted_paths: Vec<_> = self.paths.iter().collect();
        sorted_paths.sort_by_key(|(path, _)| path.len());

        for (path, info) in sorted_paths {
            let mut node = &mut root;
            for (i, segment) in path.iter().enumerate() {
                node = node.get_or_insert(segment);
                if i == path.len() - 1 {
                    node.count = info.count;
                    node.values = info.values.clone();
                }
            }
        }

        root
    }
}

impl Default for SchemaCollector {
    fn default() -> Self {
        Self::new()
    }
}

// Allow SchemaCollector to be sent across threads for parallel collection
unsafe impl Send for SchemaCollector {}

impl SchemaCollector {
    /// Merge another collector into this one
    ///
    /// Used for parallel schema collection: each thread builds its own collector,
    /// then all are merged into the final result.
    pub fn merge(&mut self, other: SchemaCollector) {
        for (path, other_info) in other.paths {
            let entry = self.paths.entry(path).or_insert(PathInfo {
                values: Vec::new(),
                count: 0,
            });
            entry.count += other_info.count;
            // Merge values, avoiding duplicates
            for value in other_info.values {
                if !entry.values.contains(&value) {
                    entry.values.push(value);
                }
            }
        }
    }
}

/// Tree node for display
#[derive(Default)]
struct TreeNode {
    children: Vec<(String, TreeNode)>,
    values: Vec<String>,
    count: usize,
}

impl TreeNode {
    fn get_or_insert(&mut self, name: &str) -> &mut TreeNode {
        if let Some(pos) = self.children.iter().position(|(n, _)| n == name) {
            &mut self.children[pos].1
        } else {
            self.children.push((name.to_string(), TreeNode::default()));
            &mut self.children.last_mut().unwrap().1
        }
    }
}

/// Count total descendants in a tree node
fn count_descendants(node: &TreeNode) -> usize {
    let mut count = node.children.len();
    for (_, child) in &node.children {
        count += count_descendants(child);
    }
    count
}

fn format_node(
    node: &TreeNode,
    prefix: &str,
    is_root: bool,
    depth: usize,
    max_depth: Option<usize>,
    use_color: bool,
    output: &mut String,
    truncated: &mut bool,
) {
    // Check if we've reached max depth - show truncation instead of children
    if let Some(max) = max_depth {
        if depth >= max && !node.children.is_empty() {
            let child_count = count_descendants(node);
            if use_color {
                output.push_str(&format!("{}\u{2514}\u{2500} \x1b[2m\u{2026} ({} more)\x1b[0m\n", prefix, child_count));
            } else {
                output.push_str(&format!("{}\u{2514}\u{2500} \u{2026} ({} more)\n", prefix, child_count));
            }
            *truncated = true;
            return;
        }
    }

    for (i, (name, child)) in node.children.iter().enumerate() {
        let is_last_child = i == node.children.len() - 1;

        let connector = if is_root {
            ""
        } else if is_last_child {
            "\u{2514}\u{2500} "
        } else {
            "\u{251C}\u{2500} "
        };

        // Show occurrence count
        let occurrence = if child.count > 1 {
            if use_color {
                format!(" \x1b[33m({})\x1b[0m", child.count)
            } else {
                format!(" ({})", child.count)
            }
        } else {
            String::new()
        };

        // Format values if any
        let values_str = if child.values.is_empty() {
            String::new()
        } else {
            let is_structural_pair = child.values.len() == 2
                && matches!(
                    (child.values[0].as_str(), child.values[1].as_str()),
                    ("{", "}") | ("(", ")") | ("[", "]") | ("<", ">")
                );

            let content = if is_structural_pair {
                format!("{}\u{2026}{}", child.values[0], child.values[1])
            } else if child.values.len() <= 5 {
                child.values.join(", ")
            } else {
                format!(
                    "{}, \u{2026} (+{})",
                    child.values[..5].join(", "),
                    child.values.len() - 5
                )
            };

            if use_color {
                format!("  \x1b[2m{}\x1b[0m", content)
            } else {
                format!("  {}", content)
            }
        };

        output.push_str(&format!(
            "{}{}{}{}{}\n",
            prefix, connector, name, occurrence, values_str
        ));

        let new_prefix = if is_root {
            String::new()
        } else if is_last_child {
            format!("{}   ", prefix)
        } else {
            format!("{}\u{2502}  ", prefix)
        };

        format_node(
            child,
            &new_prefix,
            false,
            depth + 1,
            max_depth,
            use_color,
            output,
            truncated,
        );
    }
}

/// Convenience function: format a single Xot tree as schema
pub fn format_schema(xot: &Xot, node: Node, max_depth: Option<usize>, use_color: bool) -> String {
    let mut collector = SchemaCollector::new();
    collector.collect_from_xot(xot, node);
    collector.format(max_depth, use_color)
}
