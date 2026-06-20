#!/bin/sh
set -eu

: "${GITHUB_ACCESS_TOKEN:?GITHUB_TOKEN is required}"
: "${MASTODON_BASE_URL:?MASTODON_BASE_URL is required}"
: "${MASTODON_ACCESS_TOKEN:?MASTODON_ACCESS_TOKEN is required}"
: "${MASTODON_ACCOUNT_ID:?MASTODON_ACCOUNT_ID is required}"

repo="${GITHUB_REPOSITORY:-ThePaulMcBride/data.paulmcbride.com}"
branch="${GIT_BRANCH:-main}"
commit_message="${COMMIT_MESSAGE:-sync mastodon notes}"
workdir="$(mktemp -d)"
export GIT_TERMINAL_PROMPT=0

cleanup() {
  rm -rf "$workdir"
}
trap cleanup EXIT

git clone --depth 1 --branch "$branch" "https://x-access-token:${GITHUB_ACCESS_TOKEN}@github.com/${repo}.git" "$workdir/repo"
cd "$workdir/repo"

CONTENT_DIR=content sync_mastodon --write

if [ -z "$(git status --porcelain -- content/notes)" ]; then
  echo "No new notes to commit."
  exit 0
fi

git config user.name "mastodon-sync[bot]"
git config user.email "mastodon-sync[bot]@users.noreply.github.com"
git add content/notes
git commit -m "$commit_message"
git pull --rebase origin "$branch"
git push origin "HEAD:${branch}"
