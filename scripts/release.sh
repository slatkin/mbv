#!/usr/bin/env bash
# Usage: scripts/release.sh <version> "<summary>"
# Example: scripts/release.sh 0.8.9 "add keyboard shortcuts for playback speed"
#
# Runs tests + clippy, bumps Cargo.toml, updates Cargo.lock, commits,
# and pushes. On main: pushes, tags, and pushes the tag. On a branch:
# commits only; merge via PR then tag manually.
set -euo pipefail

VERSION="${1?Usage: scripts/release.sh <version> \"<summary>\"}"
SUMMARY="${2?Usage: scripts/release.sh <version> \"<summary>\"}"

# Normalize: strip leading 'v' for Cargo.toml; v-prefix for tag and commit message
VERSION="${VERSION#v}"
TAG="v${VERSION}"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "error: working tree is not clean"
  echo "commit or stash existing changes before running a release"
  exit 1
fi

if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  echo "error: tag ${TAG} already exists locally"
  exit 1
fi

BRANCH="$(git branch --show-current)"
if [[ -z "${BRANCH}" ]]; then
  echo "error: detached HEAD is not supported for release prep"
  exit 1
fi



echo "==> Releasing ${TAG}: ${SUMMARY}"

echo
echo "==> cargo test..."
cargo test

echo
echo "==> cargo clippy..."
cargo clippy

echo
echo "==> Bumping Cargo.toml to ${VERSION}..."
sed -i "s/^version = \".*\"/version = \"${VERSION}\"/" Cargo.toml

echo
echo "==> cargo build (updates Cargo.lock)..."
cargo build

echo
echo "==> Committing..."
git add Cargo.toml Cargo.lock
git commit --no-verify -m "Release ${TAG}: ${SUMMARY}"

echo
echo "==> Release prep committed on ${BRANCH}."
if [[ "${BRANCH}" == "main" ]]; then
  echo
  echo "==> git push origin main..."
  git push origin main

  echo
  echo "==> git tag ${TAG}..."
  git tag "${TAG}"

  echo
  echo "==> git push origin ${TAG}..."
  git push origin "${TAG}"
else
  echo
  echo "==> Release committed on ${BRANCH} (not main)."
  echo "    PR-merge into main, then tag ${TAG} and push the tag manually."
fi
