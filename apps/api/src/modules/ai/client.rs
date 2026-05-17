use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;

use crate::config::{Config, ModelRoute};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    temperature: f32,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    fmt_type: &'static str,
}

#[derive(Deserialize)]
struct RawChatResponse {
    choices: Vec<RawChoice>,
    #[serde(default)]
    usage: Option<RawUsage>,
    /// OpenRouter returns the actually-routed model slug here. We treat it
    /// as the source of truth so per-decision telemetry reflects what was
    /// run, not what we asked for.
    #[serde(default)]
    model: Option<String>,
}

#[derive(Deserialize)]
struct RawChoice {
    message: Message,
}

#[derive(Deserialize, Default)]
struct RawUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    /// OpenRouter returns the per-call cost in USD when their accounting
    /// settled the route. Field name is `cost`. Absent for some providers /
    /// free routes — we treat absence as zero.
    #[serde(default)]
    cost: Option<f64>,
}

/// A single chat completion result, carrying telemetry alongside the content.
///
/// `latency_ms` is the per-call wall-clock; callers may aggregate it
/// across pipeline steps for the persisted `agent_decisions.latency_ms`.
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content: String,
    pub model_slug: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub latency_ms: u64,
    /// USD cost OpenRouter charged for this call. `None` when the field is
    /// absent (some providers + free routes). Treat absence as zero for
    /// guard comparisons.
    pub cost_usd: Option<f64>,
}

impl ChatResponse {
    /// True if the call exceeded a "slow path" wall-clock budget.
    /// Useful for logging without enforcing a hard timeout.
    pub fn was_slow(&self, budget_ms: u64) -> bool {
        self.latency_ms > budget_ms
    }
}

pub struct OpenRouterClient<'a> {
    http: &'a Client,
    config: &'a Config,
}

impl<'a> OpenRouterClient<'a> {
    pub fn new(http: &'a Client, config: &'a Config) -> Self {
        Self { http, config }
    }

    /// Send a chat completion. `route` resolves to the configured slug;
    /// returns the response wrapped in telemetry.
    pub async fn chat(
        &self,
        route: ModelRoute,
        messages: Vec<Message>,
    ) -> anyhow::Result<ChatResponse> {
        let requested_slug = self.config.model_for(route);
        let url = format!("{}/chat/completions", self.config.openrouter_base_url);

        let req = ChatRequest {
            model: requested_slug,
            messages: &messages,
            temperature: 0.3,
            // 4000 covers DeepSeek-v4-pro's reasoning mode — it emits 200-1500
            // hidden CoT tokens before the visible answer, both of which count
            // toward this budget. At ~$0.87/M output tokens, 4000 caps a single
            // call at < $0.004. Claude/OpenAI don't reason out loud so they
            // typically use < 1000 of the budget.
            max_tokens: 4000,
            response_format: Some(ResponseFormat {
                fmt_type: "json_object",
            }),
        };

        let start = Instant::now();
        let mut builder = self
            .http
            .post(&url)
            .bearer_auth(&self.config.openrouter_api_key)
            .header("X-Title", &self.config.openrouter_app_name)
            .json(&req);

        if let Some(referer) = &self.config.openrouter_app_url {
            builder = builder.header("HTTP-Referer", referer);
        }

        let resp = builder.send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        let latency_ms = start.elapsed().as_millis() as u64;

        if !status.is_success() {
            anyhow::bail!(
                "OpenRouter {} for {}: {}",
                status.as_u16(),
                requested_slug,
                openrouter_error_message(&body)
                    .unwrap_or_else(|| body.chars().take(500).collect::<String>())
            );
        }

        let raw: RawChatResponse = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!(
                "OpenRouter 200 for {} returned non-chat body ({}): {}",
                requested_slug,
                openrouter_error_message(&body).unwrap_or_else(|| e.to_string()),
                body.chars().take(500).collect::<String>()
            )
        })?;

        let choice = raw
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("empty response from OpenRouter"))?;

        let usage = raw.usage.unwrap_or_default();
        let model_slug = raw.model.unwrap_or_else(|| requested_slug.to_string());

        check_budget_guard(
            self.config.openrouter_budget_guard_usd,
            usage.cost,
            &model_slug,
            latency_ms,
        );

        Ok(ChatResponse {
            content: choice.message.content,
            model_slug,
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            latency_ms,
            cost_usd: usage.cost,
        })
    }

    /// Tool-aware variant for the agentic loop. Caller passes the message
    /// trail as `serde_json::Value` dicts (richer than `Message` because
    /// assistant turns can carry `tool_calls` and tool turns have a
    /// `tool_call_id`). Returns either the final assistant content or the
    /// list of tool calls the model asked the agent to execute.
    ///
    /// `force_final = true` disables tool selection on the last iteration so
    /// the loop is guaranteed to terminate with a parseable proposal.
    pub async fn chat_with_tools(
        &self,
        route: ModelRoute,
        messages: &[Value],
        tools: &[Value],
        force_final: bool,
    ) -> anyhow::Result<ChatToolResult> {
        let requested_slug = self.config.model_for(route);
        let url = format!("{}/chat/completions", self.config.openrouter_base_url);

        let mut body = json!({
            "model": requested_slug,
            "messages": messages,
            "temperature": 0.3,
            // See note on `chat()` — reasoning models eat budget for CoT.
            "max_tokens": 4000,
        });
        if !force_final && !tools.is_empty() {
            body["tools"] = Value::Array(tools.to_vec());
            body["tool_choice"] = Value::String("auto".into());
        } else {
            // Last iteration: force JSON object output for parseability and
            // strip tools so the model emits a proposal.
            body["response_format"] = json!({ "type": "json_object" });
        }

        let start = Instant::now();
        let mut builder = self
            .http
            .post(&url)
            .bearer_auth(&self.config.openrouter_api_key)
            .header("X-Title", &self.config.openrouter_app_name)
            .json(&body);
        if let Some(referer) = &self.config.openrouter_app_url {
            builder = builder.header("HTTP-Referer", referer);
        }

        let resp = builder.send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        let latency_ms = start.elapsed().as_millis() as u64;

        if !status.is_success() {
            anyhow::bail!(
                "OpenRouter {} for {}: {}",
                status.as_u16(),
                requested_slug,
                openrouter_error_message(&body)
                    .unwrap_or_else(|| body.chars().take(500).collect::<String>())
            );
        }

        let raw: Value = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!(
                "OpenRouter 200 for {} returned non-json body ({}): {}",
                requested_slug,
                e,
                body.chars().take(500).collect::<String>()
            )
        })?;

        let model_slug = raw
            .get("model")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| requested_slug.to_string());
        let prompt_tokens = raw
            .pointer("/usage/prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let completion_tokens = raw
            .pointer("/usage/completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let cost_usd = raw.pointer("/usage/cost").and_then(|v| v.as_f64());

        check_budget_guard(
            self.config.openrouter_budget_guard_usd,
            cost_usd,
            &model_slug,
            latency_ms,
        );

        let message = raw
            .pointer("/choices/0/message")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("OpenRouter response missing choices[0].message"))?;
        let tool_calls = message
            .get("tool_calls")
            .and_then(|v| v.as_array())
            .cloned();

        if let Some(calls) = tool_calls.filter(|a| !a.is_empty()) {
            let parsed: Vec<ToolCall> = calls
                .iter()
                .filter_map(|c| {
                    let id = c.get("id")?.as_str()?.to_string();
                    let func = c.get("function")?;
                    let name = func.get("name")?.as_str()?.to_string();
                    let arguments = func.get("arguments")?.as_str()?.to_string();
                    Some(ToolCall {
                        id,
                        name,
                        arguments,
                    })
                })
                .collect();
            return Ok(ChatToolResult::Calls {
                calls: parsed,
                assistant_message: message,
                model_slug,
                prompt_tokens,
                completion_tokens,
                latency_ms,
                cost_usd,
            });
        }

        let content = message
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        Ok(ChatToolResult::Final {
            content,
            model_slug,
            prompt_tokens,
            completion_tokens,
            latency_ms,
            cost_usd,
        })
    }
}

/// F-COST-2 enforcement: log a structured warning when an OpenRouter call's
/// settled USD cost exceeds the configured guard. We intentionally only
/// log + emit a tracing event — no auto-downshift mid-decision; the warn
/// path is the cheap escape valve documented in docs/05-open-questions.md.
/// Operator-side alerting can subscribe to the `agent.cost.guard_exceeded`
/// tracing event without touching code.
fn check_budget_guard(guard_usd: f64, cost_usd: Option<f64>, model_slug: &str, latency_ms: u64) {
    let Some(cost) = cost_usd else { return };
    if cost <= guard_usd {
        return;
    }
    tracing::warn!(
        target: "agent.cost.guard_exceeded",
        model_slug = %model_slug,
        cost_usd = cost,
        guard_usd = guard_usd,
        latency_ms = latency_ms,
        "openrouter call exceeded budget guard"
    );
}

/// One tool invocation the model wants to run.
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// Arguments as the literal JSON string the model emitted. The dispatcher
    /// parses it; we don't pre-validate so the dispatcher's error message
    /// reaches the model on the next turn.
    pub arguments: String,
}

/// Either the model finished and emitted final content, or it asked the
/// agent to run one or more tools.
#[derive(Debug, Clone)]
pub enum ChatToolResult {
    Final {
        content: String,
        model_slug: String,
        prompt_tokens: u32,
        completion_tokens: u32,
        latency_ms: u64,
        cost_usd: Option<f64>,
    },
    Calls {
        calls: Vec<ToolCall>,
        /// The raw assistant turn the model emitted — including `tool_calls`.
        /// Push this back into `messages` before appending each tool result
        /// so the next call has the full conversation trail.
        assistant_message: Value,
        model_slug: String,
        prompt_tokens: u32,
        completion_tokens: u32,
        latency_ms: u64,
        cost_usd: Option<f64>,
    },
}

/// Best-effort extractor for OpenRouter's `{"error":{"message":"..."}}` envelope.
/// Returns `None` if the body isn't JSON or doesn't carry a string message.
fn openrouter_error_message(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    let err = v.get("error")?;
    if let Some(msg) = err.get("message").and_then(|m| m.as_str()) {
        if let Some(code) = err.get("code").and_then(|c| c.as_str()) {
            return Some(format!("{code}: {msg}"));
        }
        return Some(msg.to_string());
    }
    err.as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_helpers_set_role() {
        assert_eq!(Message::system("a").role, "system");
        assert_eq!(Message::user("b").role, "user");
    }

    #[test]
    fn chat_response_is_clone_and_debug() {
        let r = ChatResponse {
            content: "x".into(),
            model_slug: "y".into(),
            prompt_tokens: 1,
            completion_tokens: 2,
            latency_ms: 3,
            cost_usd: Some(0.001),
        };
        let _ = r.clone();
        assert!(format!("{r:?}").contains("model_slug"));
    }

    #[test]
    fn check_budget_guard_warns_when_over() {
        // Compiles + does not panic — the warn is a tracing event; capturing
        // it inline would require a subscriber. The smoke is that the path
        // is reachable and never errors.
        check_budget_guard(0.01, Some(0.05), "deepseek/test", 100);
        check_budget_guard(0.01, Some(0.001), "deepseek/test", 100);
        check_budget_guard(0.01, None, "deepseek/test", 100);
    }

    #[test]
    fn openrouter_error_message_extracts_envelope() {
        let body = r#"{"error":{"message":"No endpoints found","code":"model_not_found"}}"#;
        assert_eq!(
            openrouter_error_message(body).as_deref(),
            Some("model_not_found: No endpoints found")
        );
    }

    #[test]
    fn openrouter_error_message_handles_message_only() {
        let body = r#"{"error":{"message":"rate limited"}}"#;
        assert_eq!(
            openrouter_error_message(body).as_deref(),
            Some("rate limited")
        );
    }

    #[test]
    fn openrouter_error_message_returns_none_for_non_error_body() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"hi"}}]}"#;
        assert!(openrouter_error_message(body).is_none());
    }
}
