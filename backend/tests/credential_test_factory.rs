//! Test credential factory for generating valid VPs with known keys.
//!
//! Produces cryptographically signed VPs for all 4 credential types using
//! deterministic Ed25519 and ES256 keypairs. Useful for integration tests
//! and development verification.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ring::signature::{Ed25519KeyPair, KeyPair};
use serde_json::json;

/// Known seed bytes for reproducible Ed25519 test key generation.
/// This produces a deterministic keypair for testing.
const ED25519_SEED: [u8; 32] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32,
];

/// Factory for creating test verifiable presentations with real cryptographic signatures.
pub struct TestCredentialFactory {
    ed25519_keypair: Ed25519KeyPair,
    ed25519_public_key: Vec<u8>,
    es256_signing_key: p256::ecdsa::SigningKey,
    es256_public_key_bytes: Vec<u8>,
}

impl TestCredentialFactory {
    /// Create a new factory with deterministic test keys.
    pub fn new() -> Self {
        // Ed25519 from seed
        let ed25519_keypair =
            Ed25519KeyPair::from_seed_unchecked(&ED25519_SEED).expect("Valid Ed25519 seed");
        let ed25519_public_key = ed25519_keypair.public_key().as_ref().to_vec();

        // ES256 (P-256) from deterministic seed
        let es256_secret = p256::SecretKey::from_slice(&ED25519_SEED)
            .expect("Valid P-256 secret key bytes");
        let es256_signing_key = p256::ecdsa::SigningKey::from(es256_secret);
        let es256_verifying_key = es256_signing_key.verifying_key();
        let es256_public_key_bytes = es256_verifying_key
            .to_encoded_point(false)
            .as_bytes()
            .to_vec();

        Self {
            ed25519_keypair,
            ed25519_public_key,
            es256_signing_key,
            es256_public_key_bytes,
        }
    }

    /// Get the Ed25519 issuer DID (did:key format with multicodec ed25519-pub prefix).
    pub fn ed25519_issuer_did(&self) -> String {
        // multicodec ed25519-pub prefix is 0xed01
        let mut multicodec = vec![0xed, 0x01];
        multicodec.extend_from_slice(&self.ed25519_public_key);
        let encoded = bs58::encode(&multicodec).into_string();
        format!("did:key:z{encoded}")
    }

    /// Get the ES256 issuer DID (did:key format with multicodec p256-pub prefix).
    pub fn es256_issuer_did(&self) -> String {
        // multicodec p256-pub prefix is 0x8024
        let mut multicodec = vec![0x80, 0x24];
        multicodec.extend_from_slice(&self.es256_public_key_bytes);
        let encoded = bs58::encode(&multicodec).into_string();
        format!("did:key:z{encoded}")
    }

    /// Get the raw Ed25519 public key bytes (for signature verification).
    pub fn ed25519_public_key(&self) -> &[u8] {
        &self.ed25519_public_key
    }

    /// Get the raw ES256 public key bytes (uncompressed point, for signature verification).
    pub fn es256_public_key(&self) -> &[u8] {
        &self.es256_public_key_bytes
    }

    /// Create a VP containing an entity credential signed with Ed25519.
    pub fn create_entity_vp(&self, issuer_did: &str) -> serde_json::Value {
        let vc_payload = json!({
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
        });

        let jwt = self.sign_jwt_ed25519(&vc_payload);
        self.wrap_in_vp(vec![jwt])
    }

    /// Create a VP containing a signer credential signed with Ed25519.
    pub fn create_signer_vp(&self, issuer_did: &str) -> serde_json::Value {
        let vc_payload = json!({
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
        });

        let jwt = self.sign_jwt_ed25519(&vc_payload);
        self.wrap_in_vp(vec![jwt])
    }

    /// Create a VP containing a region credential signed with Ed25519.
    pub fn create_region_vp(&self, issuer_did: &str) -> serde_json::Value {
        let vc_payload = json!({
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
        });

        let jwt = self.sign_jwt_ed25519(&vc_payload);
        self.wrap_in_vp(vec![jwt])
    }

    /// Create a VP containing a wallet credential signed with ES256.
    pub fn create_wallet_vp(&self, issuer_did: &str) -> serde_json::Value {
        let vc_payload = json!({
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
        });

        let jwt = self.sign_jwt_es256(&vc_payload);
        self.wrap_in_vp(vec![jwt])
    }

    /// Sign a JWT payload using Ed25519 (EdDSA algorithm).
    pub fn sign_jwt_ed25519(&self, payload: &serde_json::Value) -> String {
        let header = json!({
            "alg": "EdDSA",
            "typ": "JWT"
        });

        let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).unwrap());
        let payload_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(payload).unwrap());
        let signing_input = format!("{header_b64}.{payload_b64}");

        let signature = self.ed25519_keypair.sign(signing_input.as_bytes());
        let sig_b64 = URL_SAFE_NO_PAD.encode(signature.as_ref());

        format!("{signing_input}.{sig_b64}")
    }

    /// Sign a JWT payload using ES256 (P-256 ECDSA with SHA-256).
    pub fn sign_jwt_es256(&self, payload: &serde_json::Value) -> String {
        use p256::ecdsa::{signature::Signer, Signature};

        let header = json!({
            "alg": "ES256",
            "typ": "JWT"
        });

        let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).unwrap());
        let payload_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(payload).unwrap());
        let signing_input = format!("{header_b64}.{payload_b64}");

        let signature: Signature = self.es256_signing_key.sign(signing_input.as_bytes());
        let sig_b64 = URL_SAFE_NO_PAD.encode(signature.to_bytes());

        format!("{signing_input}.{sig_b64}")
    }

    /// Wrap JWT-encoded VCs into a VP envelope.
    fn wrap_in_vp(&self, jwt_credentials: Vec<String>) -> serde_json::Value {
        json!({
            "@context": ["https://www.w3.org/2018/credentials/v1"],
            "type": ["VerifiablePresentation"],
            "verifiableCredential": jwt_credentials
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_creates_valid_entity_vp() {
        let factory = TestCredentialFactory::new();
        let issuer_did = factory.ed25519_issuer_did();
        let vp = factory.create_entity_vp(&issuer_did);

        assert_eq!(vp["type"][0], "VerifiablePresentation");
        let creds = vp["verifiableCredential"].as_array().unwrap();
        assert_eq!(creds.len(), 1);

        // Verify it's a valid JWT (3 dot-separated parts)
        let jwt = creds[0].as_str().unwrap();
        assert_eq!(jwt.split('.').count(), 3);
    }

    #[test]
    fn factory_creates_valid_wallet_vp_with_es256() {
        let factory = TestCredentialFactory::new();
        let issuer_did = factory.es256_issuer_did();
        let vp = factory.create_wallet_vp(&issuer_did);

        let creds = vp["verifiableCredential"].as_array().unwrap();
        let jwt = creds[0].as_str().unwrap();

        // Decode header to verify algorithm
        let header_b64 = jwt.split('.').next().unwrap();
        let header_bytes = URL_SAFE_NO_PAD.decode(header_b64).unwrap();
        let header: serde_json::Value = serde_json::from_slice(&header_bytes).unwrap();
        assert_eq!(header["alg"], "ES256");
    }

    #[test]
    fn ed25519_signature_verification_roundtrip() {
        let factory = TestCredentialFactory::new();
        let issuer_did = factory.ed25519_issuer_did();
        let vp = factory.create_entity_vp(&issuer_did);

        let creds = vp["verifiableCredential"].as_array().unwrap();
        let jwt = creds[0].as_str().unwrap();

        // Verify signature using the public key
        let result = verigate_backend::credential::verifier::verify_jwt_signature(
            jwt,
            factory.ed25519_public_key(),
            "EdDSA",
        );
        assert!(result.is_ok(), "Ed25519 signature should verify: {result:?}");
    }

    #[test]
    fn es256_signature_verification_roundtrip() {
        let factory = TestCredentialFactory::new();
        let issuer_did = factory.es256_issuer_did();
        let vp = factory.create_wallet_vp(&issuer_did);

        let creds = vp["verifiableCredential"].as_array().unwrap();
        let jwt = creds[0].as_str().unwrap();

        // Verify signature using the public key
        let result = verigate_backend::credential::verifier::verify_jwt_signature(
            jwt,
            factory.es256_public_key(),
            "ES256",
        );
        assert!(result.is_ok(), "ES256 signature should verify: {result:?}");
    }

    #[test]
    fn tampered_jwt_fails_verification() {
        let factory = TestCredentialFactory::new();
        let issuer_did = factory.ed25519_issuer_did();
        let vp = factory.create_entity_vp(&issuer_did);

        let creds = vp["verifiableCredential"].as_array().unwrap();
        let jwt = creds[0].as_str().unwrap();

        // Tamper with the payload (change a character in the middle segment)
        let parts: Vec<&str> = jwt.split('.').collect();
        let tampered = format!("{}.{}X.{}", parts[0], parts[1], parts[2]);

        let result = verigate_backend::credential::verifier::verify_jwt_signature(
            &tampered,
            factory.ed25519_public_key(),
            "EdDSA",
        );
        assert!(result.is_err(), "Tampered JWT should fail verification");
    }
}
