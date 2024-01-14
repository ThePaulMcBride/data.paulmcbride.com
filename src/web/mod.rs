use axum::Router;
use tower_http::services::ServeDir;

mod health_check_routes;
mod post_routes;

pub fn bootstrap() -> Router {
    Router::new()
        .nest_service("/", ServeDir::new("public"))
        .nest("/health-check", health_check_routes::router())
        .nest("/posts", post_routes::router())
}
