//! Database operations for cases.

use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::audit::NewAuditEvent;
use crate::domain::case::{Case, CreateCaseRequest, TransitionRequest};
use crate::domain::state_machine;
use crate::domain::types::{ActorType, CaseStatus};
use crate::domain::{AuditEvent, AuditEventType};
use crate::error::AppError;

use super::audit_events::insert_audit_event;

/// Insert a new case and its initial audit event atomically.
pub async fn create_case(
    pool: &PgPool,
    req: &CreateCaseRequest,
    created_by: &str,
) -> Result<(Case, AuditEvent), AppError> {
    let mut tx = pool.begin().await?;

    let case = sqlx::query_as::<_, Case>(
        r#"
        INSERT INTO cases (workflow_type, entity_type, relationship_goal, jurisdiction, requested_outcome, status, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, workflow_type, entity_type, relationship_goal, jurisdiction, requested_outcome, status, created_by, created_at, updated_at
        "#,
    )
    .bind(&req.workflow_type)
    .bind(&req.entity_type)
    .bind(&req.relationship_goal)
    .bind(&req.jurisdiction)
    .bind(&req.requested_outcome)
    .bind(CaseStatus::Created)
    .bind(created_by)
    .fetch_one(&mut *tx)
    .await?;

    let event = insert_audit_event(
        &mut tx,
        &NewAuditEvent {
            case_id: case.id,
            actor_type: ActorType::System,
            actor_id: created_by.to_string(),
            action: AuditEventType::CASE_CREATED.to_string(),
            details: Some(serde_json::json!({
                "workflow_type": req.workflow_type,
                "entity_type": req.entity_type,
                "relationship_goal": req.relationship_goal,
            })),
        },
    )
    .await?;

    tx.commit().await?;

    Ok((case, event))
}

/// Fetch a single case by ID.
pub async fn get_case(pool: &PgPool, id: Uuid) -> Result<Case, AppError> {
    let case = sqlx::query_as::<_, Case>(
        "SELECT id, workflow_type, entity_type, relationship_goal, jurisdiction, requested_outcome, status, created_by, created_at, updated_at FROM cases WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Case {id} not found")))?;

    Ok(case)
}

/// List cases with optional status filter, ordered by creation time descending.
pub async fn list_cases(
    pool: &PgPool,
    status_filter: Option<&CaseStatus>,
) -> Result<Vec<Case>, AppError> {
    let cases = match status_filter {
        Some(status) => {
            sqlx::query_as::<_, Case>(
                "SELECT id, workflow_type, entity_type, relationship_goal, jurisdiction, requested_outcome, status, created_by, created_at, updated_at FROM cases WHERE status = $1 ORDER BY created_at DESC",
            )
            .bind(status)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query_as::<_, Case>(
                "SELECT id, workflow_type, entity_type, relationship_goal, jurisdiction, requested_outcome, status, created_by, created_at, updated_at FROM cases ORDER BY created_at DESC",
            )
            .fetch_all(pool)
            .await?
        }
    };

    Ok(cases)
}

/// Transition a case to a new state atomically with an audit event.
/// Validates the transition via the state machine before writing.
pub async fn transition_case(
    pool: &PgPool,
    id: Uuid,
    req: &TransitionRequest,
) -> Result<(Case, AuditEvent), AppError> {
    let mut tx = pool.begin().await?;

    // Lock the row for update to prevent race conditions
    let current = sqlx::query_as::<_, Case>(
        "SELECT id, workflow_type, entity_type, relationship_goal, jurisdiction, requested_outcome, status, created_by, created_at, updated_at FROM cases WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Case {id} not found")))?;

    // Validate state transition
    state_machine::validate_transition(&current.status, &req.target_status)?;

    // Update case status
    let updated_case = sqlx::query_as::<_, Case>(
        r#"
        UPDATE cases SET status = $1, updated_at = NOW()
        WHERE id = $2
        RETURNING id, workflow_type, entity_type, relationship_goal, jurisdiction, requested_outcome, status, created_by, created_at, updated_at
        "#,
    )
    .bind(&req.target_status)
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;

    // Record audit event in same transaction
    let event = insert_audit_event(
        &mut tx,
        &NewAuditEvent {
            case_id: id,
            actor_type: req.actor_type.clone(),
            actor_id: req.actor_id.clone(),
            action: AuditEventType::STATE_TRANSITION.to_string(),
            details: Some(serde_json::json!({
                "from_status": format!("{:?}", current.status),
                "to_status": format!("{:?}", req.target_status),
                "reason": req.reason,
            })),
        },
    )
    .await?;

    tx.commit().await?;

    Ok((updated_case, event))
}

/// Update case status directly (used by TEE orchestration result).
pub async fn update_case_status(pool: &PgPool, id: Uuid, status: &str) -> Result<(), AppError> {
    sqlx::query("UPDATE cases SET status = $1, updated_at = NOW() WHERE id = $2")
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
