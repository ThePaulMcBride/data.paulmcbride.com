use std::{env, fmt, num::ParseIntError, path::PathBuf};

pub fn required_env(name: &'static str) -> Result<String, EnvConfigError> {
    match env::var(name) {
        Ok(value) if !value.is_empty() => Ok(value),
        Ok(_) | Err(env::VarError::NotPresent) => Err(EnvConfigError::MissingEnv { name }),
        Err(source) => Err(EnvConfigError::ReadEnv { name, source }),
    }
}

pub fn optional_env(name: &'static str) -> Result<Option<String>, EnvConfigError> {
    match env::var(name) {
        Ok(value) if !value.is_empty() => Ok(Some(value)),
        Ok(_) | Err(env::VarError::NotPresent) => Ok(None),
        Err(source) => Err(EnvConfigError::ReadEnv { name, source }),
    }
}

pub fn env_or(name: &'static str, default: &str) -> Result<String, EnvConfigError> {
    optional_env(name).map(|value| value.unwrap_or_else(|| default.to_string()))
}

#[derive(Debug)]
pub enum EnvConfigError {
    MissingEnv {
        name: &'static str,
    },
    ReadEnv {
        name: &'static str,
        source: env::VarError,
    },
}

impl fmt::Display for EnvConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEnv { name } => write!(f, "missing {} environment variable", name),
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

impl std::error::Error for EnvConfigError {}

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub port: u16,
    pub content_dir: PathBuf,
    pub public_dir: PathBuf,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            port: match optional_env("PORT").map_err(ConfigError::Env)? {
                Some(value) => value
                    .parse()
                    .map_err(|source| ConfigError::InvalidPort { value, source })?,
                None => 8000,
            },
            content_dir: PathBuf::from(env_or("CONTENT_DIR", "content").map_err(ConfigError::Env)?),
            public_dir: PathBuf::from(env_or("PUBLIC_DIR", "public").map_err(ConfigError::Env)?),
        })
    }
}

#[derive(Debug)]
pub enum ConfigError {
    InvalidPort {
        value: String,
        source: ParseIntError,
    },
    Env(EnvConfigError),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPort { value, source } => {
                write!(f, "invalid PORT value '{}': {}", value, source)
            }
            Self::Env(source) => write!(f, "{}", source),
        }
    }
}

impl std::error::Error for ConfigError {}
