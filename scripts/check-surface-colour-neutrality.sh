#!/usr/bin/env bash

# Neutrality proofs for the unify-surface-colour-neutral change (design D5).
#
# Two proofs, both required before and after every migration unit:
#
#   1. The multiset of raw `Color::Rgb(...)` literals across `src/` is
#      identical to the base ref's. The surface table references existing role
#      values; it adds no hue. A count delta is only ever acceptable for a
#      purpose-named primitive split (the retirement unit), and that unit
#      narrows this check rather than deleting it.
#   2. No pre-existing test file differs from the base ref. main's buffer
#      expectations pass byte-identical, which is what pins today's values and
#      today's reactivity; the new table tests are additional files/modules,
#      never edits to an existing test.
#
# Usage: check-surface-colour-neutrality.sh [BASE_REF]
#   BASE_REF defaults to `origin/main`.
#
# The working tree is compared, so run it in the target worktree. Proof 1 reads
# plain files under `src/`, so an untracked new module is included; proof 2
# only looks at files the base ref already had.

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$script_dir/.." && pwd)"
base="${1:-origin/main}"
readonly rgb_pattern='Color::Rgb\([0-9]+, *[0-9]+, *[0-9]+\)'

failures=0

# --- Proof 1: the Rgb literal multiset is unchanged -------------------------

worktree_rgb="$(grep -rhoE "$rgb_pattern" "$root/src" | tr -d ' ' | sort)"
base_rgb="$(git -C "$root" grep -h -o -E "$rgb_pattern" "$base" -- src | tr -d ' ' | sort)"

if ! diff <(printf '%s\n' "$base_rgb") <(printf '%s\n' "$worktree_rgb") >/tmp/rgb-multiset.diff; then
    printf 'surface-colour-neutrality: Rgb literal multiset differs from %s\n' "$base" >&2
    printf '  (colour literals must not move; see design D5)\n' >&2
    cat /tmp/rgb-multiset.diff >&2
    failures=1
else
    printf 'surface-colour-neutrality: ok: Rgb literal multiset matches %s\n' "$base"
fi

# --- Proof 2: no pre-existing test file changed -----------------------------

mapfile -t test_files < <(git -C "$root" ls-tree -r --name-only "$base" -- src | grep -i 'test' || true)
if [[ "${#test_files[@]}" -eq 0 ]]; then
    printf 'surface-colour-neutrality: no test files found at %s\n' "$base" >&2
    exit 1
fi
if ! git -C "$root" diff --quiet "$base" -- "${test_files[@]}"; then
    printf 'surface-colour-neutrality: a pre-existing test file differs from %s\n' "$base" >&2
    git -C "$root" diff --stat "$base" -- "${test_files[@]}" >&2
    failures=1
else
    printf 'surface-colour-neutrality: ok: %d pre-existing test files unchanged\n' "${#test_files[@]}"
fi

if (( failures != 0 )); then
    exit 1
fi

printf 'surface-colour-neutrality: both proofs hold\n'
