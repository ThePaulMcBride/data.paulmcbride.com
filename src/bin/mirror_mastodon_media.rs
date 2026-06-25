use content_paulmcbride_com::{
    content::note::{NoteFrontMatter, NoteSource, NoteVisibility},
    media_mirror::{MediaMirror, MediaMirrorConfig, MediaMirrorTargetConfig},
};
use eyre::WrapErr;
use gray_matter::{engine::YAML, Matter};
use std::{env, fs, io, path::PathBuf};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let write_files = env::args().any(|arg| arg == "--write");
    let config = MirrorMigrationConfig::from_env();
    let mirror_target_config = MediaMirrorTargetConfig::from_env()
        .wrap_err("failed to read media mirror target config")?;
    let media_mirror = if write_files {
        let mirror_config =
            MediaMirrorConfig::from_env().wrap_err("failed to read media mirror config")?;
        Some(MediaMirror::new(mirror_config).await)
    } else {
        None
    };
    let mut summary = MigrationSummary::default();

    tracing::info!(
        notes_dir = %config.notes_dir().display(),
        mode = %if write_files { "write" } else { "dry-run" },
        "mastodon media mirror migration started"
    );

    for path in note_paths(config.notes_dir()).wrap_err("failed to list note files")? {
        summary.files_scanned += 1;

        let content = fs::read_to_string(&path)
            .wrap_err_with(|| format!("failed to read note {}", path.display()))?;
        let matter = Matter::<YAML>::new();
        let parsed = matter.parse(&content);
        let Some(front_matter_data) = parsed.data else {
            continue;
        };
        let mut front_matter = front_matter_data
            .deserialize::<NoteFrontMatter>()
            .wrap_err_with(|| format!("invalid front matter in {}", path.display()))?;
        let Some(media) = front_matter.media.as_mut() else {
            continue;
        };

        let mut changed = false;
        for (index, media) in media.iter_mut().enumerate() {
            summary.media_seen += 1;

            if mirror_target_config.is_mirrored_url(&media.url) {
                summary.skipped_existing += 1;
                continue;
            }

            let mirrored_url = if let Some(media_mirror) = &media_mirror {
                media_mirror
                    .mirror(&front_matter.source_id, index, &media.url)
                    .await
                    .wrap_err_with(|| format!("failed to mirror media {}", media.url))?
            } else {
                mirror_target_config.public_url_for(&front_matter.source_id, index, &media.url)
            };

            tracing::info!(
                path = %path.display(),
                source_id = %front_matter.source_id,
                source_url = %media.url,
                mirrored_url = %mirrored_url,
                mode = %if write_files { "write" } else { "dry-run" },
                "mastodon media mirror planned"
            );

            media.url = mirrored_url;
            changed = true;

            if write_files {
                summary.mirrored += 1;
            } else {
                summary.would_mirror += 1;
            }
        }

        if changed && write_files {
            fs::write(&path, note_markdown(&front_matter, &parsed.content))
                .wrap_err_with(|| format!("failed to write note {}", path.display()))?;
            summary.files_updated += 1;
        }
    }

    tracing::info!(
        files_scanned = summary.files_scanned,
        files_updated = summary.files_updated,
        media_seen = summary.media_seen,
        mirrored = summary.mirrored,
        would_mirror = summary.would_mirror,
        skipped_existing = summary.skipped_existing,
        mode = %if write_files { "write" } else { "dry-run" },
        "mastodon media mirror migration completed"
    );

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("mirror_mastodon_media=info"));

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .init();
}

#[derive(Debug)]
struct MirrorMigrationConfig {
    content_dir: PathBuf,
}

impl MirrorMigrationConfig {
    fn from_env() -> Self {
        Self {
            content_dir: env::var("CONTENT_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("content")),
        }
    }

    fn notes_dir(&self) -> PathBuf {
        self.content_dir.join("notes")
    }
}

#[derive(Default)]
struct MigrationSummary {
    files_scanned: usize,
    files_updated: usize,
    media_seen: usize,
    mirrored: usize,
    would_mirror: usize,
    skipped_existing: usize,
}

fn note_paths(notes_dir: PathBuf) -> io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for entry in fs::read_dir(notes_dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "md") {
            paths.push(path);
        }
    }

    paths.sort();
    Ok(paths)
}

fn note_markdown(front_matter: &NoteFrontMatter, body: &str) -> String {
    let mut lines = vec![
        "---".to_string(),
        format!("date: \"{}\"", yaml_escape(&front_matter.date)),
        format!("source: {}", note_source(front_matter)),
        format!("source_id: \"{}\"", yaml_escape(&front_matter.source_id)),
        format!("source_url: \"{}\"", yaml_escape(&front_matter.source_url)),
        format!("visibility: {}", note_visibility(front_matter)),
    ];

    if let Some(media) = &front_matter.media {
        if !media.is_empty() {
            lines.push("media:".to_string());
            for item in media {
                lines.push(format!("  - url: \"{}\"", yaml_escape(&item.url)));
                lines.extend(yaml_string_field("    alt", &item.alt));
            }
        }
    }

    if let Some(tags) = &front_matter.tags {
        if !tags.is_empty() {
            lines.push("tags:".to_string());
            for tag in tags {
                lines.push(format!("  - \"{}\"", yaml_escape(tag)));
            }
        }
    }

    lines.push("---".to_string());
    lines.push(String::new());
    lines.push(body.trim().to_string());
    lines.push(String::new());
    lines.join("\n")
}

fn note_visibility(front_matter: &NoteFrontMatter) -> &'static str {
    match &front_matter.visibility {
        NoteVisibility::Public => "public",
        NoteVisibility::Unlisted => "unlisted",
        NoteVisibility::Private => "private",
        NoteVisibility::Direct => "direct",
    }
}

fn note_source(front_matter: &NoteFrontMatter) -> &'static str {
    match &front_matter.source {
        NoteSource::Manual => "manual",
        NoteSource::Mastodon => "mastodon",
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use content_paulmcbride_com::content::note::NoteMedia;

    #[test]
    fn writes_note_markdown_with_mirrored_media() {
        let markdown = note_markdown(
            &NoteFrontMatter {
                date: "2026-06-18T20:30:00Z".to_string(),
                source: NoteSource::Mastodon,
                source_id: "123".to_string(),
                source_url: "https://example.social/@paul/123".to_string(),
                visibility: NoteVisibility::Public,
                media: Some(vec![NoteMedia {
                    url: "https://cdn.example.com/mastodon/123/1.jpg".to_string(),
                    alt: "Alt text".to_string(),
                }]),
                tags: Some(vec!["tag".to_string()]),
            },
            "Body",
        );

        assert!(markdown.contains("  - url: \"https://cdn.example.com/mastodon/123/1.jpg\""));
        assert!(markdown.contains("    alt: \"Alt text\""));
        assert!(markdown.contains("tags:\n  - \"tag\""));
    }
}
