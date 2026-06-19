use chrono::{DateTime, Utc};
use eyre::WrapErr;
use serde::Deserialize;
use std::{env, fmt, path::PathBuf};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenvy::dotenv().ok();

    let config = MastodonSyncConfig::from_env().wrap_err("failed to read Mastodon sync config")?;
    let account = fetch_account(&config)
        .await
        .wrap_err("failed to verify Mastodon account config")?;

    println!(
        "Mastodon sync configured for @{} ({}) on {}. Notes will be written under {}.",
        account.acct,
        account.display_name,
        config.base_url,
        config.notes_dir().display()
    );

    Ok(())
}

async fn fetch_account(config: &MastodonSyncConfig) -> Result<MastodonAccount, MastodonApiError> {
    let url = format!(
        "{}/api/v1/accounts/{}",
        config.base_url.trim_end_matches('/'),
        config.account_id,
    );
    let response = reqwest::Client::new()
        .get(&url)
        .bearer_auth(&config.access_token)
        .send()
        .await
        .map_err(MastodonApiError::Request)?;

    let status = response.status();
    if !status.is_success() {
        return Err(MastodonApiError::UnexpectedStatus { status });
    }

    response
        .json::<MastodonAccount>()
        .await
        .map_err(MastodonApiError::Decode)
}

#[derive(Debug, Deserialize)]
struct MastodonAccount {
    acct: String,
    display_name: String,
}

#[derive(Debug)]
struct MastodonSyncConfig {
    base_url: String,
    access_token: String,
    account_id: String,
    content_dir: PathBuf,
}

impl MastodonSyncConfig {
    fn from_env() -> Result<Self, MastodonSyncConfigError> {
        Ok(Self {
            base_url: required_env("MASTODON_BASE_URL")?,
            access_token: required_env("MASTODON_ACCESS_TOKEN")?,
            account_id: required_env("MASTODON_ACCOUNT_ID")?,
            content_dir: env::var("CONTENT_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("content")),
        })
    }

    fn notes_dir(&self) -> PathBuf {
        self.content_dir.join("notes")
    }
}

#[derive(Debug)]
enum MastodonApiError {
    Request(reqwest::Error),
    UnexpectedStatus { status: reqwest::StatusCode },
    Decode(reqwest::Error),
}

impl fmt::Display for MastodonApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Request(source) => write!(f, "Mastodon API request failed: {}", source),
            Self::UnexpectedStatus { status } => {
                write!(f, "Mastodon API returned unexpected status: {}", status)
            }
            Self::Decode(source) => write!(f, "failed to decode Mastodon API response: {}", source),
        }
    }
}

impl std::error::Error for MastodonApiError {}

#[derive(Debug)]
enum MastodonSyncConfigError {
    MissingEnv {
        name: &'static str,
    },
    ReadEnv {
        name: &'static str,
        source: env::VarError,
    },
}

impl fmt::Display for MastodonSyncConfigError {
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

impl std::error::Error for MastodonSyncConfigError {}

fn required_env(name: &'static str) -> Result<String, MastodonSyncConfigError> {
    match env::var(name) {
        Ok(value) if !value.is_empty() => Ok(value),
        Ok(_) | Err(env::VarError::NotPresent) => Err(MastodonSyncConfigError::MissingEnv { name }),
        Err(source) => Err(MastodonSyncConfigError::ReadEnv { name, source }),
    }
}

pub fn note_filename(
    created_at: &str,
    collision_suffix: Option<&str>,
) -> Result<String, NoteFilenameError> {
    let created_at = DateTime::parse_from_rfc3339(created_at).map_err(|source| {
        NoteFilenameError::InvalidCreatedAt {
            value: created_at.to_string(),
            source,
        }
    })?;
    let timestamp = created_at.with_timezone(&Utc).format("%Y-%m-%d-%H%M%S");

    match collision_suffix {
        Some(suffix) => Ok(format!("{timestamp}-{suffix}.md")),
        None => Ok(format!("{timestamp}.md")),
    }
}

#[derive(Debug)]
pub enum NoteFilenameError {
    InvalidCreatedAt {
        value: String,
        source: chrono::ParseError,
    },
}

impl fmt::Display for NoteFilenameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCreatedAt { value, source } => {
                write!(
                    f,
                    "invalid Mastodon created_at value '{}': {}",
                    value, source
                )
            }
        }
    }
}

impl std::error::Error for NoteFilenameError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_timestamp_note_filename_in_utc() {
        let filename = note_filename("2026-06-18T20:30:00Z", None).expect("filename is valid");

        assert_eq!(filename, "2026-06-18-203000.md");
    }

    #[test]
    fn normalizes_offset_timestamps_to_utc() {
        let filename = note_filename("2026-06-18T13:30:00-07:00", None).expect("filename is valid");

        assert_eq!(filename, "2026-06-18-203000.md");
    }

    #[test]
    fn appends_collision_suffix_when_needed() {
        let filename =
            note_filename("2026-06-18T20:30:00Z", Some("123456789")).expect("filename is valid");

        assert_eq!(filename, "2026-06-18-203000-123456789.md");
    }

    #[test]
    fn rejects_invalid_created_at_values() {
        let error = note_filename("not-a-date", None).expect_err("invalid date should fail");

        assert!(matches!(error, NoteFilenameError::InvalidCreatedAt { .. }));
    }
}
