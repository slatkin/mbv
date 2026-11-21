#!/usr/bin/env bash
# Usage: tools/release.sh <version> "<summary>"
# Example: tools/release.sh 0.8.9 "add keyboard shortcuts for playback speed"
#
# Runs the CI gate set, bumps Cargo.toml + PKGBUILD, updates Cargo.lock,
# commits, and pushes. On main: pushes, tags, and pushes the tag. On a branch:
# commits only; merge via PR, then tag with tools/release-tag.sh.
# PKGBUILD sha256sums is set to 'SKIP' here; build.yml fills in the real hash
# after building the release tarball.
#
# The gate set mirrors .github/workflows/build.yml step for step (same checks,
# same order; each echo names the CI step it matches). The release push is
# what triggers CI, so there is no earlier run to lean on: a check missing
# here is a red release run discovered after the tag exists. A failure in
# this script means the tag push would fail CI the same way — fix the cause,
# do not bypass the gate.
set -euo pipefail

VERSION="${1?Usage: tools/release.sh <version> \"<summary>\"}"
SUMMARY="${2?Usage: tools/release.sh <version> \"<summary>\"}"

# Normalize: strip leading 'v' for Cargo.toml; v-prefix for tag and commit message
VERSION="${VERSION#v}"
TAG="v${VERSION}"

# Uncommitted *tracked* changes would make the test run validate code that the
# tag does not contain. Untracked files never enter the release commit, but only
# untracked *.md notes are safe to ignore; any other untracked file could be a
# source the tag would silently omit.
if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
  echo "error: uncommitted changes to tracked files"
  echo "commit them before running a release (untracked *.md files are fine)"
  exit 1
fi
UNTRACKED_NON_MD="$(git ls-files --others --exclude-standard | grep -vE '\.md$' || true)"
if [[ -n "${UNTRACKED_NON_MD}" ]]; then
  echo "error: untracked non-markdown files would be left out of the release"
  echo "${UNTRACKED_NON_MD}"
  echo "commit or remove them before running a release (untracked *.md files are fine)"
  exit 1
fi

if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  echo "error: tag ${TAG} already exists locally"
  exit 1
fi

git fetch origin --quiet
if git ls-remote --exit-code --tags origin "refs/tags/${TAG}" >/dev/null 2>&1; then
  echo "error: tag ${TAG} already exists on origin"
  exit 1
fi

BRANCH="$(git branch --show-current)"
if [[ -z "${BRANCH}" ]]; then
  echo "error: detached HEAD is not supported for release prep"
  exit 1
fi



echo "==> Releasing ${TAG}: ${SUMMARY}"

echo
echo "==> cargo fmt --all -- --check (CI: Check formatting)..."
cargo fmt --all -- --check

echo
echo "==> cargo clippy --workspace --all-targets -- -D warnings (CI: Lint)..."
cargo clippy --workspace --all-targets -- -D warnings

echo
echo "==> ast-grep test + scan (CI: Ast-grep tripwires)..."
ast-grep test --skip-snapshot-tests
ast-grep scan .

echo
echo "==> cargo audit (CI: Audit dependencies)..."
cargo audit

echo
echo "==> cargo build --release --all-targets (CI: Build release)..."
cargo build --release --all-targets

echo
echo "==> cargo build --release -p mbv-gui (CI: Build GUI Client)..."
cargo build --release -p mbv-gui

echo
echo "==> cargo nextest run --release --test-threads=4 (CI: Test release)..."
# Same throttled invocation as CI: a third of the suite stands up a real
# libmpv handle or a loopback fixture per test, and unthrottled runs fail on
# fd/thread budget (see the CI step's comment).
cargo nextest run --release --test-threads=4

echo
echo "==> cargo nextest run --release --test-threads=4 -p mbv-gui (CI: Test mbv-gui)..."
cargo nextest run --release --test-threads=4 -p mbv-gui

echo
echo "==> cargo hack check --each-feature --workspace --all-targets (CI: Check feature matrix)..."
# The slow one: --each-feature re-checks the workspace once per feature.
# It gates every push in CI, so it gates a release too.
cargo hack check --each-feature --workspace --all-targets

echo
echo "==> Bumping Cargo.toml to ${VERSION}..."
sed -i "s/^version = \".*\"/version = \"${VERSION}\"/" Cargo.toml

echo
echo "==> Bumping PKGBUILD to ${VERSION}..."
sed -i "s/^pkgver=.*/pkgver=${VERSION}/" PKGBUILD
sed -i "s/^sha256sums=.*/sha256sums=('SKIP')/" PKGBUILD

echo
echo "==> cargo build (updates Cargo.lock)..."
cargo build

echo
echo "==> Committing..."
git add Cargo.toml Cargo.lock PKGBUILD
git commit --no-verify --allow-empty -m "Release ${TAG}: ${SUMMARY}"

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
  echo "    PR-merge into main, then tag with tools/release-tag.sh ${TAG}."
fi
