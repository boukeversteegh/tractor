//! Output element names — YAML's vocabulary after transform.
//!
//! Two output branches:
//!   - `/syntax` — closed vocabulary shared with JSON's syntax branch.
//!   - `/data` — open vocabulary derived from mapping keys; only the
//!     structural wrapper `item` is named here.
//!
//! No `NodeSpec` table: data-branch names are not a closed set, so
//! `node_spec` stays `None` in the language registry.

// Syntax branch — closed vocabulary
pub const OBJECT: &str = "object";
pub const ARRAY: &str = "array";
pub const PROPERTY: &str = "property";
pub const KEY: &str = "key";
pub const STRING: &str = "string";
pub const NUMBER: &str = "number";
pub const BOOL: &str = "bool";
pub const NULL: &str = "null";

// Data branch — structural wrapper for sequence items without ancestor key
pub const ITEM: &str = "item";
