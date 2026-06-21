use sqlx::PgPool;
use std::sync::Arc;
use std::time::Instant;

use crate::ai::LlmClient;
use crate::auth::cedar::PolicyEngine;
use crate::auth::requirements::RequirementEngine;
use crate::credential::issuer_trust::TrustedIssuerRegistry;
use crate::credential::verifier::CredentialVerifier;
use crate::t3::agent_identity::AgentIdentity;
use crate::t3::protected_action::ProtectedActionExecutor;
use crate::t3::verification::T3nVerificationClient;

pub mod ai;
pub mod auth;
pub mod config;
pub mod credential;
pub mod db;
pub mod domain;
pub mod error;
pub mod routes;
pub mod seed;
pub mod t3;

/// Shared application state available to all route handlers.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub agent_identity: AgentIdentity,
    pub start_time: Arc<Instant>,
    pub policy_engine: Arc<PolicyEngine>,
    pub requirement_engine: Arc<RequirementEngine>,
    pub jwt_secret: String,
    pub issuer_registry: Arc<TrustedIssuerRegistry>,
    pub credential_verifiers: Arc<Vec<Box<dyn CredentialVerifier>>>,
    pub llm_client: Arc<dyn LlmClient>,
    pub llm_api_key: Option<String>,
    pub protected_action_executor: Arc<dyn ProtectedActionExecutor>,
    pub t3n_client: Option<Arc<T3nVerificationClient>>,
}
