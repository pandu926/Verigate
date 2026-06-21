//! Case domain model and request types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::types::{ActorType, CaseStatus, EntityType, WorkflowType};

/// A case in the workflow lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Case {
    pub id: Uuid,
    pub workflow_type: WorkflowType,
    pub entity_type: EntityType,
    pub relationship_goal: String,
    pub jurisdiction: Option<String>,
    pub requested_outcome: Option<String>,
    pub status: CaseStatus,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request payload to create a new case.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateCaseRequest {
    pub workflow_type: WorkflowType,
    pub entity_type: EntityType,
    pub relationship_goal: String,
    pub jurisdiction: Option<String>,
    pub requested_outcome: Option<String>,
}

impl CreateCaseRequest {
    /// Validate the request fields. Returns an error message if invalid.
    pub fn validate(&self) -> Result<(), String> {
        let trimmed = self.relationship_goal.trim();
        if trimmed.is_empty() {
            return Err("relationship_goal must not be empty".to_string());
        }
        Ok(())
    }
}

/// Request payload to transition a case to a new state.
#[derive(Debug, Clone, Deserialize)]
pub struct TransitionRequest {
    pub target_status: CaseStatus,
    pub actor_type: ActorType,
    pub actor_id: String,
    pub reason: Option<String>,
}
