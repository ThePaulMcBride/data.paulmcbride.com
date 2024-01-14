use axum::{
    body::Body,
    http::{self, StatusCode},
    response::IntoResponse,
    Json, Router,
};
use serde::Serialize;
use tower_http::services::ServeDir;

mod health_check_routes;
mod post_routes;

enum ApiResponse<T: Serialize> {
    JsonData(T),
    NotFound,
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> http::Response<Body> {
        match self {
            Self::JsonData(data) => (StatusCode::OK, Json(data)).into_response(),
            Self::NotFound => StatusCode::NOT_FOUND.into_response(),
        }
    }
}

pub fn bootstrap() -> Router {
    Router::new()
        .nest_service("/", ServeDir::new("public"))
        .nest("/health-check", health_check_routes::router())
        .nest("/posts", post_routes::router())
}
