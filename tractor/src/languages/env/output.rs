//! Output element names — env's vocabulary after transform.
//!
//! Variable names become user-driven element names (e.g. `<DB_HOST>`),
//! so the output vocabulary is open. Only `<document>` (the renamed
//! root) and `<comment>` are closed structural names.

pub const DOCUMENT: &str = "document";
pub const COMMENT: &str = "comment";
