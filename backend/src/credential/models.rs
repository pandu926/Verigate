//! Credential verification models — result types and re-exports.

use serde::{Deserialize, Serialize};

use crate::domain::types::CredentialType;

/// Result of verifying a single verifiable credential.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether the credential passed all checks.
    pub success: bool,

    /// The type of credential that was verified.
    pub credential_type: CredentialType,

    /// Claims extracted from the credential subject.
    pub extracted_claims: serde_json::Value,

    /// The issuer DID that signed the credential.
    pub issuer_did: String,

    /// Optional subject identifier from the credential.
    pub subject: Option<String>,

    /// Reason for failure (populated when success is false).
    pub failure_reason: Option<String>,
}
