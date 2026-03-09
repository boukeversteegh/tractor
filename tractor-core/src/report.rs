//! Report types for check mode

/// Severity level for check violations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// Summary of check results
#[derive(Debug)]
pub struct CheckSummary {
    pub total: usize,
    pub files_affected: usize,
    pub errors: usize,
    pub warnings: usize,
}
