//! Integration tests for SD-JWT parsing and selective claim extraction.
//!
//! Proves that the SD-JWT parser correctly decodes the compact format,
//! extracts only policy-required claims, and drops over-disclosed fields.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::{json, Value};

use verigate_backend::credential::sd_jwt::{decode_sd_jwt, extract_disclosed_claims, SdJwtError};

// =============================================================================
// Helpers
// =============================================================================

/// Encode a disclosure array [salt, claim_name, claim_value] as base64url.
fn encode_disclosure(salt: &str, claim_name: &str, claim_value: &Value) -> String {
    let arr = json!([salt, claim_name, claim_value]);
    URL_SAFE_NO_PAD.encode(arr.to_string().as_bytes())
}

/// Build a compact SD-JWT from JWT + list of encoded disclosure strings.
fn build_sd_jwt(jwt: &str, encoded_disclosures: &[String]) -> String {
    if encoded_disclosures.is_empty() {
        jwt.to_string()
    } else {
        format!("{}~{}~", jwt, encoded_disclosures.join("~"))
    }
}

// =============================================================================
// Decode Tests
// =============================================================================

#[test]
fn decode_sd_jwt_splits_on_tilde() {
    let jwt_part = "eyJhbGciOiJFZERTQSJ9.eyJpc3MiOiJ0ZXN0In0.c2lnbmF0dXJl";
    let d1 = encode_disclosure("salt1", "legal_name", &json!("Acme Corp"));
    let d2 = encode_disclosure("salt2", "jurisdiction", &json!("US"));

    let compact = build_sd_jwt(jwt_part, &[d1, d2]);
    let sd_jwt = decode_sd_jwt(&compact).unwrap();

    assert_eq!(sd_jwt.jwt, jwt_part);
    assert_eq!(sd_jwt.disclosures.len(), 2);
}

#[test]
fn decode_disclosure_parses_json_array() {
    let jwt = "header.payload.sig";
    let d1 = encode_disclosure("random_salt_abc", "legal_name", &json!("Acme Corp"));

    let compact = build_sd_jwt(jwt, &[d1]);
    let sd_jwt = decode_sd_jwt(&compact).unwrap();

    assert_eq!(sd_jwt.disclosures.len(), 1);
    let disclosure = &sd_jwt.disclosures[0];
    assert_eq!(disclosure.salt, "random_salt_abc");
    assert_eq!(disclosure.claim_name, "legal_name");
    assert_eq!(disclosure.claim_value, json!("Acme Corp"));
}

// =============================================================================
// Selective Extraction Tests
// =============================================================================

#[test]
fn extract_disclosed_claims_filters_to_required() {
    let jwt = "eyJhbGciOiJFZERTQSJ9.payload.sig";
    let disclosures = vec![
        encode_disclosure("s1", "legal_name", &json!("Acme Corp")),
        encode_disclosure("s2", "registration_number", &json!("REG-123")),
        encode_disclosure("s3", "jurisdiction", &json!("US")),
        encode_disclosure("s4", "entity_type", &json!("corporation")),
        encode_disclosure("s5", "secret_field", &json!("sensitive_data")),
    ];

    let compact = build_sd_jwt(jwt, &disclosures);
    let sd_jwt = decode_sd_jwt(&compact).unwrap();

    let required = &["legal_name", "jurisdiction"];
    let claims = extract_disclosed_claims(&sd_jwt, required);

    assert_eq!(claims.len(), 2, "Only 2 required claims should be extracted");
    assert_eq!(claims["legal_name"], json!("Acme Corp"));
    assert_eq!(claims["jurisdiction"], json!("US"));
}

#[test]
fn over_disclosure_dropped() {
    let jwt = "eyJhbGciOiJFZERTQSJ9.payload.sig";
    let disclosures = vec![
        encode_disclosure("s1", "legal_name", &json!("Acme Corp")),
        encode_disclosure("s2", "registration_number", &json!("REG-123")),
        encode_disclosure("s3", "jurisdiction", &json!("US")),
        encode_disclosure("s4", "entity_type", &json!("corporation")),
        encode_disclosure("s5", "secret_field", &json!("sensitive_data")),
    ];

    let compact = build_sd_jwt(jwt, &disclosures);
    let sd_jwt = decode_sd_jwt(&compact).unwrap();

    let required = &["legal_name", "jurisdiction"];
    let claims = extract_disclosed_claims(&sd_jwt, required);

    // Over-disclosed fields must NOT be present
    assert!(!claims.contains_key("secret_field"), "secret_field must be dropped");
    assert!(!claims.contains_key("registration_number"), "registration_number must be dropped");
    assert!(!claims.contains_key("entity_type"), "entity_type must be dropped");
}

#[test]
fn empty_required_claims_returns_empty() {
    let jwt = "header.payload.sig";
    let disclosures = vec![
        encode_disclosure("s1", "legal_name", &json!("Acme")),
        encode_disclosure("s2", "ssn", &json!("123-45-6789")),
        encode_disclosure("s3", "dob", &json!("1990-01-01")),
    ];

    let compact = build_sd_jwt(jwt, &disclosures);
    let sd_jwt = decode_sd_jwt(&compact).unwrap();

    let empty: &[&str] = &[];
    let claims = extract_disclosed_claims(&sd_jwt, empty);

    assert!(claims.is_empty(), "Empty required_claims should produce empty output");
}

#[test]
fn missing_required_claim_not_in_output() {
    let jwt = "header.payload.sig";
    let disclosures = vec![
        encode_disclosure("s1", "legal_name", &json!("Acme Corp")),
        encode_disclosure("s2", "jurisdiction", &json!("US")),
    ];

    let compact = build_sd_jwt(jwt, &disclosures);
    let sd_jwt = decode_sd_jwt(&compact).unwrap();

    // Request a claim that doesn't exist in the disclosures
    let required = &["legal_name", "jurisdiction", "nonexistent_claim"];
    let claims = extract_disclosed_claims(&sd_jwt, required);

    // Only the 2 existing claims should be present — no error for missing
    assert_eq!(claims.len(), 2);
    assert!(claims.contains_key("legal_name"));
    assert!(claims.contains_key("jurisdiction"));
    assert!(!claims.contains_key("nonexistent_claim"));
}

#[test]
fn sd_jwt_with_no_disclosures() {
    // JWT only, no tilde separator
    let jwt = "eyJhbGciOiJFZERTQSJ9.payload.signature";
    let sd_jwt = decode_sd_jwt(jwt).unwrap();

    assert_eq!(sd_jwt.jwt, jwt);
    assert!(sd_jwt.disclosures.is_empty(), "No disclosures should be present");

    // Also test with trailing tilde (empty segment ignored)
    let compact_with_tilde = format!("{jwt}~");
    let sd_jwt2 = decode_sd_jwt(&compact_with_tilde).unwrap();
    assert_eq!(sd_jwt2.jwt, jwt);
    assert!(sd_jwt2.disclosures.is_empty());
}

#[test]
fn invalid_disclosure_base64_returns_error() {
    let compact = "header.payload.sig~!!!invalid_base64_garbage!!!~";
    let result = decode_sd_jwt(compact);

    assert!(result.is_err(), "Invalid base64 disclosure should produce error");
    match result.unwrap_err() {
        SdJwtError::Base64Error(_) => {} // Expected
        other => panic!("Expected Base64Error, got: {other:?}"),
    }
}

// =============================================================================
// Complex Scenario Tests
// =============================================================================

#[test]
fn full_sd_jwt_selective_disclosure_workflow() {
    // Simulate a real SD-JWT with many claims, where policy only requires a subset
    let jwt = "eyJhbGciOiJFZERTQSJ9.eyJpc3MiOiJkaWQ6a2V5OnRlc3QifQ.c2ln";

    let disclosures = vec![
        encode_disclosure("salt_a", "legal_name", &json!("Acme Corporation")),
        encode_disclosure("salt_b", "registration_number", &json!("REG-2024-001")),
        encode_disclosure("salt_c", "jurisdiction", &json!("US-DE")),
        encode_disclosure("salt_d", "entity_type", &json!("corporation")),
        encode_disclosure("salt_e", "ssn", &json!("123-45-6789")),
        encode_disclosure("salt_f", "date_of_birth", &json!("1985-03-15")),
        encode_disclosure("salt_g", "internal_score", &json!(95)),
        encode_disclosure("salt_h", "credit_rating", &json!("AAA")),
        encode_disclosure("salt_i", "bank_account", &json!("****1234")),
    ];

    let compact = build_sd_jwt(jwt, &disclosures);
    let sd_jwt = decode_sd_jwt(&compact).unwrap();

    // Policy requires only entity identity fields
    let required = &["legal_name", "registration_number", "jurisdiction", "entity_type"];
    let claims = extract_disclosed_claims(&sd_jwt, required);

    // Exactly 4 required claims extracted
    assert_eq!(claims.len(), 4);
    assert_eq!(claims["legal_name"], json!("Acme Corporation"));
    assert_eq!(claims["registration_number"], json!("REG-2024-001"));
    assert_eq!(claims["jurisdiction"], json!("US-DE"));
    assert_eq!(claims["entity_type"], json!("corporation"));

    // All sensitive over-disclosed fields are dropped
    assert!(!claims.contains_key("ssn"));
    assert!(!claims.contains_key("date_of_birth"));
    assert!(!claims.contains_key("internal_score"));
    assert!(!claims.contains_key("credit_rating"));
    assert!(!claims.contains_key("bank_account"));
}

#[test]
fn sd_jwt_with_numeric_and_object_values() {
    let jwt = "header.payload.sig";
    let disclosures = vec![
        encode_disclosure("s1", "age", &json!(42)),
        encode_disclosure("s2", "verified", &json!(true)),
        encode_disclosure("s3", "address", &json!({"street": "123 Main St", "city": "Wilmington"})),
        encode_disclosure("s4", "score", &json!(0.95)),
    ];

    let compact = build_sd_jwt(jwt, &disclosures);
    let sd_jwt = decode_sd_jwt(&compact).unwrap();

    let required = &["age", "verified", "address", "score"];
    let claims = extract_disclosed_claims(&sd_jwt, required);

    assert_eq!(claims.len(), 4);
    assert_eq!(claims["age"], json!(42));
    assert_eq!(claims["verified"], json!(true));
    assert_eq!(claims["address"], json!({"street": "123 Main St", "city": "Wilmington"}));
    assert_eq!(claims["score"], json!(0.95));
}
