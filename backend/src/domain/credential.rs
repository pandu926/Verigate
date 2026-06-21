//! W3C Verifiable Credentials Data Model 2.0 types.
//!
//! Supports VP/VC with JWT proof format. VCs within a VP are stored as
//! compact JWT strings (base64url header.payload.signature).

use serde::{Deserialize, Serialize};

/// A Verifiable Presentation containing one or more JWT-encoded VCs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiablePresentation {
    #[serde(rename = "@context")]
    pub context: Vec<String>,

    #[serde(rename = "type")]
    pub vp_type: Vec<String>,

    /// JWT-encoded verifiable credentials.
    #[serde(rename = "verifiableCredential")]
    pub verifiable_credential: Vec<String>,

    /// DID of the holder presenting the credentials.
    #[serde(default)]
    pub holder: Option<String>,

    /// Optional proof on the VP envelope itself.
    #[serde(default)]
    pub proof: Option<JwtProof>,
}

/// A Verifiable Credential (decoded from JWT payload).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiableCredential {
    #[serde(rename = "@context")]
    pub context: Vec<String>,

    #[serde(rename = "type")]
    pub vc_type: Vec<String>,

    /// Issuer DID (e.g., "did:key:z6Mk...")
    pub issuer: String,

    /// The claims about the subject.
    #[serde(rename = "credentialSubject")]
    pub credential_subject: serde_json::Value,

    /// ISO 8601 issuance date string.
    #[serde(rename = "issuanceDate")]
    pub issuance_date: String,

    /// Optional ISO 8601 expiration date.
    #[serde(rename = "expirationDate", default)]
    pub expiration_date: Option<String>,

    /// The JWT proof attached to this credential.
    #[serde(default)]
    pub proof: Option<JwtProof>,
}

/// JWT-based proof attached to a VC or VP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtProof {
    /// Proof type identifier (e.g., "JsonWebSignature2020", "JwtProof2020").
    #[serde(rename = "type")]
    pub proof_type: String,

    /// The compact JWT string (header.payload.signature).
    pub jwt: String,

    /// Optional ISO 8601 creation timestamp.
    #[serde(default)]
    pub created: Option<String>,
}
