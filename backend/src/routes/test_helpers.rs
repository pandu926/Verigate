//! Test helper endpoints — only available when TEST_MODE=true.
//!
//! Provides VP generation endpoints for E2E testing without requiring
//! TypeScript crypto implementation to match Rust's deterministic keys.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ring::signature::{Ed25519KeyPair, KeyPair};

use crate::db::disclosed_facts;
use crate::AppState;

/// Known seed bytes — must match the test credential factory.
const ED25519_SEED: [u8; 32] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32,
];

#[derive(Debug, Deserialize)]
pub struct GenerateVpParams {
    /// Credential type: entity, signer, region, wallet
    #[serde(rename = "type")]
    pub credential_type: String,
    /// Optional issuer DID override (defaults to the test key DID)
    pub issuer_did: Option<String>,
    /// Whether to tamper with the signature (for negative tests)
    #[serde(default)]
    pub tamper_signature: bool,
    /// Whether to use an untrusted issuer DID (for negative tests)
    #[serde(default)]
    pub untrusted_issuer: bool,
    /// Whether to generate in SD-JWT format with over-disclosed fields
    #[serde(default)]
    pub sd_jwt: bool,
}

/// GET /api/test/generate-vp — Generate a signed test VP for E2E testing.
pub async fn generate_test_vp(
    Query(params): Query<GenerateVpParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // Check TEST_MODE
    let test_mode = std::env::var("TEST_MODE").unwrap_or_default();
    if test_mode != "true" {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Not found"})),
        ));
    }

    let ed25519_keypair =
        Ed25519KeyPair::from_seed_unchecked(&ED25519_SEED).expect("Valid Ed25519 seed");
    let ed25519_public_key = ed25519_keypair.public_key().as_ref().to_vec();

    // ES256 key
    let es256_secret =
        p256::SecretKey::from_slice(&ED25519_SEED).expect("Valid P-256 secret key bytes");
    let es256_signing_key = p256::ecdsa::SigningKey::from(es256_secret);
    let es256_verifying_key = es256_signing_key.verifying_key();
    let es256_public_key_bytes = es256_verifying_key
        .to_encoded_point(false)
        .as_bytes()
        .to_vec();

    // Compute DIDs
    let ed25519_did = {
        let mut multicodec = vec![0xed, 0x01];
        multicodec.extend_from_slice(&ed25519_public_key);
        let encoded = bs58::encode(&multicodec).into_string();
        format!("did:key:z{encoded}")
    };

    let es256_did = {
        let mut multicodec: Vec<u8> = vec![0x80, 0x24];
        multicodec.extend_from_slice(&es256_public_key_bytes);
        let encoded = bs58::encode(&multicodec).into_string();
        format!("did:key:z{encoded}")
    };

    // Select issuer and algorithm based on credential type
    let (issuer_did, use_es256) = if params.untrusted_issuer {
        ("did:key:z6MkUntrustedIssuerNotInRegistry".to_string(), false)
    } else if let Some(custom_did) = params.issuer_did {
        (custom_did, false)
    } else if params.credential_type == "wallet" {
        (es256_did, true)
    } else {
        (ed25519_did, false)
    };

    // Build VC payload based on type
    let vc_payload = match params.credential_type.as_str() {
        "entity" => json!({
            "iss": issuer_did,
            "sub": "did:example:entity-subject-001",
            "iat": 1700000000,
            "exp": 1900000000,
            "vc": {
                "@context": ["https://www.w3.org/2018/credentials/v1"],
                "type": ["VerifiableCredential", "EntityCredential"],
                "issuer": issuer_did,
                "issuanceDate": "2024-01-01T00:00:00Z",
                "credentialSubject": {
                    "id": "did:example:entity-subject-001",
                    "legal_name": "Acme Corporation Ltd",
                    "registration_number": "REG-2024-001234",
                    "jurisdiction": "US-DE",
                    "entity_type": "corporation"
                }
            }
        }),
        "signer" => json!({
            "iss": issuer_did,
            "sub": "did:example:signer-subject-001",
            "iat": 1700000000,
            "exp": 1900000000,
            "vc": {
                "@context": ["https://www.w3.org/2018/credentials/v1"],
                "type": ["VerifiableCredential", "SignerCredential"],
                "issuer": issuer_did,
                "issuanceDate": "2024-01-01T00:00:00Z",
                "credentialSubject": {
                    "id": "did:example:signer-subject-001",
                    "name": "Jane Smith",
                    "title": "Chief Financial Officer",
                    "authority_level": "executive",
                    "signing_capacity": "unlimited"
                }
            }
        }),
        "region" => json!({
            "iss": issuer_did,
            "sub": "did:example:region-subject-001",
            "iat": 1700000000,
            "exp": 1900000000,
            "vc": {
                "@context": ["https://www.w3.org/2018/credentials/v1"],
                "type": ["VerifiableCredential", "RegionCredential"],
                "issuer": issuer_did,
                "issuanceDate": "2024-01-01T00:00:00Z",
                "credentialSubject": {
                    "id": "did:example:region-subject-001",
                    "country_code": "US",
                    "region": "North America",
                    "regulatory_framework": "SEC/FINRA",
                    "risk_level": "low"
                }
            }
        }),
        "wallet" => json!({
            "iss": issuer_did,
            "sub": "did:example:wallet-subject-001",
            "iat": 1700000000,
            "exp": 1900000000,
            "vc": {
                "@context": ["https://www.w3.org/2018/credentials/v1"],
                "type": ["VerifiableCredential", "WalletCredential"],
                "issuer": issuer_did,
                "issuanceDate": "2024-01-01T00:00:00Z",
                "credentialSubject": {
                    "id": "did:example:wallet-subject-001",
                    "wallet_address": "0x742d35Cc6634C0532925a3b844Bc9e7595f2bD28",
                    "chain": "ethereum",
                    "protocol": "ERC-20",
                    "verification_method": "eip-712-signature"
                }
            }
        }),
        other => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("Unknown credential type: {other}")})),
            ));
        }
    };

    // Sign the JWT
    let jwt = if use_es256 {
        sign_jwt_es256(&es256_signing_key, &vc_payload)
    } else {
        sign_jwt_ed25519(&ed25519_keypair, &vc_payload)
    };

    // Optionally tamper with the signature
    let final_jwt = if params.tamper_signature {
        let parts: Vec<&str> = jwt.split('.').collect();
        let mut sig_chars: Vec<char> = parts[2].chars().collect();
        if let Some(ch) = sig_chars.get_mut(5) {
            *ch = if *ch == 'X' { 'Y' } else { 'X' };
        }
        let corrupted_sig: String = sig_chars.into_iter().collect();
        format!("{}.{}.{}", parts[0], parts[1], corrupted_sig)
    } else {
        jwt
    };

    // If sd_jwt=true, convert to SD-JWT format with over-disclosed fields
    let credential_jwt = if params.sd_jwt {
        convert_to_sd_jwt(&final_jwt, &params.credential_type)
    } else {
        final_jwt
    };

    // Wrap in VP envelope
    let vp = json!({
        "@context": ["https://www.w3.org/2018/credentials/v1"],
        "type": ["VerifiablePresentation"],
        "verifiableCredential": [credential_jwt]
    });

    Ok((StatusCode::OK, Json(json!({
        "data": {
            "vp": vp,
            "issuer_did": issuer_did,
            "credential_type": params.credential_type,
        }
    }))))
}

fn sign_jwt_ed25519(keypair: &Ed25519KeyPair, payload: &serde_json::Value) -> String {
    let header = json!({"alg": "EdDSA", "typ": "JWT"});
    let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).unwrap());
    let payload_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(payload).unwrap());
    let signing_input = format!("{header_b64}.{payload_b64}");

    let signature = keypair.sign(signing_input.as_bytes());
    let sig_b64 = URL_SAFE_NO_PAD.encode(signature.as_ref());

    format!("{signing_input}.{sig_b64}")
}

fn sign_jwt_es256(
    signing_key: &p256::ecdsa::SigningKey,
    payload: &serde_json::Value,
) -> String {
    use p256::ecdsa::{signature::Signer, Signature};

    let header = json!({"alg": "ES256", "typ": "JWT"});
    let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).unwrap());
    let payload_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(payload).unwrap());
    let signing_input = format!("{header_b64}.{payload_b64}");

    let signature: Signature = signing_key.sign(signing_input.as_bytes());
    let sig_b64 = URL_SAFE_NO_PAD.encode(signature.to_bytes());

    format!("{signing_input}.{sig_b64}")
}

/// Convert a standard JWT credential into SD-JWT format with over-disclosed fields.
///
/// Appends ~-delimited base64url disclosures to the JWT. Includes both the
/// legitimate claims from the credential AND extra sensitive fields that should
/// be dropped by the selective disclosure filter.
fn convert_to_sd_jwt(jwt: &str, credential_type: &str) -> String {
    let over_disclosed_fields = vec![
        ("over_salt_1", "internal_score", json!(95)),
        ("over_salt_2", "ssn", json!("123-45-6789")),
        ("over_salt_3", "date_of_birth", json!("1985-03-15")),
    ];

    let legitimate_fields: Vec<(&str, &str, serde_json::Value)> = match credential_type {
        "entity" => vec![
            ("salt_e1", "legal_name", json!("Acme Corporation Ltd")),
            ("salt_e2", "registration_number", json!("REG-2024-001234")),
            ("salt_e3", "jurisdiction", json!("US-DE")),
            ("salt_e4", "entity_type", json!("corporation")),
        ],
        "signer" => vec![
            ("salt_s1", "name", json!("Jane Smith")),
            ("salt_s2", "title", json!("Chief Financial Officer")),
            ("salt_s3", "authority_level", json!("executive")),
            ("salt_s4", "signing_capacity", json!("unlimited")),
        ],
        "region" => vec![
            ("salt_r1", "country_code", json!("US")),
            ("salt_r2", "region", json!("North America")),
            ("salt_r3", "regulatory_framework", json!("SEC/FINRA")),
            ("salt_r4", "risk_level", json!("low")),
        ],
        "wallet" => vec![
            ("salt_w1", "wallet_address", json!("0x742d35Cc6634C0532925a3b844Bc9e7595f2bD28")),
            ("salt_w2", "chain", json!("ethereum")),
            ("salt_w3", "protocol", json!("ERC-20")),
            ("salt_w4", "verification_method", json!("eip-712-signature")),
        ],
        _ => vec![],
    };

    let mut disclosures: Vec<String> = Vec::new();

    // Add legitimate disclosures
    for (salt, name, value) in &legitimate_fields {
        let arr = json!([salt, name, value]);
        let encoded = URL_SAFE_NO_PAD.encode(arr.to_string().as_bytes());
        disclosures.push(encoded);
    }

    // Add over-disclosed fields (these should be filtered out by the system)
    for (salt, name, value) in &over_disclosed_fields {
        let arr = json!([salt, name, value]);
        let encoded = URL_SAFE_NO_PAD.encode(arr.to_string().as_bytes());
        disclosures.push(encoded);
    }

    // SD-JWT format: jwt~disclosure1~disclosure2~...~
    format!("{}~{}~", jwt, disclosures.join("~"))
}

/// GET /api/test/cases/:id/disclosed-facts — Return raw disclosed_facts for a case.
///
/// Test-only endpoint gated by TEST_MODE=true. Returns the raw database rows
/// so E2E tests can verify what was actually persisted without business logic filtering.
pub async fn get_test_disclosed_facts(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // Check TEST_MODE
    let test_mode = std::env::var("TEST_MODE").unwrap_or_default();
    if test_mode != "true" {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Not found"})),
        ));
    }

    let facts = disclosed_facts::get_facts_for_case(&state.pool, case_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to query disclosed facts: {e}")})),
            )
        })?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "data": facts,
            "meta": {
                "case_id": case_id,
                "count": facts.len(),
            }
        })),
    ))
}

/// Generate a demo JWT token for testing.
/// GET /api/test/token?role=reviewer
pub async fn generate_demo_token(
    State(state): State<crate::AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> axum::Json<serde_json::Value> {
    let role = params.get("role").map(|s| s.as_str()).unwrap_or("reviewer");
    let user_id = format!("demo-{}-001", role);

    let token = crate::auth::jwt::generate_token(&user_id, role, &state.jwt_secret)
        .unwrap_or_default();

    axum::Json(serde_json::json!({
        "token": token,
        "role": role,
        "expires_in": 900
    }))
}
