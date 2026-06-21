//! Integration tests for the credential verification pipeline.
//!
//! Tests VP parsing, signature verification (Ed25519 + ES256), issuer trust
//! validation, claim extraction for all 4 credential types, and type dispatch.

mod credential_test_factory;

use credential_test_factory::TestCredentialFactory;
use verigate_backend::credential::issuer_trust::{TrustedIssuer, TrustedIssuerRegistry};
use verigate_backend::credential::verifier::{
    self, decode_jwt_parts, determine_credential_type, extract_vc_from_jwt_payload,
    verify_jwt_signature, CredentialVerifier, EntityVerifier, RegionVerifier, SignerVerifier,
    VerificationError, WalletVerifier,
};
use verigate_backend::domain::credential::VerifiablePresentation;
use verigate_backend::domain::types::CredentialType;

use chrono::{DateTime, Utc};

/// Build a registry with a custom issuer entry for testing.
fn build_custom_registry(issuer: TrustedIssuer) -> TrustedIssuerRegistry {
    // Use from_file with a temp file approach, or construct directly.
    // Since TrustedIssuerRegistry fields are private, we write a temp JSON file.
    let config = serde_json::json!({
        "issuers": [
            {
                "did": issuer.did,
                "name": issuer.name,
                "credential_types": issuer.credential_types,
                "valid_from": issuer.valid_from.to_rfc3339(),
                "valid_until": issuer.valid_until.map(|d| d.to_rfc3339()),
            }
        ]
    });

    let tmp_dir = std::env::temp_dir();
    let unique_name = format!("test_issuers_{}.json", uuid::Uuid::now_v7());
    let tmp_file = tmp_dir.join(unique_name);
    std::fs::write(&tmp_file, serde_json::to_string_pretty(&config).unwrap()).unwrap();
    let registry = TrustedIssuerRegistry::from_file(&tmp_file).unwrap();
    let _ = std::fs::remove_file(&tmp_file);
    registry
}

/// Create the standard set of verifiers used in production.
fn build_verifiers() -> Vec<Box<dyn CredentialVerifier>> {
    vec![
        Box::new(EntityVerifier),
        Box::new(SignerVerifier),
        Box::new(RegionVerifier),
        Box::new(WalletVerifier),
    ]
}

/// Build a registry that trusts the test factory's Ed25519 issuer for all types.
fn build_factory_ed25519_registry(factory: &TestCredentialFactory) -> TrustedIssuerRegistry {
    let issuer = TrustedIssuer {
        did: factory.ed25519_issuer_did(),
        name: "Test Ed25519 Issuer".to_string(),
        credential_types: vec![
            "entity".to_string(),
            "signer".to_string(),
            "region".to_string(),
            "wallet".to_string(),
        ],
        valid_from: DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        valid_until: None,
    };
    build_custom_registry(issuer)
}

/// Build a registry that trusts the test factory's ES256 issuer for wallet type.
fn build_factory_es256_registry(factory: &TestCredentialFactory) -> TrustedIssuerRegistry {
    let issuer = TrustedIssuer {
        did: factory.es256_issuer_did(),
        name: "Test ES256 Issuer".to_string(),
        credential_types: vec!["wallet".to_string()],
        valid_from: DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        valid_until: None,
    };
    build_custom_registry(issuer)
}

// =============================================================================
// VP Parsing Tests
// =============================================================================

#[test]
fn parses_valid_entity_vp() {
    let factory = TestCredentialFactory::new();
    let issuer_did = factory.ed25519_issuer_did();
    let vp_json = factory.create_entity_vp(&issuer_did);

    let vp: VerifiablePresentation =
        serde_json::from_value(vp_json).expect("VP should deserialize");

    assert!(!vp.verifiable_credential.is_empty());
    assert_eq!(vp.verifiable_credential.len(), 1);
    assert!(vp.vp_type.contains(&"VerifiablePresentation".to_string()));
}

#[test]
fn parses_vp_with_multiple_credentials() {
    let factory = TestCredentialFactory::new();
    let ed_did = factory.ed25519_issuer_did();

    // Manually build a VP with 2 VCs
    let entity_jwt = {
        let vp = factory.create_entity_vp(&ed_did);
        vp["verifiableCredential"][0].as_str().unwrap().to_string()
    };
    let signer_jwt = {
        let vp = factory.create_signer_vp(&ed_did);
        vp["verifiableCredential"][0].as_str().unwrap().to_string()
    };

    let multi_vp_json = serde_json::json!({
        "@context": ["https://www.w3.org/2018/credentials/v1"],
        "type": ["VerifiablePresentation"],
        "verifiableCredential": [entity_jwt, signer_jwt]
    });

    let vp: VerifiablePresentation =
        serde_json::from_value(multi_vp_json).expect("Multi-VC VP should deserialize");

    assert_eq!(vp.verifiable_credential.len(), 2);
}

// =============================================================================
// Signature Verification Tests
// =============================================================================

#[test]
fn verifies_valid_ed25519_signature() {
    let factory = TestCredentialFactory::new();
    let issuer_did = factory.ed25519_issuer_did();
    let vp_json = factory.create_entity_vp(&issuer_did);

    let creds = vp_json["verifiableCredential"].as_array().unwrap();
    let jwt = creds[0].as_str().unwrap();

    let result = verify_jwt_signature(jwt, factory.ed25519_public_key(), "EdDSA");
    assert!(result.is_ok(), "Ed25519 signature should verify: {result:?}");
}

#[test]
fn verifies_valid_es256_signature() {
    let factory = TestCredentialFactory::new();
    let issuer_did = factory.es256_issuer_did();
    let vp_json = factory.create_wallet_vp(&issuer_did);

    let creds = vp_json["verifiableCredential"].as_array().unwrap();
    let jwt = creds[0].as_str().unwrap();

    let result = verify_jwt_signature(jwt, factory.es256_public_key(), "ES256");
    assert!(result.is_ok(), "ES256 signature should verify: {result:?}");
}

#[test]
fn rejects_tampered_payload() {
    let factory = TestCredentialFactory::new();
    let issuer_did = factory.ed25519_issuer_did();
    let vp_json = factory.create_entity_vp(&issuer_did);

    let creds = vp_json["verifiableCredential"].as_array().unwrap();
    let jwt = creds[0].as_str().unwrap();

    // Tamper with the payload section (modify a character)
    let parts: Vec<&str> = jwt.split('.').collect();
    let mut payload_bytes = parts[1].as_bytes().to_vec();
    if let Some(last) = payload_bytes.last_mut() {
        *last = if *last == b'A' { b'B' } else { b'A' };
    }
    let tampered_payload = String::from_utf8(payload_bytes).unwrap();
    let tampered_jwt = format!("{}.{}.{}", parts[0], tampered_payload, parts[2]);

    let result = verify_jwt_signature(&tampered_jwt, factory.ed25519_public_key(), "EdDSA");
    assert!(result.is_err(), "Tampered payload should fail verification");

    let err = result.unwrap_err();
    // Tampering payload bytes may break JSON parsing (InvalidFormat) or
    // produce valid JSON but fail signature check (InvalidSignature). Either is correct.
    assert!(
        matches!(
            err,
            VerificationError::InvalidSignature(_) | VerificationError::InvalidFormat(_)
        ),
        "Expected InvalidSignature or InvalidFormat, got: {err:?}"
    );
}

#[test]
fn rejects_tampered_signature() {
    let factory = TestCredentialFactory::new();
    let issuer_did = factory.ed25519_issuer_did();
    let vp_json = factory.create_entity_vp(&issuer_did);

    let creds = vp_json["verifiableCredential"].as_array().unwrap();
    let jwt = creds[0].as_str().unwrap();

    // Corrupt the signature section
    let parts: Vec<&str> = jwt.split('.').collect();
    let mut sig_chars: Vec<char> = parts[2].chars().collect();
    if let Some(ch) = sig_chars.get_mut(5) {
        *ch = if *ch == 'X' { 'Y' } else { 'X' };
    }
    let corrupted_sig: String = sig_chars.into_iter().collect();
    let tampered_jwt = format!("{}.{}.{}", parts[0], parts[1], corrupted_sig);

    let result = verify_jwt_signature(&tampered_jwt, factory.ed25519_public_key(), "EdDSA");
    assert!(
        result.is_err(),
        "Corrupted signature should fail verification"
    );
}

// =============================================================================
// Issuer Trust Tests
// =============================================================================

#[tokio::test]
async fn accepts_trusted_issuer() {
    let factory = TestCredentialFactory::new();
    let issuer_did = factory.ed25519_issuer_did();
    let registry = build_factory_ed25519_registry(&factory);
    let verifiers = build_verifiers();

    let vp_json = factory.create_entity_vp(&issuer_did);
    let vp: VerifiablePresentation = serde_json::from_value(vp_json).unwrap();

    let results = verifier::verify_presentation(&vp, &registry, &verifiers).await;

    assert_eq!(results.len(), 1);
    let result = results[0].as_ref().expect("Verification should succeed");
    assert!(result.success);
    assert_eq!(result.credential_type, CredentialType::Entity);
}

#[tokio::test]
async fn rejects_untrusted_issuer() {
    let factory = TestCredentialFactory::new();
    // Use a DID that's NOT in the registry
    let untrusted_did = "did:key:z6MkUntrustedIssuerNotInRegistry";
    let registry = build_factory_ed25519_registry(&factory);
    let verifiers = build_verifiers();

    let vp_json = factory.create_entity_vp(untrusted_did);
    let vp: VerifiablePresentation = serde_json::from_value(vp_json).unwrap();

    let results = verifier::verify_presentation(&vp, &registry, &verifiers).await;

    assert_eq!(results.len(), 1);
    let err = results[0].as_ref().unwrap_err();
    assert!(
        matches!(err, VerificationError::UntrustedIssuer(_)),
        "Expected UntrustedIssuer error, got: {err:?}"
    );
}

#[tokio::test]
async fn rejects_expired_issuer_trust() {
    let factory = TestCredentialFactory::new();
    let issuer_did = factory.ed25519_issuer_did();

    // Build a registry with an expired issuer
    let expired_issuer = TrustedIssuer {
        did: issuer_did.clone(),
        name: "Expired Issuer".to_string(),
        credential_types: vec!["entity".to_string()],
        valid_from: DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        valid_until: Some(
            DateTime::parse_from_rfc3339("2021-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        ),
    };
    let registry = build_custom_registry(expired_issuer);
    let verifiers = build_verifiers();

    let vp_json = factory.create_entity_vp(&issuer_did);
    let vp: VerifiablePresentation = serde_json::from_value(vp_json).unwrap();

    let results = verifier::verify_presentation(&vp, &registry, &verifiers).await;

    assert_eq!(results.len(), 1);
    let err = results[0].as_ref().unwrap_err();
    assert!(
        matches!(err, VerificationError::UntrustedIssuer(_)),
        "Expected UntrustedIssuer for expired issuer, got: {err:?}"
    );
}

// =============================================================================
// Claim Extraction Tests
// =============================================================================

#[tokio::test]
async fn extracts_entity_claims() {
    let factory = TestCredentialFactory::new();
    let issuer_did = factory.ed25519_issuer_did();
    let registry = build_factory_ed25519_registry(&factory);
    let verifiers = build_verifiers();

    let vp_json = factory.create_entity_vp(&issuer_did);
    let vp: VerifiablePresentation = serde_json::from_value(vp_json).unwrap();

    let results = verifier::verify_presentation(&vp, &registry, &verifiers).await;
    let result = results[0].as_ref().unwrap();

    assert!(result.success);
    assert_eq!(result.credential_type, CredentialType::Entity);

    let claims = &result.extracted_claims;
    assert_eq!(claims["legal_name"], "Acme Corporation Ltd");
    assert_eq!(claims["registration_number"], "REG-2024-001234");
    assert_eq!(claims["jurisdiction"], "US-DE");
    assert_eq!(claims["entity_type"], "corporation");
}

#[tokio::test]
async fn extracts_signer_claims() {
    let factory = TestCredentialFactory::new();
    let issuer_did = factory.ed25519_issuer_did();
    let registry = build_factory_ed25519_registry(&factory);
    let verifiers = build_verifiers();

    let vp_json = factory.create_signer_vp(&issuer_did);
    let vp: VerifiablePresentation = serde_json::from_value(vp_json).unwrap();

    let results = verifier::verify_presentation(&vp, &registry, &verifiers).await;
    let result = results[0].as_ref().unwrap();

    assert!(result.success);
    assert_eq!(result.credential_type, CredentialType::Signer);

    let claims = &result.extracted_claims;
    assert_eq!(claims["name"], "Jane Smith");
    assert_eq!(claims["title"], "Chief Financial Officer");
    assert_eq!(claims["authority_level"], "executive");
    assert_eq!(claims["signing_capacity"], "unlimited");
}

#[tokio::test]
async fn extracts_region_claims() {
    let factory = TestCredentialFactory::new();
    let issuer_did = factory.ed25519_issuer_did();
    let registry = build_factory_ed25519_registry(&factory);
    let verifiers = build_verifiers();

    let vp_json = factory.create_region_vp(&issuer_did);
    let vp: VerifiablePresentation = serde_json::from_value(vp_json).unwrap();

    let results = verifier::verify_presentation(&vp, &registry, &verifiers).await;
    let result = results[0].as_ref().unwrap();

    assert!(result.success);
    assert_eq!(result.credential_type, CredentialType::Region);

    let claims = &result.extracted_claims;
    assert_eq!(claims["country_code"], "US");
    assert_eq!(claims["region"], "North America");
    assert_eq!(claims["regulatory_framework"], "SEC/FINRA");
    assert_eq!(claims["risk_level"], "low");
}

#[tokio::test]
async fn extracts_wallet_claims() {
    let factory = TestCredentialFactory::new();
    let issuer_did = factory.es256_issuer_did();
    let registry = build_factory_es256_registry(&factory);
    let verifiers = build_verifiers();

    let vp_json = factory.create_wallet_vp(&issuer_did);
    let vp: VerifiablePresentation = serde_json::from_value(vp_json).unwrap();

    let results = verifier::verify_presentation(&vp, &registry, &verifiers).await;
    let result = results[0].as_ref().unwrap();

    assert!(result.success);
    assert_eq!(result.credential_type, CredentialType::Wallet);

    let claims = &result.extracted_claims;
    assert_eq!(claims["wallet_address"], "0x742d35Cc6634C0532925a3b844Bc9e7595f2bD28");
    assert_eq!(claims["chain"], "ethereum");
    assert_eq!(claims["protocol"], "ERC-20");
    assert_eq!(claims["verification_method"], "eip-712-signature");
}

// =============================================================================
// Type Dispatch Tests
// =============================================================================

#[tokio::test]
async fn dispatches_to_correct_verifier() {
    let factory = TestCredentialFactory::new();
    let issuer_did = factory.ed25519_issuer_did();
    let registry = build_factory_ed25519_registry(&factory);
    let verifiers = build_verifiers();

    // Submit an entity VP - should be handled by EntityVerifier
    let vp_json = factory.create_entity_vp(&issuer_did);
    let vp: VerifiablePresentation = serde_json::from_value(vp_json).unwrap();

    let results = verifier::verify_presentation(&vp, &registry, &verifiers).await;
    let result = results[0].as_ref().unwrap();

    // Confirm it was dispatched to EntityVerifier (result type is Entity)
    assert_eq!(result.credential_type, CredentialType::Entity);

    // Submit a region VP - should be handled by RegionVerifier
    let vp_json = factory.create_region_vp(&issuer_did);
    let vp: VerifiablePresentation = serde_json::from_value(vp_json).unwrap();

    let results = verifier::verify_presentation(&vp, &registry, &verifiers).await;
    let result = results[0].as_ref().unwrap();

    assert_eq!(result.credential_type, CredentialType::Region);
}

#[tokio::test]
async fn unknown_type_returns_error() {
    let factory = TestCredentialFactory::new();
    let issuer_did = factory.ed25519_issuer_did();
    let registry = build_factory_ed25519_registry(&factory);
    let verifiers = build_verifiers();

    // Create a VP with an unrecognized credential type
    let vc_payload = serde_json::json!({
        "iss": issuer_did,
        "sub": "did:example:unknown-subject",
        "iat": 1700000000,
        "exp": 1900000000,
        "vc": {
            "@context": ["https://www.w3.org/2018/credentials/v1"],
            "type": ["VerifiableCredential", "UnknownTypeCredential"],
            "issuer": issuer_did,
            "issuanceDate": "2024-01-01T00:00:00Z",
            "credentialSubject": {
                "id": "did:example:unknown",
                "some_field": "some_value"
            }
        }
    });

    let jwt = factory.sign_jwt_ed25519(&vc_payload);
    let vp_json = serde_json::json!({
        "@context": ["https://www.w3.org/2018/credentials/v1"],
        "type": ["VerifiablePresentation"],
        "verifiableCredential": [jwt]
    });

    let vp: VerifiablePresentation = serde_json::from_value(vp_json).unwrap();
    let results = verifier::verify_presentation(&vp, &registry, &verifiers).await;

    assert_eq!(results.len(), 1);
    let err = results[0].as_ref().unwrap_err();
    assert!(
        matches!(err, VerificationError::InvalidFormat(_)),
        "Expected InvalidFormat for unknown type, got: {err:?}"
    );
}

// =============================================================================
// Additional Edge Cases
// =============================================================================

#[test]
fn decode_jwt_with_invalid_format_fails() {
    // JWT with only 2 parts
    let result = decode_jwt_parts("part1.part2");
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(err, VerificationError::InvalidFormat(_)));
}

#[test]
fn extract_vc_from_payload_without_vc_claim_fails() {
    let payload = serde_json::json!({
        "iss": "did:example:test",
        "sub": "did:example:subject"
    });

    let result = extract_vc_from_jwt_payload(&payload);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        VerificationError::InvalidFormat(_)
    ));
}

#[test]
fn determine_credential_type_from_vc() {
    let factory = TestCredentialFactory::new();
    let issuer_did = factory.ed25519_issuer_did();
    let vp_json = factory.create_entity_vp(&issuer_did);
    let creds = vp_json["verifiableCredential"].as_array().unwrap();
    let jwt = creds[0].as_str().unwrap();

    let (_, payload, _) = decode_jwt_parts(jwt).unwrap();
    let vc = extract_vc_from_jwt_payload(&payload).unwrap();
    let cred_type = determine_credential_type(&vc);

    assert_eq!(cred_type, Some(CredentialType::Entity));
}
