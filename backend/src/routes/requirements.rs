//! REST API endpoint for case proof requirements.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;
use uuid::Uuid;

use crate::db::{cases, submissions};
use crate::domain::submission::SubmissionStatus;
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
/// Cross-references submissions to determine which requirements are verified.
pub async fn get_requirements(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let case = cases::get_case(&state.pool, id).await?;

    let mut requirements = state.requirement_engine.compute_requirements(
        &case.workflow_type,
        &case.entity_type,
        &case.relationship_goal,
    );

    // Cross-reference with actual submissions to update status
    let case_submissions = submissions::get_submissions_for_case(&state.pool, id)
        .await
        .unwrap_or_default();

    for req in &mut requirements {
        let has_verified = case_submissions.iter().any(|s| {
            s.requirement_claim_type == req.claim_type && s.status == SubmissionStatus::Verified
        });
        if has_verified {
            req.status = "verified".to_string();
        }
    }

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
