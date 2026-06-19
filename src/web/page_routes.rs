use axum::{
    extract::{Path, State},
    routing::get,
    Router,
};
use tower_http::compression::CompressionLayer;

use super::{ApiResponse, AppState};
use crate::content::page::Page;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/:slug", get(get_page))
        .layer(CompressionLayer::new())
}

async fn get_page(State(state): State<AppState>, Path(slug): Path<String>) -> ApiResponse<Page> {
    match state.page_index.page(&slug) {
        Some(page) => ApiResponse::JsonData(page),
        None => ApiResponse::NotFound,
    }
}
