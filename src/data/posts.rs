use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Post {
    pub date: String,
    pub title: String,
    pub description: String,
    pub banner: String,
    pub tags: Option<Vec<String>>,
    pub draft: Option<bool>,
    pub slug: String,
    pub body: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]

pub struct PostSummary {
    pub date: String,
    pub title: String,
    pub description: String,
    pub banner: String,
    pub tags: Option<Vec<String>>,
    pub draft: Option<bool>,
    pub slug: String,
}

// implement into for Post to PostSummary
impl From<Post> for PostSummary {
    fn from(post: Post) -> Self {
        Self {
            date: post.date,
            title: post.title,
            description: post.description,
            banner: post.banner,
            tags: post.tags,
            draft: post.draft,
            slug: post.slug,
        }
    }
}

#[derive(Clone)]
pub struct PostService {
    posts: Vec<Post>,
}

// load posts from markdown files in content/posts
impl PostService {
    pub fn load_from_disk() -> Self {
        let posts = vec![
            Post {
                date: "2021-01-01".to_string(),
                title: "Hello, world!".to_string(),
                description: "This is a test post".to_string(),
                banner: "https://source.unsplash.com/random".to_string(),
                tags: Some(vec!["test".to_string()]),
                draft: Some(false),
                slug: "hello-world".to_string(),
                body: "This is a test post".to_string(),
            },
            Post {
                date: "2021-01-02".to_string(),
                title: "Hello, world!".to_string(),
                description: "This is a test post".to_string(),
                banner: "https://source.unsplash.com/random".to_string(),
                tags: Some(vec!["test".to_string()]),
                draft: Some(false),
                slug: "hello-world-2".to_string(),
                body: "This is a test post".to_string(),
            },
            Post {
                date: "2021-01-03".to_string(),
                title: "Hello, world!".to_string(),
                description: "This is a test post".to_string(),
                banner: "https://source.unsplash.com/random".to_string(),
                tags: Some(vec!["test".to_string()]),
                draft: Some(false),
                slug: "hello-world-3".to_string(),
                body: "This is a test post".to_string(),
            },
        ];

        Self { posts }
    }

    pub fn get_posts(&self) -> Vec<PostSummary> {
        self.posts.iter().map(|post| post.clone().into()).collect()
    }

    pub fn get_post(&self, slug: &str) -> Option<Post> {
        self.posts
            .iter()
            .find(|post| post.slug == slug)
            .map(|post| post.clone())
    }
}
