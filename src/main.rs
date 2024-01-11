use axum::{routing::get, Router};

mod health_check;
#[tokio::main]
async fn main() {
    let app = Router::new().nest("/health-check", health_check::health_check_router());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
