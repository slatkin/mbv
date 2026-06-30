#!/usr/bin/env bash
# Usage: scripts/release.sh <version> "<summary>"
# Example: scripts/release.sh 0.8.9 "add keyboard shortcuts for playback speed"
#
# Runs tests + clippy, bumps Cargo.toml, commits, pushes, and tags.
# The GitHub Action takes over from there: updates PKGBUILD sha256,
# uploads release assets, and pushes to AUR.
set -euo pipefail

VERSION="${1?Usage: scripts/release.sh <version> \"<summary>\"}"
SUMMARY="${2?Usage: scripts/release.sh <version> \"<summary>\"}"

# Normalize: strip leading 'v' for Cargo.toml; v-prefix for tag and commit message
VERSION="${VERSION#v}"
TAG="v${VERSION}"

echo "==> Releasing ${TAG}: ${SUMMARY}"

echo
echo "==> cargo test..."
cargo test

echo
echo "==> cargo clippy..."
cargo clippy -- -D warnings

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
git push

echo
echo "==> Tagging ${TAG} and pushing..."
git tag "${TAG}"
git push origin "${TAG}"

echo
echo "==> Done! The GitHub Action will now:"
echo "    - Build and test"
echo "    - Update PKGBUILD sha256 and commit to main"
echo "    - Create the GitHub release and upload assets"
echo "    - Push to AUR"
echo
echo "    Monitor: https://github.com/slatkin/mbv/actions"
