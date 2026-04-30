//! Output element names — JSON's vocabulary after transform.
//!
//! Two output branches:
//!   - `/syntax` — closed vocabulary (object, array, property, …) shared
//!     with YAML's syntax branch. Defined here.
//!   - `/data` — open vocabulary derived from object keys. Names are
//!     user-data driven, so no constants beyond the structural
//!     wrappers `item` and `field`.
//!
//! No `NodeSpec` table: data-branch names are not a closed set, so
//! `node_spec` stays `None` in the language registry. The constants
//! below let `rules.rs` and `transformations.rs` reference output
//! names symbolically rather than via stringly-typed literals.

// Syntax branch — closed vocabulary
pub const OBJECT: &str = "object";
pub const ARRAY: &str = "array";
pub const PROPERTY: &str = "property";
pub const KEY: &str = "key";
pub const STRING: &str = "string";
pub const NUMBER: &str = "number";
pub const BOOL: &str = "bool";
pub const NULL: &str = "null";

// Data branch — structural wrappers (the rest is user-data driven)
pub const ITEM: &str = "item";
