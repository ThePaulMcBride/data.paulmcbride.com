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

enum ApiSuccess<T> {
    JsonData(T),
}

impl<T: Serialize> IntoResponse for ApiSuccess<T> {
    fn into_response(self) -> http::Response<Body> {
        match self {
            ApiSuccess::JsonData(data) => (StatusCode::OK, Json(data)).into_response(),
        }
    }
}

enum ApiError {
    NotFound,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> http::Response<Body> {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND.into_response(),
        }
    }
}

enum ApiResponse<T> {
    Ok(ApiSuccess<T>),
    Err(ApiError),
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> http::Response<Body> {
        match self {
            Self::Ok(success) => success.into_response(),
            Self::Err(error) => error.into_response(),
        }
    }
}

pub fn bootstrap() -> Router {
    Router::new()
        .nest_service("/", ServeDir::new("public"))
        .nest("/health-check", health_check_routes::router())
        .nest("/posts", post_routes::router())
}
