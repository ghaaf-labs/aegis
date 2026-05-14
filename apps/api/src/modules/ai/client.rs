use reqwest::Client;
use serde::{Deserialize, Serialize};
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
