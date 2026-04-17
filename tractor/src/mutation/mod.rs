//! Code mutation: replacement, XPath-based upsert, and declarative set operations.

pub mod replace;
#[cfg(feature = "native")]
pub mod xpath_upsert;
#[cfg(feature = "native")]
pub mod declarative_set;
