//! OpenAI-compatible LLM provider implementation.
//!
//! Works with any provider that exposes the `/v1/chat/completions` endpoint
//! in OpenAI format (Pioneer AI, OpenAI, Groq, Together, etc.).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::ai::llm::{LlmClient, LlmError, LlmResponse, Message, Role, TokenUsage};
use crate::config::AppConfig;

/// Maximum number of retries on rate-limit (429) responses.
const MAX_RETRIES: u8 = 1;

/// Base delay in milliseconds for exponential backoff.
const BASE_DELAY_MS: u64 = 1000;

/// An LLM client that speaks the OpenAI chat completions protocol.
///
/// Configured via `AppConfig` to point at any compatible endpoint.
pub struct OpenAiCompatibleClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAiCompatibleClient {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(45))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            http,
            base_url: base_url.into(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }
}

/// Request body for the chat completions endpoint.
#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    stream: bool,
}

/// A message in the OpenAI chat format.
#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// Response body from the chat completions endpoint.
#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    model: String,
    usage: Option<UsageResponse>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Debug, Deserialize)]
struct ChatMessageResponse {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsageResponse {
    prompt_tokens: u32,
    completion_tokens: u32,
}

/// Error response body from the API.
#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: Option<ErrorDetail>,
}

#[derive(Debug, Deserialize)]
struct ErrorDetail {
    message: Option<String>,
}

fn role_to_string(role: &Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
    }
}

#[async_trait]
impl LlmClient for OpenAiCompatibleClient {
    async fn chat(
        &self,
        messages: Vec<Message>,
        schema: Option<&serde_json::Value>,
    ) -> Result<LlmResponse, LlmError> {
        // Build the messages array, prepending schema instruction if provided
        let mut chat_messages: Vec<ChatMessage> = Vec::with_capacity(messages.len() + 1);

        if let Some(schema_val) = schema {
            let schema_instruction = format!(
                "You MUST respond with ONLY valid JSON matching this schema. \
                 No markdown, no explanation, no extra text — only the JSON object.\n\n\
                 JSON Schema:\n{}",
                serde_json::to_string_pretty(schema_val).unwrap_or_default()
            );
            chat_messages.push(ChatMessage {
                role: "system".to_string(),
                content: schema_instruction,
            });
        }

        for msg in &messages {
            chat_messages.push(ChatMessage {
                role: role_to_string(&msg.role).to_string(),
                content: msg.content.clone(),
            });
        }

        let request_body = ChatRequest {
            model: self.model.clone(),
            messages: chat_messages,
            temperature: 0.1,
            stream: false,
        };

        let url = format!("{}/chat/completions", self.base_url);

        // Retry loop with exponential backoff on rate limits
        let mut last_error: Option<LlmError> = None;
        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let delay = BASE_DELAY_MS * (1u64 << (attempt - 1));
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
            }

            let response = self
                .http
                .post(&url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await
                .map_err(|e| LlmError::NetworkError(e.to_string()))?;

            let status = response.status();

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let retry_after = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(BASE_DELAY_MS * (1u64 << attempt));

                last_error = Some(LlmError::RateLimit {
                    retry_after_ms: retry_after,
                });
                continue;
            }

            if status == reqwest::StatusCode::UNAUTHORIZED
                || status == reqwest::StatusCode::FORBIDDEN
            {
                let body = response.text().await.unwrap_or_default();
                return Err(LlmError::AuthError(format!(
                    "Authentication failed ({}): {}",
                    status, body
                )));
            }

            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                let detail = serde_json::from_str::<ErrorResponse>(&body)
                    .ok()
                    .and_then(|e| e.error)
                    .and_then(|e| e.message)
                    .unwrap_or_else(|| body.clone());
                return Err(LlmError::InvalidResponse(format!(
                    "HTTP {}: {}",
                    status, detail
                )));
            }

            let chat_response: ChatResponse = response
                .json()
                .await
                .map_err(|e| LlmError::InvalidResponse(e.to_string()))?;

            let content = chat_response
                .choices
                .into_iter()
                .next()
                .and_then(|c| c.message.content)
                .unwrap_or_default();

            let usage = chat_response.usage.map(|u| TokenUsage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
            });

            return Ok(LlmResponse {
                content,
                model: chat_response.model,
                usage,
            });
        }

        // Exhausted retries
        Err(last_error.unwrap_or(LlmError::NetworkError(
            "Max retries exceeded".to_string(),
        )))
    }
}

/// Create an LLM client from application configuration.
///
/// Both Pioneer AI and OpenAI use the same HTTP protocol — only the
/// base URL, model, and API key differ.
pub fn create_llm_client(config: &AppConfig) -> Box<dyn LlmClient> {
    let api_key = config
        .llm_api_key
        .clone()
        .unwrap_or_default();

    Box::new(OpenAiCompatibleClient::new(
        &config.llm_base_url,
        api_key,
        &config.llm_model,
    ))
}
