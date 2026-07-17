#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "${repo_root}"

git fetch origin
MBV_ALLOW_ROOT_BRANCH=1 git checkout main
git reset --hard origin/main

while read -r worktree; do
  if [[ -z "${worktree}" || "${worktree}" == "${repo_root}" ]]; then
    continue
  fi
  git worktree remove --force "${worktree}" || true
done < <(git worktree list --porcelain | awk '/^worktree / { print $2 }')

git worktree prune --verbose

while read -r branch; do
  if [[ -z "${branch}" || "${branch}" == "main" ]]; then
    continue
  fi
  git branch -D "${branch}"
done < <(git for-each-ref --format='%(refname:short)' refs/heads)
