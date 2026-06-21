//! Server-Sent Events endpoint for real-time case event streaming.
//!
//! Streams audit events for a case as SSE messages. Uses a poll-based approach
//! (2s interval) to check for new events since the client's last seen event ID.

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use futures::stream::Stream;
use serde::Serialize;
use std::convert::Infallible;
use std::time::Duration;
use uuid::Uuid;

use crate::db::cases;
use crate::domain::audit::AuditEvent;
use crate::error::AppError;
use crate::AppState;

/// SSE event data payload sent to clients.
#[derive(Debug, Clone, Serialize)]
struct SseEventPayload {
    #[serde(rename = "type")]
    event_type: String,
    data: serde_json::Value,
    timestamp: String,
}

/// Map audit event action strings to SSE event type names.
fn map_action_to_sse_type(action: &str) -> Option<&'static str> {
    match action {
        "credential_verified" => Some("submission_verified"),
        "credential_failed" => Some("submission_verified"),
        "case_created" => Some("status_changed"),
        "state_transition" => Some("status_changed"),
        "override_decision" => Some("assessment_complete"),
        "protected_action_executed" => Some("assessment_complete"),
        _ => {
            // Check if it looks like a requirement-related action
            if action.contains("requirement") {
                Some("requirement_added")
            } else {
                Some("status_changed")
            }
        }
    }
}

/// Build the SSE data payload from an audit event.
fn build_sse_payload(event: &AuditEvent) -> SseEventPayload {
    let event_type = map_action_to_sse_type(&event.action)
        .unwrap_or("status_changed")
        .to_string();

    let data = match event_type.as_str() {
        "submission_verified" => {
            let details = event.details.clone().unwrap_or_default();
            serde_json::json!({
                "submission_id": details.get("submission_id"),
                "requirement_id": details.get("requirement_claim_type"),
                "status": event.action,
            })
        }
        "requirement_added" => {
            let details = event.details.clone().unwrap_or_default();
            serde_json::json!({
                "requirement_id": details.get("requirement_id"),
                "claim_type": details.get("claim_type"),
                "description": details.get("description"),
            })
        }
        "assessment_complete" => {
            let details = event.details.clone().unwrap_or_default();
            serde_json::json!({
                "case_id": event.case_id,
                "decision": details.get("decision"),
            })
        }
        "status_changed" => {
            let details = event.details.clone().unwrap_or_default();
            serde_json::json!({
                "case_id": event.case_id,
                "old_status": details.get("old_status"),
                "new_status": details.get("new_status"),
            })
        }
        _ => event.details.clone().unwrap_or_default(),
    };

    SseEventPayload {
        event_type,
        data,
        timestamp: event.created_at.to_rfc3339(),
    }
}

/// GET /api/cases/:id/events/stream — SSE endpoint streaming case events.
///
/// Polls the audit_events table every 2 seconds for new events.
/// Supports `Last-Event-ID` header for reconnection (client sends the last
/// event UUID it received, and the server resumes from there).
pub async fn stream_case_events(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    // Validate case exists
    let _case = cases::get_case(&state.pool, case_id).await?;

    // Parse Last-Event-ID header for reconnection support
    let last_event_id: Option<Uuid> = headers
        .get("Last-Event-ID")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok());

    let stream = make_event_stream(state.pool.clone(), case_id, last_event_id);

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("ping"),
    ))
}

/// Creates an async stream that polls for new audit events every 2 seconds.
fn make_event_stream(
    pool: sqlx::PgPool,
    case_id: Uuid,
    initial_last_id: Option<Uuid>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    async_stream::stream! {
        let mut _last_seen_id: Option<Uuid> = initial_last_id;
        let mut last_seen_time: Option<chrono::DateTime<chrono::Utc>> = None;

        // If we have a last_event_id, look up its timestamp for cursor-based filtering
        if let Some(event_id) = _last_seen_id {
            let result = sqlx::query_as::<_, (chrono::DateTime<chrono::Utc>,)>(
                "SELECT created_at FROM audit_events WHERE id = $1"
            )
            .bind(event_id)
            .fetch_optional(&pool)
            .await;

            if let Ok(Some((ts,))) = result {
                last_seen_time = Some(ts);
            }
        }

        loop {
            // Query for events newer than the last seen timestamp
            let events_result = if let Some(after_time) = last_seen_time {
                sqlx::query_as::<_, AuditEvent>(
                    r#"
                    SELECT id, case_id, actor_type, actor_id, action, details, created_at
                    FROM audit_events
                    WHERE case_id = $1 AND created_at > $2
                    ORDER BY created_at ASC
                    LIMIT 50
                    "#,
                )
                .bind(case_id)
                .bind(after_time)
                .fetch_all(&pool)
                .await
            } else {
                // First connection: fetch recent events (last 20)
                sqlx::query_as::<_, AuditEvent>(
                    r#"
                    SELECT id, case_id, actor_type, actor_id, action, details, created_at
                    FROM audit_events
                    WHERE case_id = $1
                    ORDER BY created_at DESC
                    LIMIT 20
                    "#,
                )
                .bind(case_id)
                .fetch_all(&pool)
                .await
                .map(|mut v| { v.reverse(); v })
            };

            match events_result {
                Ok(events) => {
                    for event in &events {
                        let payload = build_sse_payload(event);
                        let sse_type = payload.event_type.clone();

                        if let Ok(json_data) = serde_json::to_string(&payload) {
                            let sse_event = Event::default()
                                .event(&sse_type)
                                .data(&json_data)
                                .id(event.id.to_string());

                            yield Ok(sse_event);
                        }

                        _last_seen_id = Some(event.id);
                        last_seen_time = Some(event.created_at);
                    }
                }
                Err(e) => {
                    tracing::error!(case_id = %case_id, error = %e, "SSE poll query failed");
                }
            }

            // Poll interval: 2 seconds
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
}
