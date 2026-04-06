use tractor_core::{report::Report, RenderOptions};
use super::gcc::render_gcc;
use super::options::HookType;

/// Render a report as Claude Code hook JSON.
///
/// The inner content is rendered in GCC format (compact, one-line-per-match).
/// The JSON envelope depends on the hook type:
///
/// - **PostToolUse / Stop**: `{ "decision": "block", "reason": "..." }`
///   Outputs nothing when the report succeeds (no violations).
///
/// - **PreToolUse**: `{ "hookSpecificOutput": { "hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": "..." } }`
///   Outputs nothing when the report succeeds.
///
/// - **Context**: `{ "hookSpecificOutput": { "hookEventName": "PostToolUse", "additionalContext": "..." } }`
///   Always outputs (non-blocking feedback).
pub fn render_claude_code(
    report: &Report,
    hook_type: HookType,
    opts: &RenderOptions,
    dimensions: &[&str],
) -> String {
    let success = report.success.unwrap_or(true);
    let inner = render_gcc(report, opts, dimensions);
    let inner = inner.trim();

    match hook_type {
        HookType::PostToolUse => {
            if success || inner.is_empty() {
                return String::new();
            }
            format!(
                "{{\"decision\":\"block\",\"reason\":{}}}",
                serde_json::to_string(inner).unwrap_or_else(|_| escape_json_string(inner)),
            )
        }
        HookType::PreToolUse => {
            if success || inner.is_empty() {
                return String::new();
            }
            format!(
                "{{\"hookSpecificOutput\":{{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\"permissionDecisionReason\":{}}}}}",
                serde_json::to_string(inner).unwrap_or_else(|_| escape_json_string(inner)),
            )
        }
        HookType::Context => {
            if inner.is_empty() {
                return String::new();
            }
            format!(
                "{{\"hookSpecificOutput\":{{\"hookEventName\":\"PostToolUse\",\"additionalContext\":{}}}}}",
                serde_json::to_string(inner).unwrap_or_else(|_| escape_json_string(inner)),
            )
        }
    }
}

/// Fallback JSON string escaping (used only if serde_json is unavailable).
fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"'  => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < '\x20' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
