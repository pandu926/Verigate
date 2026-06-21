//! Terminal 3 TEE-protected action executor.
//!
//! Implements the http-with-placeholders pattern: action templates contain
//! `{{profile.*}}` markers that are resolved HOST-SIDE inside the T3 TEE.
//! Our backend never sees the resolved PII values — we only know which
//! placeholders were present, proving privacy by design.

use std::collections::HashMap;
use std::path::Path;

use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

// ---------------------------------------------------------------------------
// Action Template
// ---------------------------------------------------------------------------

/// An HTTP action template with placeholder markers for TEE-resolved PII.
///
/// The `body_template` contains `{{profile.<field>}}` markers that the T3 TEE
/// resolves host-side. The WASM contract and our backend never see the resolved
/// values — this is Terminal 3's flagship privacy pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub body_template: serde_json::Value,
}

impl ActionTemplate {
    /// Extract all `{{...}}` placeholder markers from the body_template JSON.
    /// Returns a sorted, deduplicated list of placeholder names.
    pub fn extract_placeholders(&self) -> Vec<String> {
        let re = Regex::new(r"\{\{([^}]+)\}\}").expect("valid regex");
        let json_str = serde_json::to_string(&self.body_template).unwrap_or_default();

        let mut placeholders: Vec<String> = re
            .captures_iter(&json_str)
            .map(|cap| cap[1].trim().to_string())
            .collect();

        placeholders.sort();
        placeholders.dedup();
        placeholders
    }
}

/// Load an action template from the templates directory by action ID.
///
/// Reads `{templates_dir}/{action_id}.json` and deserializes it.
pub fn load_template(templates_dir: &Path, action_id: &str) -> Result<ActionTemplate, AppError> {
    let path = templates_dir.join(format!("{action_id}.json"));

    let content = std::fs::read_to_string(&path).map_err(|e| {
        AppError::Config(format!(
            "Failed to read action template '{}': {}",
            path.display(),
            e
        ))
    })?;

    let template: ActionTemplate = serde_json::from_str(&content).map_err(|e| {
        AppError::Config(format!(
            "Failed to parse action template '{}': {}",
            path.display(),
            e
        ))
    })?;

    Ok(template)
}

// ---------------------------------------------------------------------------
// Action Result
// ---------------------------------------------------------------------------

/// The outcome of a protected action execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub success: bool,
    pub action_id: String,
    pub template_name: String,
    /// Placeholder markers found in the template (e.g., `["profile.legal_name"]`).
    /// These prove which PII fields WOULD be resolved inside the TEE,
    /// without ever exposing the actual values.
    pub placeholders_present: Vec<String>,
    pub execution_id: String,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Executor Trait
// ---------------------------------------------------------------------------

/// Trait for executing TEE-protected actions via Terminal 3.
///
/// Implementations handle the actual dispatch to the T3 TEE (or mock it
/// for local development). The key invariant: resolved PII values never
/// flow back to the caller.
#[async_trait]
pub trait ProtectedActionExecutor: Send + Sync {
    /// Execute a protected action using the given template.
    async fn execute(
        &self,
        template: &ActionTemplate,
        case_id: Uuid,
        actor_did: &str,
    ) -> Result<ActionResult, AppError>;

    /// Human-readable name of this executor implementation.
    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Dev Mock Executor
// ---------------------------------------------------------------------------

/// Development executor that simulates T3 protected action execution.
///
/// Logs the template and placeholders, returns success without resolving
/// any PII. Used when T3 credentials are not configured.
pub struct DevProtectedActionExecutor;

#[async_trait]
impl ProtectedActionExecutor for DevProtectedActionExecutor {
    async fn execute(
        &self,
        template: &ActionTemplate,
        case_id: Uuid,
        actor_did: &str,
    ) -> Result<ActionResult, AppError> {
        let placeholders = template.extract_placeholders();
        let execution_id = Uuid::now_v7().to_string();

        tracing::info!(
            executor = "dev-mock",
            action_id = %template.id,
            action_name = %template.name,
            case_id = %case_id,
            actor_did = %actor_did,
            placeholders = ?placeholders,
            execution_id = %execution_id,
            "Simulating protected action execution (PII would be resolved in TEE)"
        );

        Ok(ActionResult {
            success: true,
            action_id: template.id.clone(),
            template_name: template.name.clone(),
            placeholders_present: placeholders,
            execution_id,
            error: None,
        })
    }

    fn name(&self) -> &str {
        "dev-mock"
    }
}

// ---------------------------------------------------------------------------
// Real T3 Executor
// ---------------------------------------------------------------------------

/// Production executor that dispatches protected actions to the T3 TEE API.
///
/// POSTs the action template to the T3 protected-actions endpoint. The TEE
/// resolves `{{profile.*}}` placeholders host-side and executes the HTTP
/// request. Resolved values never flow back to our system.
pub struct T3ProtectedActionExecutor {
    client: reqwest::Client,
    t3_api_url: String,
}

impl T3ProtectedActionExecutor {
    pub fn new(t3_api_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            t3_api_url,
        }
    }
}

/// Request payload sent to the T3 protected-actions API.
#[derive(Debug, Serialize)]
struct T3ExecuteRequest<'a> {
    action_id: &'a str,
    case_id: Uuid,
    actor_did: &'a str,
    template: &'a ActionTemplate,
}

/// Response from the T3 protected-actions API.
#[derive(Debug, Deserialize)]
struct T3ExecuteResponse {
    success: bool,
    execution_id: String,
    error: Option<String>,
}

#[async_trait]
impl ProtectedActionExecutor for T3ProtectedActionExecutor {
    async fn execute(
        &self,
        template: &ActionTemplate,
        case_id: Uuid,
        actor_did: &str,
    ) -> Result<ActionResult, AppError> {
        let placeholders = template.extract_placeholders();
        let endpoint = format!("{}/v1/protected-actions/execute", self.t3_api_url);

        let request_body = T3ExecuteRequest {
            action_id: &template.id,
            case_id,
            actor_did,
            template,
        };

        let response = self
            .client
            .post(&endpoint)
            .json(&request_body)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| {
                AppError::Internal(format!("T3 protected action request failed: {e}"))
            })?;

        if response.status().is_success() {
            let t3_response: T3ExecuteResponse =
                response.json().await.map_err(|e| {
                    AppError::Internal(format!(
                        "Failed to parse T3 protected action response: {e}"
                    ))
                })?;

            Ok(ActionResult {
                success: t3_response.success,
                action_id: template.id.clone(),
                template_name: template.name.clone(),
                placeholders_present: placeholders,
                execution_id: t3_response.execution_id,
                error: t3_response.error,
            })
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            tracing::error!(
                status = %status,
                body = %body,
                action_id = %template.id,
                "T3 protected action execution failed"
            );

            Ok(ActionResult {
                success: false,
                action_id: template.id.clone(),
                template_name: template.name.clone(),
                placeholders_present: placeholders,
                execution_id: String::new(),
                error: Some(format!("T3 API returned {status}")),
            })
        }
    }

    fn name(&self) -> &str {
        "t3-tee"
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_placeholders_from_template() {
        let template = ActionTemplate {
            id: "test".to_string(),
            name: "Test Action".to_string(),
            description: "A test".to_string(),
            url: "https://example.com".to_string(),
            method: "POST".to_string(),
            headers: HashMap::new(),
            body_template: serde_json::json!({
                "name": "{{profile.legal_name}}",
                "wallet": "{{profile.wallet_address}}",
                "agent": "{{agent.did}}",
                "static_field": "no_placeholder"
            }),
        };

        let placeholders = template.extract_placeholders();
        assert_eq!(placeholders.len(), 3);
        assert!(placeholders.contains(&"agent.did".to_string()));
        assert!(placeholders.contains(&"profile.legal_name".to_string()));
        assert!(placeholders.contains(&"profile.wallet_address".to_string()));
    }

    #[test]
    fn extracts_no_placeholders_from_static_template() {
        let template = ActionTemplate {
            id: "static".to_string(),
            name: "Static".to_string(),
            description: "No placeholders".to_string(),
            url: "https://example.com".to_string(),
            method: "GET".to_string(),
            headers: HashMap::new(),
            body_template: serde_json::json!({"key": "value"}),
        };

        let placeholders = template.extract_placeholders();
        assert!(placeholders.is_empty());
    }

    #[tokio::test]
    async fn dev_executor_returns_success() {
        let executor = DevProtectedActionExecutor;
        let template = ActionTemplate {
            id: "test_action".to_string(),
            name: "Test".to_string(),
            description: "Test action".to_string(),
            url: "https://example.com/api".to_string(),
            method: "POST".to_string(),
            headers: HashMap::new(),
            body_template: serde_json::json!({
                "entity": "{{profile.legal_name}}"
            }),
        };

        let case_id = Uuid::now_v7();
        let result = executor
            .execute(&template, case_id, "did:t3n:test-agent")
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.action_id, "test_action");
        assert_eq!(result.placeholders_present, vec!["profile.legal_name"]);
        assert!(!result.execution_id.is_empty());
        assert!(result.error.is_none());
    }
}
