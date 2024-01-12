use data::posts::PostService;

mod data;
mod web;

#[derive(Clone)]
struct AppState {
    post_service: data::posts::PostService,
}

#[tokio::main]
async fn main() {
    let post_service = PostService::load_from_disk();
    let app_state = AppState { post_service };
    let app = web::bootstrap(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();

    println!("Listening on http://localhost:8000");
    axum::serve(listener, app).await.unwrap();
}
