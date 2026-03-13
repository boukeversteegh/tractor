pub mod input;
pub mod context;
pub mod matcher;
pub mod format;

pub use input::InputMode;
pub use context::RunContext;
pub use format::{OutputFormat, ViewField, view};
pub use matcher::{
    query_files_batched, query_inline_source,
    explore_files, explore_inline,
    print_schema_from_matches,
    run_debug,
};
pub use format::{render_check_report, render_test_report};
