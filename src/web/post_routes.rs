use axum::{extract::Path, routing::get, Router};
use serde::Serialize;
use tower_http::compression::CompressionLayer;

use super::ApiResponse;
use crate::data::posts::{Post, PostService, PostSummary};

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_posts))
        .route("/:slug", get(get_post))
        .layer(CompressionLayer::new())
}

#[derive(Debug, Serialize)]
struct PostsResponse {
    posts: Vec<PostSummary>,
}

async fn list_posts() -> ApiResponse<PostsResponse> {
    let posts: Vec<PostSummary> = PostService::get_posts();

    ApiResponse::JsonData(PostsResponse { posts })
}

async fn get_post(Path(slug): Path<String>) -> ApiResponse<Post> {
    let post_option = PostService::get_post(&slug);

    match post_option {
        Some(post) => ApiResponse::JsonData(post),
        None => ApiResponse::NotFound,
    }
}
