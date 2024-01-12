use crate::AppState;
use axum::Router;

mod health_check_routes;
mod post_routes;

pub fn bootstrap(state: AppState) -> Router {
    Router::new()
        .nest("/health-check", health_check_routes::router())
        .nest("/posts", post_routes::router())
        .with_state(state)
}
