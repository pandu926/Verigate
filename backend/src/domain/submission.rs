//! Submission domain model — tracks verifiable presentation submissions per case.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Status lifecycle for a credential submission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum SubmissionStatus {
    Pending,
    Submitted,
    Verifying,
    Verified,
    Failed,
}

/// A submission record tracking a VP submitted for verification.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Submission {
    pub id: Uuid,
    pub case_id: Uuid,
    pub requirement_claim_type: String,
    pub credential_type: String,
    pub status: SubmissionStatus,
    pub raw_vp: serde_json::Value,
    pub extracted_claims: Option<serde_json::Value>,
    pub failure_reason: Option<String>,
    pub submitted_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
    pub submitted_by: String,
}

/// Request payload for creating a new submission via the API.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateSubmissionRequest {
    /// The raw verifiable presentation JSON.
    pub raw_vp: serde_json::Value,

    /// The requirement claim type this submission satisfies (e.g., "entity_identity").
    pub requirement_claim_type: String,

    /// The credential type being submitted (entity, signer, region, wallet).
    pub credential_type: String,
}
