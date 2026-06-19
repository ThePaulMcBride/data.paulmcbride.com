use chrono::NaiveDate;
use gray_matter::{engine::YAML, Matter};
use serde::{Deserialize, Serialize};
use std::{fmt, fs, io, path::PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostFrontMatter {
    pub date: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub description: String,
    pub banner: String,
    #[serde(rename = "lastUpdated")]
    pub last_updated: Option<String>,
    pub status: Option<String>,
    pub tags: Option<Vec<String>>,
    pub draft: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostSummary {
    #[serde(flatten)]
    pub front_matter: PostFrontMatter,
    pub slug: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Post {
    #[serde(flatten)]
    pub front_matter: PostFrontMatter,
    pub slug: String,
    pub body: String,
}

impl From<Post> for PostSummary {
    fn from(post: Post) -> Self {
        PostSummary {
            front_matter: post.front_matter,
            slug: post.slug,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PostIndex {
    posts: Vec<Post>,
}

impl PostIndex {
    pub fn load(content_dir: impl Into<PathBuf>) -> Result<Self, PostLoadError> {
        let posts_dir = content_dir.into().join("posts");
        let posts = FilesystemPostAdapter::new(posts_dir).load()?;

        Ok(Self { posts })
    }

    pub fn posts(&self) -> Vec<PostSummary> {
        self.posts
            .iter()
            .filter(|post| post.front_matter.draft != Some(true))
            .cloned()
            .map(PostSummary::from)
            .collect()
    }

    pub fn post(&self, slug: &str) -> Option<Post> {
        self.posts.iter().find(|post| post.slug == slug).cloned()
    }
}

#[derive(Debug)]
pub enum PostLoadError {
    ReadDirectory { path: PathBuf, source: io::Error },
    ReadFile { path: PathBuf, source: io::Error },
    MissingFrontMatter { path: PathBuf },
    InvalidFrontMatter { path: PathBuf, message: String },
    InvalidDate { path: PathBuf, date: String },
    MissingFileStem { path: PathBuf },
}

impl fmt::Display for PostLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadDirectory { path, source } => {
                write!(
                    f,
                    "unable to read posts directory {}: {}",
                    path.display(),
                    source
                )
            }
            Self::ReadFile { path, source } => {
                write!(f, "unable to read post {}: {}", path.display(), source)
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

impl std::error::Error for PostLoadError {}

struct FilesystemPostAdapter {
    posts_dir: PathBuf,
}

impl FilesystemPostAdapter {
    fn new(posts_dir: PathBuf) -> Self {
        Self { posts_dir }
    }

    fn load(&self) -> Result<Vec<Post>, PostLoadError> {
        let matter = Matter::<YAML>::new();
        let entries =
            fs::read_dir(&self.posts_dir).map_err(|source| PostLoadError::ReadDirectory {
                path: self.posts_dir.clone(),
                source,
            })?;

        let mut posts = entries
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
                    fs::read_to_string(&path).map_err(|source| PostLoadError::ReadFile {
                        path: path.clone(),
                        source,
                    })?;
                let result = matter.parse(&content);
                let front_matter_data = result
                    .data
                    .ok_or_else(|| PostLoadError::MissingFrontMatter { path: path.clone() })?;
                let front_matter =
                    front_matter_data
                        .deserialize::<PostFrontMatter>()
                        .map_err(|source| PostLoadError::InvalidFrontMatter {
                            path: path.clone(),
                            message: source.to_string(),
                        })?;

                NaiveDate::parse_from_str(&front_matter.date, "%Y-%m-%d").map_err(|_| {
                    PostLoadError::InvalidDate {
                        path: path.clone(),
                        date: front_matter.date.clone(),
                    }
                })?;

                let post_slug = path
                    .file_stem()
                    .ok_or_else(|| PostLoadError::MissingFileStem { path: path.clone() })?
                    .to_string_lossy()
                    .to_string();

                Ok(Post {
                    front_matter,
                    slug: post_slug,
                    body: result.content,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        posts.sort_by(|a, b| b.front_matter.date.cmp(&a.front_matter.date));

        Ok(posts)
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
            "content-index-test-{}-{}-{}",
            std::process::id(),
            id,
            counter
        ));
        create_dir_all(dir.join("posts")).expect("test posts dir can be created");
        dir
    }

    fn write_post(content_dir: &std::path::Path, name: &str, front_matter: &str) {
        write(
            content_dir.join("posts").join(name),
            format!("---\n{}---\n\nBody", front_matter),
        )
        .expect("test post can be written");
    }

    #[test]
    fn lists_only_published_posts_in_reverse_date_order() {
        let dir = test_content_dir();

        write_post(
            &dir,
            "older.mdx",
            "date: \"2023-01-01\"\ntitle: Older\ndescription: Older post\nbanner: /images/older.jpg\n",
        );
        write_post(
            &dir,
            "newer.mdx",
            "date: \"2024-01-01\"\ntitle: Newer\ndescription: Newer post\nbanner: /images/newer.jpg\n",
        );
        write_post(
            &dir,
            "draft.mdx",
            "date: \"2025-01-01\"\ntitle: Draft\ndescription: Draft post\nbanner: /images/draft.jpg\ndraft: true\n",
        );

        let index = PostIndex::load(&dir).expect("posts load");
        let posts = index.posts();

        assert_eq!(posts.len(), 2);
        assert_eq!(posts[0].slug, "newer");
        assert_eq!(posts[1].slug, "older");

        remove_dir_all(dir).expect("test dir can be removed");
    }

    #[test]
    fn rejects_invalid_dates() {
        let dir = test_content_dir();

        write_post(
            &dir,
            "bad-date.mdx",
            "date: \"not-a-date\"\ntitle: Bad\ndescription: Bad post\nbanner: /images/bad.jpg\n",
        );

        let error = PostIndex::load(&dir).expect_err("invalid date should fail");

        assert!(matches!(error, PostLoadError::InvalidDate { .. }));

        remove_dir_all(dir).expect("test dir can be removed");
    }
}
