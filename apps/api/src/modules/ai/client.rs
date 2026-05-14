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
            max_tokens: 1500,
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

        let resp = builder.send().await?.error_for_status()?;
        let raw: RawChatResponse = resp.json().await?;
        let latency_ms = start.elapsed().as_millis() as u64;

        let choice = raw
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("empty response from OpenRouter"))?;

        let usage = raw.usage.unwrap_or_default();
        let model_slug = raw.model.unwrap_or_else(|| requested_slug.to_string());

        Ok(ChatResponse {
            content: choice.message.content,
            model_slug,
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            latency_ms,
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
            "max_tokens": 1500,
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

        let resp = builder.send().await?.error_for_status()?;
        let raw: Value = resp.json().await?;
        let latency_ms = start.elapsed().as_millis() as u64;

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
        })
    }
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
    },
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
        };
        let _ = r.clone();
        assert!(format!("{r:?}").contains("model_slug"));
    }
}
