//! XPath 3.1 query engine using xee-xpath
//!
//! This module provides XPath query capabilities for the parsed XML AST.

mod engine;
mod match_result;

pub use engine::XPathEngine;
pub use match_result::Match;

use thiserror::Error;

/// Errors that can occur during XPath evaluation
#[derive(Error, Debug)]
pub enum XPathError {
    #[error("Failed to compile XPath: {0}")]
    Compile(String),
    #[error("Failed to execute XPath: {0}")]
    Execute(String),
    #[error("Failed to parse XML: {0}")]
    XmlParse(String),
}
