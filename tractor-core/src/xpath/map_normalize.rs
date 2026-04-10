//! Normalization of XPath maps with sequence-valued entries.
//!
//! When an XPath `map {}` constructor produces a map whose values are
//! multi-item sequences (e.g. `map { "methods": .//method/string(.) }`),
//! JSON serialization fails because JSON map values must be single items.
//!
//! This module provides:
//! - **Normalization**: wraps sequence values in `array{}` so serialization succeeds.
//! - **Detection**: identifies which keys had sequence values (for diagnostics).
//! - **XPath snippet extraction**: finds the problematic expression in the user's query.
//!
//! Ideally we'd convert `Function → XmlNode` IR directly, but xee's `Map` type
//! has all iteration methods marked `pub(crate)`. This XPath-based normalization
//! is a workaround until upstream exposes public map introspection APIs.

use std::cell::RefCell;
use xee_xpath::{Documents, Queries, Query, SerializationParameters};

// Thread-local caches for compiled normalization queries.
thread_local! {
    static NORMALIZE_MAP_QUERY: RefCell<Option<xee_xpath::query::SequenceQuery>> = RefCell::new(None);
    static DETECT_SEQUENCE_KEYS_QUERY: RefCell<Option<xee_xpath::query::SequenceQuery>> = RefCell::new(None);
}

/// The XPath expression that normalizes a map by wrapping sequence values in arrays.
/// Uses self-application (`$f($f, x)`) to achieve recursion at arbitrary depth.
const NORMALIZE_MAP_XPATH: &str = concat!(
    "let $norm := function($f, $m) { ",
    "map:merge(map:for-each($m, function($k, $v) { ",
    "map { $k: ",
    "if ($v instance of array(*)) then ",
    "array:for-each($v, function($item) { ",
    "if ($item instance of map(*)) then $f($f, $item) ",
    "else $item ",
    "}) ",
    "else if ($v instance of map(*)) then $f($f, $v) ",
    "else if (count($v) > 1) then array { $v } ",
    "else $v ",
    "} ",
    "})) ",
    "} return $norm($norm, .)"
);

/// XPath that recursively finds all map keys whose values are multi-item sequences.
/// Returns strings like "key" or "outer.inner.key" for nested maps.
const DETECT_SEQUENCE_KEYS_XPATH: &str = concat!(
    "let $detect := function($f, $m, $prefix) { ",
    "map:for-each($m, function($k, $v) { ",
    "let $path := if ($prefix) then concat($prefix, '.', $k) else string($k) ",
    "return ( ",
    "if (count($v) > 1) then $path else (), ",
    "if ($v instance of map(*)) then $f($f, $v, $path) else (), ",
    "if ($v instance of array(*)) then ",
    "for-each(1 to array:size($v), function($i) { ",
    "let $item := array:get($v, $i) ",
    "return if ($item instance of map(*)) then $f($f, $item, $path) else () ",
    "}) ",
    "else () ",
    ") ",
    "}) ",
    "} return $detect($detect, ., '')"
);

/// Result of normalizing a map: the JSON string and the list of keys that had
/// multi-item sequences.
pub(super) struct NormalizeResult {
    pub json: String,
    pub sequence_keys: Vec<String>,
}

/// Try to normalize a map that has sequence-valued entries by wrapping them in
/// arrays, then serialize the result to JSON. Also detects which keys had
/// sequence values for diagnostic messages.
pub(super) fn try_normalize_and_serialize_map(
    func: &xee_xpath::function::Function,
    documents: &mut Documents,
) -> Option<NormalizeResult> {
    use xee_interpreter::sequence::QNameOrString;

    // Only attempt normalization for maps
    if !matches!(func, xee_xpath::function::Function::Map(_)) {
        return None;
    }

    // Step 1: Normalize the map
    let json = NORMALIZE_MAP_QUERY.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.is_none() {
            let queries = Queries::default();
            match queries.sequence(NORMALIZE_MAP_XPATH) {
                Ok(query) => *cache = Some(query),
                Err(_) => return None,
            }
        }
        let query = cache.as_ref()?;
        let item = xee_xpath::Item::Function(func.clone());
        let normalized = query.execute(documents, &item).ok()?;
        let mut params = SerializationParameters::new();
        params.method = QNameOrString::String("json".to_string());
        normalized.serialize(params, documents.xot_mut()).ok()
    })?;

    // Step 2: Detect which keys had sequence values
    let sequence_keys = DETECT_SEQUENCE_KEYS_QUERY.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.is_none() {
            let queries = Queries::default();
            match queries.sequence(DETECT_SEQUENCE_KEYS_XPATH) {
                Ok(query) => *cache = Some(query),
                Err(_) => return Vec::new(),
            }
        }
        let query = match cache.as_ref() {
            Some(q) => q,
            None => return Vec::new(),
        };
        let item = xee_xpath::Item::Function(func.clone());
        match query.execute(documents, &item) {
            Ok(result) => result
                .iter()
                .filter_map(|item| {
                    if let xee_xpath::Item::Atomic(a) = item {
                        let s = a.xpath_representation();
                        // Strip surrounding quotes from string repr
                        let s = s.strip_prefix('"').unwrap_or(&s);
                        let s = s.strip_suffix('"').unwrap_or(s);
                        Some(s.to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>(),
            Err(_) => Vec::new(),
        }
    });
    // Deduplicate keys (same path can appear multiple times in repeated structures)
    let mut sequence_keys = sequence_keys;
    sequence_keys.sort();
    sequence_keys.dedup();

    Some(NormalizeResult {
        json,
        sequence_keys,
    })
}

/// Extract the value expression for a given key from an XPath map constructor.
///
/// Given XPath `map { "name": string(name), "methods": body/method/string(.) }`
/// and key `"methods"`, returns `Some("body/method/string(.)")`.
///
/// Uses brace/paren-depth tracking to find the end of the value expression.
pub(super) fn extract_map_value_expr<'a>(xpath: &'a str, key: &str) -> Option<&'a str> {
    // The key in the dotted path (e.g. "classes.methods") — use the leaf
    let leaf_key = key.rsplit('.').next().unwrap_or(key);

    // Search for "key": or 'key': patterns
    let patterns = [format!("\"{}\"", leaf_key), format!("'{}'", leaf_key)];

    for pat in &patterns {
        if let Some(key_pos) = xpath.find(pat.as_str()) {
            let after_key = &xpath[key_pos + pat.len()..];
            // Skip whitespace and colon
            let after_colon = after_key.trim_start();
            if !after_colon.starts_with(':') {
                continue;
            }
            let value_start = &after_colon[1..].trim_start();
            let offset = xpath.len() - value_start.len();

            // Scan forward tracking nesting depth to find where the value ends
            let mut depth = 0i32;
            let mut end = 0;
            let bytes = value_start.as_bytes();
            let mut in_string = None::<u8>; // tracks quote char
            for (i, &b) in bytes.iter().enumerate() {
                match in_string {
                    Some(q) if b == q => in_string = None,
                    Some(_) => {}
                    None => match b {
                        b'"' | b'\'' => in_string = Some(b),
                        b'{' | b'(' | b'[' => depth += 1,
                        b'}' | b')' | b']' => {
                            if depth == 0 {
                                end = i;
                                break;
                            }
                            depth -= 1;
                        }
                        b',' if depth == 0 => {
                            end = i;
                            break;
                        }
                        _ => {}
                    },
                }
                end = i + 1;
            }

            let expr = value_start[..end].trim();
            if !expr.is_empty() {
                return Some(&xpath[offset..offset + end].trim());
            }
        }
    }
    None
}
