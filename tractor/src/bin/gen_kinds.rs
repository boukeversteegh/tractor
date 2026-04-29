//! Generate per-language `Kind` enums from each tree-sitter grammar's
//! `node-types.json`. Run via `task gen:kinds`. Output is committed.
//!
//! Each generated file declares `pub enum <Lang>Kind { … }` plus
//! `from_str(&str) -> Option<Self>` and `as_str(&self) -> &'static str`,
//! enumerating every named, non-supertype kind the grammar can emit.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

struct LangCodegen {
    enum_name: &'static str,
    node_types: &'static str,
    output_path: &'static str,
}

const LANGUAGES: &[LangCodegen] = &[
    LangCodegen {
        enum_name: "GoKind",
        node_types: tree_sitter_go::NODE_TYPES,
        output_path: "tractor/src/languages/go/input.rs",
    },
    LangCodegen {
        enum_name: "CsKind",
        node_types: tree_sitter_c_sharp::NODE_TYPES,
        output_path: "tractor/src/languages/csharp/input.rs",
    },
    LangCodegen {
        enum_name: "JavaKind",
        node_types: tree_sitter_java::NODE_TYPES,
        output_path: "tractor/src/languages/java/input.rs",
    },
    LangCodegen {
        enum_name: "PhpKind",
        node_types: tree_sitter_php::PHP_NODE_TYPES,
        output_path: "tractor/src/languages/php/input.rs",
    },
];

fn main() -> Result<()> {
    for lang in LANGUAGES {
        let kinds = collect_named_kinds(lang.node_types)
            .with_context(|| format!("parsing node-types.json for {}", lang.enum_name))?;
        let source = render_enum(lang.enum_name, &kinds);
        write_if_changed(lang.output_path, &source)?;
        println!("{} ({} kinds)", lang.output_path, kinds.len());
    }
    Ok(())
}

fn collect_named_kinds(json: &str) -> Result<Vec<String>> {
    let v: Value = serde_json::from_str(json)?;
    let mut kinds = BTreeSet::new();
    if let Some(arr) = v.as_array() {
        for entry in arr {
            collect_from_entry(entry, &mut kinds);
        }
    }
    Ok(kinds.into_iter().collect())
}

fn collect_from_entry(v: &Value, out: &mut BTreeSet<String>) {
    let Some(obj) = v.as_object() else { return };
    let named = obj.get("named").and_then(|x| x.as_bool()).unwrap_or(false);
    let ty = obj.get("type").and_then(|x| x.as_str()).unwrap_or("");
    if named && !ty.is_empty() && !ty.starts_with('_') {
        out.insert(ty.to_string());
    }
    if let Some(subs) = obj.get("subtypes").and_then(|x| x.as_array()) {
        for s in subs {
            collect_from_entry(s, out);
        }
    }
}

fn snake_to_pascal(s: &str) -> String {
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut chars = p.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

fn render_enum(enum_name: &str, kinds: &[String]) -> String {
    let mut out = String::new();
    out.push_str("// DO NOT EDIT — regenerate via `task gen:kinds`.\n");
    out.push_str("// Source: this grammar's node-types.json (named, non-supertype kinds only).\n\n");
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n");
    out.push_str(&format!("pub enum {} {{\n", enum_name));
    for k in kinds {
        out.push_str(&format!("    {},\n", snake_to_pascal(k)));
    }
    out.push_str("}\n\n");
    out.push_str(&format!("impl {} {{\n", enum_name));
    out.push_str("    pub fn from_str(s: &str) -> Option<Self> {\n");
    out.push_str("        match s {\n");
    for k in kinds {
        out.push_str(&format!(
            "            {:?} => Some(Self::{}),\n",
            k,
            snake_to_pascal(k)
        ));
    }
    out.push_str("            _ => None,\n");
    out.push_str("        }\n");
    out.push_str("    }\n\n");
    out.push_str("    pub fn as_str(&self) -> &'static str {\n");
    out.push_str("        match *self {\n");
    for k in kinds {
        out.push_str(&format!(
            "            Self::{} => {:?},\n",
            snake_to_pascal(k),
            k
        ));
    }
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
}

fn write_if_changed(path: &str, content: &str) -> Result<()> {
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == content {
            return Ok(());
        }
    }
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}
