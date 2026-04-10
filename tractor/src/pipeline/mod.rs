pub mod input;
pub mod context;
pub mod matcher;
pub mod format;
pub mod git;

pub use input::InputMode;
pub use context::RunContext;
pub use format::{ViewField, GroupDimension};
pub use matcher::{
    run_debug,
    run_rules,
    project_report,
    apply_message_template,
};
pub use format::{render_report, TestRenderOptions};
