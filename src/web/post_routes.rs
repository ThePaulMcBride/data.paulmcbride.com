use axum::{extract::Path, extract::State, http::StatusCode, routing::get, Json, Router};
use serde::Serialize;

use crate::{
    data::posts::{Post, PostSummary},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_posts))
        .route("/:slug", get(get_post))
}

#[derive(Debug, Serialize)]
struct PostsResponse {
    posts: Vec<PostSummary>,
}

async fn list_posts(State(state): State<AppState>) -> Json<PostsResponse> {
    let posts: Vec<PostSummary> = state.post_service.get_posts();

    Json(PostsResponse { posts })
}

async fn get_post(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<Post>, StatusCode> {
    let post_option = state.post_service.get_post(&slug);

    match post_option {
        Some(post) => Ok(Json(post)),
        None => Err(StatusCode::NOT_FOUND),
    }
}
