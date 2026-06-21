//! Integration tests for the case lifecycle.
//!
//! These tests require a running PostgreSQL instance with the DATABASE_URL
//! environment variable set. They are marked #[ignore] so `cargo test` does
//! not fail in environments without a live database.

use sqlx::PgPool;
use uuid::Uuid;

use verigate_backend::db::{audit_events, cases};
use verigate_backend::domain::case::{CreateCaseRequest, TransitionRequest};
use verigate_backend::domain::types::{
    ActorType, CaseStatus, EntityType, WorkflowType,
};

/// Helper to create a valid CreateCaseRequest for tests.
fn sample_request() -> CreateCaseRequest {
    CreateCaseRequest {
        workflow_type: WorkflowType::Onboarding,
        entity_type: EntityType::Corporation,
        relationship_goal: "Establish banking relationship".to_string(),
        jurisdiction: Some("US".to_string()),
        requested_outcome: Some("Full onboarding".to_string()),
    }
}

/// Helper to create a transition request.
fn transition_req(target: CaseStatus) -> TransitionRequest {
    TransitionRequest {
        target_status: target,
        actor_type: ActorType::Reviewer,
        actor_id: "reviewer-001".to_string(),
        reason: Some("Test transition".to_string()),
    }
}

/// Connect to the test database.
async fn test_pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set for integration tests");

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("Failed to connect to test database");

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

#[tokio::test]
#[ignore]
async fn create_case_returns_created_status() {
    let pool = test_pool().await;
    let req = sample_request();

    let (case, event) = cases::create_case(&pool, &req, "test-user").await.unwrap();

    assert_eq!(case.status, CaseStatus::Created);
    assert_eq!(case.workflow_type, WorkflowType::Onboarding);
    assert_eq!(case.entity_type, EntityType::Corporation);
    assert_eq!(case.relationship_goal, "Establish banking relationship");
    assert_eq!(case.jurisdiction, Some("US".to_string()));
    assert_eq!(case.created_by, "test-user");
    assert_eq!(event.action, "case_created");
    assert_eq!(event.case_id, case.id);
}

#[tokio::test]
#[ignore]
async fn transition_created_to_discovery_succeeds() {
    let pool = test_pool().await;
    let req = sample_request();
    let (case, _) = cases::create_case(&pool, &req, "test-user").await.unwrap();

    let transition = transition_req(CaseStatus::Discovery);
    let (updated, event) = cases::transition_case(&pool, case.id, &transition)
        .await
        .unwrap();

    assert_eq!(updated.status, CaseStatus::Discovery);
    assert_eq!(event.action, "state_transition");
    assert_eq!(event.actor_type, ActorType::Reviewer);
    assert_eq!(event.actor_id, "reviewer-001");
}

#[tokio::test]
#[ignore]
async fn invalid_transition_created_to_approved_fails() {
    let pool = test_pool().await;
    let req = sample_request();
    let (case, _) = cases::create_case(&pool, &req, "test-user").await.unwrap();

    let transition = transition_req(CaseStatus::Approved);
    let result = cases::transition_case(&pool, case.id, &transition).await;

    assert!(result.is_err());
}

#[tokio::test]
#[ignore]
async fn full_happy_path_lifecycle() {
    let pool = test_pool().await;
    let req = sample_request();
    let (case, _) = cases::create_case(&pool, &req, "test-user").await.unwrap();

    let steps = [
        CaseStatus::Discovery,
        CaseStatus::Collecting,
        CaseStatus::Verifying,
        CaseStatus::Assessing,
        CaseStatus::Review,
        CaseStatus::Approved,
    ];

    let mut current_id = case.id;
    for target in steps {
        let transition = transition_req(target.clone());
        let (updated, event) = cases::transition_case(&pool, current_id, &transition)
            .await
            .unwrap();

        assert_eq!(updated.status, target);
        assert_eq!(event.action, "state_transition");
        current_id = updated.id;
    }
}

#[tokio::test]
#[ignore]
async fn get_events_for_case_returns_reverse_chronological() {
    let pool = test_pool().await;
    let req = sample_request();
    let (case, _) = cases::create_case(&pool, &req, "test-user").await.unwrap();

    // Perform a few transitions to generate events
    let transition1 = transition_req(CaseStatus::Discovery);
    cases::transition_case(&pool, case.id, &transition1)
        .await
        .unwrap();

    let transition2 = transition_req(CaseStatus::Collecting);
    cases::transition_case(&pool, case.id, &transition2)
        .await
        .unwrap();

    let events = audit_events::get_events_for_case(&pool, case.id, 50, None)
        .await
        .unwrap();

    // Should have 3 events: case_created, transition to Discovery, transition to Collecting
    assert_eq!(events.len(), 3);

    // Verify reverse chronological order
    for window in events.windows(2) {
        assert!(window[0].created_at >= window[1].created_at);
    }
}

#[tokio::test]
#[ignore]
async fn get_events_cursor_pagination_works() {
    let pool = test_pool().await;
    let req = sample_request();
    let (case, _) = cases::create_case(&pool, &req, "test-user").await.unwrap();

    let transition1 = transition_req(CaseStatus::Discovery);
    cases::transition_case(&pool, case.id, &transition1)
        .await
        .unwrap();

    let transition2 = transition_req(CaseStatus::Collecting);
    cases::transition_case(&pool, case.id, &transition2)
        .await
        .unwrap();

    // Get first page (limit 2)
    let page1 = audit_events::get_events_for_case(&pool, case.id, 2, None)
        .await
        .unwrap();
    assert_eq!(page1.len(), 2);

    // Get second page using cursor from last item of first page
    let cursor = page1.last().unwrap().created_at;
    let page2 = audit_events::get_events_for_case(&pool, case.id, 2, Some(cursor))
        .await
        .unwrap();
    assert_eq!(page2.len(), 1);
}

#[tokio::test]
#[ignore]
async fn transition_nonexistent_case_returns_not_found() {
    let pool = test_pool().await;
    let fake_id = Uuid::nil();
    let transition = transition_req(CaseStatus::Discovery);

    let result = cases::transition_case(&pool, fake_id, &transition).await;
    assert!(result.is_err());
}
