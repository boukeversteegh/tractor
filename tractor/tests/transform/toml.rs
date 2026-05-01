//! TOML semantic shape: top-level scalars, tables, dotted tables,
//! arrays of tables, inline tables, arrays of strings, quoted keys
//! with @key attribute preservation, and deeply nested table paths.
//!
//! One focused source per construct.

use crate::support::semantic::*;

#[test]
fn toml_top_level_scalars() {
    claim("top-level keys render as element-with-text under <document>",
        &mut parse_src("toml", r#"
            title = "My App"
            version = "1.0.0"
            quoted = "hello world"
        "#),
        &multi_xpath(r#"
            //document
                [title='My App']
                [version='1.0.0']
                [quoted='hello world']
        "#),
        1);
}

#[test]
fn toml_table_with_typed_scalars() {
    claim("`[database]` section nests its key/value pairs under <database>",
        &mut parse_src("toml", r#"
            [database]
            host = "localhost"
            port = 5432
            enabled = true
        "#),
        &multi_xpath(r#"
            //document/database
                [host='localhost']
                [port='5432']
                [enabled='true']
        "#),
        1);
}

#[test]
fn toml_dotted_table_renders_as_nested_path() {
    claim("`[database.credentials]` produces nested <database>/<credentials>, not a flat dotted name",
        &mut parse_src("toml", r#"
            [database.credentials]
            username = "admin"
            password = "secret"
        "#),
        &multi_xpath(r#"
            //document/database/credentials
                [username='admin']
                [password='secret']
        "#),
        1);
}

#[test]
fn toml_deeply_nested_table_path() {
    claim("`[nested.level1.level2]` chains arbitrarily deep",
        &mut parse_src("toml", r#"
            [nested.level1.level2]
            value = "deep"
        "#),
        "//document/nested/level1/level2[value='deep']",
        1);
}

#[test]
fn toml_array_of_strings() {
    claim("string array renders as <features> with one <item> per element",
        &mut parse_src("toml", r#"
            features = ["auth", "logging", "metrics"]
        "#),
        &multi_xpath(r#"
            //features
                [count(item)=3]
                [item='auth']
                [item='logging']
                [item='metrics']
        "#),
        1);
}

/// Multiple `[[servers]]` occurrences collapse into ONE `<servers>`
/// element with N `<item>` children, matching TOML's other array
/// forms (and JSON / YAML). Closes todo/35.
#[test]
fn toml_array_of_tables() {
    claim("two `[[servers]]` occurrences collapse into one <servers> with two <item>s",
        &mut parse_src("toml", r#"
            [[servers]]
            name = "web-1"
            port = 8080

            [[servers]]
            name = "web-2"
            port = 8081
        "#),
        &multi_xpath(r#"
            //document
                [count(servers)=1]
                [servers/item[name='web-1'][port='8080']]
                [servers/item[name='web-2'][port='8081']]
        "#),
        1);
}

#[test]
fn toml_inline_table() {
    claim("inline table `{x = 1, y = 2}` produces <inline> with both fields",
        &mut parse_src("toml", r#"
            inline = {x = 1, y = 2}
        "#),
        &multi_xpath(r#"
            //inline
                [x='1']
                [y='2']
        "#),
        1);
}

/// Keys with whitespace (or any other character not valid in an XML
/// element name) get sanitised to a valid identifier; the original
/// key text is preserved on the `@key` attribute so queries can
/// recover the literal source.
#[test]
fn toml_quoted_key_is_sanitised_with_key_attribute() {
    claim(r#"`"first name" = "Alice"` becomes <first_name @key="first name">"#,
        &mut parse_src("toml", r#"
            "first name" = "Alice"
        "#),
        "//first_name[@key='first name']",
        1);
}
