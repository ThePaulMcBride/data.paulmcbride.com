use chrono::DateTime;
use gray_matter::{engine::YAML, Matter};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, fs, io, path::PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoteFrontMatter {
    pub date: String,
    pub source: NoteSource,
    pub source_id: String,
    pub source_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_reply_to_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_reply_to_account_id: Option<String>,
    pub visibility: NoteVisibility,
    pub media: Option<Vec<NoteMedia>>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum NoteSource {
    Manual,
    Mastodon,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum NoteVisibility {
    Public,
    Unlisted,
    Private,
    Direct,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoteMedia {
    pub url: String,
    pub alt: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoteSummary {
    #[serde(flatten)]
    pub front_matter: NoteFrontMatter,
    pub slug: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Note {
    #[serde(flatten)]
    pub front_matter: NoteFrontMatter,
    pub slug: String,
    pub body: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoteGroup {
    pub notes: Vec<Note>,
}

#[derive(Debug, Serialize, Clone)]
pub struct NotePage {
    pub items: Vec<NoteGroup>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_cursor: Option<String>,
}

impl From<Note> for NoteSummary {
    fn from(note: Note) -> Self {
        Self {
            front_matter: note.front_matter,
            slug: note.slug,
        }
    }
}

#[derive(Clone, Debug)]
pub struct NoteIndex {
    notes: Vec<Note>,
}

impl NoteIndex {
    pub fn load(content_dir: impl Into<PathBuf>) -> Result<Self, NoteLoadError> {
        let notes_dir = content_dir.into().join("notes");
        let notes = FilesystemNoteAdapter::new(notes_dir).load()?;

        Ok(Self { notes })
    }

    pub fn notes(&self) -> Vec<NoteSummary> {
        self.notes.iter().cloned().map(NoteSummary::from).collect()
    }

    pub fn note(&self, slug: &str) -> Option<Note> {
        self.notes.iter().find(|note| note.slug == slug).cloned()
    }

    pub fn note_group(&self, slug: &str) -> Option<NoteGroup> {
        self.note_groups()
            .into_iter()
            .find(|group| group.notes.iter().any(|note| note.slug == slug))
    }

    pub fn note_groups(&self) -> Vec<NoteGroup> {
        let note_by_source_id: HashMap<&str, &Note> = self
            .notes
            .iter()
            .map(|note| (note.front_matter.source_id.as_str(), note))
            .collect();
        let mut grouped_notes: HashMap<String, Vec<Note>> = HashMap::new();

        for note in &self.notes {
            let root_id = thread_root_id(note, &note_by_source_id);
            grouped_notes.entry(root_id).or_default().push(note.clone());
        }

        let mut groups = grouped_notes
            .into_values()
            .map(|mut notes| {
                notes.sort_by(|a, b| {
                    let a_date = DateTime::parse_from_rfc3339(&a.front_matter.date)
                        .expect("note date was validated during load");
                    let b_date = DateTime::parse_from_rfc3339(&b.front_matter.date)
                        .expect("note date was validated during load");

                    a_date.cmp(&b_date)
                });

                NoteGroup { notes }
            })
            .collect::<Vec<_>>();

        groups.sort_by(|a, b| {
            let a_date = group_latest_date(a);
            let b_date = group_latest_date(b);

            b_date.cmp(&a_date)
        });

        groups
    }

    pub fn note_page(&self, after: Option<&str>, limit: usize) -> NotePage {
        let groups = self.note_groups();
        let limit = limit.clamp(1, 100);
        let cursor_index = after.and_then(|cursor| {
            groups
                .iter()
                .position(|group| group_contains_slug(group, cursor))
        });
        let start_index = cursor_index.map(|index| index + 1).unwrap_or(0);
        let items = groups
            .iter()
            .skip(start_index)
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let next_cursor = if start_index + limit < groups.len() {
            items.last().and_then(group_cursor)
        } else {
            None
        };
        let previous_page_start = start_index.saturating_sub(limit);
        let previous_cursor = if start_index == 0 {
            None
        } else if previous_page_start == 0 {
            Some(String::new())
        } else {
            groups.get(previous_page_start - 1).and_then(group_cursor)
        };

        NotePage {
            items,
            next_cursor,
            previous_cursor,
        }
    }
}

fn group_contains_slug(group: &NoteGroup, slug: &str) -> bool {
    group.notes.iter().any(|note| note.slug == slug)
}

fn group_cursor(group: &NoteGroup) -> Option<String> {
    group.notes.last().map(|note| note.slug.clone())
}

fn thread_root_id<'a>(note: &'a Note, note_by_source_id: &HashMap<&str, &'a Note>) -> String {
    let mut current = note;
    let mut seen = Vec::new();

    while let Some(parent_id) = current.front_matter.in_reply_to_id.as_deref() {
        if seen.contains(&parent_id) {
            break;
        }
        seen.push(parent_id);

        let Some(parent) = note_by_source_id.get(parent_id).copied() else {
            break;
        };

        current = parent;
    }

    current.front_matter.source_id.clone()
}

fn group_latest_date(group: &NoteGroup) -> DateTime<chrono::FixedOffset> {
    let latest = group
        .notes
        .last()
        .expect("note groups always contain at least one note");

    DateTime::parse_from_rfc3339(&latest.front_matter.date)
        .expect("note date was validated during load")
}

#[derive(Debug)]
pub enum NoteLoadError {
    ReadDirectory { path: PathBuf, source: io::Error },
    ReadFile { path: PathBuf, source: io::Error },
    MissingFrontMatter { path: PathBuf },
    InvalidFrontMatter { path: PathBuf, message: String },
    InvalidDate { path: PathBuf, date: String },
    MissingFileStem { path: PathBuf },
}

impl fmt::Display for NoteLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadDirectory { path, source } => {
                write!(
                    f,
                    "unable to read notes directory {}: {}",
                    path.display(),
                    source
                )
            }
            Self::ReadFile { path, source } => {
                write!(f, "unable to read note {}: {}", path.display(), source)
            }
            Self::MissingFrontMatter { path } => {
                write!(f, "missing front matter in {}", path.display())
            }
            Self::InvalidFrontMatter { path, message } => {
                write!(f, "invalid front matter in {}: {}", path.display(), message)
            }
            Self::InvalidDate { path, date } => {
                write!(f, "invalid date '{}' in {}", date, path.display())
            }
            Self::MissingFileStem { path } => {
                write!(f, "missing file stem for {}", path.display())
            }
        }
    }
}

impl std::error::Error for NoteLoadError {}

struct FilesystemNoteAdapter {
    notes_dir: PathBuf,
}

impl FilesystemNoteAdapter {
    fn new(notes_dir: PathBuf) -> Self {
        Self { notes_dir }
    }

    fn load(&self) -> Result<Vec<Note>, NoteLoadError> {
        let matter = Matter::<YAML>::new();
        let entries = match fs::read_dir(&self.notes_dir) {
            Ok(entries) => entries,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => {
                return Err(NoteLoadError::ReadDirectory {
                    path: self.notes_dir.clone(),
                    source,
                })
            }
        };

        let mut notes = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext == "mdx" || ext == "md")
            })
            .map(|file| {
                let path = file.path();
                let content =
                    fs::read_to_string(&path).map_err(|source| NoteLoadError::ReadFile {
                        path: path.clone(),
                        source,
                    })?;
                let result = matter.parse(&content);
                let front_matter_data = result
                    .data
                    .ok_or_else(|| NoteLoadError::MissingFrontMatter { path: path.clone() })?;
                let front_matter =
                    front_matter_data
                        .deserialize::<NoteFrontMatter>()
                        .map_err(|source| NoteLoadError::InvalidFrontMatter {
                            path: path.clone(),
                            message: source.to_string(),
                        })?;

                DateTime::parse_from_rfc3339(&front_matter.date).map_err(|_| {
                    NoteLoadError::InvalidDate {
                        path: path.clone(),
                        date: front_matter.date.clone(),
                    }
                })?;

                let note_slug = path
                    .file_stem()
                    .ok_or_else(|| NoteLoadError::MissingFileStem { path: path.clone() })?
                    .to_string_lossy()
                    .to_string();

                Ok(Note {
                    front_matter,
                    slug: note_slug,
                    body: result.content,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        notes.sort_by(|a, b| {
            let a_date = DateTime::parse_from_rfc3339(&a.front_matter.date)
                .expect("note date was validated during load");
            let b_date = DateTime::parse_from_rfc3339(&b.front_matter.date)
                .expect("note date was validated during load");

            b_date.cmp(&a_date)
        });

        Ok(notes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::{create_dir_all, remove_dir_all, write},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn test_content_dir() -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock is valid")
            .as_nanos();
        let counter = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "note-index-test-{}-{}-{}",
            std::process::id(),
            id,
            counter
        ));
        create_dir_all(dir.join("notes")).expect("test notes dir can be created");
        dir
    }

    fn write_note(content_dir: &std::path::Path, name: &str, front_matter: &str) {
        write(
            content_dir.join("notes").join(name),
            format!("---\n{}---\n\nBody", front_matter),
        )
        .expect("test note can be written");
    }

    #[test]
    fn lists_notes_in_reverse_date_order() {
        let dir = test_content_dir();

        write_note(
            &dir,
            "older.md",
            "date: \"2024-01-01T10:00:00Z\"\nsource: mastodon\nsource_id: \"1\"\nsource_url: https://example.com/1\nvisibility: public\n",
        );
        write_note(
            &dir,
            "newer.md",
            "date: \"2024-01-01T11:00:00Z\"\nsource: mastodon\nsource_id: \"2\"\nsource_url: https://example.com/2\nvisibility: unlisted\nmedia:\n  - url: /media/notes/image.jpg\n    alt: Image alt text\n",
        );

        let index = NoteIndex::load(&dir).expect("notes load");
        let notes = index.notes();

        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].slug, "newer");
        assert_eq!(notes[1].slug, "older");
        assert_eq!(notes[0].front_matter.source_id, "2");
        assert_eq!(
            notes[0].front_matter.media.as_ref().expect("media exists")[0].alt,
            "Image alt text"
        );

        remove_dir_all(dir).expect("test dir can be removed");
    }

    #[test]
    fn groups_thread_replies_with_their_root_note() {
        let dir = test_content_dir();

        write_note(
            &dir,
            "standalone.md",
            "date: \"2024-01-01T12:00:00Z\"\nsource: mastodon\nsource_id: \"3\"\nsource_url: https://example.com/3\nvisibility: public\n",
        );
        write_note(
            &dir,
            "root.md",
            "date: \"2024-01-01T10:00:00Z\"\nsource: mastodon\nsource_id: \"1\"\nsource_url: https://example.com/1\nvisibility: public\n",
        );
        write_note(
            &dir,
            "reply.md",
            "date: \"2024-01-01T11:00:00Z\"\nsource: mastodon\nsource_id: \"2\"\nsource_url: https://example.com/2\nin_reply_to_id: \"1\"\nin_reply_to_account_id: \"account\"\nvisibility: public\n",
        );

        let index = NoteIndex::load(&dir).expect("notes load");
        let groups = index.note_groups();

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].notes[0].slug, "standalone");
        assert_eq!(groups[1].notes[0].slug, "root");
        assert_eq!(groups[1].notes[1].slug, "reply");

        remove_dir_all(dir).expect("test dir can be removed");
    }

    #[test]
    fn finds_note_group_by_any_thread_slug() {
        let dir = test_content_dir();

        write_note(
            &dir,
            "root.md",
            "date: \"2024-01-01T10:00:00Z\"\nsource: mastodon\nsource_id: \"1\"\nsource_url: https://example.com/1\nvisibility: public\n",
        );
        write_note(
            &dir,
            "reply.md",
            "date: \"2024-01-01T11:00:00Z\"\nsource: mastodon\nsource_id: \"2\"\nsource_url: https://example.com/2\nin_reply_to_id: \"1\"\nin_reply_to_account_id: \"account\"\nvisibility: public\n",
        );

        let index = NoteIndex::load(&dir).expect("notes load");
        let group = index.note_group("reply").expect("group exists");

        assert_eq!(group.notes[0].slug, "root");
        assert_eq!(group.notes[1].slug, "reply");

        remove_dir_all(dir).expect("test dir can be removed");
    }

    #[test]
    fn pages_note_groups_without_splitting_threads() {
        let dir = test_content_dir();

        write_note(
            &dir,
            "newer.md",
            "date: \"2024-01-01T12:00:00Z\"\nsource: mastodon\nsource_id: \"3\"\nsource_url: https://example.com/3\nvisibility: public\n",
        );
        write_note(
            &dir,
            "root.md",
            "date: \"2024-01-01T10:00:00Z\"\nsource: mastodon\nsource_id: \"1\"\nsource_url: https://example.com/1\nvisibility: public\n",
        );
        write_note(
            &dir,
            "reply.md",
            "date: \"2024-01-01T11:00:00Z\"\nsource: mastodon\nsource_id: \"2\"\nsource_url: https://example.com/2\nin_reply_to_id: \"1\"\nin_reply_to_account_id: \"account\"\nvisibility: public\n",
        );

        let index = NoteIndex::load(&dir).expect("notes load");
        let first_page = index.note_page(None, 1);
        let second_page = index.note_page(first_page.next_cursor.as_deref(), 1);

        assert_eq!(first_page.items.len(), 1);
        assert_eq!(first_page.items[0].notes[0].slug, "newer");
        assert_eq!(first_page.next_cursor.as_deref(), Some("newer"));
        assert_eq!(second_page.items.len(), 1);
        assert_eq!(second_page.items[0].notes[0].slug, "root");
        assert_eq!(second_page.items[0].notes[1].slug, "reply");
        assert_eq!(second_page.previous_cursor.as_deref(), Some(""));

        remove_dir_all(dir).expect("test dir can be removed");
    }

    #[test]
    fn missing_notes_directory_is_empty() {
        let dir = std::env::temp_dir().join(format!(
            "note-index-missing-dir-test-{}-{}",
            std::process::id(),
            TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));

        let index = NoteIndex::load(&dir).expect("missing notes dir loads empty index");

        assert!(index.notes().is_empty());
    }

    #[test]
    fn rejects_invalid_dates() {
        let dir = test_content_dir();

        write_note(
            &dir,
            "bad-date.md",
            "date: \"not-a-date\"\nsource: mastodon\nsource_id: \"1\"\nsource_url: https://example.com/1\nvisibility: public\n",
        );

        let error = NoteIndex::load(&dir).expect_err("invalid date should fail");

        assert!(matches!(error, NoteLoadError::InvalidDate { .. }));

        remove_dir_all(dir).expect("test dir can be removed");
    }
}
