//! XPath 3.1 query engine implementation

use super::{Match, XPathError};
use once_cell::sync::Lazy;
use regex::Regex;
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use xee_xpath::{Documents, Queries, Query, query::SequenceQuery};

// Timing stats (in microseconds) for profiling
static TIMING_XML_LOAD: AtomicU64 = AtomicU64::new(0);
static TIMING_QUERY_EXEC: AtomicU64 = AtomicU64::new(0);
static TIMING_RESULT_PROC: AtomicU64 = AtomicU64::new(0);
static TIMING_COUNT: AtomicU64 = AtomicU64::new(0);

/// Print accumulated timing stats (call at end of processing)
pub fn print_timing_stats() {
    let count = TIMING_COUNT.load(Ordering::Relaxed);
    if count == 0 {
        return;
    }
    let xml_load = TIMING_XML_LOAD.load(Ordering::Relaxed);
    let query_exec = TIMING_QUERY_EXEC.load(Ordering::Relaxed);
    let result_proc = TIMING_RESULT_PROC.load(Ordering::Relaxed);

    eprintln!("\n=== XPath Timing Stats ({} files) ===", count);
    eprintln!("XML loading:    {:>8.2}ms ({:.2}ms/file)",
        xml_load as f64 / 1000.0, xml_load as f64 / 1000.0 / count as f64);
    eprintln!("Query exec:     {:>8.2}ms ({:.2}ms/file)",
        query_exec as f64 / 1000.0, query_exec as f64 / 1000.0 / count as f64);
    eprintln!("Result proc:    {:>8.2}ms ({:.2}ms/file)",
        result_proc as f64 / 1000.0, result_proc as f64 / 1000.0 / count as f64);
    eprintln!("Total XPath:    {:>8.2}ms ({:.2}ms/file)",
        (xml_load + query_exec + result_proc) as f64 / 1000.0,
        (xml_load + query_exec + result_proc) as f64 / 1000.0 / count as f64);
}

// Pre-compiled regexes for location extraction (compiled once, reused forever)
static START_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"start="(\d+):(\d+)""#).unwrap());
static END_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"end="(\d+):(\d+)""#).unwrap());
static LEGACY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"startLine="(\d+)"\s+startCol="(\d+)"\s+endLine="(\d+)"\s+endCol="(\d+)""#).unwrap()
});
static STRIP_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\s*(start|end|startLine|startCol|endLine|endCol)="[^"]*""#).unwrap()
});
static WHITESPACE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r">([^<]+)<").unwrap());

// Thread-local cache for compiled XPath queries
// Each thread gets its own compiled query to avoid RefCell conflicts
thread_local! {
    static QUERY_CACHE: RefCell<Option<(String, SequenceQuery)>> = const { RefCell::new(None) };
}

/// Execute a cached query - compiles once per thread, reuses thereafter
fn execute_cached_query(
    xpath: &str,
    xml: &str,
    source_lines: &[String],
    file_path: &str,
    ignore_whitespace: bool,
) -> Result<Vec<Match>, XPathError> {
    // Optionally strip whitespace for whitespace-insensitive matching
    let xml_to_query = if ignore_whitespace {
        strip_xml_whitespace(xml)
    } else {
        xml.to_string()
    };

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

        // Load XML into xee-xpath
        let t0 = Instant::now();
        let mut documents = Documents::new();
        let doc = documents
            .add_string(
                "file:///query".try_into().unwrap(),
                &xml_to_query,
            )
            .map_err(|e| XPathError::XmlParse(e.to_string()))?;
        let t1 = Instant::now();

        // Execute the query
        let results = query
            .execute(&mut documents, doc)
            .map_err(|e: xee_xpath::error::Error| XPathError::Execute(e.to_string()))?;
        let t2 = Instant::now();

        // Convert results to Match objects
        let mut matches = Vec::new();

        for item in results.iter() {
            match item {
                xee_xpath::Item::Node(node) => {
                    let xot = documents.xot();
                    let xml_fragment = xot.to_string(node).unwrap_or_default();
                    let (line, col, end_line, end_col) = extract_location(&xml_fragment);
                    let value = xot.string_value(node);
                    let actual_file = extract_file_path_from_xml(&xml_fragment, file_path);

                    let m = Match::with_location(
                        actual_file,
                        line,
                        col,
                        end_line,
                        end_col,
                        value,
                        source_lines.to_vec(),
                    ).with_xml_fragment(xml_fragment);

                    matches.push(m);
                }
                xee_xpath::Item::Atomic(atomic) => {
                    let value = atomic.to_string().unwrap_or_default();
                    matches.push(Match::new(file_path.to_string(), value));
                }
                xee_xpath::Item::Function(_) => {}
            }
        }
        let t3 = Instant::now();

        // Record timing stats
        TIMING_XML_LOAD.fetch_add((t1 - t0).as_micros() as u64, Ordering::Relaxed);
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

    /// Execute an XPath query against XML and return matches
    ///
    /// The query is automatically cached per-thread, so repeated calls with
    /// the same XPath expression are efficient even across many files.
    pub fn query(
        &self,
        xml: &str,
        xpath: &str,
        source_lines: &[String],
        file_path: &str,
    ) -> Result<Vec<Match>, XPathError> {
        execute_cached_query(xpath, xml, source_lines, file_path, self.ignore_whitespace)
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

/// Strip whitespace from text content in XML (content between > and <)
fn strip_xml_whitespace(xml: &str) -> String {
    WHITESPACE_RE.replace_all(xml, |caps: &regex::Captures| {
        let text = &caps[1];
        let stripped: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        format!(">{}<", stripped)
    }).to_string()
}

/// Extract location from XML fragment attributes
fn extract_location(xml: &str) -> (u32, u32, u32, u32) {
    let mut line = 1u32;
    let mut col = 1u32;
    let mut end_line = 1u32;
    let mut end_col = 1u32;

    // Try compact format: start="line:col" end="line:col"
    if let Some(caps) = START_RE.captures(xml) {
        line = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
        col = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
    }

    if let Some(caps) = END_RE.captures(xml) {
        end_line = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(line);
        end_col = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(col);
    }

    // Fallback to legacy format: startLine, startCol, endLine, endCol
    if line == 1 && col == 1 {
        if let Some(caps) = LEGACY_RE.captures(xml) {
            line = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
            col = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
            end_line = caps.get(3).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
            end_col = caps.get(4).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
        }
    }

    (line, col, end_line, end_col)
}

/// Extract file path from XML context (looks for ancestor File element)
fn extract_file_path_from_xml(_xml: &str, default: &str) -> String {
    // For now, return the default
    // Could parse XML to find File/@path attribute if needed
    default.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_location_compact() {
        let xml = r#"<class start="5:10" end="10:2">Foo</class>"#;
        let (line, col, end_line, end_col) = extract_location(xml);
        assert_eq!(line, 5);
        assert_eq!(col, 10);
        assert_eq!(end_line, 10);
        assert_eq!(end_col, 2);
    }

    #[test]
    fn test_strip_location_metadata() {
        let xml = r#"<class start="1:1" end="5:2">Foo</class>"#;
        let stripped = XPathEngine::strip_location_metadata(xml);
        assert_eq!(stripped, "<class>Foo</class>");
    }

    #[test]
    fn test_query_semantic_xml() {
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

        let engine = XPathEngine::new();
        let matches = engine.query(xml, "//variable", &[], "test.ts").unwrap();
        assert_eq!(matches.len(), 1, "Should find one variable element");

        let matches = engine.query(xml, "//name", &[], "test.ts").unwrap();
        assert_eq!(matches.len(), 1, "Should find one name element");
    }

    #[test]
    fn test_compiled_query_reuse() {
        let xml1 = r#"<root><item>a</item></root>"#;
        let xml2 = r#"<root><item>b</item><item>c</item></root>"#;

        let engine = XPathEngine::new();
        let compiled = engine.compile("//item").unwrap();

        let matches1 = compiled.execute(xml1, &[], "test1.xml").unwrap();
        assert_eq!(matches1.len(), 1);

        let matches2 = compiled.execute(xml2, &[], "test2.xml").unwrap();
        assert_eq!(matches2.len(), 2);
    }

    #[test]
    fn test_query_with_xml_prolog() {
        // Test with the actual XML prolog that generate_xml_document creates
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

        let engine = XPathEngine::new();
        let matches = engine.query(xml, "//variable", &[], "test.ts").unwrap();
        assert_eq!(matches.len(), 1, "Should find one variable element with XML prolog");
    }

    #[test]
    fn test_query_parsed_typescript() {
        use crate::{parse_string, generate_xml_document};

        let source = "let x = 1;";
        let result = parse_string(source, "typescript", "test.ts".to_string(), false).unwrap();
        let xml = generate_xml_document(&[result.clone()], false); // compact for XPath

        let engine = XPathEngine::new();
        let matches = engine.query(&xml, "//variable", &result.source_lines, "test.ts").unwrap();
        assert_eq!(matches.len(), 1, "Should find one variable element");

        // Also test querying nested elements
        let matches = engine.query(&xml, "//name", &result.source_lines, "test.ts").unwrap();
        assert_eq!(matches.len(), 1, "Should find one name element");

        let matches = engine.query(&xml, "//value/number", &result.source_lines, "test.ts").unwrap();
        assert_eq!(matches.len(), 1, "Should find number inside value");
    }
}
