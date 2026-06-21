//! REST API endpoint for case proof requirements.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;
use uuid::Uuid;

use crate::db::cases;
use crate::error::AppError;
use crate::AppState;

/// Response envelope for the requirements endpoint.
#[derive(Debug, Serialize)]
pub struct RequirementsResponse<T: Serialize> {
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

/// GET /api/cases/:id/requirements
///
/// Returns the computed list of proof requirements for a case based on its
/// workflow type, entity type, and relationship goal.
pub async fn get_requirements(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // Fetch the case (returns 404 if not found)
    let case = cases::get_case(&state.pool, id).await?;

    // Compute requirements based on case configuration
    let requirements = state.requirement_engine.compute_requirements(
        &case.workflow_type,
        &case.entity_type,
        &case.relationship_goal,
    );

    let count = requirements.len();

    let response = RequirementsResponse {
        data: requirements,
        meta: Some(serde_json::json!({
            "count": count,
            "workflow_type": case.workflow_type,
            "case_id": case.id,
        })),
    };

    Ok((StatusCode::OK, Json(response)))
}
