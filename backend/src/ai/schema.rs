//! Structured output validation with retry logic.
//!
//! Generates JSON Schema from Rust types via `schemars` and validates LLM
//! responses against those schemas. On parse failure, retries with error
//! feedback up to a configurable maximum.

use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::ai::agents::AgentError;
use crate::ai::llm::{LlmClient, Message};

/// Default maximum number of retry attempts for structured output parsing.
const DEFAULT_MAX_RETRIES: u8 = 3;

/// Attempt to get a structured, schema-conforming response from the LLM.
///
/// 1. Generates a JSON Schema from `T` using schemars.
/// 2. Sends the messages to the LLM with the schema attached.
/// 3. Attempts to deserialize the response into `T`.
/// 4. On failure: appends error feedback and retries (up to `max_retries`).
/// 5. On persistent failure: returns `AgentError::OutputValidation`.
pub async fn validate_and_retry<T: DeserializeOwned + JsonSchema>(
    client: &dyn LlmClient,
    messages: Vec<Message>,
    max_retries: u8,
) -> Result<T, AgentError> {
    let schema = schemars::schema_for!(T);
    let schema_value = serde_json::to_value(&schema)
        .map_err(|e| AgentError::OutputValidation {
            raw_response: String::new(),
            parse_error: format!("Failed to serialize schema: {e}"),
        })?;

    let mut conversation = messages;
    let mut last_raw_response = String::new();
    let mut last_error = String::new();

    let attempts = max_retries.max(1);
    for attempt in 0..attempts {
        let response = client
            .chat(conversation.clone(), Some(&schema_value))
            .await
            .map_err(AgentError::LlmFailure)?;

        last_raw_response = response.content.clone();

        // Try to extract JSON from the response (handle markdown code blocks)
        let json_str = extract_json(&response.content);

        match serde_json::from_str::<T>(json_str) {
            Ok(parsed) => return Ok(parsed),
            Err(parse_err) => {
                last_error = parse_err.to_string();

                // Don't retry on the last attempt
                if attempt + 1 >= attempts {
                    break;
                }

                // Append the failed response and error feedback for retry
                conversation.push(Message::assistant(response.content));
                conversation.push(Message::user(format!(
                    "Your previous response was invalid JSON: {last_error}. \
                     Respond with ONLY valid JSON matching the schema. \
                     No markdown, no explanation."
                )));
            }
        }
    }

    Err(AgentError::OutputValidation {
        raw_response: last_raw_response,
        parse_error: last_error,
    })
}

/// Validate and retry with the default retry count.
pub async fn validate_and_retry_default<T: DeserializeOwned + JsonSchema>(
    client: &dyn LlmClient,
    messages: Vec<Message>,
) -> Result<T, AgentError> {
    validate_and_retry::<T>(client, messages, DEFAULT_MAX_RETRIES).await
}

/// Extract JSON from a response that might be wrapped in markdown code blocks.
fn extract_json(content: &str) -> &str {
    let trimmed = content.trim();

    // Handle ```json ... ``` blocks
    if let Some(start) = trimmed.strip_prefix("```json") {
        if let Some(end_idx) = start.rfind("```") {
            return start[..end_idx].trim();
        }
    }

    // Handle ``` ... ``` blocks (no language tag)
    if let Some(start) = trimmed.strip_prefix("```") {
        if let Some(end_idx) = start.rfind("```") {
            return start[..end_idx].trim();
        }
    }

    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_json_from_plain_text() {
        let input = r#"{"key": "value"}"#;
        assert_eq!(extract_json(input), input);
    }

    #[test]
    fn extracts_json_from_markdown_code_block() {
        let input = "```json\n{\"key\": \"value\"}\n```";
        assert_eq!(extract_json(input), "{\"key\": \"value\"}");
    }

    #[test]
    fn extracts_json_from_bare_code_block() {
        let input = "```\n{\"key\": \"value\"}\n```";
        assert_eq!(extract_json(input), "{\"key\": \"value\"}");
    }

    #[test]
    fn handles_whitespace_around_json() {
        let input = "  \n{\"key\": \"value\"}\n  ";
        assert_eq!(extract_json(input), "{\"key\": \"value\"}");
    }
}
