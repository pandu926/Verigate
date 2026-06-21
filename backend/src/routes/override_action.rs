//! Reviewer override endpoint for case decisions.
//!
//! POST /api/cases/:id/override allows a reviewer to override AI recommendations
//! with an explicit rationale. On approval, triggers T3 TEE-protected action execution.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{audit_events, cases};
use crate::domain::audit::NewAuditEvent;
use crate::domain::case::TransitionRequest;
use crate::domain::types::{ActorType, CaseStatus, WorkflowType};
use crate::domain::AuditEventType;
use crate::error::AppError;
use crate::t3::protected_action::{load_template, ActionResult};
use crate::AppState;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

/// The override action a reviewer can take.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OverrideAction {
    Approve,
    Reject,
    Escalate,
}

/// Request body for POST /api/cases/:id/override.
#[derive(Debug, Deserialize)]
pub struct OverrideRequest {
    pub action: OverrideAction,
    pub rationale: String,
}

/// Response from a successful override.
#[derive(Debug, Serialize)]
pub struct OverrideResponse {
    pub case: crate::domain::case::Case,
    pub override_action: OverrideAction,
    pub rationale: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protected_action_result: Option<ActionResult>,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// POST /api/cases/:id/override
///
/// Allows a reviewer to override the AI recommendation for a case.
/// - Approve: transitions to Approved, triggers protected action execution
/// - Reject: transitions to Blocked
/// - Escalate: remains in current state (audit event only)
pub async fn override_decision(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
    Json(req): Json<OverrideRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Validate rationale is non-empty
    let rationale = req.rationale.trim().to_string();
    if rationale.is_empty() {
        return Err(AppError::Validation(
            "rationale must not be empty".to_string(),
        ));
    }

    // Fetch current case — must be in Review or Assessing state
    let current_case = cases::get_case(&state.pool, case_id).await?;
    if current_case.status != CaseStatus::Review && current_case.status != CaseStatus::Assessing {
        return Err(AppError::InvalidTransition {
            current_state: format!("{:?}", current_case.status),
            allowed: vec!["Review".to_string(), "Assessing".to_string()],
        });
    }

    // Determine target status based on action
    let updated_case = match &req.action {
        OverrideAction::Approve => {
            let transition_req = TransitionRequest {
                target_status: CaseStatus::Approved,
                actor_type: ActorType::Reviewer,
                actor_id: "reviewer".to_string(),
                reason: Some(rationale.clone()),
            };
            let (case, _event) =
                cases::transition_case(&state.pool, case_id, &transition_req).await?;
            case
        }
        OverrideAction::Reject => {
            let transition_req = TransitionRequest {
                target_status: CaseStatus::Blocked,
                actor_type: ActorType::Reviewer,
                actor_id: "reviewer".to_string(),
                reason: Some(rationale.clone()),
            };
            let (case, _event) =
                cases::transition_case(&state.pool, case_id, &transition_req).await?;
            case
        }
        OverrideAction::Escalate => {
            // No state transition — just record the escalation in audit
            current_case.clone()
        }
    };

    // Emit override audit event
    {
        let mut tx = state.pool.begin().await?;
        audit_events::insert_audit_event(
            &mut tx,
            &NewAuditEvent {
                case_id,
                actor_type: ActorType::Reviewer,
                actor_id: "reviewer".to_string(),
                action: AuditEventType::OVERRIDE_DECISION.to_string(),
                details: Some(serde_json::json!({
                    "override_action": req.action,
                    "rationale": rationale,
                    "previous_status": format!("{:?}", current_case.status),
                    "new_status": format!("{:?}", updated_case.status),
                })),
            },
        )
        .await?;
        tx.commit().await?;
    }

    // If approved, execute the protected action
    let protected_action_result = if req.action == OverrideAction::Approve {
        Some(
            execute_protected_action(&state, case_id, &current_case.workflow_type).await?,
        )
    } else {
        None
    };

    let response = OverrideResponse {
        case: updated_case,
        override_action: req.action,
        rationale,
        protected_action_result,
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// Protected action execution
// ---------------------------------------------------------------------------

/// Execute the appropriate protected action based on workflow type.
/// Emits a protected_action_executed audit event with placeholder markers
/// (never resolved values).
async fn execute_protected_action(
    state: &AppState,
    case_id: Uuid,
    workflow_type: &WorkflowType,
) -> Result<ActionResult, AppError> {
    // Determine which action template to use based on workflow type
    let action_id = match workflow_type {
        WorkflowType::Onboarding => "issue_onboarding_token",
        WorkflowType::DueDiligence
        | WorkflowType::Compliance
        | WorkflowType::Revalidation => "unlock_deal_room",
    };

    // Load template from config directory
    let templates_dir = std::path::PathBuf::from(
        std::env::var("PROTECTED_ACTIONS_DIR")
            .unwrap_or_else(|_| "config/protected_actions".to_string()),
    );
    let template = load_template(&templates_dir, action_id)?;

    // Execute via the configured executor (dev-mock or real T3 TEE)
    let actor_did = &state.agent_identity.agent_did;
    let result = state
        .protected_action_executor
        .execute(&template, case_id, actor_did)
        .await?;

    // Emit protected action audit event — shows placeholders WITHOUT resolved values
    {
        let mut tx = state.pool.begin().await?;
        audit_events::insert_audit_event(
            &mut tx,
            &NewAuditEvent {
                case_id,
                actor_type: ActorType::ProtectedAction,
                actor_id: format!("executor:{}", state.protected_action_executor.name()),
                action: AuditEventType::PROTECTED_ACTION_EXECUTED.to_string(),
                details: Some(serde_json::json!({
                    "action_id": result.action_id,
                    "template_name": result.template_name,
                    "placeholders_present": result.placeholders_present,
                    "execution_id": result.execution_id,
                    "success": result.success,
                    "error": result.error,
                })),
            },
        )
        .await?;
        tx.commit().await?;
    }

    // Push protected action audit to T3N ledger (fire-and-forget)
    if let Some(t3n) = &state.t3n_client {
        let t3n = t3n.clone();
        let actor_did = state.agent_identity.agent_did.clone();
        let action_details = serde_json::json!({
            "action_id": result.action_id,
            "template_name": result.template_name,
            "placeholders_present": result.placeholders_present,
            "execution_id": result.execution_id,
        });
        tokio::spawn(async move {
            if let Err(e) = t3n.push_audit(case_id, &actor_did, "protected_action_executed", &action_details).await {
                tracing::warn!(case_id = %case_id, error = %e, "T3N audit push for protected action failed");
            }
        });
    }

    Ok(result)
}
