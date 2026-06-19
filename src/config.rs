use std::{env, path::PathBuf};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub port: u16,
    pub content_dir: PathBuf,
    pub public_dir: PathBuf,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            port: env::var("PORT")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(8000),
            content_dir: env::var("CONTENT_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("content")),
            public_dir: env::var("PUBLIC_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("public")),
        }
    }
}
