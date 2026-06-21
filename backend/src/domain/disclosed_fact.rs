use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The type of fact disclosed from a verified credential.
///
/// Each variant represents a category of privacy-safe claim that the AI layer
/// can reason over without accessing the underlying raw credential data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum FactType {
    EntityVerified,
    JurisdictionConfirmed,
    SignerAuthorized,
    WalletOwnership,
    FinancialThreshold,
    ComplianceStatus,
    Custom,
}

/// The canonical privacy-safe data structure that the AI layer consumes.
///
/// A DisclosedFact represents a single verified claim extracted from a credential
/// presentation. The AI reasoning layer operates exclusively on these structured
/// facts rather than raw credential data, enforcing the privacy boundary.
///
/// Fields:
/// - `requirement_id`: Links this fact to the policy requirement it satisfies.
/// - `source_credential_hash`: SHA-256 hex digest of the raw VP JWT for audit
///   linkage without exposing credential content.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DisclosedFact {
    pub id: Uuid,
    pub case_id: Uuid,
    pub requirement_id: String,
    pub fact_type: FactType,
    pub claim_key: String,
    pub claim_value: serde_json::Value,
    /// Confidence score between 0.0 (no confidence) and 1.0 (fully verified).
    pub confidence: f64,
    /// SHA-256 hex digest of the original VP JWT bytes.
    pub source_credential_hash: String,
    pub verified_at: DateTime<Utc>,
}
