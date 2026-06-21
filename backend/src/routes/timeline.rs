//! Timeline endpoint for viewing case audit events with pagination and filtering.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{audit_events, cases};
use crate::domain::audit::AuditEvent;
use crate::domain::types::ActorType;
use crate::error::AppError;
use crate::AppState;

/// Default page size for timeline queries.
const DEFAULT_LIMIT: i64 = 20;
/// Maximum page size to prevent abuse.
const MAX_LIMIT: i64 = 100;

/// Query parameters for the timeline endpoint.
#[derive(Debug, Deserialize)]
pub struct TimelineParams {
    /// Maximum number of events to return (default 20, max 100).
    pub limit: Option<i64>,
    /// Cursor for pagination — ISO 8601 timestamp. Returns events older than this.
    pub cursor: Option<DateTime<Utc>>,
    /// Filter events by actor type.
    pub actor_type: Option<ActorType>,
}

/// Pagination metadata for timeline responses.
#[derive(Debug, Serialize)]
pub struct TimelineMeta {
    pub count: usize,
    pub next_cursor: Option<DateTime<Utc>>,
    pub has_more: bool,
}

/// Timeline response envelope.
#[derive(Debug, Serialize)]
pub struct TimelineResponse {
    pub data: Vec<AuditEvent>,
    pub meta: TimelineMeta,
}

/// GET /api/cases/:id/timeline — Retrieve paginated audit events for a case.
pub async fn get_timeline(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(params): Query<TimelineParams>,
) -> Result<impl IntoResponse, AppError> {
    // Verify case exists (returns 404 if not found)
    cases::get_case(&state.pool, id).await?;

    // Clamp limit to valid range
    let limit = params
        .limit
        .unwrap_or(DEFAULT_LIMIT)
        .clamp(1, MAX_LIMIT);

    // Fetch events with optional cursor and actor_type filter
    let events = audit_events::get_events_for_case_filtered(
        &state.pool,
        id,
        limit,
        params.cursor,
        params.actor_type.as_ref(),
    )
    .await?;

    let count = events.len();
    let has_more = count as i64 == limit;
    let next_cursor = if has_more {
        events.last().map(|e| e.created_at)
    } else {
        None
    };

    let response = TimelineResponse {
        data: events,
        meta: TimelineMeta {
            count,
            next_cursor,
            has_more,
        },
    };

    Ok((StatusCode::OK, Json(response)))
}
