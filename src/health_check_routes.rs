use axum::{routing::get, Json, Router};
use serde::Serialize;

use crate::AppState;

#[derive(Serialize)]
struct HealthCheckResponse {
    status: String,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(health_check))
}

async fn health_check() -> Json<HealthCheckResponse> {
    let response = HealthCheckResponse {
        status: "ok".to_string(),
    };

    Json(response)
}
