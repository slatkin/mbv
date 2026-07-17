#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"

git config --local core.hooksPath .githooks
git config --local mbv.rootCheckout "${repo_root}"

echo "branch hygiene installed for ${repo_root}"
echo "hooks path: .githooks"
echo "root checkout: ${repo_root}"
