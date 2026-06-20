use chrono::{DateTime, Utc};
use content_paulmcbride_com::content::note::NoteIndex;
use eyre::WrapErr;
use serde::Deserialize;
use std::{collections::HashSet, env, fmt, fs, path::PathBuf};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = MastodonSyncConfig::from_env().wrap_err("failed to read Mastodon sync config")?;
    let account = fetch_account(&config)
        .await
        .wrap_err("failed to verify Mastodon account config")?;
    let existing_notes =
        NoteIndex::load(&config.content_dir).wrap_err("failed to load existing notes")?;
    let existing_source_ids: HashSet<String> = existing_notes
        .notes()
        .into_iter()
        .map(|note| note.front_matter.source_id)
        .collect();
    let write_files = env::args().any(|arg| arg == "--write");
    let full_sync = env::args().any(|arg| arg == "--full");
    let statuses = fetch_statuses(&config, &existing_source_ids, full_sync)
        .await
        .wrap_err("failed to fetch Mastodon statuses")?;
    let mut summary = SyncSummary::new(statuses.len());

    tracing::info!(
        account = %account.acct,
        display_name = %account.display_name,
        base_url = %config.base_url,
        notes_dir = %config.notes_dir().display(),
        mode = %sync_mode(write_files, full_sync),
        fetched = summary.fetched,
        "mastodon sync started"
    );

    for status in statuses {
        if !status.is_importable_visibility() {
            summary.skipped_visibility += 1;
            tracing::info!(
                source_id = %status.id,
                visibility = %status.visibility,
                reason = "visibility",
                "skipped mastodon status"
            );
            continue;
        }

        if existing_source_ids.contains(&status.id) {
            summary.skipped_existing += 1;
            tracing::info!(
                source_id = %status.id,
                reason = "already_imported",
                "skipped mastodon status"
            );
            continue;
        }

        let note =
            MastodonNote::from_status(status).wrap_err("failed to build note from status")?;
        let note_path = note_path(&config, &note).wrap_err("failed to build note path")?;

        if write_files {
            fs::create_dir_all(config.notes_dir()).wrap_err("failed to create notes directory")?;
            fs::write(&note_path, note.to_markdown())
                .wrap_err_with(|| format!("failed to write note file {}", note_path.display()))?;
            summary.written += 1;
            tracing::info!(
                source_id = %note.source_id,
                path = %note_path.display(),
                "wrote mastodon note"
            );
        } else {
            summary.dry_run += 1;
            tracing::info!(
                source_id = %note.source_id,
                path = %note_path.display(),
                "would write mastodon note"
            );
        }
    }

    tracing::info!(
        fetched = summary.fetched,
        written = summary.written,
        dry_run = summary.dry_run,
        skipped_existing = summary.skipped_existing,
        skipped_visibility = summary.skipped_visibility,
        mode = %sync_mode(write_files, full_sync),
        "mastodon sync completed"
    );

    Ok(())
}

fn init_tracing() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("sync_mastodon=info"));

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .init();
}

struct SyncSummary {
    fetched: usize,
    written: usize,
    dry_run: usize,
    skipped_existing: usize,
    skipped_visibility: usize,
}

impl SyncSummary {
    fn new(fetched: usize) -> Self {
        Self {
            fetched,
            written: 0,
            dry_run: 0,
            skipped_existing: 0,
            skipped_visibility: 0,
        }
    }
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

async fn fetch_statuses(
    config: &MastodonSyncConfig,
    existing_source_ids: &HashSet<String>,
    full_sync: bool,
) -> Result<Vec<MastodonStatus>, MastodonApiError> {
    let mut statuses = Vec::new();
    let mut max_id: Option<String> = None;

    loop {
        let mut page = fetch_status_page(config, max_id.as_deref()).await?;

        if page.is_empty() {
            break;
        }

        if !full_sync && existing_source_ids.is_empty() {
            statuses.append(&mut page);
            break;
        }

        if !full_sync {
            if let Some(existing_index) = page
                .iter()
                .position(|status| existing_source_ids.contains(&status.id))
            {
                statuses.extend(page.into_iter().take(existing_index));
                break;
            }
        }

        max_id = page.last().map(|status| status.id.clone());
        statuses.append(&mut page);
    }

    Ok(statuses)
}

async fn fetch_status_page(
    config: &MastodonSyncConfig,
    max_id: Option<&str>,
) -> Result<Vec<MastodonStatus>, MastodonApiError> {
    let url = format!(
        "{}/api/v1/accounts/{}/statuses",
        config.base_url.trim_end_matches('/'),
        config.account_id,
    );

    let mut request = reqwest::Client::new()
        .get(&url)
        .bearer_auth(&config.access_token)
        .query(&[
            ("limit", "40"),
            ("exclude_reblogs", "true"),
            ("exclude_replies", "true"),
        ]);

    if let Some(max_id) = max_id {
        request = request.query(&[("max_id", max_id)]);
    }

    let response = request.send().await.map_err(MastodonApiError::Request)?;

    let status = response.status();
    if !status.is_success() {
        return Err(MastodonApiError::UnexpectedStatus { status });
    }

    response
        .json::<Vec<MastodonStatus>>()
        .await
        .map_err(MastodonApiError::Decode)
}

#[derive(Debug, Deserialize)]
struct MastodonAccount {
    acct: String,
    display_name: String,
}

#[derive(Debug, Deserialize)]
struct MastodonStatus {
    id: String,
    created_at: String,
    uri: String,
    url: Option<String>,
    visibility: String,
    content: String,
    media_attachments: Vec<MastodonMediaAttachment>,
}

impl MastodonStatus {
    fn is_importable_visibility(&self) -> bool {
        matches!(self.visibility.as_str(), "public" | "unlisted")
    }
}

#[derive(Debug, Deserialize)]
struct MastodonMediaAttachment {
    url: String,
    description: Option<String>,
}

struct MastodonNote {
    created_at: String,
    source_id: String,
    source_url: String,
    visibility: String,
    body: String,
    media: Vec<MastodonNoteMedia>,
}

impl MastodonNote {
    fn from_status(status: MastodonStatus) -> Result<Self, NoteFilenameError> {
        DateTime::parse_from_rfc3339(&status.created_at).map_err(|source| {
            NoteFilenameError::InvalidCreatedAt {
                value: status.created_at.clone(),
                source,
            }
        })?;

        Ok(Self {
            created_at: status.created_at,
            source_id: status.id,
            source_url: status.url.unwrap_or(status.uri),
            visibility: status.visibility,
            body: html_to_text(&status.content),
            media: status
                .media_attachments
                .into_iter()
                .map(|media| MastodonNoteMedia {
                    url: media.url,
                    alt: media.description.unwrap_or_default(),
                })
                .collect(),
        })
    }

    fn to_markdown(&self) -> String {
        let mut front_matter = vec![
            "---".to_string(),
            format!("date: \"{}\"", yaml_escape(&self.created_at)),
            "source: mastodon".to_string(),
            format!("source_id: \"{}\"", yaml_escape(&self.source_id)),
            format!("source_url: \"{}\"", yaml_escape(&self.source_url)),
            format!("visibility: {}", self.visibility),
        ];

        if !self.media.is_empty() {
            front_matter.push("media:".to_string());
            for media in &self.media {
                front_matter.push(format!("  - url: \"{}\"", yaml_escape(&media.url)));
                front_matter.extend(yaml_string_field("    alt", &media.alt));
            }
        }

        front_matter.push("---".to_string());
        front_matter.push(String::new());
        front_matter.push(self.body.clone());
        front_matter.push(String::new());

        front_matter.join("\n")
    }
}

struct MastodonNoteMedia {
    url: String,
    alt: String,
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

fn sync_mode(write_files: bool, full_sync: bool) -> &'static str {
    match (write_files, full_sync) {
        (true, true) => "write/full",
        (true, false) => "write/incremental",
        (false, true) => "dry-run/full",
        (false, false) => "dry-run/incremental",
    }
}

fn note_path(
    config: &MastodonSyncConfig,
    note: &MastodonNote,
) -> Result<PathBuf, NoteFilenameError> {
    let filename = note_filename(&note.created_at, None)?;
    let path = config.notes_dir().join(&filename);

    if path.exists() {
        return Ok(config
            .notes_dir()
            .join(note_filename(&note.created_at, Some(&note.source_id))?));
    }

    Ok(path)
}

fn html_to_text(html: &str) -> String {
    let with_breaks = html
        .replace("</p><p>", "\n\n")
        .replace("</p>", "\n")
        .replace("<br />", "\n")
        .replace("<br/>", "\n")
        .replace("<br>", "\n");

    let mut text = String::new();
    let mut in_tag = false;

    for character in with_breaks.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }

    decode_html_entities(text.trim()).to_string()
}

fn decode_html_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn yaml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn yaml_string_field(name: &str, value: &str) -> Vec<String> {
    if value.contains('\n') {
        let mut lines = vec![format!("{name}: |-")];
        lines.extend(value.lines().map(|line| format!("      {line}")));
        lines
    } else {
        vec![format!("{name}: \"{}\"", yaml_escape(value))]
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

    #[test]
    fn converts_mastodon_html_to_text() {
        let text = html_to_text(
            "<p>Hello <a href=\"https://example.com\">world</a> &amp; friends</p><p>Second<br />line</p>",
        );

        assert_eq!(text, "Hello world & friends\n\nSecond\nline");
    }

    #[test]
    fn builds_note_markdown_from_status() {
        let status = MastodonStatus {
            id: "123456789".to_string(),
            created_at: "2026-06-18T20:30:00Z".to_string(),
            uri: "https://example.social/users/paul/statuses/123456789".to_string(),
            url: Some("https://example.social/@paul/123456789".to_string()),
            visibility: "public".to_string(),
            content: "<p>Hello &amp; welcome</p>".to_string(),
            media_attachments: vec![MastodonMediaAttachment {
                url: "https://cdn.example.social/image.jpg".to_string(),
                description: Some("Alt text".to_string()),
            }],
        };

        let note = MastodonNote::from_status(status).expect("note is valid");
        let markdown = note.to_markdown();

        assert!(markdown.contains("date: \"2026-06-18T20:30:00Z\""));
        assert!(markdown.contains("source: mastodon"));
        assert!(markdown.contains("source_id: \"123456789\""));
        assert!(markdown.contains("source_url: \"https://example.social/@paul/123456789\""));
        assert!(markdown.contains("visibility: public"));
        assert!(markdown.contains("  - url: \"https://cdn.example.social/image.jpg\""));
        assert!(markdown.contains("    alt: \"Alt text\""));
        assert!(markdown.contains("Hello & welcome"));
    }

    #[test]
    fn writes_multiline_media_alt_as_yaml_block_scalar() {
        let status = MastodonStatus {
            id: "123456789".to_string(),
            created_at: "2026-06-18T20:30:00Z".to_string(),
            uri: "https://example.social/users/paul/statuses/123456789".to_string(),
            url: Some("https://example.social/@paul/123456789".to_string()),
            visibility: "public".to_string(),
            content: "<p>Hello</p>".to_string(),
            media_attachments: vec![MastodonMediaAttachment {
                url: "https://cdn.example.social/image.jpg".to_string(),
                description: Some("First line\n\nSecond line".to_string()),
            }],
        };

        let note = MastodonNote::from_status(status).expect("note is valid");
        let markdown = note.to_markdown();

        assert!(markdown.contains("    alt: |-\n      First line\n      \n      Second line"));
    }

    #[test]
    fn falls_back_to_uri_when_status_has_no_public_url() {
        let status = MastodonStatus {
            id: "123456789".to_string(),
            created_at: "2026-06-18T20:30:00Z".to_string(),
            uri: "https://example.social/users/paul/statuses/123456789".to_string(),
            url: None,
            visibility: "public".to_string(),
            content: "<p>Hello</p>".to_string(),
            media_attachments: Vec::new(),
        };

        let note = MastodonNote::from_status(status).expect("note is valid");

        assert_eq!(
            note.source_url,
            "https://example.social/users/paul/statuses/123456789"
        );
    }

    #[test]
    fn imports_public_and_unlisted_statuses() {
        let public_status = status_with_visibility("public");
        let unlisted_status = status_with_visibility("unlisted");

        assert!(public_status.is_importable_visibility());
        assert!(unlisted_status.is_importable_visibility());
    }

    #[test]
    fn skips_private_and_direct_statuses() {
        let private_status = status_with_visibility("private");
        let direct_status = status_with_visibility("direct");

        assert!(!private_status.is_importable_visibility());
        assert!(!direct_status.is_importable_visibility());
    }

    #[test]
    fn describes_sync_modes() {
        assert_eq!(sync_mode(false, false), "dry-run/incremental");
        assert_eq!(sync_mode(true, false), "write/incremental");
        assert_eq!(sync_mode(false, true), "dry-run/full");
        assert_eq!(sync_mode(true, true), "write/full");
    }

    fn status_with_visibility(visibility: &str) -> MastodonStatus {
        MastodonStatus {
            id: "123456789".to_string(),
            created_at: "2026-06-18T20:30:00Z".to_string(),
            uri: "https://example.social/users/paul/statuses/123456789".to_string(),
            url: Some("https://example.social/@paul/123456789".to_string()),
            visibility: visibility.to_string(),
            content: "<p>Hello</p>".to_string(),
            media_attachments: Vec::new(),
        }
    }
}
