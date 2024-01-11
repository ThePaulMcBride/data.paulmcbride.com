use axum::Router;

mod health_check_routes;
mod post_routes;
mod posts;

#[derive(Clone)]
struct AppState {
    post_service: posts::PostService,
}

#[tokio::main]
async fn main() {
    let post_service = posts::PostService::load_from_disk();
    let app_state = AppState { post_service };

    let app = Router::new()
        .nest("/health-check", health_check_routes::router())
        .nest("/posts", post_routes::router())
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
