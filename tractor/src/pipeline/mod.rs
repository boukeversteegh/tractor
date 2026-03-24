pub mod input;
pub mod context;
pub mod matcher;
pub mod format;

pub use input::InputMode;
pub use context::RunContext;
pub use format::ViewField;
pub use matcher::{
    query_files_batched, query_inline_source,
    print_schema_from_matches,
    run_debug,
    match_to_report_match,
    run_rules,
};
pub use format::{render_check_report, render_test_report, render_set_report};
