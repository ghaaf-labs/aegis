//! Agent tool-use module.
//!
//! Lets the strategist call `fetch_news`, `fetch_onchain_metric`, and
//! `fetch_correlation` mid-decision. Each tool gets a typed handler; the
//! dispatcher routes by name and returns a short string the model can
//! consume on the next turn.
//!
//! Tool calls are observable: every invocation emits an
//! `agent.tool.invoked` SSE event so the UI shows what the agent looked at
//! before committing to a recommendation.

use serde_json::{json, Value};

use crate::router::AppState;

pub mod correlation;
pub mod news;
pub mod onchain;

/// Maximum number of tool-call rounds before the loop must terminate. Keeps
/// runaway-loop costs bounded even when the model keeps asking for more
/// signals.
pub const MAX_TOOL_ITERATIONS: usize = 5;

/// OpenAI-compatible tool specs the strategist sees. Returns a `Vec<Value>`
/// so the caller can splice it straight into the chat-completions request.
pub fn tool_specs() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "fetch_news",
                "description": "Fetch the latest 3 headlines for a crypto asset. Use this to check for narrative-level catalysts that aren't visible in the price feed.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "symbol": { "type": "string", "description": "Ticker symbol, e.g. BTC, ETH, SOL." }
                    },
                    "required": ["symbol"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "fetch_onchain_metric",
                "description": "Fetch a single on-chain metric for a chain/asset pair. Useful for confirming whether market moves are supported by on-chain activity.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "chain":  { "type": "string", "description": "arc | base | ethereum | solana" },
                        "asset":  { "type": "string", "description": "Ticker symbol on that chain." },
                        "metric": { "type": "string", "description": "active_addresses_24h | tx_count_24h | fee_revenue_24h" }
                    },
                    "required": ["chain", "asset", "metric"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "fetch_correlation",
                "description": "Pearson correlation of 24h returns between two symbols over a window. Use to test diversification claims.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "symbol_a":   { "type": "string" },
                        "symbol_b":   { "type": "string" },
                        "window_days":{ "type": "integer", "description": "7, 30, or 90." }
                    },
                    "required": ["symbol_a", "symbol_b", "window_days"]
                }
            }
        }),
    ]
}

/// Route a tool call to its handler. Unknown tools return a structured error
/// the model can read on the next turn; we never panic on bad model output.
pub async fn dispatch(state: &AppState, call: &crate::modules::ai::ToolCall) -> String {
    let args: Value = serde_json::from_str(&call.arguments).unwrap_or(Value::Null);
    let result = match call.name.as_str() {
        "fetch_news" => news::run(state, &args).await,
        "fetch_onchain_metric" => onchain::run(state, &args).await,
        "fetch_correlation" => correlation::run(state, &args).await,
        other => Err(format!("unknown tool: {other}")),
    };
    match result {
        Ok(payload) => payload,
        Err(reason) => json!({ "error": reason }).to_string(),
    }
}

/// Build the `role=tool` message to append to the conversation trail after a
/// dispatch — kept here so callers don't reinvent the shape.
pub fn tool_message(call_id: &str, content: String) -> Value {
    json!({
        "role": "tool",
        "tool_call_id": call_id,
        "content": content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_specs_lists_three_functions() {
        let specs = tool_specs();
        assert_eq!(specs.len(), 3);
        let names: Vec<&str> = specs
            .iter()
            .map(|s| {
                s.pointer("/function/name")
                    .and_then(|v| v.as_str())
                    .unwrap()
            })
            .collect();
        assert!(names.contains(&"fetch_news"));
        assert!(names.contains(&"fetch_onchain_metric"));
        assert!(names.contains(&"fetch_correlation"));
    }

    #[test]
    fn tool_message_includes_role_and_id() {
        let m = tool_message("call_abc", "hello".to_string());
        assert_eq!(m["role"], "tool");
        assert_eq!(m["tool_call_id"], "call_abc");
        assert_eq!(m["content"], "hello");
    }
}
