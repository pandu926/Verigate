//! Integration tests for authentication, authorization, and requirements.
//!
//! These tests build the full Axum router with auth middleware and exercise
//! JWT authentication, Cedar authorization, and the requirements endpoint.
//! They require a live PostgreSQL database. Marked #[ignore] for CI safety.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware as axum_middleware;
use axum::routing::get;
use axum::Router;
use serde_json::Value;
use sqlx::PgPool;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tower::ServiceExt;

use verigate_backend::auth::cedar::PolicyEngine;
use verigate_backend::auth::jwt::generate_token;
use verigate_backend::auth::middleware::{auth_middleware, DEV_JWT_SECRET};
use verigate_backend::auth::requirements::RequirementEngine;
use verigate_backend::credential::issuer_trust::TrustedIssuerRegistry;
use verigate_backend::credential::verifier::{
    CredentialVerifier, EntityVerifier, RegionVerifier, SignerVerifier, WalletVerifier,
};
use verigate_backend::routes::{cases, requirements, timeline};
use verigate_backend::t3::agent_identity::AgentIdentity;
use verigate_backend::AppState;

/// Build a test app with the full router including auth middleware.
fn build_auth_app(pool: PgPool) -> Router {
    let policies_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("policies");
    let policy_engine = PolicyEngine::from_directory(&policies_dir)
        .expect("PolicyEngine should load for tests");

    let requirements_dir = policies_dir.join("requirements");
    let requirement_engine = RequirementEngine::from_directory(&requirements_dir)
        .expect("RequirementEngine should load for tests");

    let state = AppState {
        pool,
        agent_identity: AgentIdentity {
            agent_did: "did:test:auth-integration".to_string(),
            authenticated: false,
            sdk_version: "test".to_string(),
            capabilities: vec![],
        },
        start_time: Arc::new(Instant::now()),
        policy_engine: Arc::new(policy_engine),
        requirement_engine: Arc::new(requirement_engine),
        jwt_secret: DEV_JWT_SECRET.to_string(),
        issuer_registry: Arc::new(TrustedIssuerRegistry::empty()),
        credential_verifiers: Arc::new(vec![
            Box::new(EntityVerifier) as Box<dyn CredentialVerifier>,
            Box::new(SignerVerifier),
            Box::new(RegionVerifier),
            Box::new(WalletVerifier),
        ]),
    };

    // Protected routes with auth middleware (mirrors main.rs)
    let protected_routes = Router::new()
        .route("/api/cases/:id/timeline", get(timeline::get_timeline))
        .route(
            "/api/cases/:id/requirements",
            get(requirements::get_requirements),
        )
        .merge(cases::router())
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new().merge(protected_routes).with_state(state)
}

/// Connect to test database and run migrations.
async fn test_pool() -> PgPool {
    let url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("Failed to connect to test database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

/// Helper: send a request and get (status, body JSON).
async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let response = ServiceExt::oneshot(app.clone(), req).await.unwrap();
    let status = response.status();
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap_or(Value::Null);
    (status, body)
}

/// Generate a reviewer JWT token.
fn reviewer_token() -> String {
    generate_token("reviewer-001", "reviewer", DEV_JWT_SECRET)
        .expect("Token generation should succeed")
}

/// Generate a counterparty JWT token.
fn counterparty_token() -> String {
    generate_token("counterparty-001", "counterparty", DEV_JWT_SECRET)
        .expect("Token generation should succeed")
}

// ============================================================================
// Authentication tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_unauthenticated_request_returns_401() {
    let pool = test_pool().await;
    let app = build_auth_app(pool);

    // Use a non-dev secret so the middleware doesn't fall back to dev identity
    // Actually, since we're using DEV_JWT_SECRET, missing auth header gets dev reviewer.
    // Instead, send an invalid token to trigger 401.
    let req = Request::builder()
        .method("GET")
        .uri("/api/cases")
        .header("Authorization", "Bearer invalid-token-here")
        .body(Body::empty())
        .unwrap();

    let (status, body) = send(&app, req).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body["error"].as_str().unwrap().contains("Invalid"));
}

// ============================================================================
// Role enforcement tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_reviewer_can_create_case() {
    let pool = test_pool().await;
    let app = build_auth_app(pool);

    let token = reviewer_token();
    let req = Request::builder()
        .method("POST")
        .uri("/api/cases")
        .header("Authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "workflow_type": "Onboarding",
                "entity_type": "Corporation",
                "relationship_goal": "auth test case"
            })
            .to_string(),
        ))
        .unwrap();

    let (status, body) = send(&app, req).await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(body["data"]["id"].is_string());
}

#[tokio::test]
#[ignore]
async fn test_counterparty_cannot_create_case() {
    let pool = test_pool().await;
    let app = build_auth_app(pool);

    let token = counterparty_token();
    let req = Request::builder()
        .method("POST")
        .uri("/api/cases")
        .header("Authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "workflow_type": "Onboarding",
                "entity_type": "Individual",
                "relationship_goal": "should be denied"
            })
            .to_string(),
        ))
        .unwrap();

    let (status, body) = send(&app, req).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body["error"].is_string());
}

// ============================================================================
// Requirements endpoint tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_reviewer_can_get_requirements() {
    let pool = test_pool().await;
    let app = build_auth_app(pool);

    // Create a case first (as reviewer)
    let token = reviewer_token();
    let create_req = Request::builder()
        .method("POST")
        .uri("/api/cases")
        .header("Authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "workflow_type": "Onboarding",
                "entity_type": "Corporation",
                "relationship_goal": "web3_partner_integration"
            })
            .to_string(),
        ))
        .unwrap();

    let (status, body) = send(&app, create_req).await;
    assert_eq!(status, StatusCode::CREATED);
    let case_id = body["data"]["id"].as_str().unwrap();

    // Now get requirements
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/cases/{case_id}/requirements"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();

    let (status, body) = send(&app, req).await;

    assert_eq!(status, StatusCode::OK);
    let data = body["data"].as_array().unwrap();
    assert!(!data.is_empty());
    assert!(body["meta"]["count"].as_u64().unwrap() > 0);
    assert_eq!(body["meta"]["case_id"].as_str().unwrap(), case_id);
}

#[tokio::test]
#[ignore]
async fn test_requirements_vary_by_workflow() {
    let pool = test_pool().await;
    let app = build_auth_app(pool);
    let token = reviewer_token();

    // Create Onboarding case
    let req = Request::builder()
        .method("POST")
        .uri("/api/cases")
        .header("Authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "workflow_type": "Onboarding",
                "entity_type": "Individual",
                "relationship_goal": "standard_partner"
            })
            .to_string(),
        ))
        .unwrap();
    let (_, body) = send(&app, req).await;
    let case_id_onboarding = body["data"]["id"].as_str().unwrap().to_string();

    // Create Compliance case
    let req = Request::builder()
        .method("POST")
        .uri("/api/cases")
        .header("Authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "workflow_type": "Compliance",
                "entity_type": "Individual",
                "relationship_goal": "standard_partner"
            })
            .to_string(),
        ))
        .unwrap();
    let (_, body) = send(&app, req).await;
    let case_id_compliance = body["data"]["id"].as_str().unwrap().to_string();

    // Get requirements for both
    let req1 = Request::builder()
        .method("GET")
        .uri(format!("/api/cases/{case_id_onboarding}/requirements"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let (_, body1) = send(&app, req1).await;

    let req2 = Request::builder()
        .method("GET")
        .uri(format!("/api/cases/{case_id_compliance}/requirements"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let (_, body2) = send(&app, req2).await;

    let reqs1 = body1["data"].as_array().unwrap();
    let reqs2 = body2["data"].as_array().unwrap();

    // Different workflow types produce different requirement sets
    assert_ne!(reqs1.len(), reqs2.len());

    // Onboarding individual standard has 3; Compliance always has 4
    assert_eq!(reqs1.len(), 3);
    assert_eq!(reqs2.len(), 4);
}

#[tokio::test]
#[ignore]
async fn test_conditional_requirements_apply() {
    let pool = test_pool().await;
    let app = build_auth_app(pool);
    let token = reviewer_token();

    // Case with web3 relationship_goal and Corporation entity
    let req = Request::builder()
        .method("POST")
        .uri("/api/cases")
        .header("Authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "workflow_type": "Onboarding",
                "entity_type": "Corporation",
                "relationship_goal": "web3_partner_integration"
            })
            .to_string(),
        ))
        .unwrap();
    let (_, body) = send(&app, req).await;
    let case_id_web3 = body["data"]["id"].as_str().unwrap().to_string();

    // Case without web3 and Individual entity
    let req = Request::builder()
        .method("POST")
        .uri("/api/cases")
        .header("Authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "workflow_type": "Onboarding",
                "entity_type": "Individual",
                "relationship_goal": "standard_partner"
            })
            .to_string(),
        ))
        .unwrap();
    let (_, body) = send(&app, req).await;
    let case_id_standard = body["data"]["id"].as_str().unwrap().to_string();

    // Get requirements for web3 case
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/cases/{case_id_web3}/requirements"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let (_, body) = send(&app, req).await;
    let web3_reqs = body["data"].as_array().unwrap();
    let web3_claims: Vec<&str> = web3_reqs
        .iter()
        .map(|r| r["claim_type"].as_str().unwrap())
        .collect();

    assert!(web3_claims.contains(&"wallet_proof"), "web3 case should have wallet_proof");
    assert!(
        web3_claims.contains(&"beneficial_ownership"),
        "Corporation case should have beneficial_ownership"
    );

    // Get requirements for standard case
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/cases/{case_id_standard}/requirements"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let (_, body) = send(&app, req).await;
    let std_reqs = body["data"].as_array().unwrap();
    let std_claims: Vec<&str> = std_reqs
        .iter()
        .map(|r| r["claim_type"].as_str().unwrap())
        .collect();

    assert!(
        !std_claims.contains(&"wallet_proof"),
        "standard case should NOT have wallet_proof"
    );
    assert!(
        !std_claims.contains(&"beneficial_ownership"),
        "Individual case should NOT have beneficial_ownership"
    );
    assert!(
        std_claims.contains(&"entity_registration"),
        "All cases should have entity_registration"
    );
}
