//! REST API endpoints for credential submission and verification.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use serde_json::json;
use uuid::Uuid;

use crate::ai::AssessmentService;
use crate::auth::jwt::Claims;
use crate::credential::normalizer::normalize_verification_result;
use crate::credential::verifier::{self, VerificationError};
use crate::db::{audit_events, cases, disclosed_facts, submissions};
use crate::domain::audit::{AuditEventType, NewAuditEvent};
use crate::domain::credential::VerifiablePresentation;
use crate::domain::submission::{CreateSubmissionRequest, SubmissionStatus};
use crate::domain::types::ActorType;
use crate::error::AppError;
use crate::AppState;

/// POST /api/cases/:id/submissions — Submit a verifiable presentation for verification.
pub async fn submit_presentation(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<CreateSubmissionRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Verify case exists and is in a valid status for submissions
    let case = cases::get_case(&state.pool, case_id).await?;
    let valid_statuses = ["collecting", "verifying"];
    let case_status_str = serde_json::to_value(&case.status)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
        .to_lowercase();

    if !valid_statuses.contains(&case_status_str.as_str()) {
        return Err(AppError::Validation(format!(
            "Case status '{}' does not accept submissions. Must be 'collecting' or 'verifying'.",
            case_status_str
        )));
    }

    // Create submission record with status Submitted
    let submission =
        submissions::create_submission(&state.pool, case_id, &req, &claims.sub).await?;

    // Parse VP from the raw JSON
    let vp: VerifiablePresentation = serde_json::from_value(req.raw_vp.clone()).map_err(|e| {
        AppError::Validation(format!("Invalid Verifiable Presentation format: {e}"))
    })?;

    // Update status to Verifying
    let _ = submissions::update_submission_status(
        &state.pool,
        submission.id,
        SubmissionStatus::Verifying,
        None,
        None,
    )
    .await?;

    // Run verification pipeline
    let results = verifier::verify_presentation(
        &vp,
        &state.issuer_registry,
        &state.credential_verifiers,
    )
    .await;

    // Determine overall outcome
    let mut all_success = true;
    let mut extracted_claims = serde_json::json!([]);
    let mut failure_reasons: Vec<String> = Vec::new();

    for result in &results {
        match result {
            Ok(vr) if vr.success => {
                if let Some(arr) = extracted_claims.as_array_mut() {
                    arr.push(vr.extracted_claims.clone());
                }
            }
            Ok(vr) => {
                all_success = false;
                if let Some(reason) = &vr.failure_reason {
                    failure_reasons.push(reason.clone());
                }
            }
            Err(e) => {
                all_success = false;
                failure_reasons.push(format_verification_error(e));
            }
        }
    }

    // Update submission with final status
    let (final_status, failure_reason_str) = if all_success && !results.is_empty() {
        (SubmissionStatus::Verified, None)
    } else {
        let reason = if results.is_empty() {
            "No verifiable credentials found in presentation".to_string()
        } else {
            failure_reasons.join("; ")
        };
        (SubmissionStatus::Failed, Some(reason))
    };

    let updated = submissions::update_submission_status(
        &state.pool,
        submission.id,
        final_status.clone(),
        if all_success {
            Some(&extracted_claims)
        } else {
            None
        },
        failure_reason_str.as_deref(),
    )
    .await?;

    // After successful verification, normalize and persist DisclosedFacts
    if all_success && !results.is_empty() {
        let mut all_facts = Vec::new();
        for (i, result) in results.iter().enumerate() {
            if let Ok(vr) = result {
                if vr.success {
                    // Use the raw JWT string from the VP for hashing
                    let vp_jwt = vp
                        .verifiable_credential
                        .get(i)
                        .map(|s| s.as_str())
                        .unwrap_or("");

                    let facts = normalize_verification_result(
                        &vr.credential_type,
                        case_id,
                        &req.requirement_claim_type,
                        vr,
                        vp_jwt,
                    );
                    all_facts.extend(facts);
                }
            }
        }

        if !all_facts.is_empty() {
            if let Err(e) = disclosed_facts::insert_disclosed_facts(&state.pool, &all_facts).await
            {
                tracing::error!(
                    case_id = %case_id,
                    error = %e,
                    "Failed to persist disclosed facts"
                );
            }
        }
    }

    // Emit audit event
    let (audit_action, audit_details) = match &final_status {
        SubmissionStatus::Verified => (
            AuditEventType::CREDENTIAL_VERIFIED,
            json!({
                "submission_id": submission.id,
                "credential_type": req.credential_type,
                "requirement_claim_type": req.requirement_claim_type,
                "credentials_verified": results.len(),
            }),
        ),
        _ => (
            AuditEventType::CREDENTIAL_FAILED,
            json!({
                "submission_id": submission.id,
                "credential_type": req.credential_type,
                "requirement_claim_type": req.requirement_claim_type,
                "failure_reason": failure_reason_str,
            }),
        ),
    };

    // Emit audit event in a transaction
    let mut tx = state.pool.begin().await.map_err(|e| {
        AppError::Internal(format!("Failed to begin transaction: {e}"))
    })?;

    let _audit_event = audit_events::insert_audit_event(
        &mut tx,
        &NewAuditEvent {
            case_id,
            actor_type: ActorType::Verifier,
            actor_id: claims.sub.clone(),
            action: audit_action.to_string(),
            details: Some(audit_details.clone()),
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        AppError::Internal(format!("Failed to commit audit event: {e}"))
    })?;

    // T3N integration — verify via TEE, store facts in KV, push audit (fire-and-forget)
    let t3n_execution_id = if let Some(t3n) = &state.t3n_client {
        let t3n = t3n.clone();
        let vp_json = req.raw_vp.clone();
        let details_clone = audit_details.clone();
        let actor_did = state.agent_identity.agent_did.clone();

        // Verify via T3N TEE contract
        let tee_result = t3n.verify_via_tee(&vp_json, case_id).await.ok();
        let exec_id = tee_result.as_ref().map(|r| r.execution_id.clone());

        // Store facts in T3N KV (fire-and-forget)
        if all_success {
            let facts_json = extracted_claims.clone();
            let t3n_kv = t3n.clone();
            tokio::spawn(async move {
                if let Err(e) = t3n_kv.store_facts(case_id, &facts_json).await {
                    tracing::warn!(case_id = %case_id, error = %e, "T3N KV store failed (non-blocking)");
                }
            });
        }

        // Push audit to T3N ledger (fire-and-forget)
        let t3n_audit = t3n.clone();
        tokio::spawn(async move {
            if let Err(e) = t3n_audit.push_audit(case_id, &actor_did, &audit_action.to_string(), &details_clone).await {
                tracing::warn!(case_id = %case_id, error = %e, "T3N audit push failed (non-blocking)");
            }
        });

        exec_id
    } else {
        None
    };

    // Auto-trigger AI assessment after successful verification (fire-and-forget)
    if final_status == SubmissionStatus::Verified {
        let pool = state.pool.clone();
        let llm_client = state.llm_client.clone();
        let req_engine = state.requirement_engine.clone();
        tokio::spawn(async move {
            if let Err(e) = AssessmentService::run_assessment(
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
                    "Auto-triggered assessment failed (non-blocking)"
                );
            }
        });
    }

    // Build response
    let response = json!({
        "data": {
            "submission_id": updated.id,
            "status": updated.status,
            "extracted_claims": updated.extracted_claims,
            "failure_reason": updated.failure_reason,
            "t3n_execution_id": t3n_execution_id,
        },
        "error": null,
        "meta": {
            "case_id": case_id,
        }
    });

    let status_code = if final_status == SubmissionStatus::Verified {
        StatusCode::OK
    } else {
        StatusCode::OK
    };

    Ok((status_code, Json(response)))
}

/// GET /api/cases/:id/submissions — List all submissions for a case.
pub async fn get_case_submissions(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // Verify case exists
    let _case = cases::get_case(&state.pool, case_id).await?;

    let subs = submissions::get_submissions_for_case(&state.pool, case_id).await?;
    let count = subs.len();

    let response = json!({
        "data": subs,
        "error": null,
        "meta": {
            "count": count,
            "case_id": case_id,
        }
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Format a VerificationError into a user-friendly string.
fn format_verification_error(err: &VerificationError) -> String {
    match err {
        VerificationError::InvalidFormat(msg) => format!("Invalid format: {msg}"),
        VerificationError::InvalidSignature(msg) => format!("Invalid signature: {msg}"),
        VerificationError::UntrustedIssuer(did) => format!("Untrusted issuer: {did}"),
        VerificationError::ExpiredCredential => "Credential has expired".to_string(),
        VerificationError::MissingClaims(msg) => format!("Missing claims: {msg}"),
    }
}
