pub mod input;
pub mod context;
pub mod matcher;
pub mod format;
pub mod git;

pub use input::InputMode;
pub use context::RunContext;
pub use format::ViewField;
pub use matcher::{
    query_files_batched, query_inline_source,
    run_debug,
    run_rules,
    project_report,
    apply_message_template,
};
pub use format::{render_check_report, render_test_report, render_set_report};
