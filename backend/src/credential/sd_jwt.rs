//! SD-JWT (Selective Disclosure JWT) parser per IETF draft.
//!
//! SD-JWT format: `<issuer-jwt>~<disclosure1>~<disclosure2>~...~`
//! Each disclosure is: base64url(json([salt, claim_name, claim_value]))
//!
//! This module decodes the compact representation and extracts only
//! policy-required claims, dropping any over-disclosed fields.

use std::collections::HashMap;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::Value;
use thiserror::Error;

/// Errors that can occur during SD-JWT parsing.
#[derive(Debug, Error)]
pub enum SdJwtError {
    #[error("SD-JWT must contain at least the issuer JWT segment")]
    EmptyInput,

    #[error("Invalid base64url encoding in disclosure: {0}")]
    Base64Error(String),

    #[error("Disclosure is not a valid JSON array: {0}")]
    InvalidJson(String),

    #[error("Disclosure array must have exactly 3 elements [salt, claim_name, claim_value]")]
    InvalidDisclosureFormat,

    #[error("Disclosure claim_name must be a string")]
    InvalidClaimName,
}

/// A parsed SD-JWT containing the issuer JWT and decoded disclosures.
#[derive(Debug, Clone)]
pub struct SdJwt {
    /// The issuer-signed JWT (first segment before any `~`).
    pub jwt: String,
    /// Decoded selective disclosures.
    pub disclosures: Vec<SdJwtDisclosure>,
}

/// A single decoded SD-JWT disclosure.
#[derive(Debug, Clone)]
pub struct SdJwtDisclosure {
    /// Random salt for unlinkability.
    pub salt: String,
    /// The claim name being disclosed.
    pub claim_name: String,
    /// The claim value.
    pub claim_value: Value,
}

/// Decode a compact SD-JWT string into its constituent parts.
///
/// Format: `<issuer-jwt>~<disclosure1>~<disclosure2>~...~`
/// The trailing `~` is optional. Empty segments (from trailing `~`) are ignored.
pub fn decode_sd_jwt(compact: &str) -> Result<SdJwt, SdJwtError> {
    if compact.is_empty() {
        return Err(SdJwtError::EmptyInput);
    }

    let segments: Vec<&str> = compact.split('~').collect();

    if segments.is_empty() || segments[0].is_empty() {
        return Err(SdJwtError::EmptyInput);
    }

    let jwt = segments[0].to_string();

    let mut disclosures = Vec::new();
    for &segment in &segments[1..] {
        if segment.is_empty() {
            continue;
        }
        let disclosure = decode_disclosure(segment)?;
        disclosures.push(disclosure);
    }

    Ok(SdJwt { jwt, disclosures })
}

/// Decode a single base64url-encoded disclosure into its components.
///
/// Expected format after decoding: JSON array `[salt, claim_name, claim_value]`
pub fn decode_disclosure(encoded: &str) -> Result<SdJwtDisclosure, SdJwtError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|e| SdJwtError::Base64Error(e.to_string()))?;

    let arr: Value = serde_json::from_slice(&bytes)
        .map_err(|e| SdJwtError::InvalidJson(e.to_string()))?;

    let elements = arr
        .as_array()
        .ok_or(SdJwtError::InvalidDisclosureFormat)?;

    if elements.len() != 3 {
        return Err(SdJwtError::InvalidDisclosureFormat);
    }

    let salt = match &elements[0] {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };

    let claim_name = elements[1]
        .as_str()
        .ok_or(SdJwtError::InvalidClaimName)?
        .to_string();

    let claim_value = elements[2].clone();

    Ok(SdJwtDisclosure {
        salt,
        claim_name,
        claim_value,
    })
}

/// Extract only the claims whose names appear in `required_claims`.
///
/// This enforces selective disclosure: any over-disclosed fields present in
/// the SD-JWT but not listed in `required_claims` are silently dropped and
/// never persisted to the database.
pub fn extract_disclosed_claims(
    sd_jwt: &SdJwt,
    required_claims: &[&str],
) -> HashMap<String, Value> {
    sd_jwt
        .disclosures
        .iter()
        .filter(|d| required_claims.contains(&d.claim_name.as_str()))
        .map(|d| (d.claim_name.clone(), d.claim_value.clone()))
        .collect()
}

/// Check whether a JWT string contains SD-JWT disclosures (has `~` separator).
pub fn is_sd_jwt(token: &str) -> bool {
    token.contains('~')
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Helper: encode a disclosure array as base64url.
    fn encode_disclosure(salt: &str, claim_name: &str, claim_value: &Value) -> String {
        let arr = json!([salt, claim_name, claim_value]);
        URL_SAFE_NO_PAD.encode(arr.to_string().as_bytes())
    }

    #[test]
    fn decodes_valid_sd_jwt_with_multiple_disclosures() {
        let jwt = "eyJhbGciOiJFZERTQSJ9.eyJpc3MiOiJkaWQ6a2V5Onh5eiJ9.c2lnbmF0dXJl";
        let d1 = encode_disclosure("salt1", "legal_name", &json!("Acme Corp"));
        let d2 = encode_disclosure("salt2", "jurisdiction", &json!("US"));
        let d3 = encode_disclosure("salt3", "extra_field", &json!("should_be_dropped"));

        let compact = format!("{jwt}~{d1}~{d2}~{d3}~");
        let sd_jwt = decode_sd_jwt(&compact).unwrap();

        assert_eq!(sd_jwt.jwt, jwt);
        assert_eq!(sd_jwt.disclosures.len(), 3);
        assert_eq!(sd_jwt.disclosures[0].claim_name, "legal_name");
        assert_eq!(sd_jwt.disclosures[1].claim_name, "jurisdiction");
        assert_eq!(sd_jwt.disclosures[2].claim_name, "extra_field");
    }

    #[test]
    fn extracts_only_required_claims() {
        let jwt = "eyJhbGciOiJFZERTQSJ9.eyJpc3MiOiJkaWQ6a2V5Onh5eiJ9.c2lnbmF0dXJl";
        let d1 = encode_disclosure("s1", "legal_name", &json!("Acme Corp"));
        let d2 = encode_disclosure("s2", "jurisdiction", &json!("US"));
        let d3 = encode_disclosure("s3", "secret_field", &json!("sensitive_data"));
        let d4 = encode_disclosure("s4", "registration_number", &json!("REG-123"));

        let compact = format!("{jwt}~{d1}~{d2}~{d3}~{d4}~");
        let sd_jwt = decode_sd_jwt(&compact).unwrap();

        let required = &["legal_name", "jurisdiction", "registration_number"];
        let claims = extract_disclosed_claims(&sd_jwt, required);

        assert_eq!(claims.len(), 3);
        assert_eq!(claims["legal_name"], json!("Acme Corp"));
        assert_eq!(claims["jurisdiction"], json!("US"));
        assert_eq!(claims["registration_number"], json!("REG-123"));
        // Over-disclosed field is dropped
        assert!(!claims.contains_key("secret_field"));
    }

    #[test]
    fn drops_all_over_disclosed_fields() {
        let jwt = "header.payload.sig";
        let d1 = encode_disclosure("s1", "name", &json!("Alice"));
        let d2 = encode_disclosure("s2", "age", &json!(30));
        let d3 = encode_disclosure("s3", "ssn", &json!("123-45-6789"));

        let compact = format!("{jwt}~{d1}~{d2}~{d3}~");
        let sd_jwt = decode_sd_jwt(&compact).unwrap();

        // Only request "name" — everything else is over-disclosure
        let claims = extract_disclosed_claims(&sd_jwt, &["name"]);
        assert_eq!(claims.len(), 1);
        assert_eq!(claims["name"], json!("Alice"));
    }

    #[test]
    fn handles_sd_jwt_without_trailing_tilde() {
        let jwt = "header.payload.sig";
        let d1 = encode_disclosure("s1", "field", &json!("value"));

        let compact = format!("{jwt}~{d1}");
        let sd_jwt = decode_sd_jwt(&compact).unwrap();

        assert_eq!(sd_jwt.jwt, jwt);
        assert_eq!(sd_jwt.disclosures.len(), 1);
    }

    #[test]
    fn handles_jwt_only_no_disclosures() {
        let jwt = "header.payload.sig";
        let sd_jwt = decode_sd_jwt(jwt).unwrap();

        assert_eq!(sd_jwt.jwt, jwt);
        assert!(sd_jwt.disclosures.is_empty());
    }

    #[test]
    fn rejects_empty_input() {
        assert!(decode_sd_jwt("").is_err());
    }

    #[test]
    fn rejects_invalid_base64_disclosure() {
        let compact = "header.payload.sig~!!!invalid_base64!!!~";
        let result = decode_sd_jwt(compact);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SdJwtError::Base64Error(_)));
    }

    #[test]
    fn rejects_disclosure_with_wrong_element_count() {
        // Only 2 elements instead of 3
        let bad_disclosure = URL_SAFE_NO_PAD.encode(json!(["salt", "name"]).to_string().as_bytes());
        let compact = format!("header.payload.sig~{bad_disclosure}~");
        let result = decode_sd_jwt(&compact);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SdJwtError::InvalidDisclosureFormat
        ));
    }

    #[test]
    fn is_sd_jwt_detects_tilde_separator() {
        assert!(is_sd_jwt("jwt~disclosure1~disclosure2~"));
        assert!(is_sd_jwt("jwt~single~"));
        assert!(!is_sd_jwt("header.payload.signature"));
    }
}
