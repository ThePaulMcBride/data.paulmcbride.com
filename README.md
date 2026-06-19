# data.paulmcbride.com

Content API for paulmcbride.com.

The app serves static assets from `public/` and exposes JSON endpoints for content in `content/`.

## Requirements

- Rust 1.85 or newer
- Docker, optional

## Local Development

Run the server:

```sh
cargo run
```

Run checks:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## Configuration

The app reads configuration from environment variables.

| Variable | Default | Description |
| --- | --- | --- |
| `PORT` | `8000` | Port to listen on. The host is always `0.0.0.0`. |
| `CONTENT_DIR` | `content` | Directory containing content collections. |
| `PUBLIC_DIR` | `public` | Directory containing static assets. |

Invalid `PORT` values fail startup.

## Content

Blog posts are loaded from `content/posts` at startup.

Supported file extensions:

- `.md`
- `.mdx`

Post front matter currently requires:

- `date`, formatted as `YYYY-MM-DD`
- `title`
- `description`
- `banner`

Optional post front matter:

- `tags`
- `draft`

Draft posts are excluded from the post list endpoint.

## Endpoints

- `GET /health-check`
- `GET /posts`
- `GET /posts/`
- `GET /posts/:slug`
- `GET /notes/`
- `POST /notes/`

## Docker

Build the image:

```sh
docker build -t data-paulmcbride-com .
```

Run the image:

```sh
docker run --rm -p 8000:8000 data-paulmcbride-com
```

Use a custom port inside the container:

```sh
docker run --rm -e PORT=8080 -p 8080:8080 data-paulmcbride-com
```
