use axum::{
    extract::{Path, State},
    routing::get,
    Router,
};
use serde::Serialize;
use tower_http::compression::CompressionLayer;

use super::{ApiResponse, AppState};
use crate::content::now::NowEntry;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_now_entries))
        .route("/:slug", get(get_now_entry))
        .layer(CompressionLayer::new())
}

#[derive(Debug, Serialize)]
pub(super) struct NowEntriesResponse {
    entries: Vec<NowEntry>,
}

pub async fn list_now_entries(State(state): State<AppState>) -> ApiResponse<NowEntriesResponse> {
    ApiResponse::JsonData(NowEntriesResponse {
        entries: state.now_entries(),
    })
}

async fn get_now_entry(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> ApiResponse<NowEntry> {
    match state.now_entry(&slug) {
        Some(entry) => ApiResponse::JsonData(entry),
        None => ApiResponse::NotFound,
    }
}
