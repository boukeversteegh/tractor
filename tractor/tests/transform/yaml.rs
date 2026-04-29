//! YAML semantic shape: top-level scalars, mappings, sequences,
//! flow-style collections, quoted scalars, multi-line block
//! scalars, sanitised keys, and multi-document streams.
//!
//! YAML's default tree mode is **Data** (keys → elements, scalars
//! → text); `parse_src_with_mode(..., Some(TreeMode::Structure))`
//! exposes the deeper structural vocabulary (`<property>`, `<key>`,
//! `<value>`, `<string>`, `<object>`) used by a few tests at the
//! bottom of this file.
//!
//! One focused source per construct.

use crate::support::semantic::*;
use tractor::TreeMode;

#[test]
fn yaml_top_level_scalar() {
    claim("`name: my-app` renders as <name>my-app</name>",
        &mut parse_src("yaml", "name: my-app\nversion: 1.0.0\n"),
        "//document[name='my-app'][version='1.0.0']",
        1);
}

#[test]
fn yaml_nested_mapping() {
    claim("`database: { host: ... }` nests scalars under <database>",
        &mut parse_src("yaml", r#"
            database:
              host: localhost
              port: 5432
        "#),
        "//document/database[host='localhost'][port='5432']",
        1);
}

#[test]
fn yaml_deep_mapping_path() {
    claim("`database.credentials.username` reaches under three nested mappings",
        &mut parse_src("yaml", r#"
            database:
              credentials:
                username: admin
        "#),
        "//document/database/credentials[username='admin']",
        1);
}

/// YAML sequences of mappings currently render as a separate
/// `<servers>` per item rather than one `<servers>` with two items.
/// Tracked as **TODO #35**; flip this test to assert one parent
/// once fixed.
#[test]
fn yaml_sequence_of_mappings() {
    claim("two list items under `servers:` currently render as two <servers> siblings (TODO #35: should flatten to one)",
        &mut parse_src("yaml", r#"
            servers:
              - name: web-1
                port: 8080
              - name: web-2
                port: 8081
        "#),
        &multi_xpath(r#"
            //document
                [count(servers)=2]
                [servers[name='web-1'][port='8080']]
                [servers[name='web-2'][port='8081']]
        "#),
        1);
}

/// Same pattern as `yaml_sequence_of_mappings` but with scalar
/// items — TODO #35 covers both forms.
#[test]
fn yaml_sequence_of_scalars() {
    claim("`features: [...]` (block style) currently renders as one <features> per scalar (TODO #35: should flatten)",
        &mut parse_src("yaml", r#"
            features:
              - auth
              - logging
              - metrics
        "#),
        &multi_xpath(r#"
            //document
                [count(features)=3]
                [features='auth']
                [features='logging']
                [features='metrics']
        "#),
        1);
}

#[test]
fn yaml_flow_map() {
    claim("flow-style `{x: 1, y: 2}` renders as <flow_map> with both fields",
        &mut parse_src("yaml", "flow_map: {x: 1, y: 2}\n"),
        "//document/flow_map[x='1'][y='2']",
        1);
}

#[test]
fn yaml_flow_list() {
    claim("flow-style `[a, b, c]` follows the same per-item-element shape as block sequences",
        &mut parse_src("yaml", "flow_list: [a, b, c]\n"),
        "//document[count(flow_list)=3]",
        1);
}

#[test]
fn yaml_quoted_scalar() {
    claim(r#"`quoted: "hello world"` strips the quotes; the text is the scalar value"#,
        &mut parse_src("yaml", "quoted: \"hello world\"\n"),
        "//document[quoted='hello world']",
        1);
}

#[test]
fn yaml_multiline_block_scalar() {
    claim("`multiline: |` preserves embedded newlines so `contains(., 'line one')` matches",
        &mut parse_src("yaml", "multiline: |\n  line one\n  line two\n"),
        "//document/multiline[contains(.,'line one') and contains(.,'line two')]",
        1);
}

#[test]
fn yaml_quoted_key_with_whitespace_is_sanitised() {
    let mut tree = parse_src("yaml", "\"first name\": Alice\n");

    claim("`\"first name\"` is sanitised to <first_name> with @key holding the literal source",
        &mut tree, "//first_name[@key='first name']", 1);

    claim("the sanitised element holds the scalar text",
        &mut tree, "//first_name[text()='Alice']", 1);
}

#[test]
fn yaml_multi_document_stream() {
    let mut tree = parse_src("yaml", r#"---
name: doc1
value: one
---
name: doc2
value: two
---
name: doc3
value: three
"#);

    claim("`---`-separated YAML produces one <document> per doc",
        &mut tree, "//document", 3);

    claim("indexed access targets a specific document",
        &mut tree,
        &multi_xpath(r#"
            //document[2]
                [name='doc2']
        "#),
        1);

    claim("descendant queries walk all documents",
        &mut tree, "//name", 3);
}

/// Default `Data` tree mode collapses YAML structure into the
/// keyed shape used in the other tests. `Structure` mode preserves
/// the parser vocabulary (`<property>`/`<key>`/`<value>`/`<string>`)
/// for use cases that need to inspect the YAML grammar itself —
/// e.g. linting key style independent of value content.
#[test]
fn yaml_structure_mode_exposes_grammar_vocabulary() {
    let mut tree = parse_src_with_mode(
        "yaml",
        "name: my-app\n",
        Some(TreeMode::Structure),
    );

    claim("structure mode renders the YAML stream as <document>/<object>",
        &mut tree, "//document/object", 1);

    claim("structure mode exposes <property>/<key>/<string> + <value>/<string> for each scalar",
        &mut tree,
        &multi_xpath(r#"
            //property[key/string='name']
                /value/string[.='my-app']
        "#),
        1);
}
