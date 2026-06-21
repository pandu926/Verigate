//! Assessment domain model.
//!
//! Represents the output of the AI assessment pipeline: a structured evaluation
//! of a case's completeness, risks, and recommended decision.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The decision produced by the AI assessment pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum AssessmentDecision {
    Ready,
    MoreProofRequired,
    NeedsReview,
    Blocked,
}

impl AssessmentDecision {
    /// Parse a decision string into the enum variant.
    pub fn from_str_value(s: &str) -> Self {
        match s {
            "ready" => Self::Ready,
            "more_proof_required" => Self::MoreProofRequired,
            "needs_review" => Self::NeedsReview,
            "blocked" => Self::Blocked,
            _ => Self::NeedsReview,
        }
    }
}

/// A link between an assessment claim and a specific disclosed fact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceLink {
    pub fact_id: Uuid,
    pub claim_key: String,
    pub relevance: String,
}

/// An AI-suggested requirement that supplements policy-mandated ones.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicRequirement {
    pub requirement_id: String,
    pub claim_type: String,
    pub reason: String,
    pub source: String,
}

impl DynamicRequirement {
    /// Create a new AI-suggested dynamic requirement.
    pub fn new(requirement_id: String, claim_type: String, reason: String) -> Self {
        Self {
            requirement_id,
            claim_type,
            reason,
            source: "ai_planner".to_string(),
        }
    }
}

/// A persisted assessment record from the database.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Assessment {
    pub id: Uuid,
    pub case_id: Uuid,
    pub summary_text: String,
    pub decision: AssessmentDecision,
    pub evidence_links: serde_json::Value,
    pub confidence: f64,
    pub agent_outputs: Option<serde_json::Value>,
    pub dynamic_requirements: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// Input struct for inserting a new assessment (DB generates id and created_at).
#[derive(Debug, Clone)]
pub struct NewAssessment {
    pub case_id: Uuid,
    pub summary_text: String,
    pub decision: AssessmentDecision,
    pub evidence_links: serde_json::Value,
    pub confidence: f64,
    pub agent_outputs: Option<serde_json::Value>,
    pub dynamic_requirements: Option<serde_json::Value>,
}
