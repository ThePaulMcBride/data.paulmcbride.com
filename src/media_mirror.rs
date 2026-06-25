use aws_config::{BehaviorVersion, Region};
use aws_credential_types::Credentials;
use aws_sdk_s3::{config::Builder as S3ConfigBuilder, primitives::ByteStream, Client};
use reqwest::Url;
use std::{env, fmt};

#[derive(Clone, Debug)]
pub struct MediaMirrorConfig {
    endpoint_url: String,
    bucket: String,
    region: String,
    access_key_id: String,
    secret_access_key: String,
    target: MediaMirrorTargetConfig,
}

#[derive(Clone, Debug)]
pub struct MediaMirrorTargetConfig {
    public_base_url: String,
    key_prefix: String,
}

impl MediaMirrorConfig {
    pub fn optional_from_env() -> Result<Option<Self>, MediaMirrorConfigError> {
        let names = [
            "B2_S3_ENDPOINT",
            "B2_BUCKET",
            "B2_KEY_ID",
            "B2_APPLICATION_KEY",
            "B2_PUBLIC_BASE_URL",
        ];
        let any_present = names.iter().any(|name| env::var(name).is_ok());

        if !any_present {
            return Ok(None);
        }

        Ok(Some(Self::from_env()?))
    }

    pub fn from_env() -> Result<Self, MediaMirrorConfigError> {
        Ok(Self {
            endpoint_url: endpoint_env("B2_S3_ENDPOINT")?,
            bucket: required_env("B2_BUCKET")?,
            region: env::var("B2_REGION").unwrap_or_else(|_| "us-west-004".to_string()),
            access_key_id: required_env("B2_KEY_ID")?,
            secret_access_key: required_env("B2_APPLICATION_KEY")?,
            target: MediaMirrorTargetConfig::from_env()?,
        })
    }

    pub fn public_url_for(&self, source_id: &str, media_index: usize, source_url: &str) -> String {
        self.target
            .public_url_for(source_id, media_index, source_url)
    }

    pub fn is_mirrored_url(&self, url: &str) -> bool {
        self.target.is_mirrored_url(url)
    }
}

impl MediaMirrorTargetConfig {
    pub fn from_env() -> Result<Self, MediaMirrorConfigError> {
        Ok(Self {
            public_base_url: required_env("B2_PUBLIC_BASE_URL")?
                .trim_end_matches('/')
                .to_string(),
            key_prefix: env::var("B2_KEY_PREFIX").unwrap_or_else(|_| "mastodon".to_string()),
        })
    }

    pub fn public_url_for(&self, source_id: &str, media_index: usize, source_url: &str) -> String {
        format!(
            "{}/{}",
            self.public_base_url,
            self.object_key(source_id, media_index, source_url, None)
        )
    }

    pub fn is_mirrored_url(&self, url: &str) -> bool {
        url.starts_with(&format!("{}/", self.public_base_url))
    }

    fn object_key(
        &self,
        source_id: &str,
        media_index: usize,
        source_url: &str,
        content_type: Option<&str>,
    ) -> String {
        let extension = extension_from_url(source_url)
            .or_else(|| {
                content_type
                    .and_then(extension_from_content_type)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "bin".to_string());

        format!(
            "{}/{}/{}.{}",
            self.key_prefix.trim_matches('/'),
            sanitize_key_segment(source_id),
            media_index + 1,
            extension
        )
    }
}

pub struct MediaMirror {
    config: MediaMirrorConfig,
    http_client: reqwest::Client,
    s3_client: Client,
}

impl MediaMirror {
    pub async fn new(config: MediaMirrorConfig) -> Self {
        let credentials = Credentials::new(
            config.access_key_id.clone(),
            config.secret_access_key.clone(),
            None,
            None,
            "b2-static",
        );
        let sdk_config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(config.region.clone()))
            .credentials_provider(credentials)
            .endpoint_url(config.endpoint_url.clone())
            .load()
            .await;
        let s3_config = S3ConfigBuilder::from(&sdk_config)
            .force_path_style(true)
            .build();

        Self {
            config,
            http_client: reqwest::Client::new(),
            s3_client: Client::from_conf(s3_config),
        }
    }

    pub fn public_url_for(&self, source_id: &str, media_index: usize, source_url: &str) -> String {
        self.config
            .public_url_for(source_id, media_index, source_url)
    }

    pub fn is_mirrored_url(&self, url: &str) -> bool {
        self.config.is_mirrored_url(url)
    }

    pub async fn mirror(
        &self,
        source_id: &str,
        media_index: usize,
        source_url: &str,
    ) -> Result<String, MediaMirrorError> {
        if self.is_mirrored_url(source_url) {
            return Ok(source_url.to_string());
        }

        let response = self
            .http_client
            .get(source_url)
            .send()
            .await
            .map_err(MediaMirrorError::Download)?;
        let status = response.status();
        if !status.is_success() {
            return Err(MediaMirrorError::UnexpectedDownloadStatus { status });
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.split(';').next().unwrap_or(value).trim().to_string());
        let bytes = response.bytes().await.map_err(MediaMirrorError::Download)?;
        let object_key = self.config.target.object_key(
            source_id,
            media_index,
            source_url,
            content_type.as_deref(),
        );

        let mut request = self
            .s3_client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&object_key)
            .body(ByteStream::from(bytes));
        if let Some(content_type) = content_type {
            request = request.content_type(content_type);
        }

        request
            .send()
            .await
            .map_err(|source| MediaMirrorError::Upload(source.to_string()))?;

        Ok(format!(
            "{}/{}",
            self.config.target.public_base_url, object_key
        ))
    }
}

#[derive(Debug)]
pub enum MediaMirrorConfigError {
    MissingEnv {
        name: &'static str,
    },
    ReadEnv {
        name: &'static str,
        source: env::VarError,
    },
    InvalidEndpointUrl {
        name: &'static str,
        value: String,
        reason: &'static str,
    },
}

impl fmt::Display for MediaMirrorConfigError {
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
            Self::InvalidEndpointUrl {
                name,
                value,
                reason,
            } => write!(
                f,
                "invalid {} environment variable `{}`: {}",
                name, value, reason
            ),
        }
    }
}

impl std::error::Error for MediaMirrorConfigError {}

#[derive(Debug)]
pub enum MediaMirrorError {
    Download(reqwest::Error),
    UnexpectedDownloadStatus { status: reqwest::StatusCode },
    Upload(String),
}

impl fmt::Display for MediaMirrorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Download(source) => write!(f, "failed to download media: {}", source),
            Self::UnexpectedDownloadStatus { status } => {
                write!(f, "media download returned unexpected status: {}", status)
            }
            Self::Upload(source) => write!(f, "failed to upload media: {}", source),
        }
    }
}

impl std::error::Error for MediaMirrorError {}

fn required_env(name: &'static str) -> Result<String, MediaMirrorConfigError> {
    match env::var(name) {
        Ok(value) if !value.is_empty() => Ok(value),
        Ok(_) | Err(env::VarError::NotPresent) => Err(MediaMirrorConfigError::MissingEnv { name }),
        Err(source) => Err(MediaMirrorConfigError::ReadEnv { name, source }),
    }
}

fn endpoint_env(name: &'static str) -> Result<String, MediaMirrorConfigError> {
    let value = required_env(name)?;
    validate_endpoint_url(name, value)
}

fn validate_endpoint_url(
    name: &'static str,
    value: String,
) -> Result<String, MediaMirrorConfigError> {
    let url = Url::parse(&value).map_err(|_| MediaMirrorConfigError::InvalidEndpointUrl {
        name,
        value: value.clone(),
        reason: "expected an absolute URL such as https://s3.us-west-004.backblazeb2.com",
    })?;

    if !matches!(url.scheme(), "http" | "https") {
        return Err(MediaMirrorConfigError::InvalidEndpointUrl {
            name,
            value,
            reason: "expected http or https scheme",
        });
    }

    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return Err(MediaMirrorConfigError::InvalidEndpointUrl {
            name,
            value,
            reason: "expected the S3 service root without a bucket path, query, or fragment",
        });
    }

    Ok(value.trim_end_matches('/').to_string())
}

fn extension_from_url(url: &str) -> Option<String> {
    Url::parse(url)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(|mut segments| segments.next_back().map(str::to_string))
        })
        .and_then(|filename| {
            filename
                .rsplit_once('.')
                .map(|(_, extension)| extension.to_string())
        })
        .filter(|extension| {
            !extension.is_empty()
                && extension.len() <= 8
                && extension
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        })
}

fn extension_from_content_type(content_type: &str) -> Option<&'static str> {
    match content_type {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "video/mp4" => Some("mp4"),
        "video/webm" => Some("webm"),
        _ => mime_guess::get_mime_extensions_str(content_type)
            .and_then(|extensions| extensions.first().copied()),
    }
}

fn sanitize_key_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => character,
            _ => '-',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> MediaMirrorTargetConfig {
        MediaMirrorTargetConfig {
            public_base_url: "https://cdn.example.com".to_string(),
            key_prefix: "mastodon".to_string(),
        }
    }

    #[test]
    fn builds_public_url_from_source_id_and_media_index() {
        let url = config().public_url_for(
            "123456789",
            1,
            "https://cdn.masto.host/example/original/image.jpeg",
        );

        assert_eq!(url, "https://cdn.example.com/mastodon/123456789/2.jpeg");
    }

    #[test]
    fn detects_already_mirrored_urls() {
        assert!(config().is_mirrored_url("https://cdn.example.com/mastodon/123/1.jpg"));
        assert!(!config().is_mirrored_url("https://cdn.masto.host/example/image.jpg"));
    }

    #[test]
    fn falls_back_to_content_type_for_extension() {
        let key = config().object_key(
            "source/id",
            0,
            "https://example.com/media",
            Some("image/jpeg"),
        );

        assert_eq!(key, "mastodon/source-id/1.jpg");
    }

    #[test]
    fn validates_s3_endpoint_url_shape() {
        assert_eq!(
            validate_endpoint_url(
                "B2_S3_ENDPOINT",
                "https://s3.us-west-004.backblazeb2.com".to_string(),
            )
            .unwrap(),
            "https://s3.us-west-004.backblazeb2.com"
        );
        assert_eq!(
            validate_endpoint_url(
                "B2_S3_ENDPOINT",
                "https://s3.us-west-004.backblazeb2.com/".to_string(),
            )
            .unwrap(),
            "https://s3.us-west-004.backblazeb2.com"
        );
        assert!(validate_endpoint_url(
            "B2_S3_ENDPOINT",
            "s3.us-west-004.backblazeb2.com/media.paulmcbride.com".to_string(),
        )
        .is_err());
        assert!(validate_endpoint_url(
            "B2_S3_ENDPOINT",
            "https://s3.us-west-004.backblazeb2.com/media.paulmcbride.com".to_string(),
        )
        .is_err());
    }
}
