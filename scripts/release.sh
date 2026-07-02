#!/usr/bin/env bash
# Usage: scripts/release.sh <version> "<summary>"
# Example: scripts/release.sh 0.8.9 "add keyboard shortcuts for playback speed"
#
# Standard release entrypoint. Runs tests + clippy, bumps Cargo.toml,
# commits, pushes main, then tags and pushes the tag.
# The tag push is what causes GitHub Actions to create the release,
# upload assets, update PKGBUILD on main, and push to AUR.
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
git commit -m "Release ${TAG}: ${SUMMARY}"

echo
echo "==> Pushing..."
git push origin main

echo
echo "==> Tagging ${TAG} and pushing..."
git tag "${TAG}"
git push origin "${TAG}"

echo
echo "==> Done! The tag-triggered GitHub Action will now:"
echo "    - Build and test"
echo "    - Update PKGBUILD sha256 and commit to main"
echo "    - Create the GitHub release and upload assets"
echo "    - Push to AUR"
