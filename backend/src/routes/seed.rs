use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;

use crate::AppState;

pub async fn seed_database(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let test_mode = std::env::var("TEST_MODE")
        .unwrap_or_default()
        .to_lowercase();

    if test_mode != "true" && test_mode != "1" {
        return Err(StatusCode::NOT_FOUND);
    }

    match crate::seed::seed_demo_data(&state.pool).await {
        Ok(()) => Ok(Json(json!({
            "data": { "message": "Demo data seeded successfully", "scenarios": 3 },
            "error": null,
            "meta": {}
        }))),
        Err(e) => {
            tracing::error!("Seed failed: {e}");
            Ok(Json(json!({
                "data": null,
                "error": format!("Seed failed: {e}"),
                "meta": {}
            })))
        }
    }
}
