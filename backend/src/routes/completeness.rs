//! GET /api/cases/:id/completeness — Verification completeness tracking.
//!
//! Computes how many required claims have been verified for a case by
//! comparing disclosed_facts against policy requirement definitions.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use crate::db::{cases, disclosed_facts};
use crate::error::AppError;
use crate::AppState;

/// Per-requirement completeness status.
#[derive(Debug, Serialize)]
pub struct RequirementCompleteness {
    pub requirement_id: String,
    pub claim_type: String,
    pub required_claims_count: usize,
    pub verified_claims_count: i64,
    pub status: String,
}

/// Overall completeness response payload.
#[derive(Debug, Serialize)]
pub struct CompletenessResponse {
    pub total_required: usize,
    pub verified: usize,
    pub pending: usize,
    pub failed: usize,
    pub percentage: u32,
    pub by_requirement: Vec<RequirementCompleteness>,
}

/// GET /api/cases/:id/completeness
///
/// Computes verification completeness by comparing disclosed facts against
/// the policy requirements for the case's workflow type.
pub async fn get_case_completeness(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // Load case to get workflow type, entity type, relationship goal
    let case = cases::get_case(&state.pool, case_id).await?;

    // Compute applicable requirements for this case
    let requirements = state.requirement_engine.compute_requirements(
        &case.workflow_type,
        &case.entity_type,
        &case.relationship_goal,
    );

    // Get fact counts grouped by requirement_id
    let fact_counts = disclosed_facts::count_facts_by_requirement(&state.pool, case_id).await?;
    let fact_count_map: std::collections::HashMap<String, i64> =
        fact_counts.into_iter().collect();

    // Build per-requirement completeness
    let mut by_requirement = Vec::with_capacity(requirements.len());
    let mut total_required: usize = 0;
    let mut verified: usize = 0;

    for req in &requirements {
        // Look up required_claims count from the requirement engine configs
        let required_claims_count = get_required_claims_count(&state, &case.workflow_type, &req.claim_type);
        let verified_count = fact_count_map
            .get(&req.claim_type)
            .copied()
            .unwrap_or(0);

        let status = if verified_count >= required_claims_count as i64 {
            "complete"
        } else if verified_count > 0 {
            "partial"
        } else {
            "missing"
        };

        total_required += required_claims_count;
        if verified_count >= required_claims_count as i64 {
            verified += required_claims_count;
        } else {
            verified += verified_count as usize;
        }

        by_requirement.push(RequirementCompleteness {
            requirement_id: req.claim_type.clone(),
            claim_type: req.claim_type.clone(),
            required_claims_count,
            verified_claims_count: verified_count,
            status: status.to_string(),
        });
    }

    let pending = total_required.saturating_sub(verified);
    let percentage = if total_required > 0 {
        ((verified as f64 / total_required as f64) * 100.0).round() as u32
    } else {
        0
    };

    let completeness = CompletenessResponse {
        total_required,
        verified,
        pending,
        failed: 0,
        percentage,
        by_requirement,
    };

    let response = json!({
        "data": completeness,
        "error": null,
        "meta": {
            "case_id": case_id,
        }
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Look up the required_claims count for a specific claim_type within a workflow.
fn get_required_claims_count(
    state: &AppState,
    workflow_type: &crate::domain::types::WorkflowType,
    claim_type: &str,
) -> usize {
    use crate::domain::types::WorkflowType;

    let workflow_key = match workflow_type {
        WorkflowType::Onboarding => "Onboarding",
        WorkflowType::DueDiligence => "DueDiligence",
        WorkflowType::Compliance => "Compliance",
        WorkflowType::Revalidation => "Revalidation",
    };

    // Access the requirement engine's configs to get required_claims
    state
        .requirement_engine
        .get_required_claims_count(workflow_key, claim_type)
        .unwrap_or(1)
}
