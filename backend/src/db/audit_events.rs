//! Database operations for audit events.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::audit::{AuditEvent, NewAuditEvent};
use crate::domain::types::ActorType;
use crate::error::AppError;

/// Insert a new audit event within an existing transaction.
pub async fn insert_audit_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event: &NewAuditEvent,
) -> Result<AuditEvent, AppError> {
    let row = sqlx::query_as::<_, AuditEvent>(
        r#"
        INSERT INTO audit_events (case_id, actor_type, actor_id, action, details)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, case_id, actor_type, actor_id, action, details, created_at
        "#,
    )
    .bind(event.case_id)
    .bind(&event.actor_type)
    .bind(&event.actor_id)
    .bind(&event.action)
    .bind(&event.details)
    .fetch_one(&mut **tx)
    .await?;

    Ok(row)
}

/// Fetch audit events for a case with cursor-based pagination (reverse chronological).
/// Supports optional actor_type filtering.
pub async fn get_events_for_case(
    pool: &PgPool,
    case_id: Uuid,
    limit: i64,
    cursor: Option<DateTime<Utc>>,
) -> Result<Vec<AuditEvent>, AppError> {
    get_events_for_case_filtered(pool, case_id, limit, cursor, None).await
}

/// Fetch audit events for a case with cursor-based pagination and optional actor_type filter.
pub async fn get_events_for_case_filtered(
    pool: &PgPool,
    case_id: Uuid,
    limit: i64,
    cursor: Option<DateTime<Utc>>,
    actor_type_filter: Option<&ActorType>,
) -> Result<Vec<AuditEvent>, AppError> {
    let events = match (cursor, actor_type_filter) {
        (Some(before), Some(actor_type)) => {
            sqlx::query_as::<_, AuditEvent>(
                r#"
                SELECT id, case_id, actor_type, actor_id, action, details, created_at
                FROM audit_events
                WHERE case_id = $1 AND created_at < $2 AND actor_type = $3
                ORDER BY created_at DESC
                LIMIT $4
                "#,
            )
            .bind(case_id)
            .bind(before)
            .bind(actor_type)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        (Some(before), None) => {
            sqlx::query_as::<_, AuditEvent>(
                r#"
                SELECT id, case_id, actor_type, actor_id, action, details, created_at
                FROM audit_events
                WHERE case_id = $1 AND created_at < $2
                ORDER BY created_at DESC
                LIMIT $3
                "#,
            )
            .bind(case_id)
            .bind(before)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        (None, Some(actor_type)) => {
            sqlx::query_as::<_, AuditEvent>(
                r#"
                SELECT id, case_id, actor_type, actor_id, action, details, created_at
                FROM audit_events
                WHERE case_id = $1 AND actor_type = $2
                ORDER BY created_at DESC
                LIMIT $3
                "#,
            )
            .bind(case_id)
            .bind(actor_type)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        (None, None) => {
            sqlx::query_as::<_, AuditEvent>(
                r#"
                SELECT id, case_id, actor_type, actor_id, action, details, created_at
                FROM audit_events
                WHERE case_id = $1
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(case_id)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
    };

    Ok(events)
}

/// Count total audit events for a case.
pub async fn count_events_for_case(
    pool: &PgPool,
    case_id: Uuid,
) -> Result<i64, AppError> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM audit_events WHERE case_id = $1",
    )
    .bind(case_id)
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}
