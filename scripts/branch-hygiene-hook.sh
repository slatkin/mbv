#!/usr/bin/env bash
set -euo pipefail

HOOK="${1?hook name required}"
shift || true

branch="$(git branch --show-current 2>/dev/null || true)"
repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
root_checkout="$(git config --local --get mbv.rootCheckout 2>/dev/null || true)"

fail() {
  echo "branch hygiene: $*" >&2
  exit 1
}

is_root_checkout() {
  [[ -n "${root_checkout}" && "${repo_root}" == "${root_checkout}" ]]
}

case "${HOOK}" in
  pre-commit)
    if [[ -z "${branch}" ]]; then
      exit 0
    fi
    if [[ "${branch}" == "main" ]]; then
      fail "commits on main are blocked; create an isolated worktree branch from origin/main"
    fi
    if is_root_checkout; then
      fail "the root checkout must stay on main; commit from an isolated worktree instead"
    fi
    ;;
  pre-merge-commit)
    if [[ "${branch}" == "main" ]]; then
      fail "merging into local main is blocked; merge through origin/main instead"
    fi
    if is_root_checkout; then
      fail "the root checkout must stay on main; do merge work in an isolated worktree"
    fi
    ;;
  pre-rebase)
    if [[ "${branch}" == "main" ]]; then
      fail "rebasing local main is blocked; reset it to origin/main instead"
    fi
    if is_root_checkout; then
      fail "the root checkout must stay on main; rebase from an isolated worktree"
    fi
    ;;
  post-checkout)
    if [[ "${MBV_ALLOW_ROOT_BRANCH:-}" == "1" ]]; then
      exit 0
    fi
    if is_root_checkout && [[ "${branch}" != "main" ]]; then
      echo "branch hygiene: root checkout drifted to '${branch}', restoring 'main'" >&2
      MBV_ALLOW_ROOT_BRANCH=1 git checkout -q main >&2 || {
        echo "branch hygiene: automatic restore failed; run scripts/reset-root-checkout.sh" >&2
        exit 1
      }
    fi
    ;;
  *)
    fail "unknown hook '${HOOK}'"
    ;;
esac
