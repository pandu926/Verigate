//! Trusted issuer registry for credential verification.
//!
//! Loads a whitelist of trusted issuer DIDs from a JSON configuration file
//! and validates incoming credentials against it.

use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A trusted issuer entry with validity bounds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedIssuer {
    pub did: String,
    pub name: String,
    pub credential_types: Vec<String>,
    pub valid_from: DateTime<Utc>,
    #[serde(default)]
    pub valid_until: Option<DateTime<Utc>>,
}

/// Configuration file format for trusted issuers.
#[derive(Debug, Deserialize)]
struct TrustedIssuersConfig {
    issuers: Vec<TrustedIssuer>,
}

/// Registry that holds trusted issuer DIDs and checks them during verification.
#[derive(Debug, Clone)]
pub struct TrustedIssuerRegistry {
    issuers: HashMap<String, TrustedIssuer>,
}

impl TrustedIssuerRegistry {
    /// Load the registry from a JSON configuration file.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read trusted issuers file: {e}"))?;

        let config: TrustedIssuersConfig = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse trusted issuers JSON: {e}"))?;

        let mut issuers = HashMap::new();
        for issuer in config.issuers {
            issuers.insert(issuer.did.clone(), issuer);
        }

        Ok(Self { issuers })
    }

    /// Create an empty registry (useful for testing).
    pub fn empty() -> Self {
        Self {
            issuers: HashMap::new(),
        }
    }

    /// Check if a DID is trusted for a given credential type at the current time.
    pub fn is_trusted(&self, did: &str, credential_type: &str) -> bool {
        let now = Utc::now();
        match self.issuers.get(did) {
            Some(issuer) => {
                // Check validity window
                if now < issuer.valid_from {
                    return false;
                }
                if let Some(until) = issuer.valid_until {
                    if now > until {
                        return false;
                    }
                }
                // Check credential type support
                issuer.credential_types.contains(&credential_type.to_string())
            }
            None => false,
        }
    }

    /// Get a trusted issuer by DID (for extracting metadata).
    pub fn get_issuer(&self, did: &str) -> Option<&TrustedIssuer> {
        self.issuers.get(did)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_registry() -> TrustedIssuerRegistry {
        let issuer = TrustedIssuer {
            did: "did:key:z6MkTestIssuer1".to_string(),
            name: "Test Issuer".to_string(),
            credential_types: vec!["entity".to_string(), "signer".to_string()],
            valid_from: DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            valid_until: None,
        };

        let mut issuers = HashMap::new();
        issuers.insert(issuer.did.clone(), issuer);

        TrustedIssuerRegistry { issuers }
    }

    #[test]
    fn trusted_issuer_with_valid_type_passes() {
        let registry = test_registry();
        assert!(registry.is_trusted("did:key:z6MkTestIssuer1", "entity"));
        assert!(registry.is_trusted("did:key:z6MkTestIssuer1", "signer"));
    }

    #[test]
    fn untrusted_did_fails() {
        let registry = test_registry();
        assert!(!registry.is_trusted("did:key:z6MkUnknown", "entity"));
    }

    #[test]
    fn unsupported_credential_type_fails() {
        let registry = test_registry();
        assert!(!registry.is_trusted("did:key:z6MkTestIssuer1", "wallet"));
    }
}
