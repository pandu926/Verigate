use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::{Signature as EdSignature, VerifyingKey as EdVerifyingKey};
use p256::ecdsa::{signature::Verifier, Signature as P256Signature, VerifyingKey as P256VerifyingKey};
use p256::EncodedPoint;

pub enum Algorithm {
    EdDSA,
    ES256,
}

pub struct JwtParts {
    pub header: serde_json::Value,
    pub payload: serde_json::Value,
    pub signature: Vec<u8>,
    pub signing_input: Vec<u8>,
}

pub fn decode_jwt_parts(jwt: &str) -> Result<JwtParts, String> {
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err("JWT must have 3 parts".to_string());
    }

    let header_bytes = URL_SAFE_NO_PAD
        .decode(parts[0])
        .map_err(|e| format!("Header base64 decode: {e}"))?;
    let payload_bytes = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|e| format!("Payload base64 decode: {e}"))?;
    let signature = URL_SAFE_NO_PAD
        .decode(parts[2])
        .map_err(|e| format!("Signature base64 decode: {e}"))?;

    let header: serde_json::Value =
        serde_json::from_slice(&header_bytes).map_err(|e| format!("Header JSON: {e}"))?;
    let payload: serde_json::Value =
        serde_json::from_slice(&payload_bytes).map_err(|e| format!("Payload JSON: {e}"))?;

    let signing_input = format!("{}.{}", parts[0], parts[1]).into_bytes();

    Ok(JwtParts {
        header,
        payload,
        signature,
        signing_input,
    })
}

pub fn resolve_did_key(did: &str) -> Result<(Algorithm, Vec<u8>), String> {
    let multibase = did
        .strip_prefix("did:key:z")
        .ok_or_else(|| format!("Not a did:key: {did}"))?;

    let decoded = bs58::decode(multibase)
        .into_vec()
        .map_err(|e| format!("bs58 decode: {e}"))?;

    if decoded.len() < 3 {
        return Err("DID key too short".to_string());
    }

    // Ed25519: multicodec prefix 0xed 0x01
    if decoded[0] == 0xed && decoded[1] == 0x01 {
        return Ok((Algorithm::EdDSA, decoded[2..].to_vec()));
    }

    // P-256: multicodec prefix 0x80 0x24
    if decoded[0] == 0x80 && decoded[1] == 0x24 {
        return Ok((Algorithm::ES256, decoded[2..].to_vec()));
    }

    Err(format!(
        "Unknown multicodec prefix: 0x{:02x}{:02x}",
        decoded[0], decoded[1]
    ))
}

pub fn verify_signature(
    alg: &Algorithm,
    signing_input: &[u8],
    signature_bytes: &[u8],
    public_key_bytes: &[u8],
) -> Result<(), String> {
    match alg {
        Algorithm::EdDSA => verify_ed25519(signing_input, signature_bytes, public_key_bytes),
        Algorithm::ES256 => verify_es256(signing_input, signature_bytes, public_key_bytes),
    }
}

fn verify_ed25519(
    signing_input: &[u8],
    signature_bytes: &[u8],
    public_key_bytes: &[u8],
) -> Result<(), String> {
    let key_bytes: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| format!("Ed25519 key must be 32 bytes, got {}", public_key_bytes.len()))?;

    let verifying_key =
        EdVerifyingKey::from_bytes(&key_bytes).map_err(|e| format!("Ed25519 key invalid: {e}"))?;

    let sig_bytes: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| format!("Ed25519 sig must be 64 bytes, got {}", signature_bytes.len()))?;

    let signature = EdSignature::from_bytes(&sig_bytes);

    verifying_key
        .verify_strict(signing_input, &signature)
        .map_err(|e| format!("Ed25519 signature invalid: {e}"))
}

fn verify_es256(
    signing_input: &[u8],
    signature_bytes: &[u8],
    public_key_bytes: &[u8],
) -> Result<(), String> {
    let point = EncodedPoint::from_bytes(public_key_bytes)
        .map_err(|e| format!("P-256 point invalid: {e}"))?;

    let verifying_key =
        P256VerifyingKey::from_encoded_point(&point).map_err(|e| format!("P-256 key invalid: {e}"))?;

    let signature =
        P256Signature::from_slice(signature_bytes).map_err(|e| format!("P-256 sig invalid: {e}"))?;

    verifying_key
        .verify(signing_input, &signature)
        .map_err(|e| format!("ES256 signature invalid: {e}"))
}
