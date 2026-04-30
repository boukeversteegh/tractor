//! Output element names — INI's vocabulary after transform.
//!
//! Section names and setting keys become user-driven element names
//! (e.g. `<database>`, `<host>`), so the output vocabulary is open.
//! Only `<comment>` is a closed structural name.
//!
//! No `TractorNodeSpec` table: data-language convention shared with JSON,
//! YAML, and TOML — `node_spec` stays `None` in the language registry.

pub const COMMENT: &str = "comment";
