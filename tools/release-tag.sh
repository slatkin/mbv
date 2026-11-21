#!/usr/bin/env bash
# Usage: tools/release-tag.sh <version>
# Example: tools/release-tag.sh 0.28.1
#
# Tags the merged release. On main, tools/release.sh already tags and pushes;
# run this only for a release committed on a feature branch, once that branch
# has merged: fetches origin/main, tags it, pushes the tag. The tag
# push is what triggers the PKGBUILD publish run (the tarball, the debs, the
# GitHub release, the AUR hand-off).
set -euo pipefail

VERSION="${1?Usage: tools/release-tag.sh <version>}"

# Normalize: strip a leading 'v'; the tag always carries it.
VERSION="${VERSION#v}"
TAG="v${VERSION}"

if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  echo "error: tag ${TAG} already exists locally"
  exit 1
fi

git fetch origin --quiet
if git ls-remote --exit-code --tags origin "refs/tags/${TAG}" >/dev/null 2>&1; then
  echo "error: tag ${TAG} already exists on origin"
  exit 1
fi

COMMIT="$(git rev-parse origin/main)"
echo "==> Tagging origin/main (${COMMIT}) as ${TAG}"
git tag "${TAG}" "${COMMIT}"

echo "==> git push origin ${TAG}..."
git push origin "${TAG}"

echo "==> Pushed ${TAG} at ${COMMIT}."
echo "    The PKGBUILD publish run: https://github.com/slatkin/mbv/actions"
