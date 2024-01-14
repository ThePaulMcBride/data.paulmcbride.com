use axum::{routing::get, Router};
use serde::Serialize;

use super::{ApiResponse, ApiSuccess};

#[derive(Serialize)]
struct HealthCheckResponse {
    status: String,
}

pub fn router() -> Router {
    Router::new().route("/", get(health_check))
}

async fn health_check() -> ApiResponse<HealthCheckResponse> {
    let response = HealthCheckResponse {
        status: "ok".to_string(),
    };

    ApiResponse::Ok(ApiSuccess::JsonData(response))
}
