use super::gcc::render_gcc;
use super::options::HookType;
use serde::Serialize;
use tractor_core::{report::Report, RenderOptions};

// ---------------------------------------------------------------------------
// Claude Code hook output structures
// ---------------------------------------------------------------------------

/// PostToolUse / Stop blocking response.
/// Example: `{ "decision": "block", "reason": "..." }`
#[derive(Serialize)]
struct BlockResponse<'a> {
    decision: &'static str,
    reason: &'a str,
}

/// PreToolUse deny response.
/// Example: `{ "hookSpecificOutput": { "hookEventName": "PreToolUse", "permissionDecision": "deny", ... } }`
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreToolUseResponse<'a> {
    hook_specific_output: PreToolUseOutput<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreToolUseOutput<'a> {
    hook_event_name: &'static str,
    permission_decision: &'static str,
    permission_decision_reason: &'a str,
}

/// PostToolUse context (non-blocking) response.
/// Example: `{ "hookSpecificOutput": { "hookEventName": "PostToolUse", "additionalContext": "..." } }`
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContextResponse<'a> {
    hook_specific_output: ContextOutput<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContextOutput<'a> {
    hook_event_name: &'static str,
    additional_context: &'a str,
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

/// Render a report as Claude Code hook JSON.
///
/// The inner content is rendered in GCC format (compact, one-line-per-match).
/// The JSON envelope depends on the hook type:
///
/// - **PostToolUse / Stop**: `{ "decision": "block", "reason": "..." }`
///   Outputs nothing when the report succeeds (no violations).
///
/// - **PreToolUse**: `{ "hookSpecificOutput": { "hookEventName": "PreToolUse", "permissionDecision": "deny", ... } }`
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
            serde_json::to_string(&BlockResponse {
                decision: "block",
                reason: inner,
            })
            .expect("BlockResponse serialization cannot fail")
        }
        HookType::PreToolUse => {
            if success || inner.is_empty() {
                return String::new();
            }
            serde_json::to_string(&PreToolUseResponse {
                hook_specific_output: PreToolUseOutput {
                    hook_event_name: "PreToolUse",
                    permission_decision: "deny",
                    permission_decision_reason: inner,
                },
            })
            .expect("PreToolUseResponse serialization cannot fail")
        }
        HookType::Context => {
            if inner.is_empty() {
                return String::new();
            }
            serde_json::to_string(&ContextResponse {
                hook_specific_output: ContextOutput {
                    hook_event_name: "PostToolUse",
                    additional_context: inner,
                },
            })
            .expect("ContextResponse serialization cannot fail")
        }
    }
}
