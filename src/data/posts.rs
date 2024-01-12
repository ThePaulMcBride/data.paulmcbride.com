use std::fs;

use gray_matter::{engine::YAML, Matter};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostFrontMatter {
    pub date: String,
    pub title: String,
    pub description: String,
    pub banner: String,
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

#[derive(Clone)]
pub struct PostService {
    posts: Vec<Post>,
}
// implement into for Post to PostSummary
impl From<Post> for PostSummary {
    fn from(post: Post) -> Self {
        PostSummary {
            front_matter: post.front_matter,
            slug: post.slug,
        }
    }
}

impl PostService {
    pub fn load_from_disk() -> Self {
        let matter = Matter::<YAML>::new();
        let posts_dir = "./content/posts";

        let posts = fs::read_dir(posts_dir)
            .expect("Unable to read posts directory")
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .map_or(false, |ext| ext == "mdx" || ext == "md")
            })
            .map(|file| {
                let path = file.path();
                let content_string = fs::read_to_string(&path)
                    .map_err(|err| format!("Unable to read {}: {}", path.display(), err));
                let content = content_string.as_deref().unwrap_or_default();
                let result = matter.parse(content);
                let front_matter_data = result.data.expect(&format!(
                    "Unable to get front matter from {}",
                    path.display()
                ));
                let front_matter =
                    front_matter_data
                        .deserialize::<PostFrontMatter>()
                        .expect(&format!(
                            "Unable to deserialize front matter from {}",
                            path.display()
                        ));
                let post_slug = path.file_stem().unwrap().to_string_lossy().to_string();
                let slug = format!("/{}", post_slug);
                let body = result.content;
                Post {
                    front_matter,
                    slug,
                    body,
                }
            })
            .collect();

        Self { posts }
    }

    pub fn get_posts(&self) -> Vec<PostSummary> {
        self.posts
            .iter()
            .filter(|post| post.front_matter.draft != Some(true))
            .map(|post| post.clone().into())
            .collect()
    }

    pub fn get_post(&self, slug: &str) -> Option<Post> {
        let slug = format!("/{}", slug);
        self.posts
            .iter()
            .find(|post| post.slug == slug)
            .map(|post| post.clone())
    }
}
