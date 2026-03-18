//! XPath 3.1 query engine implementation

use super::{Match, XPathError};
use super::match_result::XmlNode;
use once_cell::sync::Lazy;
use regex::Regex;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use xee_xpath::{Documents, DocumentHandle, Queries, Query, Sequence, SerializationParameters, query::SequenceQuery};
use xot::{Node, Value, Xot};

// Timing stats (in microseconds) for profiling
static TIMING_XML_LOAD: AtomicU64 = AtomicU64::new(0);
static TIMING_QUERY_EXEC: AtomicU64 = AtomicU64::new(0);
static TIMING_RESULT_PROC: AtomicU64 = AtomicU64::new(0);
static TIMING_XML_SERIALIZE: AtomicU64 = AtomicU64::new(0);
static TIMING_STRING_VALUE: AtomicU64 = AtomicU64::new(0);
static TIMING_COUNT: AtomicU64 = AtomicU64::new(0);
static TIMING_MATCH_COUNT: AtomicU64 = AtomicU64::new(0);

/// Print accumulated timing stats (call at end of processing)
pub fn print_timing_stats() {
    let count = TIMING_COUNT.load(Ordering::Relaxed);
    if count == 0 {
        return;
    }
    let _xml_load = TIMING_XML_LOAD.load(Ordering::Relaxed);
    let query_exec = TIMING_QUERY_EXEC.load(Ordering::Relaxed);
    let result_proc = TIMING_RESULT_PROC.load(Ordering::Relaxed);
    let xml_serialize = TIMING_XML_SERIALIZE.load(Ordering::Relaxed);
    let string_value = TIMING_STRING_VALUE.load(Ordering::Relaxed);
    let match_count = TIMING_MATCH_COUNT.load(Ordering::Relaxed);

    eprintln!("\n=== XPath Timing Stats ({} files, {} matches) ===", count, match_count);
    eprintln!("Query exec:       {:>8.2}ms ({:.2}ms/file)",
        query_exec as f64 / 1000.0, query_exec as f64 / 1000.0 / count as f64);
    eprintln!("Result proc:      {:>8.2}ms ({:.2}ms/file)",
        result_proc as f64 / 1000.0, result_proc as f64 / 1000.0 / count as f64);
    eprintln!("  - xml_fragment: {:>8.2}ms ({:.3}ms/match)",
        xml_serialize as f64 / 1000.0,
        if match_count > 0 { xml_serialize as f64 / 1000.0 / match_count as f64 } else { 0.0 });
    eprintln!("  - string_value: {:>8.2}ms ({:.3}ms/match)",
        string_value as f64 / 1000.0,
        if match_count > 0 { string_value as f64 / 1000.0 / match_count as f64 } else { 0.0 });
    eprintln!("Total XPath:      {:>8.2}ms ({:.2}ms/file)",
        (query_exec + result_proc) as f64 / 1000.0,
        (query_exec + result_proc) as f64 / 1000.0 / count as f64);
}

// Pre-compiled regex for stripping location metadata from XML
static STRIP_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\s*(start|end|startLine|startCol|endLine|endCol)="[^"]*""#).unwrap()
});

/// Extract location directly from xot node attributes (fast path - no serialization)
fn extract_location_from_xot(xot: &Xot, node: Node) -> (u32, u32, u32, u32) {
    if let Value::Element(_) = xot.value(node) {
        let mut line = 1u32;
        let mut col = 1u32;
        let mut end_line = 1u32;
        let mut end_col = 1u32;

        for (name_id, value) in xot.attributes(node).iter() {
            let name = xot.local_name_str(name_id);
            match name {
                "start" => {
                    if let Some((l, c)) = parse_location_attr(value) {
                        line = l;
                        col = c;
                    }
                }
                "end" => {
                    if let Some((l, c)) = parse_location_attr(value) {
                        end_line = l;
                        end_col = c;
                    }
                }
                _ => {}
            }
        }

        // If end wasn't found, use start
        if end_line == 1 && end_col == 1 && (line != 1 || col != 1) {
            end_line = line;
            end_col = col;
        }

        (line, col, end_line, end_col)
    } else {
        (1, 1, 1, 1)
    }
}

/// Parse "line:col" format from attribute value
#[inline]
fn parse_location_attr(value: &str) -> Option<(u32, u32)> {
    let mut parts = value.split(':');
    let line = parts.next()?.parse().ok()?;
    let col = parts.next()?.parse().ok()?;
    Some((line, col))
}

/// Walk an xot node tree and build a native `XmlNode` IR.
fn xot_node_to_xml_node(xot: &Xot, node: Node) -> XmlNode {
    match xot.value(node) {
        Value::Element(element) => {
            let name = xot.local_name_str(element.name()).to_string();
            let attributes: Vec<(String, String)> = xot
                .attributes(node)
                .iter()
                .map(|(name_id, value)| {
                    (xot.local_name_str(name_id).to_string(), value.to_string())
                })
                .collect();
            let children: Vec<XmlNode> = xot
                .children(node)
                .map(|child| xot_node_to_xml_node(xot, child))
                .collect();
            XmlNode::Element { name, attributes, children }
        }
        Value::Text(text) => {
            XmlNode::Text(text.get().to_string())
        }
        Value::Comment(comment) => {
            XmlNode::Comment(comment.get().to_string())
        }
        Value::ProcessingInstruction(pi) => {
            XmlNode::ProcessingInstruction {
                target: xot.local_name_str(pi.target()).to_string(),
                data: pi.data().map(|d| d.to_string()),
            }
        }
        Value::Document => {
            // For document nodes, collect children into a wrapper element
            let children: Vec<XmlNode> = xot
                .children(node)
                .map(|child| xot_node_to_xml_node(xot, child))
                .collect();
            if children.len() == 1 {
                children.into_iter().next().unwrap()
            } else {
                XmlNode::Element {
                    name: "_document_".to_string(),
                    attributes: Vec::new(),
                    children,
                }
            }
        }
        _ => {
            // Namespace nodes etc — emit as empty text
            XmlNode::Text(String::new())
        }
    }
}

/// Convert an XPath Function (map or array) to a JSON string representation.
fn function_to_json_string(func: &xee_xpath::function::Function, xot: &mut Xot) -> String {
    use xee_interpreter::sequence::QNameOrString;
    let item = xee_xpath::Item::Function(func.clone());
    let seq = Sequence::from(item);
    let mut params = SerializationParameters::new();
    params.method = QNameOrString::String("json".to_string());
    match seq.serialize(params, xot) {
        Ok(json) => json,
        Err(_) => format!("{:?}", func),
    }
}

/// Convert a `serde_json::Value` into an `XmlNode` tree.
///
/// This is the robust bridge between xee's JSON serializer (the only public
/// API for inspecting map/array contents) and our native IR. The JSON string
/// is parsed exactly once at query time; downstream renderers work with the
/// structured `XmlNode` directly.
///
/// Map keys are sorted to maximise readability: scalar values (strings,
/// numbers, booleans, null) appear first, then nested map values, then
/// arrays/lists. Within each tier the keys are ordered lexicographically.
/// This places identifying fields like `name` before large child collections
/// like `methods`, mirroring the typical top-to-bottom reading order used in
/// source code. The ordering is fully deterministic regardless of xee's
/// internal hash-map iteration order.
fn json_value_to_xml_node(val: &serde_json::Value) -> XmlNode {
    /// Assign a display tier to a value so that scalars print before maps
    /// and maps print before arrays.
    fn value_tier(v: &XmlNode) -> u8 {
        match v {
            XmlNode::Text(_) | XmlNode::Number(_) | XmlNode::Boolean(_) | XmlNode::Null => 0,
            XmlNode::Map { .. } => 1,
            XmlNode::Array { .. } => 2,
            // XML node types (Element, Comment, ProcessingInstruction) cannot
            // appear in a JSON-derived tree; treat them as complex (tier 1).
            XmlNode::Element { .. } | XmlNode::Comment(_) | XmlNode::ProcessingInstruction { .. } => 1,
        }
    }
    match val {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<_> = map.iter()
                .map(|(k, v)| (k.clone(), json_value_to_xml_node(v)))
                .collect();
            entries.sort_by(|(a, av), (b, bv)| {
                value_tier(av).cmp(&value_tier(bv)).then_with(|| a.cmp(b))
            });
            XmlNode::Map { entries }
        }
        serde_json::Value::Array(arr) => {
            let items = arr.iter().map(json_value_to_xml_node).collect();
            XmlNode::Array { items }
        }
        serde_json::Value::String(s) => XmlNode::Text(s.clone()),
        serde_json::Value::Number(n) => XmlNode::Number(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::Bool(b) => XmlNode::Boolean(*b),
        serde_json::Value::Null => XmlNode::Null,
    }
}

// Thread-local cache for compiled XPath queries
// Each thread gets its own compiled query to avoid RefCell conflicts
thread_local! {
    static QUERY_CACHE: RefCell<Option<(String, SequenceQuery)>> = const { RefCell::new(None) };
}

/// Execute a query directly on Documents (no XML parsing needed)
///
/// This is the fast path - use when you've built directly into Documents
/// using XeeBuilder.
fn execute_direct_query(
    xpath: &str,
    documents: &mut Documents,
    doc_handle: DocumentHandle,
    source_lines: Arc<Vec<String>>,
    file_path: &str,
) -> Result<Vec<Match>, XPathError> {
    QUERY_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();

        // Check if we have a cached query for this XPath
        let query = if let Some((cached_xpath, cached_query)) = cache.as_ref() {
            if cached_xpath == xpath {
                cached_query
            } else {
                // Different XPath, need to recompile
                let queries = Queries::default();
                let new_query = queries
                    .sequence(xpath)
                    .map_err(|e| XPathError::Compile(e.to_string()))?;
                *cache = Some((xpath.to_string(), new_query));
                &cache.as_ref().unwrap().1
            }
        } else {
            // No cached query, compile it
            let queries = Queries::default();
            let new_query = queries
                .sequence(xpath)
                .map_err(|e| XPathError::Compile(e.to_string()))?;
            *cache = Some((xpath.to_string(), new_query));
            &cache.as_ref().unwrap().1
        };

        // Execute the query directly - no XML loading needed!
        let t1 = Instant::now();
        let results = query
            .execute(documents, doc_handle)
            .map_err(|e: xee_xpath::error::Error| XPathError::Execute(e.to_string()))?;
        let t2 = Instant::now();

        // Convert results to Match objects
        let mut matches = Vec::new();
        let mut xml_serialize_time = 0u64;
        let mut string_value_time = 0u64;

        for item in results.iter() {
            match item {
                xee_xpath::Item::Node(node) => {
                    let xot = documents.xot();
                    // Extract location directly from xot attributes (fast - no serialization)
                    let (line, col, end_line, end_col) = extract_location_from_xot(xot, node);

                    let ts0 = Instant::now();
                    let value = xot.string_value(node);
                    let ts1 = Instant::now();
                    // Build native XmlNode IR (no XML string serialization)
                    let xml_node = xot_node_to_xml_node(xot, node);
                    let ts2 = Instant::now();

                    string_value_time += (ts1 - ts0).as_micros() as u64;
                    xml_serialize_time += (ts2 - ts1).as_micros() as u64;

                    let m = Match::with_location(
                        file_path.to_string(),
                        line,
                        col,
                        end_line,
                        end_col,
                        value,
                        Arc::clone(&source_lines),
                    ).with_xml_node(xml_node);

                    matches.push(m);
                }
                xee_xpath::Item::Atomic(atomic) => {
                    let value = atomic.xpath_representation();
                    matches.push(Match::new(file_path.to_string(), value));
                }
                xee_xpath::Item::Function(func) => {
                    let json_str = function_to_json_string(&func, documents.xot_mut());
                    let mut m = Match::new(file_path.to_string(), String::new());
                    // Parse the JSON into structured XmlNode IR — value stays empty,
                    // all data lives in the tree field.
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json_str) {
                        m.xml_node = Some(json_value_to_xml_node(&parsed));
                    }
                    matches.push(m);
                }
            }
        }
        let t3 = Instant::now();

        TIMING_XML_SERIALIZE.fetch_add(xml_serialize_time, Ordering::Relaxed);
        TIMING_STRING_VALUE.fetch_add(string_value_time, Ordering::Relaxed);
        TIMING_MATCH_COUNT.fetch_add(matches.len() as u64, Ordering::Relaxed);

        // Record timing stats (no XML load time for direct queries!)
        TIMING_QUERY_EXEC.fetch_add((t2 - t1).as_micros() as u64, Ordering::Relaxed);
        TIMING_RESULT_PROC.fetch_add((t3 - t2).as_micros() as u64, Ordering::Relaxed);
        TIMING_COUNT.fetch_add(1, Ordering::Relaxed);

        Ok(matches)
    })
}

/// XPath query engine using xee-xpath
///
/// Queries are automatically cached per-thread for efficiency when querying
/// many files with the same XPath expression.
pub struct XPathEngine {
    verbose: bool,
    ignore_whitespace: bool,
}

impl XPathEngine {
    /// Create a new XPath engine
    pub fn new() -> Self {
        XPathEngine { verbose: false, ignore_whitespace: false }
    }

    /// Enable verbose mode for debugging
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Enable whitespace-insensitive matching
    /// When enabled, whitespace is stripped from text nodes before XPath matching
    pub fn with_ignore_whitespace(mut self, ignore: bool) -> Self {
        self.ignore_whitespace = ignore;
        self
    }

    /// Execute an XPath query on Documents
    ///
    /// This method avoids the XML serialization/parsing roundtrip by querying
    /// directly on the Documents instance. Use this when you've built the AST
    /// directly into Documents using XeeBuilder.
    ///
    /// source_lines is wrapped in Arc to avoid cloning for each match.
    pub fn query_documents(
        &self,
        documents: &mut Documents,
        doc_handle: DocumentHandle,
        xpath: &str,
        source_lines: Arc<Vec<String>>,
        file_path: &str,
    ) -> Result<Vec<Match>, XPathError> {
        execute_direct_query(xpath, documents, doc_handle, source_lines, file_path)
    }

    /// Strip location metadata from XML
    pub fn strip_location_metadata(xml: &str) -> String {
        STRIP_RE.replace_all(xml, "").to_string()
    }
}

impl Default for XPathEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_location_metadata() {
        let xml = r#"<class start="1:1" end="5:2">Foo</class>"#;
        let stripped = XPathEngine::strip_location_metadata(xml);
        assert_eq!(stripped, "<class>Foo</class>");
    }

    #[test]
    fn test_query_semantic_xml() {
        use crate::parser::load_xml_string_to_documents;

        let xml = r#"<Files>
  <File path="test.ts">
    <program start="1:1" end="2:1">
      <variable start="1:1" end="1:11">
        <let/>
        <name>x</name>
        <value>
          <number start="1:9" end="1:10">1</number>
        </value>
      </variable>
    </program>
  </File>
</Files>"#;

        let mut result = load_xml_string_to_documents(xml, "test.ts".to_string()).unwrap();
        let engine = XPathEngine::new();

        let matches = engine.query_documents(
            &mut result.documents, result.doc_handle,
            "//variable", Arc::new(vec![]), "test.ts"
        ).unwrap();
        assert_eq!(matches.len(), 1, "Should find one variable element");

        let matches = engine.query_documents(
            &mut result.documents, result.doc_handle,
            "//name", Arc::new(vec![]), "test.ts"
        ).unwrap();
        assert_eq!(matches.len(), 1, "Should find one name element");
    }

    #[test]
    fn test_query_caching() {
        use crate::parser::load_xml_string_to_documents;

        // Test that querying the same XPath multiple times works
        // (queries are cached per-thread automatically)
        let xml1 = r#"<root><item>a</item></root>"#;
        let xml2 = r#"<root><item>b</item><item>c</item></root>"#;

        let engine = XPathEngine::new();

        let mut result1 = load_xml_string_to_documents(xml1, "test1.xml".to_string()).unwrap();
        let matches1 = engine.query_documents(
            &mut result1.documents, result1.doc_handle,
            "//item", Arc::new(vec![]), "test1.xml"
        ).unwrap();
        assert_eq!(matches1.len(), 1);

        // Same query, different document - should use cached query
        let mut result2 = load_xml_string_to_documents(xml2, "test2.xml".to_string()).unwrap();
        let matches2 = engine.query_documents(
            &mut result2.documents, result2.doc_handle,
            "//item", Arc::new(vec![]), "test2.xml"
        ).unwrap();
        assert_eq!(matches2.len(), 2);
    }

    #[test]
    fn test_map_constructor() {
        use crate::parser::load_xml_string_to_documents;

        let xml = r#"<root><item><name>foo</name><value>1</value></item><item><name>bar</name><value>2</value></item></root>"#;
        let mut result = load_xml_string_to_documents(xml, "test.xml".to_string()).unwrap();
        let engine = XPathEngine::new();

        let matches = engine.query_documents(
            &mut result.documents, result.doc_handle,
            r#"//item ! map { "n": string(name), "v": string(value) }"#,
            Arc::new(vec![]), "test.xml"
        );
        assert!(matches.is_ok(), "Map constructor should parse: {:?}", matches.err());
        let m = matches.unwrap();
        assert_eq!(m.len(), 2, "Should get 2 maps");
        // Verify structured tree
        match &m[0].xml_node {
            Some(XmlNode::Map { entries }) => {
                assert!(entries.iter().any(|(k, _)| k == "n"), "Map should have key 'n'");
                assert!(entries.iter().any(|(k, _)| k == "v"), "Map should have key 'v'");
            }
            other => panic!("Expected XmlNode::Map, got: {:?}", other),
        }
    }

    #[test]
    fn test_map_constructor_json_format() {
        use crate::parser::load_xml_string_to_documents;

        let xml = r#"<root><item><name>foo</name><value>1</value></item></root>"#;
        let mut result = load_xml_string_to_documents(xml, "test.xml".to_string()).unwrap();
        let engine = XPathEngine::new();

        let matches = engine.query_documents(
            &mut result.documents, result.doc_handle,
            r#"//item ! map { "name": string(name), "val": string(value) }"#,
            Arc::new(vec![]), "test.xml"
        ).unwrap();
        assert_eq!(matches.len(), 1);
        // Tree should be a structured Map
        match &matches[0].xml_node {
            Some(XmlNode::Map { entries }) => {
                let name_entry = entries.iter().find(|(k, _)| k == "name").expect("key 'name'");
                let val_entry = entries.iter().find(|(k, _)| k == "val").expect("key 'val'");
                assert_eq!(name_entry.1, XmlNode::Text("foo".into()));
                assert_eq!(val_entry.1, XmlNode::Text("1".into()));
            }
            other => panic!("Expected XmlNode::Map, got: {:?}", other),
        }
    }

    #[test]
    fn test_map_result_has_structured_tree() {
        use crate::parser::load_xml_string_to_documents;

        let xml = r#"<root><item><name>foo</name><value>1</value></item></root>"#;
        let mut result = load_xml_string_to_documents(xml, "test.xml".to_string()).unwrap();
        let engine = XPathEngine::new();

        // Map results should have a Map variant in xml_node
        let matches = engine.query_documents(
            &mut result.documents, result.doc_handle,
            r#"//item ! map { "name": string(name) }"#,
            Arc::new(vec![]), "test.xml"
        ).unwrap();
        assert_eq!(matches.len(), 1);
        assert!(matches!(matches[0].xml_node, Some(XmlNode::Map { .. })),
            "Map results should have XmlNode::Map in tree, got: {:?}", matches[0].xml_node);

        // Node results should have an Element variant
        let matches = engine.query_documents(
            &mut result.documents, result.doc_handle,
            "//name",
            Arc::new(vec![]), "test.xml"
        ).unwrap();
        assert_eq!(matches.len(), 1);
        assert!(matches!(matches[0].xml_node, Some(XmlNode::Element { .. })),
            "Node results should have XmlNode::Element in tree");

        // Atomic results should have no tree
        let matches = engine.query_documents(
            &mut result.documents, result.doc_handle,
            "string(//name)",
            Arc::new(vec![]), "test.xml"
        ).unwrap();
        assert_eq!(matches.len(), 1);
        assert!(matches[0].xml_node.is_none(),
            "Atomic results should have no tree");
    }

    #[test]
    fn test_map_with_node_values() {
        use crate::parser::load_xml_string_to_documents;

        let xml = r#"<root><item><name>foo</name><value>1</value></item></root>"#;
        let mut result = load_xml_string_to_documents(xml, "test.xml".to_string()).unwrap();
        let engine = XPathEngine::new();

        // Map with node values (not string() wrapped) — xee serializes nodes
        // as their XML representation, then we parse back to XmlNode::Map
        let matches = engine.query_documents(
            &mut result.documents, result.doc_handle,
            r#"//item ! map { "name": name, "val": value }"#,
            Arc::new(vec![]), "test.xml"
        ).unwrap();
        assert_eq!(matches.len(), 1);
        // Should have structured Map in tree
        match &matches[0].xml_node {
            Some(XmlNode::Map { entries }) => {
                assert_eq!(entries.len(), 2);
                // Look up by key (order depends on serde_json's BTreeMap)
                let name_val = entries.iter().find(|(k, _)| k == "name")
                    .expect("Should have 'name' key");
                let val_val = entries.iter().find(|(k, _)| k == "val")
                    .expect("Should have 'val' key");
                // Node values become Text strings (from xee's XML serialization)
                if let XmlNode::Text(ref s) = name_val.1 {
                    assert!(s.contains("foo"), "Node value should contain 'foo', got: {}", s);
                } else {
                    panic!("Expected Text for 'name' value, got: {:?}", name_val.1);
                }
                if let XmlNode::Text(ref s) = val_val.1 {
                    assert!(s.contains("1"), "Node value should contain '1', got: {}", s);
                } else {
                    panic!("Expected Text for 'val' value, got: {:?}", val_val.1);
                }
            }
            other => panic!("Expected XmlNode::Map, got: {:?}", other),
        }
    }

    #[test]
    fn test_query_with_xml_prolog() {
        use crate::parser::load_xml_string_to_documents;

        // Test with the actual XML prolog
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Files>
  <File path="test.ts">
    <program start="1:1" end="2:1">
      <variable start="1:1" end="1:11">
        <let/>
        <name>x</name>
        <value>
          <number start="1:9" end="1:10">1</number>
        </value>
      </variable>
    </program>
  </File>
</Files>"#;

        let mut result = load_xml_string_to_documents(xml, "test.ts".to_string()).unwrap();
        let engine = XPathEngine::new();
        let matches = engine.query_documents(
            &mut result.documents, result.doc_handle,
            "//variable", Arc::new(vec![]), "test.ts"
        ).unwrap();
        assert_eq!(matches.len(), 1, "Should find one variable element with XML prolog");
    }

    #[test]
    fn test_query_parsed_typescript() {
        use crate::parse_string_to_documents;

        let source = "let x = 1;";
        let mut result = parse_string_to_documents(
            source, "typescript", "test.ts".to_string(), false, false
        ).unwrap();

        let engine = XPathEngine::new();

        let matches = engine.query_documents(
            &mut result.documents, result.doc_handle,
            "//variable", result.source_lines.clone(), "test.ts"
        ).unwrap();
        assert_eq!(matches.len(), 1, "Should find one variable element");

        // Also test querying nested elements
        let matches = engine.query_documents(
            &mut result.documents, result.doc_handle,
            "//name", result.source_lines.clone(), "test.ts"
        ).unwrap();
        assert_eq!(matches.len(), 1, "Should find one name element");

        let matches = engine.query_documents(
            &mut result.documents, result.doc_handle,
            "//value/number", result.source_lines.clone(), "test.ts"
        ).unwrap();
        assert_eq!(matches.len(), 1, "Should find number inside value");
    }

}
