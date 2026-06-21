//! DisclosedFact normalization from verification results.
//!
//! Transforms raw VerificationResult + VP JWT into structured DisclosedFact
//! objects containing only policy-required claims. Each verifier type has a
//! corresponding normalizer that maps extracted claims to typed facts.

use chrono::Utc;
use ring::digest::{digest, SHA256};
use uuid::Uuid;

use crate::credential::models::VerificationResult;
use crate::domain::disclosed_fact::{DisclosedFact, FactType};
use crate::domain::types::CredentialType;

/// Trait for normalizing verification results into DisclosedFacts.
///
/// Each credential type implements this to map its type-specific claims
/// to the canonical DisclosedFact structure.
pub trait FactNormalizer: Send + Sync {
    /// The credential type this normalizer handles.
    fn credential_type(&self) -> CredentialType;

    /// Normalize a verification result into a set of DisclosedFacts.
    ///
    /// - `case_id`: The case this submission belongs to.
    /// - `requirement_id`: The policy requirement being satisfied.
    /// - `verification_result`: The successful verification output.
    /// - `vp_jwt`: The raw VP JWT string (hashed for audit linkage).
    fn normalize(
        &self,
        case_id: Uuid,
        requirement_id: &str,
        verification_result: &VerificationResult,
        vp_jwt: &str,
    ) -> Vec<DisclosedFact>;
}

/// Compute SHA-256 hex digest of the VP JWT bytes for audit linkage.
fn compute_credential_hash(vp_jwt: &str) -> String {
    let hash = digest(&SHA256, vp_jwt.as_bytes());
    hex_encode(hash.as_ref())
}

/// Encode bytes as lowercase hex string.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Create a single DisclosedFact from claim key/value pair.
fn make_fact(
    case_id: Uuid,
    requirement_id: &str,
    fact_type: FactType,
    claim_key: &str,
    claim_value: &serde_json::Value,
    credential_hash: &str,
) -> DisclosedFact {
    DisclosedFact {
        id: Uuid::now_v7(),
        case_id,
        requirement_id: requirement_id.to_string(),
        fact_type,
        claim_key: claim_key.to_string(),
        claim_value: claim_value.clone(),
        confidence: 1.0,
        source_credential_hash: credential_hash.to_string(),
        verified_at: Utc::now(),
    }
}

/// Extract facts from a verification result's extracted_claims JSON object.
fn extract_facts_from_claims(
    case_id: Uuid,
    requirement_id: &str,
    fact_type: FactType,
    claims: &serde_json::Value,
    credential_hash: &str,
    allowed_keys: &[&str],
) -> Vec<DisclosedFact> {
    let obj = match claims.as_object() {
        Some(o) => o,
        None => return Vec::new(),
    };

    obj.iter()
        .filter(|(key, value)| allowed_keys.contains(&key.as_str()) && !value.is_null())
        .map(|(key, value)| {
            make_fact(
                case_id,
                requirement_id,
                fact_type.clone(),
                key,
                value,
                credential_hash,
            )
        })
        .collect()
}

// --- Type-Specific Normalizer Implementations ---

/// Normalizer for entity identity credentials.
pub struct EntityNormalizer;

impl FactNormalizer for EntityNormalizer {
    fn credential_type(&self) -> CredentialType {
        CredentialType::Entity
    }

    fn normalize(
        &self,
        case_id: Uuid,
        requirement_id: &str,
        result: &VerificationResult,
        vp_jwt: &str,
    ) -> Vec<DisclosedFact> {
        let hash = compute_credential_hash(vp_jwt);
        let allowed = &["legal_name", "registration_number", "jurisdiction", "entity_type"];
        extract_facts_from_claims(
            case_id,
            requirement_id,
            FactType::EntityVerified,
            &result.extracted_claims,
            &hash,
            allowed,
        )
    }
}

/// Normalizer for authorized signer credentials.
pub struct SignerNormalizer;

impl FactNormalizer for SignerNormalizer {
    fn credential_type(&self) -> CredentialType {
        CredentialType::Signer
    }

    fn normalize(
        &self,
        case_id: Uuid,
        requirement_id: &str,
        result: &VerificationResult,
        vp_jwt: &str,
    ) -> Vec<DisclosedFact> {
        let hash = compute_credential_hash(vp_jwt);
        let allowed = &["name", "title", "authority_level", "signing_capacity"];
        extract_facts_from_claims(
            case_id,
            requirement_id,
            FactType::SignerAuthorized,
            &result.extracted_claims,
            &hash,
            allowed,
        )
    }
}

/// Normalizer for regional/jurisdictional credentials.
pub struct RegionNormalizer;

impl FactNormalizer for RegionNormalizer {
    fn credential_type(&self) -> CredentialType {
        CredentialType::Region
    }

    fn normalize(
        &self,
        case_id: Uuid,
        requirement_id: &str,
        result: &VerificationResult,
        vp_jwt: &str,
    ) -> Vec<DisclosedFact> {
        let hash = compute_credential_hash(vp_jwt);
        let allowed = &["country_code", "region", "regulatory_framework", "risk_level"];
        extract_facts_from_claims(
            case_id,
            requirement_id,
            FactType::JurisdictionConfirmed,
            &result.extracted_claims,
            &hash,
            allowed,
        )
    }
}

/// Normalizer for wallet ownership credentials.
pub struct WalletNormalizer;

impl FactNormalizer for WalletNormalizer {
    fn credential_type(&self) -> CredentialType {
        CredentialType::Wallet
    }

    fn normalize(
        &self,
        case_id: Uuid,
        requirement_id: &str,
        result: &VerificationResult,
        vp_jwt: &str,
    ) -> Vec<DisclosedFact> {
        let hash = compute_credential_hash(vp_jwt);
        let allowed = &["wallet_address", "chain", "protocol", "verification_method"];
        extract_facts_from_claims(
            case_id,
            requirement_id,
            FactType::WalletOwnership,
            &result.extracted_claims,
            &hash,
            allowed,
        )
    }
}

/// Dispatch normalization to the correct type-specific normalizer.
///
/// This is the primary entry point called from the submission flow after
/// a successful verification.
pub fn normalize_verification_result(
    credential_type: &CredentialType,
    case_id: Uuid,
    requirement_id: &str,
    result: &VerificationResult,
    vp_jwt: &str,
) -> Vec<DisclosedFact> {
    match credential_type {
        CredentialType::Entity => {
            EntityNormalizer.normalize(case_id, requirement_id, result, vp_jwt)
        }
        CredentialType::Signer => {
            SignerNormalizer.normalize(case_id, requirement_id, result, vp_jwt)
        }
        CredentialType::Region => {
            RegionNormalizer.normalize(case_id, requirement_id, result, vp_jwt)
        }
        CredentialType::Wallet => {
            WalletNormalizer.normalize(case_id, requirement_id, result, vp_jwt)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_entity_result() -> VerificationResult {
        VerificationResult {
            success: true,
            credential_type: CredentialType::Entity,
            extracted_claims: json!({
                "legal_name": "Acme Corp",
                "registration_number": "REG-12345",
                "jurisdiction": "US",
                "entity_type": "Corporation",
                "extra_field": "should_not_appear"
            }),
            issuer_did: "did:key:z6MkTest".to_string(),
            subject: Some("did:key:z6MkHolder".to_string()),
            failure_reason: None,
        }
    }

    #[test]
    fn entity_normalizer_extracts_only_allowed_claims() {
        let case_id = Uuid::now_v7();
        let result = sample_entity_result();
        let vp_jwt = "eyJhbGciOiJFZERTQSJ9.payload.signature";

        let facts = EntityNormalizer.normalize(case_id, "entity_registration", &result, vp_jwt);

        assert_eq!(facts.len(), 4);
        let keys: Vec<&str> = facts.iter().map(|f| f.claim_key.as_str()).collect();
        assert!(keys.contains(&"legal_name"));
        assert!(keys.contains(&"registration_number"));
        assert!(keys.contains(&"jurisdiction"));
        assert!(keys.contains(&"entity_type"));
        // extra_field must not appear
        assert!(!keys.contains(&"extra_field"));
    }

    #[test]
    fn normalizer_sets_correct_metadata() {
        let case_id = Uuid::now_v7();
        let result = sample_entity_result();
        let vp_jwt = "test.jwt.token";

        let facts = EntityNormalizer.normalize(case_id, "entity_registration", &result, vp_jwt);

        for fact in &facts {
            assert_eq!(fact.case_id, case_id);
            assert_eq!(fact.requirement_id, "entity_registration");
            assert_eq!(fact.fact_type, FactType::EntityVerified);
            assert_eq!(fact.confidence, 1.0);
            assert!(!fact.source_credential_hash.is_empty());
        }
    }

    #[test]
    fn credential_hash_is_deterministic() {
        let hash1 = compute_credential_hash("test.jwt.data");
        let hash2 = compute_credential_hash("test.jwt.data");
        assert_eq!(hash1, hash2);
        // SHA-256 produces 64 hex chars
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn credential_hash_differs_for_different_input() {
        let hash1 = compute_credential_hash("jwt1");
        let hash2 = compute_credential_hash("jwt2");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn dispatch_routes_to_correct_normalizer() {
        let case_id = Uuid::now_v7();
        let result = VerificationResult {
            success: true,
            credential_type: CredentialType::Signer,
            extracted_claims: json!({
                "name": "Alice",
                "title": "CEO",
                "authority_level": "full",
                "signing_capacity": "individual"
            }),
            issuer_did: "did:key:z6MkTest".to_string(),
            subject: None,
            failure_reason: None,
        };

        let facts = normalize_verification_result(
            &CredentialType::Signer,
            case_id,
            "authorized_signer",
            &result,
            "signer.jwt.token",
        );

        assert_eq!(facts.len(), 4);
        for fact in &facts {
            assert_eq!(fact.fact_type, FactType::SignerAuthorized);
        }
    }

    #[test]
    fn null_claims_are_excluded() {
        let case_id = Uuid::now_v7();
        let result = VerificationResult {
            success: true,
            credential_type: CredentialType::Wallet,
            extracted_claims: json!({
                "wallet_address": "0xABC",
                "chain": null,
                "protocol": "ethereum",
                "verification_method": null
            }),
            issuer_did: "did:key:z6MkTest".to_string(),
            subject: None,
            failure_reason: None,
        };

        let facts = normalize_verification_result(
            &CredentialType::Wallet,
            case_id,
            "wallet_proof",
            &result,
            "wallet.jwt",
        );

        // Only non-null values should produce facts
        assert_eq!(facts.len(), 2);
        let keys: Vec<&str> = facts.iter().map(|f| f.claim_key.as_str()).collect();
        assert!(keys.contains(&"wallet_address"));
        assert!(keys.contains(&"protocol"));
    }
}
