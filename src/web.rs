use axum::{
    body::Body,
    http::{self, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use tower_http::services::ServeDir;

use crate::{config::AppConfig, content::post::PostIndex};

mod health_check_routes;
mod note_routes;
mod post_routes;

#[derive(Clone)]
pub struct AppState {
    pub post_index: PostIndex,
}

enum ApiResponse<T: Serialize> {
    JsonData(T),
    NotFound,
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> http::Response<Body> {
        match self {
            Self::JsonData(data) => (StatusCode::OK, Json(data)).into_response(),
            Self::NotFound => StatusCode::NOT_FOUND.into_response(),
        }
    }
}

pub fn bootstrap(config: AppConfig, state: AppState) -> Router {
    Router::<AppState>::new()
        .nest("/health-check", health_check_routes::router())
        .route("/posts/", get(post_routes::list_posts))
        .nest("/posts", post_routes::router())
        .nest("/notes", note_routes::router())
        .fallback_service(ServeDir::new(config.public_dir))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use serde_json::Value;
    use std::{
        fs::{create_dir_all, remove_dir_all, write},
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };
    use tower::ServiceExt;

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn test_dir() -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock is valid")
            .as_nanos();
        let counter = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "web-route-test-{}-{}-{}",
            std::process::id(),
            id,
            counter
        ));
        create_dir_all(dir.join("content/posts")).expect("test posts dir can be created");
        create_dir_all(dir.join("public")).expect("test public dir can be created");
        dir
    }

    fn write_post(root: &Path, name: &str, front_matter: &str, body: &str) {
        write(
            root.join("content/posts").join(name),
            format!("---\n{}---\n\n{}", front_matter, body),
        )
        .expect("test post can be written");
    }

    fn app(root: &Path) -> Router {
        let config = AppConfig {
            port: 0,
            content_dir: root.join("content"),
            public_dir: root.join("public"),
        };
        let post_index = PostIndex::load(&config.content_dir).expect("test posts load");

        bootstrap(config, AppState { post_index })
    }

    async fn get_json(app: Router, path: &str) -> (StatusCode, Value) {
        let response = app
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("request can be built"),
            )
            .await
            .expect("request succeeds");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body can be read");
        let json = serde_json::from_slice(&body).expect("body is json");

        (status, json)
    }

    #[tokio::test]
    async fn lists_posts_without_drafts() {
        let root = test_dir();
        write_post(
            &root,
            "published.mdx",
            "date: \"2024-01-01\"\ntitle: Published\ndescription: Published post\nbanner: /images/published.jpg\n",
            "Published body",
        );
        write_post(
            &root,
            "draft.mdx",
            "date: \"2025-01-01\"\ntitle: Draft\ndescription: Draft post\nbanner: /images/draft.jpg\ndraft: true\n",
            "Draft body",
        );

        let (status, json) = get_json(app(&root), "/posts/").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["posts"].as_array().expect("posts is array").len(), 1);
        assert_eq!(json["posts"][0]["slug"], "published");

        remove_dir_all(root).expect("test dir can be removed");
    }

    #[tokio::test]
    async fn gets_post_by_slug() {
        let root = test_dir();
        write_post(
            &root,
            "published.mdx",
            "date: \"2024-01-01\"\ntitle: Published\ndescription: Published post\nbanner: /images/published.jpg\n",
            "Published body",
        );

        let (status, json) = get_json(app(&root), "/posts/published").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["slug"], "published");
        assert_eq!(json["body"], "Published body");

        remove_dir_all(root).expect("test dir can be removed");
    }

    #[tokio::test]
    async fn returns_not_found_for_missing_post() {
        let root = test_dir();
        write_post(
            &root,
            "published.mdx",
            "date: \"2024-01-01\"\ntitle: Published\ndescription: Published post\nbanner: /images/published.jpg\n",
            "Published body",
        );

        let response = app(&root)
            .oneshot(
                Request::builder()
                    .uri("/posts/missing")
                    .body(Body::empty())
                    .expect("request can be built"),
            )
            .await
            .expect("request succeeds");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        remove_dir_all(root).expect("test dir can be removed");
    }
}
