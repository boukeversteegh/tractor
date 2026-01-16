//! Experiments for creating XPath-queryable documents without string serialization
//!
//! The problem: xee-xpath requires a Document to run XPath queries, but we have
//! a raw xot::Xot tree. Currently we serialize to string and re-parse, which is inefficient.
//!
//! This module tests several approaches to avoid the roundtrip:
//! 1. Create a shell document, then replace its children with our tree
//! 2. Build our tree directly into a pre-made document's root node
//! 3. Wrap XPath result nodes in documents for chained queries

use xot::{Xot, Node as XotNode};
use xee_xpath::{Documents, Queries, Query, DocumentHandle};

/// Approach 1: Create a shell document and replace its children
///
/// The idea is:
/// 1. Create a minimal document: `<shell/>`
/// 2. Get the root element from the document
/// 3. Copy/move children from our source tree to the shell's root
pub fn test_shell_document_approach() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Approach 1: Shell Document with Child Replacement ===\n");

    // Step 1: Create a source tree (simulating what XotBuilder produces)
    let mut source_xot = Xot::new();
    let root_name = source_xot.add_name("Files");
    let file_name = source_xot.add_name("File");
    let program_name = source_xot.add_name("program");
    let variable_name = source_xot.add_name("variable");
    let name_name = source_xot.add_name("name");

    // Build: <Files><File><program><variable><name>x</name></variable></program></File></Files>
    let root_el = source_xot.new_element(root_name);
    let file_el = source_xot.new_element(file_name);
    let program_el = source_xot.new_element(program_name);
    let variable_el = source_xot.new_element(variable_name);
    let name_el = source_xot.new_element(name_name);
    let text = source_xot.new_text("x");

    source_xot.append(name_el, text)?;
    source_xot.append(variable_el, name_el)?;
    source_xot.append(program_el, variable_el)?;
    source_xot.append(file_el, program_el)?;
    source_xot.append(root_el, file_el)?;

    // Now we have a tree without a document wrapper
    println!("Source tree (no document): {}", source_xot.to_string(root_el)?);

    // Step 2: Create a shell document
    let shell_name = source_xot.add_name("shell");
    let shell_el = source_xot.new_element(shell_name);
    let doc = source_xot.new_document_with_element(shell_el)?;

    println!("Shell document created: {}", source_xot.to_string(doc)?);

    // Step 3: Get the shell element and try to replace it
    let shell_in_doc = source_xot.document_element(doc)?;

    // Option A: Try to move all children of root_el into shell_in_doc
    // First, collect children (we need to collect because iteration will be invalidated)
    let children: Vec<XotNode> = source_xot.children(root_el).collect();
    println!("Children to move: {:?}", children.len());

    for child in children {
        // Detach from source and append to shell
        source_xot.detach(child)?;
        source_xot.append(shell_in_doc, child)?;
    }

    println!("After moving children: {}", source_xot.to_string(doc)?);

    // Step 4: Rename shell to Files
    if let Some(elem) = source_xot.element_mut(shell_in_doc) {
        elem.set_name(root_name);
    }

    println!("After renaming shell to Files: {}", source_xot.to_string(doc)?);

    // Step 5: Now try XPath!
    // The catch: xee-xpath has its own Documents type that wraps a Xot internally.
    // We need to see if we can use our Xot with it.

    // Let's try to use xee-xpath's Documents::new() and see if we can access its xot
    let documents = Documents::new();

    // The problem: Documents::add_string parses a new document.
    // We can't directly insert our existing Xot tree.
    // Let's check what we can do with documents.xot()
    let _xot_ref = documents.xot();
    println!("We have access to Documents' internal Xot (immutable)");

    // Unfortunately, Documents doesn't expose a mutable xot or a way to add existing trees.
    // The only way to add content is via add_string, add_url, etc.

    println!("\n❌ Approach 1 LIMITATION: xee-xpath's Documents doesn't allow inserting existing Xot trees");
    println!("   We can manipulate our own Xot, but can't use it with xee-xpath's query engine.\n");

    Ok(())
}

/// Approach 2: Build directly into Documents' Xot
///
/// The idea is:
/// 1. Create xee-xpath::Documents with a minimal XML string
/// 2. Get access to its internal Xot (if possible mutably)
/// 3. Build our tree directly into Documents' Xot
pub fn test_build_into_documents_approach() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Approach 2: Build Directly into Documents' Xot ===\n");

    let mut documents = Documents::new();

    // Parse a minimal document
    let doc_handle: DocumentHandle = documents.add_string(
        "file:///shell".try_into().unwrap(),
        "<Files/>",
    )?;

    println!("Created shell document in Documents");
    println!("DocumentHandle: {:?}", doc_handle);

    // Now we need to modify documents' internal Xot...
    // documents.xot() returns &Xot (immutable!)
    let _xot = documents.xot(); // We have access but it's immutable

    // Note: DocumentHandle is NOT the same as xot::Node
    // DocumentHandle is an opaque handle used by xee-xpath
    // We cannot directly use it with xot.document_element()

    // Let's try to query it and examine what we get back
    let queries = Queries::default();
    let root_query = queries.sequence("/*")?;
    let root_results = root_query.execute(&mut documents, doc_handle)?;

    println!("Query '/*' to find root element:");
    for item in root_results.iter() {
        if let xee_xpath::Item::Node(node) = item {
            let xot = documents.xot();
            println!("  Root element: {}", xot.to_string(node)?);
            // Now we have an xot::Node, but we still can't mutate it
            // because documents.xot() returns &Xot, not &mut Xot
        }
    }

    // The problem: we only have &Xot, not &mut Xot
    // We cannot modify the Documents' internal Xot!

    println!("\n❌ Approach 2 LIMITATION: Documents only provides immutable access to its Xot");
    println!("   documents.xot() returns &Xot, not &mut Xot\n");

    Ok(())
}

/// Approach 3: Check if Documents has mutable access methods
///
/// Let's exhaustively check what methods Documents provides
pub fn test_documents_api_exploration() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Approach 3: Documents API Exploration ===\n");

    let mut documents = Documents::new();

    // Methods we know about:
    // - add_string(uri, xml) -> parses XML string, returns doc node
    // - add_url(uri) -> fetches and parses from URL
    // - xot() -> &Xot (immutable)

    // Let's see if there are any other methods by trying various patterns...

    // Try creating a document and examining its structure
    let doc = documents.add_string(
        "file:///test".try_into().unwrap(),
        r#"<root><child>text</child></root>"#,
    )?;

    // We can query it!
    let queries = Queries::default();
    let query = queries.sequence("//child")?;
    let results = query.execute(&mut documents, doc)?;

    println!("XPath query '//child' results:");
    for item in results.iter() {
        match item {
            xee_xpath::Item::Node(node) => {
                let xot = documents.xot();
                println!("  Node: {}", xot.to_string(node)?);
            }
            xee_xpath::Item::Atomic(atomic) => {
                println!("  Atomic: {:?}", atomic);
            }
            _ => {}
        }
    }

    // Can we use results as context for another query?
    // The results are xot::Node values from documents.xot()

    println!("\n=== Testing chained queries (secondary use case) ===\n");

    // Create a more complex document
    let doc2 = documents.add_string(
        "file:///complex".try_into().unwrap(),
        r#"<Files>
            <File path="a.ts">
                <class><name>Foo</name></class>
                <class><name>Bar</name></class>
            </File>
            <File path="b.ts">
                <class><name>Baz</name></class>
            </File>
        </Files>"#,
    )?;

    // First query: find all class elements
    let class_query = queries.sequence("//class")?;
    let classes = class_query.execute(&mut documents, doc2)?;

    println!("First query '//class' found {} results", classes.len());

    // Now we want to query each result for //name
    // The problem: these are nodes in the existing document, not new documents
    //
    // XPath allows querying with a context node. Let's try!

    let name_query = queries.sequence("name")?; // relative path from context

    for (i, item) in classes.iter().enumerate() {
        if let xee_xpath::Item::Node(class_node) = item {
            // Can we execute with class_node as context?
            let names = name_query.execute(&mut documents, class_node)?;
            let xot = documents.xot();

            println!("Class {}: {}", i, xot.to_string(class_node)?);
            for name_item in names.iter() {
                if let xee_xpath::Item::Node(name_node) = name_item {
                    println!("  -> name: {}", xot.string_value(name_node));
                }
            }
        }
    }

    println!("\n✅ Chained queries WORK! We can use result nodes as context for new queries.");
    println!("   No need to wrap in new documents.\n");

    Ok(())
}

/// Approach 4: Use xot's internal document node directly
///
/// Perhaps we misunderstood - let's see if creating a document in our Xot
/// and then somehow integrating it works
pub fn test_direct_document_creation() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Approach 4: Direct Document Creation in Own Xot ===\n");

    // Create our Xot with a proper document
    let mut xot = Xot::new();

    let files_name = xot.add_name("Files");
    let file_name = xot.add_name("File");
    let class_name = xot.add_name("class");
    let name_name = xot.add_name("name");

    // Build structure
    let files_el = xot.new_element(files_name);
    let file_el = xot.new_element(file_name);
    let class_el = xot.new_element(class_name);
    let name_el = xot.new_element(name_name);
    let text = xot.new_text("MyClass");

    xot.append(name_el, text)?;
    xot.append(class_el, name_el)?;
    xot.append(file_el, class_el)?;
    xot.append(files_el, file_el)?;

    // Create document WITH the root element
    let doc = xot.new_document_with_element(files_el)?;

    println!("Created document: {}", xot.to_string(doc)?);

    // The document is valid, but we can't use it with xee-xpath::Documents
    // because Documents maintains its own internal Xot.

    // What if we serialize and immediately re-parse into Documents?
    // This is what we're trying to avoid, but let's measure it

    let xml_string = xot.to_string(doc)?;
    println!("Serialized: {} bytes", xml_string.len());

    let mut documents = Documents::new();
    let reparsed_doc = documents.add_string(
        "file:///reparsed".try_into().unwrap(),
        &xml_string,
    )?;

    // Now we can query
    let queries = Queries::default();
    let query = queries.sequence("//name")?;
    let results = query.execute(&mut documents, reparsed_doc)?;

    println!("XPath '//name' found {} results after reparse", results.len());

    println!("\n❌ Approach 4: Still requires serialize + reparse");
    println!("   No direct path from xot::Xot document to xee-xpath::Documents\n");

    Ok(())
}

/// Approach 5: Build into Documents' Xot by getting mutable access
///
/// DISCOVERY: Documents DOES have xot_mut() method!
pub fn test_documents_mutable_exploration() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Approach 5: Deep Dive into Documents API ===\n");

    let mut documents = Documents::new();

    // Create a shell document
    let doc_handle = documents.add_string(
        "file:///base".try_into().unwrap(),
        "<shell/>",
    )?;

    println!("Shell document created: {:?}", doc_handle);

    // KEY DISCOVERY: Documents HAS xot_mut()!
    // Also has document_node(handle) to get xot::Node from DocumentHandle!

    // Get the document node from handle
    let doc_node = documents.document_node(doc_handle)
        .ok_or("Failed to get document node")?;

    println!("Got document node: {:?}", doc_node);

    // Now get mutable access to Xot!
    let xot = documents.xot_mut();

    // Get the root element
    let root = xot.document_element(doc_node)?;
    println!("Root element before modification: {}", xot.to_string(root)?);

    // Now let's try to modify it - add children!
    let files_name = xot.add_name("Files");
    let file_name = xot.add_name("File");
    let class_name = xot.add_name("class");
    let name_name = xot.add_name("name");

    // Create new elements
    let file_el = xot.new_element(file_name);
    let class_el = xot.new_element(class_name);
    let name_el = xot.new_element(name_name);
    let text = xot.new_text("MyClass");

    // Build the tree
    xot.append(name_el, text)?;
    xot.append(class_el, name_el)?;
    xot.append(file_el, class_el)?;
    xot.append(root, file_el)?;

    // Rename root from "shell" to "Files"
    if let Some(elem) = xot.element_mut(root) {
        elem.set_name(files_name);
    }

    println!("After modification: {}", xot.to_string(doc_node)?);

    // Now try XPath on the modified document!
    let queries = Queries::default();
    let query = queries.sequence("//name")?;
    let results = query.execute(&mut documents, doc_handle)?;

    println!("\n✅ XPath '//name' on modified document found {} results:", results.len());
    for item in results.iter() {
        if let xee_xpath::Item::Node(node) = item {
            let xot = documents.xot();
            println!("  Result: {}", xot.to_string(node)?);
            println!("  String value: {}", xot.string_value(node));
        }
    }

    println!("\n✅✅✅ SUCCESS! We CAN modify Documents' Xot and then query it!");
    println!("    The workflow is:");
    println!("    1. documents.add_string(uri, \"<shell/>\") -> DocumentHandle");
    println!("    2. documents.document_node(handle) -> xot::Node");
    println!("    3. documents.xot_mut() -> &mut Xot");
    println!("    4. Build tree using xot.append(), etc.");
    println!("    5. query.execute(&mut documents, handle) -> results\n");

    Ok(())
}

/// Summary and recommendations
pub fn run_all_experiments() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  XOT Document Manipulation Experiments                       ║");
    println!("║  Goal: Avoid serialize+parse roundtrip for XPath queries     ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    test_shell_document_approach()?;
    test_build_into_documents_approach()?;
    test_documents_api_exploration()?;
    test_direct_document_creation()?;
    test_documents_mutable_exploration()?;

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  SUMMARY AND RECOMMENDATIONS                                 ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("KEY DISCOVERY:");
    println!("✅✅✅ xee-xpath::Documents DOES have xot_mut() and document_node()!");
    println!();
    println!("FINDINGS:");
    println!("1. xot::Xot nodes ARE mutable - we can build and modify trees freely");
    println!("2. xee-xpath::Documents HAS xot_mut() -> &mut Xot");
    println!("3. Documents HAS document_node(handle) -> Option<xot::Node>");
    println!("4. We CAN build directly into Documents' Xot and query it!");
    println!("5. Chained XPath queries WORK - result nodes can be query contexts");
    println!();
    println!("PRIMARY USE CASE (SOLVED!):");
    println!("✅ We CAN avoid the serialize+parse roundtrip!");
    println!("   The workflow is:");
    println!("   1. Create Documents::new()");
    println!("   2. Add shell document: documents.add_string(uri, \"<shell/>\")");
    println!("   3. Get doc node: documents.document_node(handle)");
    println!("   4. Get mutable xot: documents.xot_mut()");
    println!("   5. Build our tree directly into the shell document's root");
    println!("   6. Query with: query.execute(&mut documents, handle)");
    println!();
    println!("SECONDARY USE CASE (SOLVED):");
    println!("✅ Chained queries don't need document wrapping!");
    println!("   XPath result nodes can directly be used as context for new queries.");
    println!("   Example: query.execute(&mut documents, result_node)");
    println!();
    println!("IMPLEMENTATION NOTES:");
    println!("- XotBuilder should be modified to build directly into Documents' Xot");
    println!("- Alternatively, create a new XeeBuilder that wraps Documents");
    println!("- The shell document approach works: create minimal doc, then modify");
    println!();

    Ok(())
}

/// Approach 6: Complete workflow - build TreeSitter AST directly into Documents
///
/// This demonstrates the full solution: TreeSitter -> Documents -> XPath
/// without any serialize/parse roundtrip!
pub fn test_full_workflow_no_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Approach 6: Full Workflow Without Roundtrip ===\n");

    // Step 1: Create Documents and add a shell document
    let mut documents = Documents::new();
    let doc_handle = documents.add_string(
        "file:///source.ts".try_into().unwrap(),
        "<Files/>",  // Start with the correct root element name
    )?;

    // Step 2: Get the document node and mutable Xot
    let doc_node = documents.document_node(doc_handle)
        .ok_or("Failed to get document node")?;

    let xot = documents.xot_mut();
    let root = xot.document_element(doc_node)?;

    // Step 3: Build tree structure (simulating what XotBuilder does from TreeSitter AST)
    // This would normally come from TreeSitter, but we simulate it here

    // Create name IDs
    let file_name = xot.add_name("File");
    let program_name = xot.add_name("program");
    let variable_name = xot.add_name("variable");
    let name_name = xot.add_name("name");
    let value_name = xot.add_name("value");
    let number_name = xot.add_name("number");
    let path_attr = xot.add_name("path");
    let start_attr = xot.add_name("start");
    let end_attr = xot.add_name("end");

    // Build: <File path="test.ts"><program><variable><name>x</name><value><number>42</number></value></variable></program></File>
    let file_el = xot.new_element(file_name);
    xot.attributes_mut(file_el).insert(path_attr, "test.ts".to_string());

    let program_el = xot.new_element(program_name);
    xot.attributes_mut(program_el).insert(start_attr, "1:1".to_string());
    xot.attributes_mut(program_el).insert(end_attr, "1:12".to_string());

    let variable_el = xot.new_element(variable_name);
    xot.attributes_mut(variable_el).insert(start_attr, "1:1".to_string());
    xot.attributes_mut(variable_el).insert(end_attr, "1:12".to_string());

    let name_el = xot.new_element(name_name);
    let name_text = xot.new_text("x");
    xot.append(name_el, name_text)?;

    let value_el = xot.new_element(value_name);
    let number_el = xot.new_element(number_name);
    xot.attributes_mut(number_el).insert(start_attr, "1:9".to_string());
    xot.attributes_mut(number_el).insert(end_attr, "1:11".to_string());
    let number_text = xot.new_text("42");
    xot.append(number_el, number_text)?;
    xot.append(value_el, number_el)?;

    // Assemble the tree
    xot.append(variable_el, name_el)?;
    xot.append(variable_el, value_el)?;
    xot.append(program_el, variable_el)?;
    xot.append(file_el, program_el)?;
    xot.append(root, file_el)?;

    println!("Built tree directly in Documents (no serialization):");
    println!("{}", xot.to_string(doc_node)?);

    // Step 4: Run XPath queries!
    let queries = Queries::default();

    // Query 1: Find all variables
    let var_query = queries.sequence("//variable")?;
    let var_results = var_query.execute(&mut documents, doc_handle)?;
    println!("\nQuery '//variable' found {} results", var_results.len());

    // Query 2: Find variable names
    let name_query = queries.sequence("//variable/name")?;
    let name_results = name_query.execute(&mut documents, doc_handle)?;
    println!("Query '//variable/name' found {} results", name_results.len());
    for item in name_results.iter() {
        if let xee_xpath::Item::Node(node) = item {
            println!("  Variable name: {}", documents.xot().string_value(node));
        }
    }

    // Query 3: Find numbers with their location
    let num_query = queries.sequence("//number/@start")?;
    let num_results = num_query.execute(&mut documents, doc_handle)?;
    println!("Query '//number/@start' found {} results", num_results.len());
    for item in num_results.iter() {
        if let xee_xpath::Item::Node(node) = item {
            println!("  Number location: {}", documents.xot().string_value(node));
        }
    }

    // Query 4: Complex query - find value elements containing numbers > 10
    let complex_query = queries.sequence("//value[number > 10]/number/text()")?;
    let complex_results = complex_query.execute(&mut documents, doc_handle)?;
    println!("Query '//value[number > 10]/number/text()' found {} results", complex_results.len());
    for item in complex_results.iter() {
        match item {
            xee_xpath::Item::Node(node) => {
                println!("  Value: {}", documents.xot().string_value(node));
            }
            xee_xpath::Item::Atomic(atomic) => {
                println!("  Atomic value: {:?}", atomic);
            }
            _ => {}
        }
    }

    println!("\n✅ Full workflow successful! TreeSitter AST -> Documents -> XPath");
    println!("   No serialize/parse roundtrip needed!\n");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_experiments() {
        run_all_experiments().expect("experiments should run");
    }

    #[test]
    fn test_chained_queries() {
        test_documents_api_exploration().expect("chained queries should work");
    }

    #[test]
    fn test_full_workflow() {
        test_full_workflow_no_roundtrip().expect("full workflow should work");
    }
}
