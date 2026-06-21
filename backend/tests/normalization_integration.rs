//! Integration tests for DisclosedFact normalization from verification results.
//!
//! Proves that each verifier type produces correct DisclosedFacts with proper
//! metadata, and that the AI module boundary is architecturally enforced.

use serde_json::json;
use uuid::Uuid;

use verigate_backend::credential::normalizer::normalize_verification_result;
use verigate_backend::credential::models::VerificationResult;
use verigate_backend::domain::disclosed_fact::FactType;
use verigate_backend::domain::types::CredentialType;

// =============================================================================
// Entity Normalizer Tests
// =============================================================================

#[test]
fn normalize_entity_verification_produces_correct_facts() {
    let case_id = Uuid::now_v7();
    let result = VerificationResult {
        success: true,
        credential_type: CredentialType::Entity,
        extracted_claims: json!({
            "legal_name": "Acme Corp",
            "registration_number": "REG-12345",
            "jurisdiction": "US",
            "entity_type": "Corporation"
        }),
        issuer_did: "did:key:z6MkTest".to_string(),
        subject: Some("did:key:z6MkHolder".to_string()),
        failure_reason: None,
    };

    let facts = normalize_verification_result(
        &CredentialType::Entity,
        case_id,
        "entity_registration",
        &result,
        "eyJhbGciOiJFZERTQSJ9.entity_payload.signature",
    );

    assert_eq!(facts.len(), 4, "Entity normalizer should produce exactly 4 facts");

    let keys: Vec<&str> = facts.iter().map(|f| f.claim_key.as_str()).collect();
    assert!(keys.contains(&"legal_name"));
    assert!(keys.contains(&"registration_number"));
    assert!(keys.contains(&"jurisdiction"));
    assert!(keys.contains(&"entity_type"));

    for fact in &facts {
        assert_eq!(fact.fact_type, FactType::EntityVerified);
        assert_eq!(fact.case_id, case_id);
        assert_eq!(fact.requirement_id, "entity_registration");
        assert_eq!(fact.confidence, 1.0);
        assert!(!fact.source_credential_hash.is_empty());
    }

    // Verify correct values
    let name_fact = facts.iter().find(|f| f.claim_key == "legal_name").unwrap();
    assert_eq!(name_fact.claim_value, json!("Acme Corp"));
}

// =============================================================================
// Signer Normalizer Tests
// =============================================================================

#[test]
fn normalize_signer_verification_produces_correct_facts() {
    let case_id = Uuid::now_v7();
    let result = VerificationResult {
        success: true,
        credential_type: CredentialType::Signer,
        extracted_claims: json!({
            "name": "Jane Smith",
            "title": "CFO",
            "authority_level": "executive",
            "signing_capacity": "unlimited"
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

    assert_eq!(facts.len(), 4, "Signer normalizer should produce exactly 4 facts");

    let keys: Vec<&str> = facts.iter().map(|f| f.claim_key.as_str()).collect();
    assert!(keys.contains(&"name"));
    assert!(keys.contains(&"title"));
    assert!(keys.contains(&"authority_level"));
    assert!(keys.contains(&"signing_capacity"));

    for fact in &facts {
        assert_eq!(fact.fact_type, FactType::SignerAuthorized);
        assert_eq!(fact.requirement_id, "authorized_signer");
    }
}

// =============================================================================
// Region Normalizer Tests
// =============================================================================

#[test]
fn normalize_region_verification_produces_correct_facts() {
    let case_id = Uuid::now_v7();
    let result = VerificationResult {
        success: true,
        credential_type: CredentialType::Region,
        extracted_claims: json!({
            "country_code": "US",
            "region": "North America",
            "regulatory_framework": "SEC/FINRA",
            "risk_level": "low"
        }),
        issuer_did: "did:key:z6MkTest".to_string(),
        subject: None,
        failure_reason: None,
    };

    let facts = normalize_verification_result(
        &CredentialType::Region,
        case_id,
        "jurisdiction_compliance",
        &result,
        "region.jwt.token",
    );

    assert_eq!(facts.len(), 4, "Region normalizer should produce exactly 4 facts");

    let keys: Vec<&str> = facts.iter().map(|f| f.claim_key.as_str()).collect();
    assert!(keys.contains(&"country_code"));
    assert!(keys.contains(&"region"));
    assert!(keys.contains(&"regulatory_framework"));
    assert!(keys.contains(&"risk_level"));

    for fact in &facts {
        assert_eq!(fact.fact_type, FactType::JurisdictionConfirmed);
        assert_eq!(fact.requirement_id, "jurisdiction_compliance");
    }
}

// =============================================================================
// Wallet Normalizer Tests
// =============================================================================

#[test]
fn normalize_wallet_verification_produces_correct_facts() {
    let case_id = Uuid::now_v7();
    let result = VerificationResult {
        success: true,
        credential_type: CredentialType::Wallet,
        extracted_claims: json!({
            "wallet_address": "0x742d35Cc6634C0532925a3b844Bc9e7595f2bD28",
            "chain": "ethereum",
            "protocol": "ERC-20",
            "verification_method": "eip-712-signature"
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
        "wallet.jwt.token",
    );

    assert_eq!(facts.len(), 4, "Wallet normalizer should produce exactly 4 facts");

    let keys: Vec<&str> = facts.iter().map(|f| f.claim_key.as_str()).collect();
    assert!(keys.contains(&"wallet_address"));
    assert!(keys.contains(&"chain"));
    assert!(keys.contains(&"protocol"));
    assert!(keys.contains(&"verification_method"));

    for fact in &facts {
        assert_eq!(fact.fact_type, FactType::WalletOwnership);
        assert_eq!(fact.requirement_id, "wallet_proof");
    }
}

// =============================================================================
// Hash Tests
// =============================================================================

#[test]
fn source_credential_hash_is_sha256_hex() {
    let case_id = Uuid::now_v7();
    let result = VerificationResult {
        success: true,
        credential_type: CredentialType::Entity,
        extracted_claims: json!({"legal_name": "Test Corp"}),
        issuer_did: "did:key:z6MkTest".to_string(),
        subject: None,
        failure_reason: None,
    };

    let vp_jwt = "eyJhbGciOiJFZERTQSJ9.test_payload.test_signature";
    let facts = normalize_verification_result(
        &CredentialType::Entity,
        case_id,
        "entity_registration",
        &result,
        vp_jwt,
    );

    assert!(!facts.is_empty());
    let hash = &facts[0].source_credential_hash;

    // SHA-256 produces 64 hex characters
    assert_eq!(hash.len(), 64, "SHA-256 hash should be 64 hex chars, got {}", hash.len());
    // All characters should be valid hex
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()), "Hash should be all hex chars");

    // Same input produces same hash (deterministic)
    let facts2 = normalize_verification_result(
        &CredentialType::Entity,
        case_id,
        "entity_registration",
        &result,
        vp_jwt,
    );
    assert_eq!(facts[0].source_credential_hash, facts2[0].source_credential_hash);
}

#[test]
fn different_vp_jwts_produce_different_hashes() {
    let case_id = Uuid::now_v7();
    let result = VerificationResult {
        success: true,
        credential_type: CredentialType::Entity,
        extracted_claims: json!({"legal_name": "Test Corp"}),
        issuer_did: "did:key:z6MkTest".to_string(),
        subject: None,
        failure_reason: None,
    };

    let facts1 = normalize_verification_result(
        &CredentialType::Entity,
        case_id,
        "entity_registration",
        &result,
        "jwt_token_one.payload.signature",
    );

    let facts2 = normalize_verification_result(
        &CredentialType::Entity,
        case_id,
        "entity_registration",
        &result,
        "jwt_token_two.payload.signature",
    );

    assert_ne!(
        facts1[0].source_credential_hash,
        facts2[0].source_credential_hash,
        "Different VP JWTs must produce different hashes"
    );
}

// =============================================================================
// Filtering Tests
// =============================================================================

#[test]
fn normalize_only_includes_extracted_claims_from_verification_result() {
    let case_id = Uuid::now_v7();
    // The normalizer reads from extracted_claims which is already filtered by the verifier.
    // If extracted_claims contains fields beyond the normalizer's allowed list, they are excluded.
    let result = VerificationResult {
        success: true,
        credential_type: CredentialType::Entity,
        extracted_claims: json!({
            "legal_name": "Acme Corp",
            "registration_number": "REG-12345",
            "jurisdiction": "US",
            "entity_type": "Corporation",
            "extra_field": "should_be_dropped",
            "ssn": "123-45-6789",
            "internal_score": 95
        }),
        issuer_did: "did:key:z6MkTest".to_string(),
        subject: None,
        failure_reason: None,
    };

    let facts = normalize_verification_result(
        &CredentialType::Entity,
        case_id,
        "entity_registration",
        &result,
        "test.jwt.data",
    );

    // Only 4 allowed claims should produce facts
    assert_eq!(facts.len(), 4);

    let keys: Vec<&str> = facts.iter().map(|f| f.claim_key.as_str()).collect();
    assert!(!keys.contains(&"extra_field"), "extra_field must not appear in facts");
    assert!(!keys.contains(&"ssn"), "ssn must not appear in facts");
    assert!(!keys.contains(&"internal_score"), "internal_score must not appear in facts");
}

// =============================================================================
// AI Module Boundary Test
// =============================================================================

/// Verify AI module boundary - this test documents the architectural constraint.
/// The ai module only accepts Vec<DisclosedFact> and cannot import raw credential types.
#[test]
fn ai_module_boundary_holds() {
    let ai_mod_path = std::path::Path::new("src/ai/mod.rs");
    let content = std::fs::read_to_string(ai_mod_path).expect("ai/mod.rs must exist");

    // Verify forbidden raw credential types are not imported (only doc comments allowed)
    let non_comment_lines: Vec<&str> = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with("//") && !trimmed.starts_with("///") && !trimmed.starts_with("//!")
        })
        .collect();

    let non_comment_content = non_comment_lines.join("\n");

    assert!(
        !non_comment_content.contains("VerifiablePresentation"),
        "AI module must not import VerifiablePresentation in non-comment code"
    );
    assert!(
        !non_comment_content.contains("VerifiableCredential"),
        "AI module must not import VerifiableCredential in non-comment code"
    );
    assert!(
        !non_comment_content.contains("crate::credential::"),
        "AI module must not import from crate::credential"
    );

    // Verify it DOES use DisclosedFact
    assert!(
        content.contains("DisclosedFact"),
        "AI module must use DisclosedFact"
    );
    assert!(
        content.contains("FactType"),
        "AI module must use FactType"
    );
}
