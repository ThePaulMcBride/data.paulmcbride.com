mod config;
mod content;
mod web;

use eyre::WrapErr;
use std::net::{Ipv4Addr, SocketAddr};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let config = config::AppConfig::from_env();
    let post_index = content::post::PostIndex::load(&config.content_dir)
        .wrap_err("failed to load posts content")?;
    let bind_addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, config.port));
    let app = web::bootstrap(config, web::AppState { post_index });

    println!("Listening on http://{}", bind_addr);
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .wrap_err_with(|| format!("failed to bind to {}", bind_addr))?;
    axum::serve(listener, app)
        .await
        .wrap_err("server stopped unexpectedly")?;

    Ok(())
}
