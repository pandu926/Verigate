//! REST API endpoints for case management.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ai::AssessmentService;
use crate::db::{audit_events, cases};
use crate::domain::case::{CreateCaseRequest, TransitionRequest};
use crate::domain::types::CaseStatus;
use crate::error::AppError;
use crate::AppState;

/// Generic API response envelope.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

/// Query parameters for listing cases.
#[derive(Debug, Deserialize)]
pub struct CaseListParams {
    pub status: Option<CaseStatus>,
}

/// Build the cases router with all endpoints.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/cases", post(create_case).get(list_cases))
        .route("/api/cases/:id", get(get_case))
        .route("/api/cases/:id/transitions", post(transition_case))
}

/// POST /api/cases — Create a new case.
async fn create_case(
    State(state): State<AppState>,
    Json(req): Json<CreateCaseRequest>,
) -> Result<impl IntoResponse, AppError> {
    req.validate().map_err(AppError::Validation)?;

    let (case, _event) = cases::create_case(&state.pool, &req, "system").await?;

    // Auto-transition: Created → Discovery → Collecting (so counterparty can submit immediately)
    let case_id = case.id;
    {
        use crate::domain::case::TransitionRequest;
        use crate::domain::types::{ActorType, CaseStatus};

        let discovery_req = TransitionRequest {
            target_status: CaseStatus::Discovery,
            actor_type: ActorType::System,
            actor_id: "auto-transition".to_string(),
            reason: Some("Auto-transition on creation".to_string()),
        };
        let _ = cases::transition_case(&state.pool, case_id, &discovery_req).await;

        let collecting_req = TransitionRequest {
            target_status: CaseStatus::Collecting,
            actor_type: ActorType::System,
            actor_id: "auto-transition".to_string(),
            reason: Some("Auto-transition on creation".to_string()),
        };
        let _ = cases::transition_case(&state.pool, case_id, &collecting_req).await;
    }

    // Reload case with updated status
    let updated_case = cases::get_case(&state.pool, case_id).await?;

    // Spawn lightweight AI initialization (fire-and-forget, non-blocking)
    {
        let pool = state.pool.clone();
        let llm_client = state.llm_client.clone();
        let req_engine = state.requirement_engine.clone();
        tokio::spawn(async move {
            if let Err(e) = AssessmentService::initialize_case(
                &pool,
                llm_client.as_ref(),
                case_id,
                &req_engine,
            )
            .await
            {
                tracing::warn!(
                    case_id = %case_id,
                    error = %e,
                    "AI case initialization failed (non-blocking)"
                );
            }
        });
    }

    let response = ApiResponse {
        data: updated_case,
        meta: None,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// GET /api/cases — List cases with optional status filter.
async fn list_cases(
    State(state): State<AppState>,
    Query(params): Query<CaseListParams>,
) -> Result<impl IntoResponse, AppError> {
    let cases = cases::list_cases(&state.pool, params.status.as_ref()).await?;
    let count = cases.len();

    let response = ApiResponse {
        data: cases,
        meta: Some(serde_json::json!({ "count": count })),
    };

    Ok((StatusCode::OK, Json(response)))
}

/// GET /api/cases/:id — Get a single case by ID.
async fn get_case(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let case = cases::get_case(&state.pool, id).await?;

    let response = ApiResponse {
        data: case,
        meta: None,
    };

    Ok((StatusCode::OK, Json(response)))
}

/// POST /api/cases/:id/transitions — Transition a case to a new state.
async fn transition_case(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<TransitionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (case, event) = cases::transition_case(&state.pool, id, &req).await?;

    let response = ApiResponse {
        data: serde_json::json!({
            "case": case,
            "event": event,
        }),
        meta: None,
    };

    Ok((StatusCode::OK, Json(response)))
}

/// GET /api/cases/:id/events — Get audit events for a case (for future timeline endpoint).
#[allow(dead_code)]
async fn get_case_events(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let events = audit_events::get_events_for_case(&state.pool, id, 50, None).await?;
    let count = events.len();

    let response = ApiResponse {
        data: events,
        meta: Some(serde_json::json!({ "count": count })),
    };

    Ok((StatusCode::OK, Json(response)))
}
