//! Color mode detection for terminal output

/// Determine if color should be used based on mode and environment
pub fn should_use_color(mode: &str) -> bool {
    match mode {
        "always" => true,
        "never" => false,
        _ => {
            // Auto mode: check if stdout is a tty and NO_COLOR is not set
            atty::is(atty::Stream::Stdout) && std::env::var("NO_COLOR").is_err()
        }
    }
}
