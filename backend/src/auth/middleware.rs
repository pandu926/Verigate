//! Axum middleware for JWT authentication and Cedar authorization.
//!
//! Extracts the Bearer token from the Authorization header, validates it,
//! then evaluates Cedar policies to determine if the request is authorized.

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::auth::jwt;
use crate::AppState;

/// Default JWT secret used in development when JWT_SECRET is not configured.
pub const DEV_JWT_SECRET: &str = "dev-secret-do-not-use-in-prod";

/// Axum middleware that enforces JWT authentication and Cedar authorization.
///
/// Flow:
/// 1. Extract Bearer token from Authorization header
/// 2. Validate JWT and extract claims
/// 3. Map request method + path to a Cedar action
/// 4. Evaluate Cedar policy engine
/// 5. Allow (insert Claims into extensions) or reject (401/403)
///
/// In dev mode (using default secret), if no Authorization header is present,
/// a default reviewer identity is assumed with a logged warning.
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Extract and validate token
    let claims = match extract_bearer_token(&auth_header) {
        Some(token) => match jwt::validate_token(token, &state.jwt_secret) {
            Ok(claims) => claims,
            Err(_) => {
                return unauthorized_response("Invalid or expired token");
            }
        },
        None => {
            return unauthorized_response("Missing Authorization header");
        }
    };

    // Determine Cedar action from request method + path
    let action = map_request_to_action(request.method(), request.uri().path());

    // Determine resource type and ID
    let (resource_type, resource_id) = map_request_to_resource(request.uri().path());

    // Determine principal type (User or Agent)
    let (principal_type, role_or_type) = if claims.role == "system" {
        ("Agent", "system")
    } else {
        ("User", claims.role.as_str())
    };

    // Evaluate Cedar authorization
    let decision = state.policy_engine.is_authorized(
        principal_type,
        &claims.sub,
        role_or_type,
        &action,
        &resource_type,
        &resource_id,
    );

    if !decision.allowed {
        tracing::info!(
            user = %claims.sub,
            role = %claims.role,
            action = %action,
            resource_type = %resource_type,
            resource_id = %resource_id,
            reason = ?decision.reason,
            "Authorization denied"
        );
        return forbidden_response(
            decision
                .reason
                .as_deref()
                .unwrap_or("Access denied by policy"),
        );
    }

    // Insert claims into request extensions for downstream handlers
    request.extensions_mut().insert(claims);

    next.run(request).await
}

/// Extract bearer token from Authorization header value.
fn extract_bearer_token(header_value: &Option<String>) -> Option<&str> {
    header_value
        .as_deref()
        .and_then(|v| v.strip_prefix("Bearer "))
}

/// Map HTTP method + path to a Cedar action name.
fn map_request_to_action(method: &axum::http::Method, path: &str) -> String {
    match (method.as_str(), path) {
        ("GET", _) => "view".to_string(),
        ("POST", p) if p.contains("/transitions") => "transition".to_string(),
        ("POST", p) if p.contains("/submissions") => "submit_proof".to_string(),
        ("POST", p) if p.contains("/submit_proof") => "submit_proof".to_string(),
        ("POST", p) if p.contains("/override") => "override".to_string(),
        ("POST", _) => "create".to_string(),
        ("PUT", _) | ("PATCH", _) => "transition".to_string(),
        ("DELETE", _) => "override".to_string(),
        _ => "view".to_string(),
    }
}

/// Map request path to a Cedar resource type and ID.
fn map_request_to_resource(path: &str) -> (String, String) {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    // Pattern: /api/cases/:id/... -> Case resource with given ID
    // Pattern: /api/cases -> Application resource (list/create)
    match segments.as_slice() {
        ["api", "cases", id, ..] => ("Case".to_string(), id.to_string()),
        ["api", "cases"] => ("Application".to_string(), "verigate".to_string()),
        _ => ("Application".to_string(), "verigate".to_string()),
    }
}

fn unauthorized_response(message: &str) -> Response {
    let body = json!({
        "data": null,
        "error": message,
        "meta": { "status": 401 }
    });
    (StatusCode::UNAUTHORIZED, axum::Json(body)).into_response()
}

fn forbidden_response(message: &str) -> Response {
    let body = json!({
        "data": null,
        "error": message,
        "meta": { "status": 403 }
    });
    (StatusCode::FORBIDDEN, axum::Json(body)).into_response()
}
