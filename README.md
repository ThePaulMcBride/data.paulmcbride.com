# data.paulmcbride.com

[![CI](https://github.com/ThePaulMcBride/data.paulmcbride.com/actions/workflows/ci.yml/badge.svg)](https://github.com/ThePaulMcBride/data.paulmcbride.com/actions/workflows/ci.yml)

Content API for paulmcbride.com.

The app serves static assets from `public/` and exposes JSON endpoints for content in `content/`.

## Requirements

- Rust 1.86 or newer
- Docker, optional

## Local Development

Run the server:

```sh
cargo run --bin web
```

Run checks:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

Run the Mastodon sync skeleton:

```sh
MASTODON_BASE_URL=https://example.social \
MASTODON_ACCESS_TOKEN=... \
MASTODON_ACCOUNT_ID=123456 \
cargo run --bin sync_mastodon
```

The sync command runs in dry-run mode by default. To write new note files to `CONTENT_DIR/notes`, pass `--write`:

```sh
cargo run --bin sync_mastodon -- --write
```

By default, the sync command runs incrementally. It fetches newest statuses first and stops paginating when it reaches a status that already exists in `CONTENT_DIR/notes`. If there are no existing notes yet, incremental mode fetches only the newest page.

For an initial backfill, pass `--full`:

```sh
cargo run --bin sync_mastodon -- --full --write
```

The sync command imports `public` and `unlisted` statuses, and skips `private` and `direct` statuses.

If Backblaze B2 S3-compatible media mirroring is configured, `sync_mastodon --write` downloads media attachments from Mastodon, uploads them to B2, and writes the mirrored public URLs into note front matter. Without the B2 variables, sync keeps the original Mastodon media URLs.

Mirror existing Mastodon media URLs in already-imported notes:

```sh
cargo run --bin mirror_mastodon_media
```

The migration command is dry-run by default. To upload media and rewrite note files, pass `--write`:

```sh
cargo run --bin mirror_mastodon_media -- --write
```

### Recurring Mastodon sync on Railway

The Docker image includes both binaries:

- `web`, the API server
- `sync_mastodon`, the Mastodon import command
- `sync-mastodon-and-push`, the Railway cron entrypoint

Deploy the recurring sync as a separate Railway cron service from the same repo as the API service, but with this command:

```sh
sync-mastodon-and-push
```

The repository includes a separate Railway config and Dockerfile for this service:

```sh
railway.mastodon-sync.toml
Dockerfile.sync
```

Use that config for the cron service so the API service can keep using `railway.toml` with its healthcheck.

Set the cron schedule in Railway, for example hourly. The cron command clones the repository, runs the Mastodon sync against the checkout, commits new files under `content/notes`, and pushes them to `main`. That push triggers Railway to rebuild and redeploy the API service with the new notes baked into the image.

Configure the cron service with these variables:

- `MASTODON_BASE_URL`
- `MASTODON_ACCESS_TOKEN`
- `MASTODON_ACCOUNT_ID`
- `GITHUB_ACCESS_TOKEN`, a token with permission to push to this repository

Configure these optional variables to mirror new Mastodon media into Backblaze B2 before note files are committed:

- `B2_S3_ENDPOINT`, for example `https://s3.us-west-004.backblazeb2.com`
- `B2_BUCKET`
- `B2_REGION`, defaults to `us-west-004`
- `B2_KEY_ID`
- `B2_APPLICATION_KEY`
- `B2_PUBLIC_BASE_URL`, for example a Backblaze public bucket URL or custom CDN domain
- `B2_KEY_PREFIX`, defaults to `mastodon`

Optional variables:

- `GITHUB_REPOSITORY`, defaults to `ThePaulMcBride/data.paulmcbride.com`
- `GIT_BRANCH`, defaults to `main`
- `COMMIT_MESSAGE`, defaults to `sync mastodon notes`

The cron command exits without committing when there are no new notes.

To test the cron flow locally:

```sh
GITHUB_ACCESS_TOKEN=... \
MASTODON_BASE_URL=https://example.social \
MASTODON_ACCESS_TOKEN=... \
MASTODON_ACCOUNT_ID=123456 \
scripts/sync-mastodon-and-push.sh
```

For local development, copy `.env.example` to `.env` and fill in local values. The binaries load `.env` automatically when it exists. `.env` files are ignored by Git.

## Configuration

The app reads configuration from environment variables.

| Variable | Default | Description |
| --- | --- | --- |
| `PORT` | `8000` | Port to listen on. The host is always `0.0.0.0`. |
| `CONTENT_DIR` | `content` | Directory containing content collections. |
| `PUBLIC_DIR` | `public` | Directory containing static assets. |

Invalid `PORT` values fail startup.

The Mastodon sync command also reads configuration from environment variables.

| Variable | Default | Description |
| --- | --- | --- |
| `MASTODON_BASE_URL` | Required | Base URL for the Mastodon instance. |
| `MASTODON_ACCESS_TOKEN` | Required | Access token for the Mastodon API. |
| `MASTODON_ACCOUNT_ID` | Required | Account ID to sync statuses from. |
| `CONTENT_DIR` | `content` | Directory containing content collections. |
| `B2_S3_ENDPOINT` | Optional | Backblaze B2 S3-compatible endpoint for media mirroring. |
| `B2_BUCKET` | Optional | B2 bucket to upload mirrored Mastodon media into. |
| `B2_REGION` | `us-west-004` | B2 S3 region. |
| `B2_KEY_ID` | Optional | B2 application key ID. |
| `B2_APPLICATION_KEY` | Optional | B2 application key secret. |
| `B2_PUBLIC_BASE_URL` | Optional | Public base URL used in note front matter after uploads. |
| `B2_KEY_PREFIX` | `mastodon` | Object key prefix for mirrored Mastodon media. |

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

Notes are loaded from `content/notes` at startup. If `content/notes` does not exist, the app starts with an empty note index.

Note slugs come from the note filename. Timestamp-based filenames are preferred:

```text
content/notes/2026-06-18-203000.md
```

Use UTC and format timestamps as `YYYY-MM-DD-HHMMSS`. If two imported notes share the same second, append the source ID:

```text
content/notes/2026-06-18-203000-123456789.md
```

Note front matter currently requires:

- `date`, formatted as RFC 3339, for example `2026-06-18T20:30:00Z`
- `source`, one of `manual` or `mastodon`
- `source_id`
- `source_url`
- `visibility`, one of `public`, `unlisted`, `private`, or `direct`

Optional note front matter:

- `media`, a list of objects with `url` and `alt`
- `tags`

Pages are loaded from `content/pages` at startup. If `content/pages` does not exist, the app starts with an empty page index. Page slugs come from filenames.

Now entries are loaded from `content/now` at startup. If `content/now` does not exist, the app starts with an empty now index.

Now entry front matter requires:

- `date`, formatted as `YYYY-MM-DD`
- `title`

## Endpoints

- `GET /health-check`
- `GET /posts`
- `GET /posts/`
- `GET /posts/:slug`
- `GET /notes`
- `GET /notes/`
- `GET /notes/:slug`
- `POST /notes/`
- `GET /pages/:slug`
- `GET /now`
- `GET /now/`
- `GET /now/:slug`

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
