//! Login endpoint with hardcoded demo users for hackathon.
//!
//! Validates email/password against pre-seeded demo accounts and returns a JWT.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use serde::{Deserialize, Serialize};

use crate::auth::jwt;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub role: String,
    pub user_id: String,
    pub expires_in: u64,
}

struct DemoUser {
    email: &'static str,
    password_hash: &'static str,
    role: &'static str,
    user_id: &'static str,
}

const DEMO_USERS: &[DemoUser] = &[
    DemoUser {
        email: "reviewer@verigate.io",
        password_hash: "$argon2id$v=19$m=19456,t=2,p=1$kVsYT7GNN8lb4/MYonLl0g$i4zaxSA/ZmogXa32balGR2hKE4o75oUo4U4RdL5NKns",
        role: "reviewer",
        user_id: "reviewer-001",
    },
    DemoUser {
        email: "counterparty@verigate.io",
        password_hash: "$argon2id$v=19$m=19456,t=2,p=1$HpM4dUkcrQL9sSAH3z1yfA$AF6RdL2RgeHr9j7KSQB0iDcmC/Z1hhDNQWp3Pkz/FGE",
        role: "counterparty",
        user_id: "counterparty-001",
    },
];

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let email = req.email.trim().to_lowercase();

    let user = DEMO_USERS.iter().find(|u| u.email == email);

    let Some(user) = user else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Invalid credentials" })),
        );
    };

    let password_valid = verify_password(&req.password, user.password_hash);

    if !password_valid {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Invalid credentials" })),
        );
    }

    let expires_in = 900u64; // 15 minutes

    match jwt::generate_token(user.user_id, user.role, &state.jwt_secret) {
        Ok(token) => {
            let response = LoginResponse {
                token,
                role: user.role.to_string(),
                user_id: user.user_id.to_string(),
                expires_in,
            };
            (StatusCode::OK, Json(serde_json::to_value(response).unwrap()))
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Token generation failed" })),
        ),
    }
}

fn verify_password(password: &str, hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}
