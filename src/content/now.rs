use chrono::NaiveDate;
use gray_matter::{engine::YAML, Matter};
use serde::{Deserialize, Serialize};
use std::{fmt, fs, io, path::PathBuf};

use super::markdown::markdown_files;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NowFrontMatter {
    pub date: String,
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NowEntry {
    #[serde(flatten)]
    pub front_matter: NowFrontMatter,
    pub slug: String,
    pub body: String,
}

#[derive(Clone, Debug)]
pub struct NowIndex {
    entries: Vec<NowEntry>,
}

impl NowIndex {
    pub fn load(content_dir: impl Into<PathBuf>) -> Result<Self, NowLoadError> {
        let now_dir = content_dir.into().join("now");
        let entries = FilesystemNowAdapter::new(now_dir).load()?;

        Ok(Self { entries })
    }

    pub fn entries(&self) -> Vec<NowEntry> {
        self.entries.clone()
    }

    pub fn entry(&self, slug: &str) -> Option<NowEntry> {
        self.entries
            .iter()
            .find(|entry| entry.slug == slug)
            .cloned()
    }
}

#[derive(Debug)]
pub enum NowLoadError {
    ReadDirectory { path: PathBuf, source: io::Error },
    ReadFile { path: PathBuf, source: io::Error },
    MissingFrontMatter { path: PathBuf },
    InvalidFrontMatter { path: PathBuf, message: String },
    InvalidDate { path: PathBuf, date: String },
    MissingFileStem { path: PathBuf },
}

impl fmt::Display for NowLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadDirectory { path, source } => {
                write!(
                    f,
                    "unable to read now directory {}: {}",
                    path.display(),
                    source
                )
            }
            Self::ReadFile { path, source } => {
                write!(f, "unable to read now entry {}: {}", path.display(), source)
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

impl std::error::Error for NowLoadError {}

struct FilesystemNowAdapter {
    now_dir: PathBuf,
}

impl FilesystemNowAdapter {
    fn new(now_dir: PathBuf) -> Self {
        Self { now_dir }
    }

    fn load(&self) -> Result<Vec<NowEntry>, NowLoadError> {
        let matter = Matter::<YAML>::new();
        let paths =
            markdown_files(&self.now_dir, true).map_err(|source| NowLoadError::ReadDirectory {
                path: self.now_dir.clone(),
                source,
            })?;

        let mut entries = paths
            .into_iter()
            .map(|path| {
                let content =
                    fs::read_to_string(&path).map_err(|source| NowLoadError::ReadFile {
                        path: path.clone(),
                        source,
                    })?;
                let result = matter.parse(&content);
                let front_matter_data = result
                    .data
                    .ok_or_else(|| NowLoadError::MissingFrontMatter { path: path.clone() })?;
                let front_matter =
                    front_matter_data
                        .deserialize::<NowFrontMatter>()
                        .map_err(|source| NowLoadError::InvalidFrontMatter {
                            path: path.clone(),
                            message: source.to_string(),
                        })?;

                NaiveDate::parse_from_str(&front_matter.date, "%Y-%m-%d").map_err(|_| {
                    NowLoadError::InvalidDate {
                        path: path.clone(),
                        date: front_matter.date.clone(),
                    }
                })?;

                let slug = path
                    .file_stem()
                    .ok_or_else(|| NowLoadError::MissingFileStem { path: path.clone() })?
                    .to_string_lossy()
                    .to_string();

                Ok(NowEntry {
                    front_matter,
                    slug,
                    body: result.content,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        entries.sort_by(|a, b| b.front_matter.date.cmp(&a.front_matter.date));

        Ok(entries)
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
            "now-index-test-{}-{}-{}",
            std::process::id(),
            id,
            counter
        ));
        create_dir_all(dir.join("now")).expect("test now dir can be created");
        dir
    }

    fn write_now(content_dir: &std::path::Path, name: &str, front_matter: &str) {
        write(
            content_dir.join("now").join(name),
            format!("---\n{}---\n\nBody", front_matter),
        )
        .expect("test now entry can be written");
    }

    #[test]
    fn lists_entries_in_reverse_date_order() {
        let dir = test_content_dir();
        write_now(&dir, "older.mdx", "date: \"2023-01-01\"\ntitle: Older\n");
        write_now(&dir, "newer.mdx", "date: \"2024-01-01\"\ntitle: Newer\n");

        let index = NowIndex::load(&dir).expect("now entries load");
        let entries = index.entries();

        assert_eq!(entries[0].slug, "newer");
        assert_eq!(entries[1].slug, "older");

        remove_dir_all(dir).expect("test dir can be removed");
    }

    #[test]
    fn rejects_invalid_dates() {
        let dir = test_content_dir();
        write_now(&dir, "bad.mdx", "date: \"not-a-date\"\ntitle: Bad\n");

        let error = NowIndex::load(&dir).expect_err("invalid date should fail");

        assert!(matches!(error, NowLoadError::InvalidDate { .. }));

        remove_dir_all(dir).expect("test dir can be removed");
    }
}
