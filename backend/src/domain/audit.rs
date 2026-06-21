//! Audit event domain model and types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::types::ActorType;

/// An immutable audit event recording a case action.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditEvent {
    pub id: Uuid,
    pub case_id: Uuid,
    pub actor_type: ActorType,
    pub actor_id: String,
    pub action: String,
    pub details: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// Input struct for inserting a new audit event (DB generates id and created_at).
#[derive(Debug, Clone)]
pub struct NewAuditEvent {
    pub case_id: Uuid,
    pub actor_type: ActorType,
    pub actor_id: String,
    pub action: String,
    pub details: Option<serde_json::Value>,
}

/// Well-known audit event action types.
/// Stored as string constants rather than a sqlx::Type enum for extensibility.
pub struct AuditEventType;

impl AuditEventType {
    pub const CASE_CREATED: &'static str = "case_created";
    pub const STATE_TRANSITION: &'static str = "state_transition";
    pub const CREDENTIAL_SUBMITTED: &'static str = "credential_submitted";
    pub const CREDENTIAL_VERIFIED: &'static str = "credential_verified";
    pub const CREDENTIAL_FAILED: &'static str = "credential_failed";
    pub const OVERRIDE_DECISION: &'static str = "override_decision";
    pub const PROTECTED_ACTION_EXECUTED: &'static str = "protected_action_executed";
}
