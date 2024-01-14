use axum::{extract::Path, routing::get, Router};
use serde::Serialize;

use crate::data::posts::{Post, PostService, PostSummary};

use super::{ApiError, ApiResponse, ApiSuccess};

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_posts))
        .route("/:slug", get(get_post))
}

#[derive(Debug, Serialize)]
struct PostsResponse {
    posts: Vec<PostSummary>,
}

async fn list_posts() -> ApiResponse<PostsResponse> {
    let posts: Vec<PostSummary> = PostService::get_posts();

    ApiResponse::Ok(ApiSuccess::JsonData(PostsResponse { posts }))
}

async fn get_post(Path(slug): Path<String>) -> ApiResponse<Post> {
    let post_option = PostService::get_post(&slug);

    match post_option {
        Some(post) => ApiResponse::Ok(ApiSuccess::JsonData(post)),
        None => ApiResponse::Err(ApiError::NotFound),
    }
}
