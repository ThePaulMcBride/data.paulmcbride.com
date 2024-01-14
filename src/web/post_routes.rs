use axum::{extract::Path, http::StatusCode, routing::get, Json, Router};
use serde::Serialize;

use crate::data::posts::{Post, PostService, PostSummary};

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_posts))
        .route("/:slug", get(get_post))
}

#[derive(Debug, Serialize)]
struct PostsResponse {
    posts: Vec<PostSummary>,
}

async fn list_posts() -> Json<PostsResponse> {
    let posts: Vec<PostSummary> = PostService::get_posts();

    Json(PostsResponse { posts })
}

async fn get_post(Path(slug): Path<String>) -> Result<Json<Post>, StatusCode> {
    let post_option = PostService::get_post(&slug);

    match post_option {
        Some(post) => Ok(Json(post)),
        None => Err(StatusCode::NOT_FOUND),
    }
}
