#!/usr/bin/env bash

# Neutrality proofs for the unify-surface-colour-neutral change (design D5).
#
# Two proofs, both required before and after every migration unit:
#
#   1. The multiset of raw `Color::Rgb(...)` literals across `src/` matches
#      the base ref's, except for the retirement unit's purpose-named
#      primitive splits (unify-surface-colour-neutral task 4.2): a listed
#      literal may appear exactly one more time than the base, and only if
#      the base already carries the same bytes (two names, identical bytes).
#      Any deletion or any unlisted delta is still a hard failure.
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

# Task 4.2's purpose-named primitive splits: each adds one literal whose bytes
# the base already carries under another name (same bytes, two names).
#   SOFT_CONTENT_BODY_BG / SCROLLBAR          (72, 88, 78)
#   SURFACE_FOCUSED_BG / BG_GREEN             (60, 72, 65)
#   ARTWORK_LOADING_PLACEHOLDER / OVERLAY     (63, 63, 63)
#   PLAYBACK_THROBBER_FG / AQUA               (53, 167, 124)
#   PILL_SELECTOR_SELECTED_BG / FOAM          (58, 148, 197)
readonly split_literals=(
    'Color::Rgb(72,88,78):1'
    'Color::Rgb(60,72,65):1'
    'Color::Rgb(63,63,63):1'
    'Color::Rgb(53,167,124):1'
    'Color::Rgb(58,148,197):1'
)

failures=0

# --- Proof 1: the Rgb literal multiset is unchanged (split literals excepted)

worktree_rgb="$(grep -rhoE "$rgb_pattern" "$root/src" | tr -d ' ' | sort)"
base_rgb="$(git -C "$root" grep -h -o -E "$rgb_pattern" "$base" -- src | tr -d ' ' | sort)"

# A listed split literal may exceed its base count by exactly the declared
# allowance, and only when the base already carries the same bytes (two
# names, identical bytes); every other difference — including any deletion
# or any unlisted delta — fails.
allowances_file="$(mktemp)"
trap 'rm -f "$allowances_file"' EXIT
for entry in "${split_literals[@]}"; do
    literal="${entry%:*}"
    allowed="${entry##*:}"
    base_count="$(printf '%s\n' "$base_rgb" | grep -cxF "$literal" || true)"
    work_count="$(printf '%s\n' "$worktree_rgb" | grep -cxF "$literal" || true)"
    if (( base_count == 0 )); then
        printf 'surface-colour-neutrality: split literal %s carries bytes the base does not have\n' "$literal" >&2
        failures=1
    elif (( work_count - base_count > allowed )); then
        printf 'surface-colour-neutrality: %s exceeds its declared split allowance (%d extra found, %d allowed)\n' \
            "$literal" "$((work_count - base_count))" "$allowed" >&2
        failures=1
    else
        printf '%s\t%d\n' "$literal" "$allowed" >>"$allowances_file"
    fi
done

# Drop at most the allowed number of copies of each split literal from the
# working tree's stream, then require exact multiset equality.
if ! diff \
    <(printf '%s\n' "$base_rgb") \
    <(awk -F'\t' 'NR==FNR { drop[$1]=$2; next } { if (drop[$0] > 0) { drop[$0]--; next } print }' \
        "$allowances_file" <(printf '%s\n' "$worktree_rgb")) \
    >/tmp/rgb-multiset.diff; then
    printf 'surface-colour-neutrality: Rgb literal multiset differs from %s\n' "$base" >&2
    printf '  (colour literals must not move; see design D5)\n' >&2
    cat /tmp/rgb-multiset.diff >&2
    failures=1
else
    printf 'surface-colour-neutrality: ok: Rgb literal multiset matches %s (plus declared splits)\n' "$base"
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
