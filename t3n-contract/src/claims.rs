use sha2::{Sha256, Digest};
use serde_json::Value;

const ALLOWED_CLAIM_KEYS: &[&str] = &[
    "entity_name", "legal_name", "jurisdiction", "incorporation_date",
    "registration_number", "entity_type", "country", "region",
    "wallet_address", "chain", "verified", "name", "role",
    "signing_authority", "document_type", "expiry_date",
    "sanctions_clear", "aml_checked", "pep_status",
];

pub struct ExtractedFact {
    pub claim_key: String,
    pub claim_value: String,
}

pub fn extract_claims(credential_subject: &Value) -> Vec<ExtractedFact> {
    let Some(obj) = credential_subject.as_object() else {
        return Vec::new();
    };

    obj.iter()
        .filter(|(key, _)| ALLOWED_CLAIM_KEYS.contains(&key.as_str()))
        .map(|(key, value)| ExtractedFact {
            claim_key: key.clone(),
            claim_value: match value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            },
        })
        .collect()
}

pub fn credential_hash(jwt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(jwt.as_bytes());
    let result = hasher.finalize();
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn strip_sd_jwt(credential: &str) -> &str {
    credential.split('~').next().unwrap_or(credential)
}
