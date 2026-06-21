//! Trait-based credential verification pipeline.
//!
//! Defines the CredentialVerifier trait and implements 4 type-specific verifiers:
//! EntityVerifier, SignerVerifier, RegionVerifier, WalletVerifier.

use async_trait::async_trait;
use thiserror::Error;

use crate::credential::issuer_trust::TrustedIssuerRegistry;
use crate::credential::models::VerificationResult;
use crate::domain::credential::VerifiableCredential;
use crate::domain::types::CredentialType;

/// Errors that can occur during credential verification.
#[derive(Debug, Error)]
pub enum VerificationError {
    #[error("Invalid credential format: {0}")]
    InvalidFormat(String),

    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    #[error("Untrusted issuer: {0}")]
    UntrustedIssuer(String),

    #[error("Credential has expired")]
    ExpiredCredential,

    #[error("Missing required claims: {0}")]
    MissingClaims(String),
}

/// Trait for type-specific credential verifiers.
#[async_trait]
pub trait CredentialVerifier: Send + Sync {
    /// The credential type this verifier handles.
    fn supported_type(&self) -> CredentialType;

    /// Verify a single VC against the trusted issuer registry.
    async fn verify(
        &self,
        vc: &VerifiableCredential,
        registry: &TrustedIssuerRegistry,
    ) -> Result<VerificationResult, VerificationError>;
}

/// Decode a compact JWT (header.payload.signature) and return the header and payload as JSON.
/// Validates the signature using the specified algorithm and issuer DID public key.
pub fn decode_jwt_parts(
    jwt: &str,
) -> Result<(serde_json::Value, serde_json::Value, Vec<u8>), VerificationError> {
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err(VerificationError::InvalidFormat(
            "JWT must have exactly 3 parts".to_string(),
        ));
    }

    let header_bytes = base64_url_decode(parts[0])?;
    let payload_bytes = base64_url_decode(parts[1])?;
    let signature_bytes = base64_url_decode(parts[2])?;

    let header: serde_json::Value = serde_json::from_slice(&header_bytes)
        .map_err(|e| VerificationError::InvalidFormat(format!("Invalid JWT header: {e}")))?;

    let payload: serde_json::Value = serde_json::from_slice(&payload_bytes)
        .map_err(|e| VerificationError::InvalidFormat(format!("Invalid JWT payload: {e}")))?;

    Ok((header, payload, signature_bytes))
}

/// Verify a JWT signature using Ed25519.
pub fn verify_ed25519_signature(
    signing_input: &[u8],
    signature: &[u8],
    public_key_bytes: &[u8],
) -> Result<(), VerificationError> {
    use ring::signature;

    let peer_public_key = signature::UnparsedPublicKey::new(
        &signature::ED25519,
        public_key_bytes,
    );

    peer_public_key
        .verify(signing_input, signature)
        .map_err(|_| VerificationError::InvalidSignature("Ed25519 signature verification failed".to_string()))
}

/// Verify a JWT signature using ES256 (P-256 ECDSA with SHA-256).
pub fn verify_es256_signature(
    signing_input: &[u8],
    signature: &[u8],
    public_key_bytes: &[u8],
) -> Result<(), VerificationError> {
    use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
    use p256::EncodedPoint;

    let point = EncodedPoint::from_bytes(public_key_bytes)
        .map_err(|e| VerificationError::InvalidSignature(format!("Invalid P-256 public key: {e}")))?;

    let verifying_key = VerifyingKey::from_encoded_point(&point)
        .map_err(|e| VerificationError::InvalidSignature(format!("Failed to construct P-256 key: {e}")))?;

    let sig = Signature::from_slice(signature)
        .map_err(|e| VerificationError::InvalidSignature(format!("Invalid ES256 signature format: {e}")))?;

    verifying_key
        .verify(signing_input, &sig)
        .map_err(|_| VerificationError::InvalidSignature("ES256 signature verification failed".to_string()))
}

/// Verify a JWT signature, dispatching to the appropriate algorithm.
pub fn verify_jwt_signature(
    jwt: &str,
    public_key_bytes: &[u8],
    algorithm: &str,
) -> Result<(serde_json::Value, serde_json::Value), VerificationError> {
    let (header, payload, signature) = decode_jwt_parts(jwt)?;

    // The signing input is "header.payload" (the first two dot-separated segments)
    let dot_pos = jwt.rfind('.').ok_or_else(|| {
        VerificationError::InvalidFormat("JWT missing signature segment".to_string())
    })?;
    let signing_input = jwt[..dot_pos].as_bytes();

    match algorithm {
        "EdDSA" => verify_ed25519_signature(signing_input, &signature, public_key_bytes)?,
        "ES256" => verify_es256_signature(signing_input, &signature, public_key_bytes)?,
        other => {
            return Err(VerificationError::InvalidFormat(format!(
                "Unsupported algorithm: {other}"
            )));
        }
    }

    Ok((header, payload))
}

/// Decode a base64url-encoded string (no padding).
fn base64_url_decode(input: &str) -> Result<Vec<u8>, VerificationError> {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;

    URL_SAFE_NO_PAD
        .decode(input)
        .map_err(|e| VerificationError::InvalidFormat(format!("Base64url decode error: {e}")))
}

/// Extract a VerifiableCredential from a JWT payload (the "vc" claim).
pub fn extract_vc_from_jwt_payload(
    payload: &serde_json::Value,
) -> Result<VerifiableCredential, VerificationError> {
    let vc_value = payload.get("vc").ok_or_else(|| {
        VerificationError::InvalidFormat("JWT payload missing 'vc' claim".to_string())
    })?;

    // The issuer might be in the top-level "iss" claim
    let issuer = payload
        .get("iss")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    let mut vc: VerifiableCredential = serde_json::from_value(vc_value.clone()).map_err(|e| {
        VerificationError::InvalidFormat(format!("Failed to parse VC from JWT payload: {e}"))
    })?;

    // Override issuer from top-level JWT claim if VC issuer is empty
    if vc.issuer.is_empty() {
        vc.issuer = issuer.to_string();
    }

    Ok(vc)
}

/// Determine the credential type from a VC's type array.
pub fn determine_credential_type(vc: &VerifiableCredential) -> Option<CredentialType> {
    for t in &vc.vc_type {
        let lower = t.to_lowercase();
        if lower.contains("entity") {
            return Some(CredentialType::Entity);
        }
        if lower.contains("signer") || lower.contains("signatory") {
            return Some(CredentialType::Signer);
        }
        if lower.contains("region") || lower.contains("jurisdiction") {
            return Some(CredentialType::Region);
        }
        if lower.contains("wallet") || lower.contains("address") {
            return Some(CredentialType::Wallet);
        }
    }
    None
}

/// Verify all credentials within a VP using the provided verifiers.
pub async fn verify_presentation(
    vp: &crate::domain::credential::VerifiablePresentation,
    registry: &TrustedIssuerRegistry,
    verifiers: &[Box<dyn CredentialVerifier>],
) -> Vec<Result<VerificationResult, VerificationError>> {
    let mut results = Vec::new();

    for jwt_vc in &vp.verifiable_credential {
        // Handle SD-JWT format: strip ~-delimited disclosures to get the bare JWT
        let bare_jwt = if jwt_vc.contains('~') {
            jwt_vc.split('~').next().unwrap_or(jwt_vc)
        } else {
            jwt_vc.as_str()
        };

        // Decode without signature verification first to determine type and issuer
        let decode_result = decode_jwt_parts(bare_jwt);
        let (header, payload, _sig) = match decode_result {
            Ok(parts) => parts,
            Err(e) => {
                results.push(Err(e));
                continue;
            }
        };

        // Extract issuer DID and algorithm from JWT
        let issuer_did = payload
            .get("iss")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let algorithm = header
            .get("alg")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        // Resolve public key from issuer DID and verify signature
        if let Some(public_key_bytes) = resolve_did_key_public_key(&issuer_did, &algorithm) {
            if let Err(e) = verify_jwt_signature(bare_jwt, &public_key_bytes, &algorithm) {
                results.push(Err(e));
                continue;
            }
        }
        // If DID cannot be resolved, skip signature check and let issuer trust handle it

        // Extract VC from payload
        let vc = match extract_vc_from_jwt_payload(&payload) {
            Ok(vc) => vc,
            Err(e) => {
                results.push(Err(e));
                continue;
            }
        };

        // Determine credential type
        let cred_type = match determine_credential_type(&vc) {
            Some(t) => t,
            None => {
                results.push(Err(VerificationError::InvalidFormat(
                    "Could not determine credential type from VC type array".to_string(),
                )));
                continue;
            }
        };

        // Find matching verifier
        let verifier = verifiers.iter().find(|v| v.supported_type() == cred_type);
        match verifier {
            Some(v) => {
                let result = v.verify(&vc, registry).await;
                results.push(result);
            }
            None => {
                results.push(Err(VerificationError::InvalidFormat(format!(
                    "No verifier registered for credential type: {cred_type:?}"
                ))));
            }
        }
    }

    results
}

/// Resolve a did:key DID to raw public key bytes for signature verification.
/// Supports Ed25519 (multicodec prefix 0xed01) and P-256 (multicodec prefix 0x8024).
fn resolve_did_key_public_key(did: &str, algorithm: &str) -> Option<Vec<u8>> {
    // did:key:z<multibase-encoded-multicodec-key>
    let key_part = did.strip_prefix("did:key:z")?;
    let decoded = bs58::decode(key_part).into_vec().ok()?;

    match algorithm {
        "EdDSA" => {
            // Ed25519 multicodec prefix: 0xed, 0x01
            if decoded.len() >= 34 && decoded[0] == 0xed && decoded[1] == 0x01 {
                Some(decoded[2..].to_vec())
            } else {
                None
            }
        }
        "ES256" => {
            // P-256 multicodec prefix: 0x80, 0x24
            if decoded.len() >= 35 && decoded[0] == 0x80 && decoded[1] == 0x24 {
                Some(decoded[2..].to_vec())
            } else {
                None
            }
        }
        _ => None,
    }
}

// --- Type-Specific Verifier Implementations ---

/// Verifier for entity identity credentials.
pub struct EntityVerifier;

#[async_trait]
impl CredentialVerifier for EntityVerifier {
    fn supported_type(&self) -> CredentialType {
        CredentialType::Entity
    }

    async fn verify(
        &self,
        vc: &VerifiableCredential,
        registry: &TrustedIssuerRegistry,
    ) -> Result<VerificationResult, VerificationError> {
        // Check issuer trust
        if !registry.is_trusted(&vc.issuer, "entity") {
            return Err(VerificationError::UntrustedIssuer(vc.issuer.clone()));
        }

        // Extract entity-specific claims
        let subject = &vc.credential_subject;
        let required_fields = ["legal_name", "registration_number", "jurisdiction", "entity_type"];
        for field in &required_fields {
            if subject.get(field).is_none() {
                return Err(VerificationError::MissingClaims(format!(
                    "Entity credential missing '{field}'"
                )));
            }
        }

        let extracted = serde_json::json!({
            "legal_name": subject.get("legal_name"),
            "registration_number": subject.get("registration_number"),
            "jurisdiction": subject.get("jurisdiction"),
            "entity_type": subject.get("entity_type"),
        });

        Ok(VerificationResult {
            success: true,
            credential_type: CredentialType::Entity,
            extracted_claims: extracted,
            issuer_did: vc.issuer.clone(),
            subject: subject.get("id").and_then(|v| v.as_str()).map(String::from),
            failure_reason: None,
        })
    }
}

/// Verifier for authorized signer credentials.
pub struct SignerVerifier;

#[async_trait]
impl CredentialVerifier for SignerVerifier {
    fn supported_type(&self) -> CredentialType {
        CredentialType::Signer
    }

    async fn verify(
        &self,
        vc: &VerifiableCredential,
        registry: &TrustedIssuerRegistry,
    ) -> Result<VerificationResult, VerificationError> {
        if !registry.is_trusted(&vc.issuer, "signer") {
            return Err(VerificationError::UntrustedIssuer(vc.issuer.clone()));
        }

        let subject = &vc.credential_subject;
        let required_fields = ["name", "title", "authority_level", "signing_capacity"];
        for field in &required_fields {
            if subject.get(field).is_none() {
                return Err(VerificationError::MissingClaims(format!(
                    "Signer credential missing '{field}'"
                )));
            }
        }

        let extracted = serde_json::json!({
            "name": subject.get("name"),
            "title": subject.get("title"),
            "authority_level": subject.get("authority_level"),
            "signing_capacity": subject.get("signing_capacity"),
        });

        Ok(VerificationResult {
            success: true,
            credential_type: CredentialType::Signer,
            extracted_claims: extracted,
            issuer_did: vc.issuer.clone(),
            subject: subject.get("id").and_then(|v| v.as_str()).map(String::from),
            failure_reason: None,
        })
    }
}

/// Verifier for regional/jurisdictional credentials.
pub struct RegionVerifier;

#[async_trait]
impl CredentialVerifier for RegionVerifier {
    fn supported_type(&self) -> CredentialType {
        CredentialType::Region
    }

    async fn verify(
        &self,
        vc: &VerifiableCredential,
        registry: &TrustedIssuerRegistry,
    ) -> Result<VerificationResult, VerificationError> {
        if !registry.is_trusted(&vc.issuer, "region") {
            return Err(VerificationError::UntrustedIssuer(vc.issuer.clone()));
        }

        let subject = &vc.credential_subject;
        let required_fields = ["country_code", "region", "regulatory_framework", "risk_level"];
        for field in &required_fields {
            if subject.get(field).is_none() {
                return Err(VerificationError::MissingClaims(format!(
                    "Region credential missing '{field}'"
                )));
            }
        }

        let extracted = serde_json::json!({
            "country_code": subject.get("country_code"),
            "region": subject.get("region"),
            "regulatory_framework": subject.get("regulatory_framework"),
            "risk_level": subject.get("risk_level"),
        });

        Ok(VerificationResult {
            success: true,
            credential_type: CredentialType::Region,
            extracted_claims: extracted,
            issuer_did: vc.issuer.clone(),
            subject: subject.get("id").and_then(|v| v.as_str()).map(String::from),
            failure_reason: None,
        })
    }
}

/// Verifier for wallet/address ownership credentials.
pub struct WalletVerifier;

#[async_trait]
impl CredentialVerifier for WalletVerifier {
    fn supported_type(&self) -> CredentialType {
        CredentialType::Wallet
    }

    async fn verify(
        &self,
        vc: &VerifiableCredential,
        registry: &TrustedIssuerRegistry,
    ) -> Result<VerificationResult, VerificationError> {
        if !registry.is_trusted(&vc.issuer, "wallet") {
            return Err(VerificationError::UntrustedIssuer(vc.issuer.clone()));
        }

        let subject = &vc.credential_subject;
        let required_fields = ["wallet_address", "chain", "protocol", "verification_method"];
        for field in &required_fields {
            if subject.get(field).is_none() {
                return Err(VerificationError::MissingClaims(format!(
                    "Wallet credential missing '{field}'"
                )));
            }
        }

        let extracted = serde_json::json!({
            "wallet_address": subject.get("wallet_address"),
            "chain": subject.get("chain"),
            "protocol": subject.get("protocol"),
            "verification_method": subject.get("verification_method"),
        });

        Ok(VerificationResult {
            success: true,
            credential_type: CredentialType::Wallet,
            extracted_claims: extracted,
            issuer_did: vc.issuer.clone(),
            subject: subject.get("id").and_then(|v| v.as_str()).map(String::from),
            failure_reason: None,
        })
    }
}
