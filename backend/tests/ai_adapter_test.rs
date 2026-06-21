//! Unit tests for the LLM adapter layer and structured output validation.
//!
//! Uses a MockLlmClient to test retry logic, error propagation, and schema
//! validation without making real API calls.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use verigate_backend::ai::{
    validate_and_retry, AgentError, LlmClient, LlmError, LlmResponse, Message,
};

// ---------------------------------------------------------------------------
// Mock LLM Client
// ---------------------------------------------------------------------------

/// A mock LLM client that returns canned responses from a queue.
struct MockLlmClient {
    responses: Arc<Mutex<VecDeque<Result<LlmResponse, LlmError>>>>,
}

impl MockLlmClient {
    fn new(responses: Vec<Result<LlmResponse, LlmError>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::from(responses))),
        }
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn chat(
        &self,
        _messages: Vec<Message>,
        _schema: Option<&serde_json::Value>,
    ) -> Result<LlmResponse, LlmError> {
        let mut queue = self.responses.lock().unwrap();
        queue.pop_front().unwrap_or(Err(LlmError::NetworkError(
            "No more canned responses".to_string(),
        )))
    }
}

/// A mock that also tracks how many times it was called.
struct CountingMockLlmClient {
    responses: Arc<Mutex<VecDeque<Result<LlmResponse, LlmError>>>>,
    call_count: Arc<Mutex<usize>>,
}

impl CountingMockLlmClient {
    fn new(responses: Vec<Result<LlmResponse, LlmError>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::from(responses))),
            call_count: Arc::new(Mutex::new(0)),
        }
    }

    fn calls(&self) -> usize {
        *self.call_count.lock().unwrap()
    }
}

#[async_trait]
impl LlmClient for CountingMockLlmClient {
    async fn chat(
        &self,
        _messages: Vec<Message>,
        _schema: Option<&serde_json::Value>,
    ) -> Result<LlmResponse, LlmError> {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        drop(count);

        let mut queue = self.responses.lock().unwrap();
        queue.pop_front().unwrap_or(Err(LlmError::NetworkError(
            "No more canned responses".to_string(),
        )))
    }
}

// ---------------------------------------------------------------------------
// Test output struct for schema validation tests
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
struct TestOutput {
    value: String,
    count: u32,
}

// ---------------------------------------------------------------------------
// Helper to create a successful LlmResponse
// ---------------------------------------------------------------------------

fn ok_response(content: &str) -> Result<LlmResponse, LlmError> {
    Ok(LlmResponse {
        content: content.to_string(),
        model: "test-model".to_string(),
        usage: None,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn successful_chat_returns_content() {
    let client = MockLlmClient::new(vec![ok_response("Hello, world!")]);

    let messages = vec![Message::user("Hi")];
    let result = client.chat(messages, None).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.content, "Hello, world!");
    assert_eq!(response.model, "test-model");
}

#[tokio::test]
async fn rate_limit_error_propagates() {
    let client = MockLlmClient::new(vec![Err(LlmError::RateLimit {
        retry_after_ms: 5000,
    })]);

    let messages = vec![Message::user("Hi")];
    let result = client.chat(messages, None).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        LlmError::RateLimit { retry_after_ms } => {
            assert_eq!(retry_after_ms, 5000);
        }
        other => panic!("Expected RateLimit error, got: {:?}", other),
    }
}

#[tokio::test]
async fn validate_and_retry_succeeds_on_first_try() {
    let valid_json = r#"{"value": "hello", "count": 42}"#;
    let client = MockLlmClient::new(vec![ok_response(valid_json)]);

    let messages = vec![Message::user("Give me JSON")];
    let result: Result<TestOutput, AgentError> =
        validate_and_retry(&client, messages, 3).await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert_eq!(output.value, "hello");
    assert_eq!(output.count, 42);
}

#[tokio::test]
async fn validate_and_retry_retries_on_invalid_json() {
    let invalid_json = "not json {";
    let valid_json = r#"{"value": "retried", "count": 7}"#;

    let client = CountingMockLlmClient::new(vec![
        ok_response(invalid_json),
        ok_response(valid_json),
    ]);

    let messages = vec![Message::user("Give me JSON")];
    let result: Result<TestOutput, AgentError> =
        validate_and_retry(&client, messages, 3).await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert_eq!(output.value, "retried");
    assert_eq!(output.count, 7);
    assert_eq!(client.calls(), 2);
}

#[tokio::test]
async fn validate_and_retry_exhausts_retries() {
    let invalid1 = "not json at all";
    let invalid2 = "{broken";
    let invalid3 = "still not valid}}}";

    let client = CountingMockLlmClient::new(vec![
        ok_response(invalid1),
        ok_response(invalid2),
        ok_response(invalid3),
    ]);

    let messages = vec![Message::user("Give me JSON")];
    let result: Result<TestOutput, AgentError> =
        validate_and_retry(&client, messages, 3).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AgentError::OutputValidation {
            raw_response,
            parse_error,
        } => {
            // Last raw response should be the final invalid attempt
            assert!(!raw_response.is_empty());
            assert!(!parse_error.is_empty());
        }
        other => panic!("Expected OutputValidation, got: {:?}", other),
    }
    assert_eq!(client.calls(), 3);
}

#[tokio::test]
async fn schema_generation_produces_valid_json_schema() {
    let schema = schemars::schema_for!(TestOutput);
    let schema_json = serde_json::to_value(&schema).unwrap();

    // The root schema should have "type" and "properties"
    let schema_obj = schema_json.as_object().unwrap();
    assert!(
        schema_obj.contains_key("type") || schema_obj.contains_key("properties"),
        "Schema should contain type or properties at root level"
    );

    // Check that our fields are present in properties
    if let Some(properties) = schema_obj.get("properties") {
        let props = properties.as_object().unwrap();
        assert!(props.contains_key("value"), "Should have 'value' property");
        assert!(props.contains_key("count"), "Should have 'count' property");
    }
}

#[tokio::test]
async fn validate_and_retry_handles_markdown_wrapped_json() {
    let markdown_json = "```json\n{\"value\": \"wrapped\", \"count\": 99}\n```";
    let client = MockLlmClient::new(vec![ok_response(markdown_json)]);

    let messages = vec![Message::user("Give me JSON")];
    let result: Result<TestOutput, AgentError> =
        validate_and_retry(&client, messages, 3).await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert_eq!(output.value, "wrapped");
    assert_eq!(output.count, 99);
}

#[tokio::test]
async fn validate_and_retry_propagates_llm_error() {
    let client = MockLlmClient::new(vec![Err(LlmError::AuthError(
        "Invalid API key".to_string(),
    ))]);

    let messages = vec![Message::user("Give me JSON")];
    let result: Result<TestOutput, AgentError> =
        validate_and_retry(&client, messages, 3).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AgentError::LlmFailure(LlmError::AuthError(msg)) => {
            assert_eq!(msg, "Invalid API key");
        }
        other => panic!("Expected LlmFailure(AuthError), got: {:?}", other),
    }
}
