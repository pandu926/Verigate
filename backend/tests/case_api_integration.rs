//! HTTP-level integration tests for the case API and timeline.
//!
//! These tests build the full Axum router and call it via tower::ServiceExt::oneshot,
//! exercising the API at the HTTP transport level. They require a live PostgreSQL
//! database (DATABASE_URL must be set). Marked #[ignore] for CI safety.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use serde_json::Value;
use sqlx::PgPool;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tower::ServiceExt;

use verigate_backend::auth::cedar::PolicyEngine;
use verigate_backend::auth::middleware::DEV_JWT_SECRET;
use verigate_backend::auth::requirements::RequirementEngine;
use verigate_backend::credential::issuer_trust::TrustedIssuerRegistry;
use verigate_backend::credential::verifier::{
    CredentialVerifier, EntityVerifier, RegionVerifier, SignerVerifier, WalletVerifier,
};
use verigate_backend::routes::{cases, timeline};
use verigate_backend::t3::agent_identity::AgentIdentity;
use verigate_backend::AppState;

/// Build a test app with the full router (no auth middleware for these legacy tests).
fn build_test_app(pool: PgPool) -> Router {
    let policies_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("policies");
    let policy_engine = PolicyEngine::from_directory(&policies_dir)
        .expect("PolicyEngine should load for tests");

    let requirements_dir = policies_dir.join("requirements");
    let requirement_engine = RequirementEngine::from_directory(&requirements_dir)
        .expect("RequirementEngine should load for tests");

    let state = AppState {
        pool,
        agent_identity: AgentIdentity {
            agent_did: "did:test:integration".to_string(),
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

    Router::new()
        .route("/api/cases/:id/timeline", get(timeline::get_timeline))
        .merge(cases::router())
        .with_state(state)
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
async fn send_request(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let response = ServiceExt::oneshot(app.clone(), req).await.unwrap();
    let status = response.status();
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap_or(Value::Null);
    (status, body)
}

/// Helper: create a case via HTTP and return its ID.
async fn create_test_case(app: &Router) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/api/cases")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "workflow_type": "Onboarding",
                "entity_type": "Corporation",
                "relationship_goal": "supplier qualification",
                "jurisdiction": "US-DE",
                "requested_outcome": "approved_vendor"
            })
            .to_string(),
        ))
        .unwrap();

    let (status, body) = send_request(app, req).await;
    assert_eq!(status, StatusCode::CREATED);
    body["data"]["id"].as_str().unwrap().to_string()
}

/// Helper: transition a case and return the response.
async fn transition_case(
    app: &Router,
    case_id: &str,
    target_status: &str,
    actor_type: &str,
    actor_id: &str,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/cases/{case_id}/transitions"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "target_status": target_status,
                "actor_type": actor_type,
                "actor_id": actor_id,
                "reason": "integration test"
            })
            .to_string(),
        ))
        .unwrap();

    send_request(app, req).await
}

#[tokio::test]
#[ignore]
async fn test_create_case_returns_201() {
    let pool = test_pool().await;
    let app = build_test_app(pool);

    let req = Request::builder()
        .method("POST")
        .uri("/api/cases")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "workflow_type": "Onboarding",
                "entity_type": "Corporation",
                "relationship_goal": "supplier qualification",
                "jurisdiction": "US-DE",
                "requested_outcome": "approved_vendor"
            })
            .to_string(),
        ))
        .unwrap();

    let (status, body) = send_request(&app, req).await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(body["data"]["id"].is_string());
    assert_eq!(body["data"]["status"].as_str().unwrap(), "Created");
    assert_eq!(
        body["data"]["workflow_type"].as_str().unwrap(),
        "Onboarding"
    );
    assert_eq!(
        body["data"]["entity_type"].as_str().unwrap(),
        "Corporation"
    );
}

#[tokio::test]
#[ignore]
async fn test_create_case_validates_input() {
    let pool = test_pool().await;
    let app = build_test_app(pool);

    let req = Request::builder()
        .method("POST")
        .uri("/api/cases")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "workflow_type": "Onboarding",
                "entity_type": "Corporation",
                "relationship_goal": "",
                "jurisdiction": null,
                "requested_outcome": null
            })
            .to_string(),
        ))
        .unwrap();

    let (status, body) = send_request(&app, req).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["error"].as_str().unwrap().contains("relationship_goal"));
}

#[tokio::test]
#[ignore]
async fn test_get_case_returns_404_for_unknown() {
    let pool = test_pool().await;
    let app = build_test_app(pool);

    let fake_id = "00000000-0000-0000-0000-000000000000";
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/cases/{fake_id}"))
        .body(Body::empty())
        .unwrap();

    let (status, body) = send_request(&app, req).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
#[ignore]
async fn test_transition_valid() {
    let pool = test_pool().await;
    let app = build_test_app(pool);

    let case_id = create_test_case(&app).await;
    let (status, body) = transition_case(&app, &case_id, "Discovery", "Reviewer", "rev-001").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["data"]["case"]["status"].as_str().unwrap(),
        "Discovery"
    );
    assert_eq!(
        body["data"]["event"]["action"].as_str().unwrap(),
        "state_transition"
    );
}

#[tokio::test]
#[ignore]
async fn test_transition_invalid_returns_409() {
    let pool = test_pool().await;
    let app = build_test_app(pool);

    let case_id = create_test_case(&app).await;
    let (status, body) = transition_case(&app, &case_id, "Approved", "Reviewer", "rev-001").await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert!(body["data"]["allowed_transitions"].is_array());
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("Invalid state transition"));
}

#[tokio::test]
#[ignore]
async fn test_timeline_returns_events_in_order() {
    let pool = test_pool().await;
    let app = build_test_app(pool);

    let case_id = create_test_case(&app).await;

    // Transition twice: created -> discovery -> collecting
    transition_case(&app, &case_id, "Discovery", "Reviewer", "rev-001").await;
    transition_case(&app, &case_id, "Collecting", "Reviewer", "rev-001").await;

    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/cases/{case_id}/timeline"))
        .body(Body::empty())
        .unwrap();

    let (status, body) = send_request(&app, req).await;

    assert_eq!(status, StatusCode::OK);

    let events = body["data"].as_array().unwrap();
    // 1 case_created + 2 transitions = 3
    assert_eq!(events.len(), 3);

    // Verify reverse chronological (newest first)
    let ts0 = events[0]["created_at"].as_str().unwrap();
    let ts1 = events[1]["created_at"].as_str().unwrap();
    let ts2 = events[2]["created_at"].as_str().unwrap();
    assert!(ts0 >= ts1);
    assert!(ts1 >= ts2);
}

#[tokio::test]
#[ignore]
async fn test_timeline_cursor_pagination() {
    let pool = test_pool().await;
    let app = build_test_app(pool);

    let case_id = create_test_case(&app).await;

    // Create 5 transitions to get 6 total events (1 create + 5 transitions)
    let steps = ["Discovery", "Collecting", "Verifying", "Assessing", "Review"];
    for step in steps {
        transition_case(&app, &case_id, step, "Reviewer", "rev-001").await;
    }

    // Fetch first page with limit=3
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/cases/{case_id}/timeline?limit=3"))
        .body(Body::empty())
        .unwrap();

    let (status, body) = send_request(&app, req).await;
    assert_eq!(status, StatusCode::OK);

    let events = body["data"].as_array().unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(body["meta"]["has_more"].as_bool().unwrap(), true);
    assert!(body["meta"]["next_cursor"].is_string());

    // Fetch second page using cursor
    let next_cursor = body["meta"]["next_cursor"].as_str().unwrap();
    let req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/cases/{case_id}/timeline?limit=3&cursor={next_cursor}"
        ))
        .body(Body::empty())
        .unwrap();

    let (status, body) = send_request(&app, req).await;
    assert_eq!(status, StatusCode::OK);

    let events2 = body["data"].as_array().unwrap();
    assert_eq!(events2.len(), 3);
}

#[tokio::test]
#[ignore]
async fn test_timeline_actor_type_filter() {
    let pool = test_pool().await;
    let app = build_test_app(pool);

    let case_id = create_test_case(&app).await;

    // The create event is from "System" actor_type.
    // Now transition with "Reviewer" actor_type.
    transition_case(&app, &case_id, "Discovery", "Reviewer", "rev-001").await;

    // Filter for Reviewer only
    let req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/cases/{case_id}/timeline?actor_type=Reviewer"
        ))
        .body(Body::empty())
        .unwrap();

    let (status, body) = send_request(&app, req).await;
    assert_eq!(status, StatusCode::OK);

    let events = body["data"].as_array().unwrap();
    // Only the transition event should be returned (not the System create event)
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["actor_type"].as_str().unwrap(), "Reviewer");
}
