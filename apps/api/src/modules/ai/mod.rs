//! OpenRouter-backed AI gateway.
//!
//! All inference flows through `OpenRouterClient`. Per-task model selection
//! is via the `ModelRoute` enum, which resolves to a configurable slug in
//! `Config`. The client never references provider names directly.

pub mod client;
pub mod prompts;

#[allow(unused_imports)]
pub use client::{ChatResponse, ChatToolResult, Message, OpenRouterClient, ToolCall};
pub use prompts::{PromptKey, PromptRegistry};

/// Pull a JSON body out of an LLM response that may wrap it in a markdown
/// fence. Handles three shapes observed in the wild:
///   1. `{"a":1}` — plain JSON, returned as-is.
///   2. ```` ```json\n{...}\n``` ```` — fence at the start (Claude, OpenAI).
///   3. `Here is my proposal: ```json\n{...}\n``` — that's it.` — prose
///      preamble *then* fence (DeepSeek, Qwen reasoning models).
///
/// Falls back to the trimmed raw input when no fence is found, so callers
/// don't need to special-case it.
pub fn strip_json_fences(raw: &str) -> &str {
    let t = raw.trim();
    if let Some(rest) = t.strip_prefix("```json").or_else(|| t.strip_prefix("```")) {
        return rest.trim_end_matches("```").trim();
    }
    if let Some(open) = t.find("```json").or_else(|| t.find("```")) {
        let after_open = &t[open..];
        let inner = after_open
            .strip_prefix("```json")
            .or_else(|| after_open.strip_prefix("```"))
            .unwrap_or(after_open);
        if let Some(close) = inner.find("```") {
            return inner[..close].trim();
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_handles_plain_json() {
        assert_eq!(strip_json_fences("{\"a\":1}"), "{\"a\":1}");
    }

    #[test]
    fn strip_handles_json_fence_at_start() {
        assert_eq!(strip_json_fences("```json\n{\"a\":1}\n```"), "{\"a\":1}");
    }

    #[test]
    fn strip_handles_bare_fence() {
        assert_eq!(strip_json_fences("```\n{\"a\":1}\n```"), "{\"a\":1}");
    }

    #[test]
    fn strip_extracts_fence_after_preamble() {
        let raw = "Here is the proposal:\n\n```json\n{\"a\":1}\n```\n\nThanks.";
        assert_eq!(strip_json_fences(raw), "{\"a\":1}");
    }
}
