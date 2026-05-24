use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

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
    message: RawAssistantMessage,
}

#[derive(Deserialize)]
struct RawAssistantMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning: Option<String>,
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
        let models = self.config.models_for(route);
        let primary = models.first().copied().unwrap_or_default();

        let mut req = json!({
            "messages": messages,
            "temperature": 0.3,
            // 4000 covers DeepSeek-v4-pro's reasoning mode — it emits 200-1500
            // hidden CoT tokens before the visible answer, both of which count
            // toward this budget. Claude/OpenAI don't reason out loud so they
            // typically use < 1000 of the budget.
            "max_tokens": 4000,
            "response_format": { "type": "json_object" },
        });
        self.apply_routing(&mut req, &models, route, true);

        let (body, latency_ms) = self.post_with_retry(&req, primary).await?;

        let raw: RawChatResponse = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!(
                "OpenRouter 200 for {} returned non-chat body ({}): {}",
                primary,
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
        let model_slug = raw.model.unwrap_or_else(|| primary.to_string());

        check_budget_guard(
            self.config.openrouter_budget_guard_usd,
            usage.cost,
            &model_slug,
            latency_ms,
        );

        Ok(ChatResponse {
            content: assistant_text(
                choice.message.content.as_deref(),
                choice.message.reasoning.as_deref(),
            ),
            model_slug,
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            latency_ms,
            cost_usd: usage.cost,
        })
    }

    /// Attach the model fallback chain, JSON-repair plugin, and provider
    /// preferences to a chat-completions body. `wants_json` is true when the
    /// request asks for a structured response (so the repair plugin can act on
    /// it); it is false on tool-selection turns.
    fn apply_routing(&self, req: &mut Value, models: &[&str], route: ModelRoute, wants_json: bool) {
        if let Some(primary) = models.first() {
            req["model"] = json!(primary);
        }
        // A multi-entry chain triggers OpenRouter's automatic model fallback —
        // it tries each in order and returns the first that doesn't 5xx/429/
        // refuse, so a single bad provider can't stall or 500 the decision.
        if models.len() > 1 {
            req["models"] = json!(models);
        }
        if self.config.openrouter_response_healing && wants_json {
            req["plugins"] = json!([{ "id": "response-healing" }]);
        }
        // Cheap, latency-sensitive classification: prefer the fastest provider.
        if matches!(route, ModelRoute::RegimeClassify) {
            req["provider"] = json!({ "sort": "latency" });
        }
    }

    /// Send a chat-completions request with bounded, transient-only retries.
    ///
    /// Each attempt is capped by `openrouter_attempt_timeout_secs` (shorter than
    /// reqwest's overall ceiling) so a stalled provider is abandoned and retried
    /// rather than hanging. Retries fire only on a per-attempt timeout, a
    /// transport error (incl. reqwest's "error decoding response body" on a
    /// truncated stream), a 429, or a 5xx — never on a 4xx that retrying can't
    /// fix. Returns the success body and total wall-clock latency across attempts.
    async fn post_with_retry(
        &self,
        req: &Value,
        primary_slug: &str,
    ) -> anyhow::Result<(String, u64)> {
        let url = format!("{}/chat/completions", self.config.openrouter_base_url);
        let per_attempt = Duration::from_secs(self.config.openrouter_attempt_timeout_secs.max(1));
        let max_retries = self.config.openrouter_max_retries;
        let start = Instant::now();

        let mut attempt: u32 = 0;
        loop {
            let mut builder = self
                .http
                .post(&url)
                .bearer_auth(&self.config.openrouter_api_key)
                .header("X-Title", &self.config.openrouter_app_name)
                .json(req);
            if let Some(referer) = &self.config.openrouter_app_url {
                builder = builder.header("HTTP-Referer", referer);
            }

            let outcome = tokio::time::timeout(per_attempt, async move {
                let resp = builder.send().await?;
                let status = resp.status();
                let text = resp.text().await?;
                Ok::<(reqwest::StatusCode, String), reqwest::Error>((status, text))
            })
            .await;

            // Success returns; a non-retryable HTTP error is terminal; everything
            // else (retryable status, transport error, timeout) yields a reason
            // string handled by the single exhaustion check below.
            let reason = match outcome {
                Ok(Ok((status, text))) if status.is_success() => {
                    return Ok((text, start.elapsed().as_millis() as u64));
                }
                Ok(Ok((status, _text))) if is_retryable_status(status) => {
                    format!("HTTP {} from {primary_slug}", status.as_u16())
                }
                Ok(Ok((status, text))) => {
                    anyhow::bail!(
                        "OpenRouter {} for {}: {}",
                        status.as_u16(),
                        primary_slug,
                        openrouter_error_message(&text)
                            .unwrap_or_else(|| text.chars().take(500).collect::<String>())
                    );
                }
                Ok(Err(e)) => format!("transport error from {primary_slug}: {e}"),
                Err(_elapsed) => {
                    format!(
                        "attempt exceeded {}s for {primary_slug}",
                        per_attempt.as_secs()
                    )
                }
            };

            if attempt >= max_retries {
                anyhow::bail!(
                    "OpenRouter call failed for {primary_slug} after {} attempt(s): {reason}",
                    attempt + 1
                );
            }
            backoff(attempt, &reason).await;
            attempt += 1;
        }
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
        let models = self.config.models_for(route);
        let primary = models.first().copied().unwrap_or_default();

        let mut req = json!({
            "messages": messages,
            "temperature": 0.3,
            // See note on `chat()` — reasoning models eat budget for CoT.
            "max_tokens": 4000,
        });
        let wants_json = force_final || tools.is_empty();
        if !force_final && !tools.is_empty() {
            req["tools"] = Value::Array(tools.to_vec());
            req["tool_choice"] = Value::String("auto".into());
        } else {
            // Last iteration: force JSON object output for parseability and
            // strip tools so the model emits a proposal.
            req["response_format"] = json!({ "type": "json_object" });
        }
        self.apply_routing(&mut req, &models, route, wants_json);

        let (body, latency_ms) = self.post_with_retry(&req, primary).await?;

        let raw: Value = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!(
                "OpenRouter 200 for {} returned non-json body ({}): {}",
                primary,
                e,
                body.chars().take(500).collect::<String>()
            )
        })?;

        let model_slug = raw
            .get("model")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| primary.to_string());
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

        let content = assistant_message_text(&message);
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

fn assistant_message_text(message: &Value) -> String {
    assistant_text(
        message.get("content").and_then(|v| v.as_str()),
        message.get("reasoning").and_then(|v| v.as_str()),
    )
}

fn assistant_text(content: Option<&str>, reasoning: Option<&str>) -> String {
    content
        .filter(|s| !s.trim().is_empty())
        .or_else(|| reasoning.filter(|s| !s.trim().is_empty()))
        .unwrap_or_default()
        .to_string()
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

/// Transient HTTP statuses worth retrying: rate-limit (429) and any 5xx.
/// A 4xx (bad request / auth / unsupported parameter) is the caller's bug and
/// retrying won't fix it, so it is treated as terminal.
fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

/// Exponential backoff (400ms · 2^attempt, capped) with a structured warn so a
/// flaky provider is visible in logs. Mirrors the regime-backtest retry cadence.
async fn backoff(attempt: u32, reason: &str) {
    let delay = Duration::from_millis(400 * (1u64 << attempt.min(4)));
    tracing::warn!(
        target: "agent.openrouter.retry",
        attempt,
        delay_ms = delay.as_millis() as u64,
        "retrying OpenRouter call: {reason}"
    );
    tokio::time::sleep(delay).await;
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
    fn assistant_text_prefers_visible_content() {
        assert_eq!(
            assistant_text(Some("visible"), Some("reasoning fallback")),
            "visible"
        );
    }

    #[test]
    fn assistant_text_falls_back_to_reasoning_when_content_is_null_or_empty() {
        assert_eq!(
            assistant_text(None, Some("reasoned answer")),
            "reasoned answer"
        );
        assert_eq!(
            assistant_text(Some("   "), Some("reasoned answer")),
            "reasoned answer"
        );
    }

    #[test]
    fn assistant_message_text_reads_openrouter_reasoning_fallback() {
        let message = json!({
            "role": "assistant",
            "content": null,
            "reasoning": "{\"reasoning\":\"hold\",\"confidence\":0.7}"
        });
        assert_eq!(
            assistant_message_text(&message),
            "{\"reasoning\":\"hold\",\"confidence\":0.7}"
        );
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

    #[test]
    fn retryable_status_covers_429_and_5xx_only() {
        use reqwest::StatusCode;
        assert!(is_retryable_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable_status(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(is_retryable_status(StatusCode::BAD_GATEWAY));
        assert!(is_retryable_status(StatusCode::SERVICE_UNAVAILABLE));
        // Client errors are the caller's bug — never retried.
        assert!(!is_retryable_status(StatusCode::BAD_REQUEST));
        assert!(!is_retryable_status(StatusCode::UNAUTHORIZED));
        assert!(!is_retryable_status(StatusCode::UNPROCESSABLE_ENTITY));
        assert!(!is_retryable_status(StatusCode::OK));
    }
}
