//! REST API endpoints for AI assessments.
//!
//! Trigger assessment runs the full pipeline through T3N TEE (primary)
//! or falls back to local 4-agent pipeline if TEE is unavailable.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::ai::AssessmentService;
use crate::db::assessments;
use crate::domain::assessment::{AssessmentDecision, NewAssessment};
use crate::error::AppError;
use crate::AppState;

/// POST /api/cases/:id/assess — Trigger an AI assessment.
/// Primary path: full orchestration through T3N TEE (delegation + verify + AI + decide).
/// Fallback: local 4-agent pipeline (only if T3N unavailable).
pub async fn trigger_assessment(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let _case = crate::db::cases::get_case(&state.pool, case_id).await?;

    if let Some(ref t3n) = state.t3n_client {
        let credentials = build_credentials_from_submissions(&state.pool, case_id).await;
        let llm_key = state.llm_api_key.clone().unwrap_or_default();

        let pool = state.pool.clone();
        let t3n = t3n.clone();
        tokio::spawn(async move {
            match t3n.orchestrate_full(case_id, &credentials, None, &llm_key).await {
                Ok(result) => {
                    if let Err(e) = persist_tee_assessment(&pool, case_id, &result).await {
                        tracing::error!(case_id = %case_id, error = %e, "Failed to persist TEE assessment");
                    } else {
                        tracing::info!(
                            case_id = %case_id,
                            decision = ?result.get("decision"),
                            confidence = ?result.get("confidence"),
                            "TEE assessment completed and persisted"
                        );
                    }
                }
                Err(e) => {
                    tracing::error!(case_id = %case_id, error = %e, "TEE orchestration failed");
                }
            }
        });

        let response = json!({
            "data": {
                "status": "processing",
                "case_id": case_id,
                "pipeline": "T3N TEE: delegation → verify-credential → assess-risk (AI) → decide",
                "execution_mode": "tee_enclave",
                "tee_state_machine": "orchestrate_full",
            },
            "error": null,
            "meta": { "case_id": case_id }
        });

        Ok((StatusCode::ACCEPTED, Json(response)))
    } else {
        // Fallback: local pipeline (no T3N configured)
        let pool = state.pool.clone();
        let llm_client = state.llm_client.clone();
        let req_engine = state.requirement_engine.clone();

        tokio::spawn(async move {
            match AssessmentService::run_assessment(&pool, llm_client.as_ref(), case_id, &req_engine).await {
                Ok(assessment) => {
                    tracing::info!(case_id = %case_id, decision = ?assessment.decision, "Local assessment completed");
                }
                Err(e) => {
                    tracing::error!(case_id = %case_id, error = %e, "Local assessment failed");
                }
            }
        });

        let response = json!({
            "data": {
                "status": "processing",
                "case_id": case_id,
                "pipeline": "local: planner → interpreter → summarizer → recommender",
                "execution_mode": "local_fallback",
            },
            "error": null,
            "meta": { "case_id": case_id }
        });

        Ok((StatusCode::ACCEPTED, Json(response)))
    }
}

/// Build credential payloads from stored submissions for TEE orchestration.
async fn build_credentials_from_submissions(pool: &PgPool, case_id: Uuid) -> Vec<serde_json::Value> {
    let submissions = crate::db::submissions::get_submissions_for_case(pool, case_id)
        .await
        .unwrap_or_default();

    submissions
        .iter()
        .map(|s| {
            json!({
                "requirement_id": s.requirement_claim_type,
                "vp": s.raw_vp,
                "trusted_issuers": []
            })
        })
        .collect()
}

/// Persist TEE orchestration result as an assessment in the database.
async fn persist_tee_assessment(
    pool: &PgPool,
    case_id: Uuid,
    result: &serde_json::Value,
) -> Result<(), AppError> {
    let decision_str = result["decision"].as_str().unwrap_or("needs_review");
    let confidence = result["confidence"].as_f64().unwrap_or(0.0);

    let reasoning = result["timeline"]
        .as_array()
        .and_then(|t| t.iter().find(|s| s["step"] == "assess-risk"))
        .and_then(|s| s["result"]["reasoning"].as_str())
        .unwrap_or("Assessment completed in TEE");

    let delegation_info = result.get("delegation").cloned().unwrap_or(json!(null));
    let timeline = result.get("timeline").cloned().unwrap_or(json!([]));

    let decision = match decision_str {
        "approved" | "ready" => AssessmentDecision::Ready,
        "blocked" => AssessmentDecision::Blocked,
        "needs_review" => AssessmentDecision::NeedsReview,
        _ => AssessmentDecision::NeedsReview,
    };

    let summary = format!(
        "**TEE Assessment Result**\n\n{}\n\n*Executed inside T3N TEE enclave with delegation credential.*",
        reasoning
    );

    let new_assessment = NewAssessment {
        case_id,
        summary_text: summary,
        decision,
        evidence_links: json!([]),
        confidence,
        agent_outputs: Some(json!({
            "tee_mode": "live",
            "delegation": delegation_info,
            "timeline": timeline,
            "total_elapsed_ms": result.get("total_elapsed_ms"),
        })),
        dynamic_requirements: None,
    };

    assessments::insert_assessment(pool, &new_assessment).await?;

    // Update case status based on decision
    let new_status = match decision_str {
        "approved" | "ready" => "approved",
        "blocked" => "blocked",
        _ => "review",
    };
    crate::db::cases::update_case_status(pool, case_id, new_status).await?;

    Ok(())
}

/// GET /api/cases/:id/assessment — Get the latest assessment for a case.
pub async fn get_assessment(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let _case = crate::db::cases::get_case(&state.pool, case_id).await?;

    let assessment = assessments::get_latest_assessment(&state.pool, case_id).await?;

    let response = json!({
        "data": assessment,
        "error": null,
        "meta": { "case_id": case_id }
    });

    Ok((StatusCode::OK, Json(response)))
}
