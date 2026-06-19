use std::{env, fmt, num::ParseIntError, path::PathBuf};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub port: u16,
    pub content_dir: PathBuf,
    pub public_dir: PathBuf,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            port: match env::var("PORT") {
                Ok(value) => value
                    .parse()
                    .map_err(|source| ConfigError::InvalidPort { value, source })?,
                Err(env::VarError::NotPresent) => 8000,
                Err(source) => {
                    return Err(ConfigError::ReadEnv {
                        name: "PORT",
                        source,
                    })
                }
            },
            content_dir: env::var("CONTENT_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("content")),
            public_dir: env::var("PUBLIC_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("public")),
        })
    }
}

#[derive(Debug)]
pub enum ConfigError {
    InvalidPort {
        value: String,
        source: ParseIntError,
    },
    ReadEnv {
        name: &'static str,
        source: env::VarError,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPort { value, source } => {
                write!(f, "invalid PORT value '{}': {}", value, source)
            }
            Self::ReadEnv { name, source } => {
                write!(
                    f,
                    "failed to read {} environment variable: {}",
                    name, source
                )
            }
        }
    }
}

impl std::error::Error for ConfigError {}
