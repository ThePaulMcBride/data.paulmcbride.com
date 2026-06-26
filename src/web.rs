use axum::{
    body::Body,
    http::{self, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use tower_http::services::ServeDir;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::{
    config::AppConfig,
    content::{note::NoteIndex, now::NowIndex, page::PageIndex, post::PostIndex},
};

mod health_check_routes;
mod note_routes;
mod now_routes;
mod page_routes;
mod post_routes;

#[derive(Clone)]
pub struct AppState {
    pub post_index: PostIndex,
    pub note_index: NoteIndex,
    pub page_index: PageIndex,
    pub now_index: NowIndex,
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
        .route("/notes/", get(note_routes::list_notes))
        .nest("/notes", note_routes::router())
        .route("/now/", get(now_routes::list_now_entries))
        .nest("/now", now_routes::router())
        .nest("/pages", page_routes::router())
        .fallback_service(ServeDir::new(config.public_dir))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
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

    fn write_note(root: &Path, name: &str, front_matter: &str, body: &str) {
        create_dir_all(root.join("content/notes")).expect("test notes dir can be created");
        write(
            root.join("content/notes").join(name),
            format!("---\n{}---\n\n{}", front_matter, body),
        )
        .expect("test note can be written");
    }

    fn write_page(root: &Path, name: &str, body: &str) {
        create_dir_all(root.join("content/pages")).expect("test pages dir can be created");
        write(root.join("content/pages").join(name), body).expect("test page can be written");
    }

    fn write_now(root: &Path, name: &str, front_matter: &str, body: &str) {
        create_dir_all(root.join("content/now")).expect("test now dir can be created");
        write(
            root.join("content/now").join(name),
            format!("---\n{}---\n\n{}", front_matter, body),
        )
        .expect("test now entry can be written");
    }

    fn app(root: &Path) -> Router {
        let config = AppConfig {
            port: 0,
            content_dir: root.join("content"),
            public_dir: root.join("public"),
        };
        let post_index = PostIndex::load(&config.content_dir).expect("test posts load");
        let note_index = NoteIndex::load(&config.content_dir).expect("test notes load");
        let page_index = PageIndex::load(&config.content_dir).expect("test pages load");
        let now_index = NowIndex::load(&config.content_dir).expect("test now entries load");

        bootstrap(
            config,
            AppState {
                post_index,
                note_index,
                page_index,
                now_index,
            },
        )
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

    #[tokio::test]
    async fn lists_notes_from_content() {
        let root = test_dir();
        write_post(
            &root,
            "published.mdx",
            "date: \"2024-01-01\"\ntitle: Published\ndescription: Published post\nbanner: /images/published.jpg\n",
            "Published body",
        );
        write_note(
            &root,
            "note.md",
            "date: \"2024-01-01T10:00:00Z\"\nsource: mastodon\nsource_id: \"1\"\nsource_url: https://example.com/1\nvisibility: public\n",
            "Note body",
        );

        let (status, json) = get_json(app(&root), "/notes/").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["notes"].as_array().expect("notes is array").len(), 1);
        assert_eq!(json["notes"][0]["slug"], "note");
        assert_eq!(json["notes"][0]["source"], "mastodon");

        remove_dir_all(root).expect("test dir can be removed");
    }

    #[tokio::test]
    async fn lists_grouped_note_page_from_content() {
        let root = test_dir();
        write_note(
            &root,
            "root.md",
            "date: \"2024-01-01T10:00:00Z\"\nsource: mastodon\nsource_id: \"1\"\nsource_url: https://example.com/1\nvisibility: public\n",
            "Root body",
        );
        write_note(
            &root,
            "reply.md",
            "date: \"2024-01-01T11:00:00Z\"\nsource: mastodon\nsource_id: \"2\"\nsource_url: https://example.com/2\nin_reply_to_id: \"1\"\nin_reply_to_account_id: \"account\"\nvisibility: public\n",
            "Reply body",
        );

        let (status, json) = get_json(app(&root), "/notes/page?limit=25").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["items"].as_array().expect("items is array").len(), 1);
        assert_eq!(
            json["items"][0]["notes"]
                .as_array()
                .expect("notes is array")
                .len(),
            2
        );
        assert_eq!(json["items"][0]["notes"][0]["slug"], "root");
        assert_eq!(json["items"][0]["notes"][1]["slug"], "reply");

        remove_dir_all(root).expect("test dir can be removed");
    }

    #[tokio::test]
    async fn gets_note_by_slug() {
        let root = test_dir();
        write_post(
            &root,
            "published.mdx",
            "date: \"2024-01-01\"\ntitle: Published\ndescription: Published post\nbanner: /images/published.jpg\n",
            "Published body",
        );
        write_note(
            &root,
            "note.md",
            "date: \"2024-01-01T10:00:00Z\"\nsource: mastodon\nsource_id: \"1\"\nsource_url: https://example.com/1\nvisibility: public\n",
            "Note body",
        );

        let (status, json) = get_json(app(&root), "/notes/note").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["slug"], "note");
        assert_eq!(json["body"], "Note body");

        remove_dir_all(root).expect("test dir can be removed");
    }

    #[tokio::test]
    async fn gets_page_by_slug() {
        let root = test_dir();
        write_post(
            &root,
            "published.mdx",
            "date: \"2024-01-01\"\ntitle: Published\ndescription: Published post\nbanner: /images/published.jpg\n",
            "Published body",
        );
        write_page(&root, "homepage.mdx", "Homepage body");

        let (status, json) = get_json(app(&root), "/pages/homepage").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["slug"], "homepage");
        assert_eq!(json["body"], "Homepage body");

        remove_dir_all(root).expect("test dir can be removed");
    }

    #[tokio::test]
    async fn lists_now_entries() {
        let root = test_dir();
        write_post(
            &root,
            "published.mdx",
            "date: \"2024-01-01\"\ntitle: Published\ndescription: Published post\nbanner: /images/published.jpg\n",
            "Published body",
        );
        write_now(
            &root,
            "2023-01.mdx",
            "date: \"2023-01-04\"\ntitle: January\n",
            "January body",
        );
        write_now(
            &root,
            "2022-12.mdx",
            "date: \"2022-12-01\"\ntitle: December\n",
            "December body",
        );

        let (status, json) = get_json(app(&root), "/now/").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json["entries"].as_array().expect("entries is array").len(),
            2
        );
        assert_eq!(json["entries"][0]["slug"], "2023-01");

        remove_dir_all(root).expect("test dir can be removed");
    }

    #[tokio::test]
    async fn gets_now_entry_by_slug() {
        let root = test_dir();
        write_post(
            &root,
            "published.mdx",
            "date: \"2024-01-01\"\ntitle: Published\ndescription: Published post\nbanner: /images/published.jpg\n",
            "Published body",
        );
        write_now(
            &root,
            "2023-01.mdx",
            "date: \"2023-01-04\"\ntitle: January\n",
            "January body",
        );

        let (status, json) = get_json(app(&root), "/now/2023-01").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["slug"], "2023-01");
        assert_eq!(json["body"], "January body");

        remove_dir_all(root).expect("test dir can be removed");
    }
}
