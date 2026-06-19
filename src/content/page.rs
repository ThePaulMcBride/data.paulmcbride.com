use gray_matter::{engine::YAML, Matter};
use serde::{Deserialize, Serialize};
use std::{fmt, fs, io, path::PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Page {
    pub slug: String,
    pub body: String,
}

#[derive(Clone, Debug)]
pub struct PageIndex {
    pages: Vec<Page>,
}

impl PageIndex {
    pub fn load(content_dir: impl Into<PathBuf>) -> Result<Self, PageLoadError> {
        let pages_dir = content_dir.into().join("pages");
        let pages = FilesystemPageAdapter::new(pages_dir).load()?;

        Ok(Self { pages })
    }

    pub fn page(&self, slug: &str) -> Option<Page> {
        self.pages.iter().find(|page| page.slug == slug).cloned()
    }
}

#[derive(Debug)]
pub enum PageLoadError {
    ReadDirectory { path: PathBuf, source: io::Error },
    ReadFile { path: PathBuf, source: io::Error },
    MissingFileStem { path: PathBuf },
}

impl fmt::Display for PageLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadDirectory { path, source } => {
                write!(
                    f,
                    "unable to read pages directory {}: {}",
                    path.display(),
                    source
                )
            }
            Self::ReadFile { path, source } => {
                write!(f, "unable to read page {}: {}", path.display(), source)
            }
            Self::MissingFileStem { path } => {
                write!(f, "missing file stem for {}", path.display())
            }
        }
    }
}

impl std::error::Error for PageLoadError {}

struct FilesystemPageAdapter {
    pages_dir: PathBuf,
}

impl FilesystemPageAdapter {
    fn new(pages_dir: PathBuf) -> Self {
        Self { pages_dir }
    }

    fn load(&self) -> Result<Vec<Page>, PageLoadError> {
        let matter = Matter::<YAML>::new();
        let entries = match fs::read_dir(&self.pages_dir) {
            Ok(entries) => entries,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => {
                return Err(PageLoadError::ReadDirectory {
                    path: self.pages_dir.clone(),
                    source,
                })
            }
        };

        entries
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
                    fs::read_to_string(&path).map_err(|source| PageLoadError::ReadFile {
                        path: path.clone(),
                        source,
                    })?;
                let result = matter.parse(&content);
                let slug = path
                    .file_stem()
                    .ok_or_else(|| PageLoadError::MissingFileStem { path: path.clone() })?
                    .to_string_lossy()
                    .to_string();

                Ok(Page {
                    slug,
                    body: result.content,
                })
            })
            .collect()
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
            "page-index-test-{}-{}-{}",
            std::process::id(),
            id,
            counter
        ));
        create_dir_all(dir.join("pages")).expect("test pages dir can be created");
        dir
    }

    #[test]
    fn loads_pages_by_slug() {
        let dir = test_content_dir();
        write(dir.join("pages/about.mdx"), "About body").expect("test page can be written");

        let index = PageIndex::load(&dir).expect("pages load");
        let page = index.page("about").expect("page exists");

        assert_eq!(page.slug, "about");
        assert_eq!(page.body, "About body");

        remove_dir_all(dir).expect("test dir can be removed");
    }

    #[test]
    fn missing_pages_directory_is_empty() {
        let dir = test_content_dir();
        remove_dir_all(dir.join("pages")).expect("test pages dir can be removed");

        let index = PageIndex::load(&dir).expect("pages load");

        assert!(index.page("missing").is_none());

        remove_dir_all(dir).expect("test dir can be removed");
    }
}
