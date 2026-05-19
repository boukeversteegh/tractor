//! INI semantic shape: top-level keys, sections, dotted sections,
//! and comments (both `#` and `;` prefixes).
//!
//! One focused source per construct.

use crate::support::semantic::*;

#[test]
fn ini_top_level_key_value() {
    claim("`name = my-app` renders as <name>my-app</name> at the document root",
        &mut parse_src("ini", "name = my-app\n"),
        "//document[name='my-app']",
        1);
}

#[test]
fn ini_section_groups_keys_under_section_name() {
    claim("`[database]` followed by key/value pairs nests them under <database>",
        &mut parse_src("ini", r#"
            [database]
            host = localhost
            port = 5432
        "#),
        &multi_xpath(r#"
            //document/database
                [host='localhost']
                [port='5432']
        "#),
        1);
}

/// Unlike TOML, INI's `[database.credentials]` does NOT split on
/// the dot — it stays as one element with a literal dotted name.
/// This pins that behaviour so a future "split dotted INI sections"
/// change is a deliberate decision, not a silent regression.
#[test]
fn ini_dotted_section_name_stays_literal() {
    claim("`[database.credentials]` keeps the dot in the element name (no nested split)",
        &mut parse_src("ini", r#"
            [database.credentials]
            username = admin
        "#),
        "//document/database.credentials[username='admin']",
        1);
}

#[test]
fn ini_hash_comment_renders_as_comment_element() {
    claim("`# comment` renders as <comment> with the body text (no `#` prefix)",
        &mut parse_src("ini", "# Global settings\nname = my-app\n"),
        "//comment[.='Global settings']",
        1);
}

#[test]
fn ini_semicolon_comment_renders_as_comment_element() {
    claim("`; comment` collapses to the same <comment> element as `#`",
        &mut parse_src("ini", "; semicolon style\nname = x\n"),
        "//comment[.='semicolon style']",
        1);
}
