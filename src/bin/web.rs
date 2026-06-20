use content_paulmcbride_com::{config, content, web};
use eyre::WrapErr;
use std::net::{Ipv4Addr, SocketAddr};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = config::AppConfig::from_env().wrap_err("failed to read app config")?;
    let post_index = content::post::PostIndex::load(&config.content_dir)
        .wrap_err("failed to load posts content")?;
    let note_index = content::note::NoteIndex::load(&config.content_dir)
        .wrap_err("failed to load notes content")?;
    let page_index = content::page::PageIndex::load(&config.content_dir)
        .wrap_err("failed to load pages content")?;
    let now_index =
        content::now::NowIndex::load(&config.content_dir).wrap_err("failed to load now content")?;
    let bind_addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, config.port));
    let app = web::bootstrap(
        config,
        web::AppState {
            post_index,
            note_index,
            page_index,
            now_index,
        },
    );

    tracing::info!(%bind_addr, "listening");
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .wrap_err_with(|| format!("failed to bind to {}", bind_addr))?;
    axum::serve(listener, app)
        .await
        .wrap_err("server stopped unexpectedly")?;

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("content_paulmcbride_com=info,web=info,tower_http=info")
    });

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .init();
}
