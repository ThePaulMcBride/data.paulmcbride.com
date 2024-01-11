use axum::Router;
mod health_check_routes;

#[tokio::main]
async fn main() {
    let app = Router::new().nest("/health-check", health_check_routes::router());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
