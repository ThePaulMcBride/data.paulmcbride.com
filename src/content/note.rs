use chrono::DateTime;
use gray_matter::{engine::YAML, Matter};
use serde::{Deserialize, Serialize};
use std::{fmt, fs, io, path::PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoteFrontMatter {
    pub date: String,
    pub source: NoteSource,
    pub source_id: String,
    pub source_url: String,
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
