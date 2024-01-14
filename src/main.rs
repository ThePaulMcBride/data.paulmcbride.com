mod data;
mod web;

#[tokio::main]
async fn main() {
    let app = web::bootstrap();

    println!("Listening on http://localhost:8000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
