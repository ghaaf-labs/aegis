//! OpenRouter-backed AI gateway.
//!
//! All inference flows through `OpenRouterClient`. Per-task model selection
//! is via the `ModelRoute` enum, which resolves to a configurable slug in
//! `Config`. The client never references provider names directly.

pub mod client;
pub mod prompts;

#[allow(unused_imports)]
pub use client::{ChatResponse, Message, OpenRouterClient};
pub use prompts::{PromptKey, PromptRegistry};
