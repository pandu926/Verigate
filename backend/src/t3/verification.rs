//! T3N verification client — routes verification through the T3N bridge.
//!
//! Provides methods to verify credentials via TEE contract execution,
//! store facts in T3N KV store, and push audit events to T3N ledger.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

/// Result of a T3N TEE verification call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct T3nVerificationResult {
    pub success: bool,
    pub execution_id: String,
    pub tee_mode: String,
    pub result: Option<serde_json::Value>,
}

/// Result of a KV store operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct T3nKvResult {
    pub stored: bool,
    pub key: String,
    pub mode: String,
}

/// Result of an audit push operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct T3nAuditResult {
    pub pushed: bool,
    pub t3n_event_id: String,
}

/// Client for T3N bridge operations.
pub struct T3nVerificationClient {
    http: reqwest::Client,
    bridge_url: String,
}

impl T3nVerificationClient {
    pub fn new(bridge_url: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .connect_timeout(std::time::Duration::from_secs(3))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            http,
            bridge_url: bridge_url.into(),
        }
    }

    /// Verify a VP through the T3N TEE contract.
    pub async fn verify_via_tee(
        &self,
        vp_json: &serde_json::Value,
        case_id: Uuid,
    ) -> Result<T3nVerificationResult, AppError> {
        let payload = serde_json::json!({
            "contract": "verigate-verify",
            "function_name": "verify_credential",
            "args": {
                "vp": vp_json,
                "case_id": case_id.to_string(),
            }
        });

        let resp = self.http
            .post(format!("{}/contract/execute", self.bridge_url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("T3N bridge unreachable: {e}")))?;

        let result: T3nVerificationResult = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("T3N bridge invalid response: {e}")))?;

        Ok(result)
    }

    /// Store disclosed facts in T3N KV store.
    pub async fn store_facts(
        &self,
        case_id: Uuid,
        facts: &serde_json::Value,
    ) -> Result<T3nKvResult, AppError> {
        let payload = serde_json::json!({
            "map_name": "verigate-facts",
            "key": case_id.to_string(),
            "value": facts,
        });

        let resp = self.http
            .post(format!("{}/kv/put", self.bridge_url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("T3N KV store unreachable: {e}")))?;

        let result: T3nKvResult = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("T3N KV store invalid response: {e}")))?;

        Ok(result)
    }

    /// Push an audit event to the T3N ledger.
    pub async fn push_audit(
        &self,
        case_id: Uuid,
        actor_did: &str,
        action: &str,
        details: &serde_json::Value,
    ) -> Result<T3nAuditResult, AppError> {
        let payload = serde_json::json!({
            "event_type": action,
            "case_id": case_id.to_string(),
            "actor_did": actor_did,
            "action": action,
            "details": details,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        let resp = self.http
            .post(format!("{}/audit/push", self.bridge_url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("T3N audit push unreachable: {e}")))?;

        let result: T3nAuditResult = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("T3N audit push invalid response: {e}")))?;

        Ok(result)
    }

    /// Set compliance policy for a case via TEE contract.
    pub async fn set_policy(
        &self,
        case_id: Uuid,
        policy: &serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let payload = serde_json::json!({
            "case_id": case_id.to_string(),
            "policy": policy,
        });

        let resp = self.http
            .post(format!("{}/policy/set", self.bridge_url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("T3N set-policy unreachable: {e}")))?;

        resp.json().await
            .map_err(|e| AppError::Internal(format!("T3N set-policy invalid response: {e}")))
    }

    /// Commit assessment plan — locks execution order in TEE.
    pub async fn commit_plan(
        &self,
        case_id: Uuid,
        steps: &[serde_json::Value],
        ttl_secs: u64,
    ) -> Result<serde_json::Value, AppError> {
        let payload = serde_json::json!({
            "case_id": case_id.to_string(),
            "steps": steps,
            "ttl_secs": ttl_secs,
        });

        let resp = self.http
            .post(format!("{}/plan/commit", self.bridge_url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("T3N commit-plan unreachable: {e}")))?;

        resp.json().await
            .map_err(|e| AppError::Internal(format!("T3N commit-plan invalid response: {e}")))
    }

    /// Get plan status from TEE.
    pub async fn get_plan_status(
        &self,
        case_id: Uuid,
    ) -> Result<serde_json::Value, AppError> {
        let resp = self.http
            .get(format!("{}/plan/status?case_id={}", self.bridge_url, case_id))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("T3N plan-status unreachable: {e}")))?;

        resp.json().await
            .map_err(|e| AppError::Internal(format!("T3N plan-status invalid response: {e}")))
    }

    /// Get evidence chain from TEE.
    pub async fn get_evidence_chain(
        &self,
        case_id: Uuid,
    ) -> Result<serde_json::Value, AppError> {
        let resp = self.http
            .get(format!("{}/evidence?case_id={}", self.bridge_url, case_id))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("T3N evidence unreachable: {e}")))?;

        resp.json().await
            .map_err(|e| AppError::Internal(format!("T3N evidence invalid response: {e}")))
    }

    /// Get violations from TEE.
    pub async fn get_violations(
        &self,
        case_id: Uuid,
    ) -> Result<serde_json::Value, AppError> {
        let resp = self.http
            .get(format!("{}/violations?case_id={}", self.bridge_url, case_id))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("T3N violations unreachable: {e}")))?;

        resp.json().await
            .map_err(|e| AppError::Internal(format!("T3N violations invalid response: {e}")))
    }

    /// Trigger decision in TEE.
    pub async fn decide(
        &self,
        case_id: Uuid,
    ) -> Result<serde_json::Value, AppError> {
        let payload = serde_json::json!({ "case_id": case_id.to_string() });

        let resp = self.http
            .post(format!("{}/decide", self.bridge_url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("T3N decide unreachable: {e}")))?;

        resp.json().await
            .map_err(|e| AppError::Internal(format!("T3N decide invalid response: {e}")))
    }

    /// Execute protected action (post-approval, TEE-gated).
    pub async fn execute_protected(
        &self,
        case_id: Uuid,
        action_type: &str,
        action_payload: &serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let payload = serde_json::json!({
            "case_id": case_id.to_string(),
            "action_type": action_type,
            "action_payload": action_payload,
        });

        let resp = self.http
            .post(format!("{}/protected/execute", self.bridge_url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("T3N protected-action unreachable: {e}")))?;

        resp.json().await
            .map_err(|e| AppError::Internal(format!("T3N protected-action invalid response: {e}")))
    }

    /// Run full orchestration (policy→plan→verify(N)→assess→decide) in one call.
    pub async fn orchestrate_full(
        &self,
        case_id: Uuid,
        credentials: &[serde_json::Value],
        policy: Option<&serde_json::Value>,
        llm_api_key: &str,
    ) -> Result<serde_json::Value, AppError> {
        let payload = serde_json::json!({
            "case_id": case_id.to_string(),
            "credentials": credentials,
            "policy": policy,
            "llm_api_key": llm_api_key,
        });

        let resp = self.http
            .post(format!("{}/orchestrate/full", self.bridge_url))
            .timeout(std::time::Duration::from_secs(300))
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("T3N orchestrate unreachable: {e}")))?;

        resp.json().await
            .map_err(|e| AppError::Internal(format!("T3N orchestrate invalid response: {e}")))
    }
}
