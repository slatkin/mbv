#!/usr/bin/env bash
# Usage: scripts/release.sh <version> "<summary>"
# Example: scripts/release.sh 0.8.9 "add keyboard shortcuts for playback speed"
#
# Standard release-prep entrypoint for the protected-main workflow.
# Runs tests + clippy, bumps Cargo.toml, updates Cargo.lock, and commits
# the release change on the current branch. After that branch is merged,
# tag origin/main and push the tag to trigger the release workflow.
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
  echo "==> Next:"
  echo "    1. git push origin main"
  echo "    2. git tag ${TAG}"
  echo "    3. git push origin ${TAG}"
else
  echo "==> Next:"
  echo "    1. git push origin ${BRANCH}"
  echo "    2. Open and merge a pull request into main"
  echo "    3. git fetch origin"
  echo "    4. git tag ${TAG} origin/main"
  echo "    5. git push origin ${TAG}"
fi
echo
echo "==> The tag-triggered GitHub Action will then:"
echo "    - Build and test"
echo "    - Create the GitHub release and upload assets"
echo "    - Update PKGBUILD for AUR"
echo "    - Push to AUR"
