use axum::{extract::{Path, State}, Json};
use uuid::Uuid;

use crate::error::AppError;
use crate::AppState;

/// GET /api/cases/:id/evidence — Get evidence chain from TEE
pub async fn get_evidence_chain(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let t3n = state.t3n_client.as_ref().ok_or_else(|| {
        AppError::Internal("T3N client not configured".to_string())
    })?;

    let result = t3n.get_evidence_chain(case_id).await?;
    Ok(Json(result))
}

/// GET /api/cases/:id/violations — Get violations from TEE
pub async fn get_violations(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let t3n = state.t3n_client.as_ref().ok_or_else(|| {
        AppError::Internal("T3N client not configured".to_string())
    })?;

    let result = t3n.get_violations(case_id).await?;
    Ok(Json(result))
}

/// GET /api/cases/:id/plan-status — Get plan status from TEE
pub async fn get_plan_status(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let t3n = state.t3n_client.as_ref().ok_or_else(|| {
        AppError::Internal("T3N client not configured".to_string())
    })?;

    let result = t3n.get_plan_status(case_id).await?;
    Ok(Json(result))
}

/// POST /api/cases/:id/policy — Set compliance policy
pub async fn set_policy(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let t3n = state.t3n_client.as_ref().ok_or_else(|| {
        AppError::Internal("T3N client not configured".to_string())
    })?;

    let policy = &body["policy"];
    if policy.is_null() {
        return Err(AppError::Validation("Missing 'policy' field".to_string()));
    }

    let result = t3n.set_policy(case_id, policy).await?;
    Ok(Json(result))
}

/// POST /api/cases/:id/decide — Trigger TEE decision
pub async fn trigger_decide(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let t3n = state.t3n_client.as_ref().ok_or_else(|| {
        AppError::Internal("T3N client not configured".to_string())
    })?;

    let result = t3n.decide(case_id).await?;
    Ok(Json(result))
}

/// POST /api/cases/:id/protected-action — Execute TEE-gated protected action
pub async fn execute_protected_action(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let t3n = state.t3n_client.as_ref().ok_or_else(|| {
        AppError::Internal("T3N client not configured".to_string())
    })?;

    let action_type = body["action_type"].as_str().ok_or_else(|| {
        AppError::Validation("Missing 'action_type' field".to_string())
    })?;
    let action_payload = &body["action_payload"];

    let result = t3n.execute_protected(case_id, action_type, action_payload).await?;
    Ok(Json(result))
}
