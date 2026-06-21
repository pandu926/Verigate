use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::t3::agent_identity::AgentIdentity;
use crate::AppState;

/// Health check response including system status and agent identity metadata.
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub agent_identity: AgentIdentity,
    pub database_connected: bool,
    pub uptime_seconds: u64,
}

/// GET /api/health — returns service health with agent identity metadata.
pub async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    let db_connected = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();

    let uptime = state.start_time.elapsed().as_secs();

    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        agent_identity: state.agent_identity.clone(),
        database_connected: db_connected,
        uptime_seconds: uptime,
    })
}
