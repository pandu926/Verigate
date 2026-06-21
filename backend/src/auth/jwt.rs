//! JWT token validation and generation for authentication.
//!
//! Uses HS256 symmetric signing for MVP. Claims include user identity and role.

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// JWT claims embedded in every authenticated request token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject — the user identifier
    pub sub: String,
    /// Role — "reviewer" or "counterparty"
    pub role: String,
    /// Expiration time (UTC timestamp)
    pub exp: usize,
    /// Issued at (UTC timestamp)
    pub iat: usize,
}

/// Validate a JWT token string and extract claims.
///
/// Returns an error if the token is expired, has an invalid signature,
/// or is malformed.
pub fn validate_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let key = DecodingKey::from_secret(secret.as_bytes());
    let validation = Validation::new(jsonwebtoken::Algorithm::HS256);

    let token_data = decode::<Claims>(token, &key, &validation).map_err(|e| {
        AppError::Unauthorized(format!("Invalid token: {e}"))
    })?;

    Ok(token_data.claims)
}

/// Generate a JWT token for testing and development.
///
/// Creates a token with 15-minute expiry using HS256 signing.
pub fn generate_token(user_id: &str, role: &str, secret: &str) -> Result<String, AppError> {
    let now = chrono::Utc::now().timestamp() as usize;
    let expiry = now + (15 * 60); // 15 minutes

    let claims = Claims {
        sub: user_id.to_string(),
        role: role.to_string(),
        exp: expiry,
        iat: now,
    };

    let key = EncodingKey::from_secret(secret.as_bytes());
    let header = Header::new(jsonwebtoken::Algorithm::HS256);

    encode(&header, &claims, &key)
        .map_err(|e| AppError::Internal(format!("Token generation failed: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &str = "test-secret-key-for-unit-tests";

    #[test]
    fn test_generate_and_validate_token() {
        let token = generate_token("user-42", "reviewer", TEST_SECRET)
            .expect("Token generation should succeed");

        let claims = validate_token(&token, TEST_SECRET)
            .expect("Token validation should succeed");

        assert_eq!(claims.sub, "user-42");
        assert_eq!(claims.role, "reviewer");
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_expired_token_rejected() {
        let now = chrono::Utc::now().timestamp() as usize;
        let claims = Claims {
            sub: "user-99".to_string(),
            role: "reviewer".to_string(),
            exp: now.saturating_sub(300), // expired 5 minutes ago (well beyond default leeway)
            iat: now.saturating_sub(600),
        };

        let key = EncodingKey::from_secret(TEST_SECRET.as_bytes());
        let header = Header::new(jsonwebtoken::Algorithm::HS256);
        let token = encode(&header, &claims, &key).expect("encode should work");

        let result = validate_token(&token, TEST_SECRET);
        assert!(result.is_err(), "Expired token should be rejected");
    }

    #[test]
    fn test_invalid_signature_rejected() {
        let token = generate_token("user-42", "reviewer", TEST_SECRET)
            .expect("Token generation should succeed");

        let result = validate_token(&token, "wrong-secret");
        assert!(result.is_err(), "Token with wrong secret should be rejected");
    }

    #[test]
    fn test_malformed_token_rejected() {
        let result = validate_token("not.a.valid.token", TEST_SECRET);
        assert!(result.is_err(), "Malformed token should be rejected");
    }
}
