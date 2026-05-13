use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::config::Config;

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

pub struct OpenAiClient<'a> {
    client: &'a Client,
    config: &'a Config,
}

impl<'a> OpenAiClient<'a> {
    pub fn new(client: &'a Client, config: &'a Config) -> Self {
        Self { client, config }
    }

    pub async fn chat(&self, messages: Vec<Message>) -> anyhow::Result<String> {
        let req = ChatRequest {
            model: self.config.openai_model.clone(),
            messages,
            temperature: 0.3,
            max_tokens: 1024,
        };

        let resp: ChatResponse = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.config.openai_api_key)
            .json(&req)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        resp.choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| anyhow::anyhow!("empty response from OpenAI"))
    }
}
