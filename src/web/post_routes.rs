use axum::{
    extract::{Path, State},
    routing::get,
    Router,
};
use serde::Serialize;
use tower_http::compression::CompressionLayer;

use super::{ApiResponse, AppState};
use crate::content::post::{Post, PostSummary};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_posts))
        .route("/:slug", get(get_post))
        .layer(CompressionLayer::new())
}

#[derive(Debug, Serialize)]
pub(super) struct PostsResponse {
    posts: Vec<PostSummary>,
}

pub async fn list_posts(State(state): State<AppState>) -> ApiResponse<PostsResponse> {
    let posts: Vec<PostSummary> = state.post_index.posts();

    ApiResponse::JsonData(PostsResponse { posts })
}

async fn get_post(State(state): State<AppState>, Path(slug): Path<String>) -> ApiResponse<Post> {
    let post_option = state.post_index.post(&slug);

    match post_option {
        Some(post) => ApiResponse::JsonData(post),
        None => ApiResponse::NotFound,
    }
}
