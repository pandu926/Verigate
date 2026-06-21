//! LLM client abstraction layer.
//!
//! Defines the trait and types for interacting with any OpenAI-compatible LLM API.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Role of a message in a chat conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// A single message in a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

/// Token usage information from an LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

/// Response from an LLM chat completion request.
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub model: String,
    pub usage: Option<TokenUsage>,
}

/// Errors that can occur when communicating with an LLM provider.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("Rate limited, retry after {retry_after_ms}ms")]
    RateLimit { retry_after_ms: u64 },

    #[error("Invalid response from LLM: {0}")]
    InvalidResponse(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Authentication error: {0}")]
    AuthError(String),
}

/// Trait for LLM clients.
///
/// Implementations handle the HTTP transport to a specific provider while
/// presenting a uniform interface to the agent framework.
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Send a chat completion request.
    ///
    /// When `schema` is `Some`, the provider should instruct the model to
    /// respond with JSON conforming to the given JSON Schema value.
    async fn chat(
        &self,
        messages: Vec<Message>,
        schema: Option<&serde_json::Value>,
    ) -> Result<LlmResponse, LlmError>;
}
