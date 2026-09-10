#!/usr/bin/env bash

# Self-test for scripts/check-media-list-ownership.sh: proves the inventory
# check fails against representative forbidden fixtures and passes on a
# conforming production-like tree, asserting the per-flow result lines rather
# than only an aggregate total (openspec/changes/
# complete-shared-media-list-ownership row 8.3, design.md D7).

set -euo pipefail

CHECKER="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/check-media-list-ownership.sh"
readonly CHECKER
TEST_ROOT="$(mktemp -d)"
readonly TEST_ROOT

cleanup() {
    rm -rf "$TEST_ROOT"
}
trap cleanup EXIT

fail() {
    printf 'check-media-list-ownership-test: %s\n' "$1" >&2
    exit 1
}

run_checker() {
    local root=$1

    "$CHECKER" "$root" 2>&1
}

# owner file|required shared-owner field token (mirrors the checker's
# deduplicated inventory)
readonly -a OWNERS=(
    'src/app/components/queue.rs|: MediaListCarrier<'
    'src/app/components/home.rs|: MediaListCarrier<'
    'src/app/components/browser/mod.rs|: MediaListCarrier<'
    'src/app/components/music_workspace.rs|: MediaListCarrier<'
    'src/app/components/music_workspace.rs|: WideMediaList<'
    'src/app/components/tv_workspace/mod.rs|: WideMediaList<'
    'src/app/components/feeds.rs|: MediaListCarrier<'
    'src/app/components/audiobookshelf_podcast.rs|: MediaListCarrier<'
    'src/app/components/audiobookshelf_podcast.rs|: WideMediaList<'
    'src/app/components/audiobookshelf_book.rs|: MediaListCarrier<'
    'src/app/components/audiobookshelf_book.rs|: WideMediaList<'
)

# The deduplicated distinct-flow names the checker reports on success.
readonly -a FLOWS=(
    'queue slots'
    'home rows'
    'generic Emby catalog rows (Movies, homevideos)'
    'grouped Music albums'
    'grouped Music tracks'
    'TV series and episodes'
    'Feeds entries'
    'Podcast shows'
    'Podcast filtered episodes'
    'Book titles'
    'Book chapter/audio-part rows'
)

# Writes a *field-shaped* shared-owner declaration for every inventory entry.
seed_owners() {
    local root=$1
    local entry file token field=0

    for entry in "${OWNERS[@]}"; do
        IFS='|' read -r file token <<<"$entry"
        mkdir -p "$root/$(dirname "$file")" || return 1
        printf 'struct Owner;\nstruct Seed%s { carrier%sString> }\n' \
            "$field" "$token" >> "$root/$file" || return 1
        field=$((field + 1))
    done
}

seed_components() {
    local root=$1

    mkdir -p "$root/src/app/components" || return 1
    printf '// legitimate parent chrome: season_cursor, pane focus, queue scope\n' \
        > "$root/src/app/components/benign.rs" || return 1
}

# A conforming production-like tree passes, reports every flow by name, and
# reports the deduplicated count.
repo="$TEST_ROOT/pass"
seed_owners "$repo" || fail 'unable to seed conforming owners'
seed_components "$repo" || fail 'unable to seed conforming components'
if ! output=$(run_checker "$repo"); then
    fail "checker rejected a conforming inventory: $output"
fi
for flow in "${FLOWS[@]}"; do
    [[ "$output" == *"ok: $flow"* ]] ||
        fail "checker did not report the '$flow' flow"
done
[[ "$output" == *'11 in-scope flows store the shared owner'* ]] ||
    fail 'checker did not report the deduplicated in-scope count'
if [[ "$output" == *'14 in-scope flows'* ]]; then
    fail 'checker reported the inflated pre-dedup flow count'
fi

# A return-type-only owner is not a stored field and must fail even when every
# other flow stores its owner correctly.
repo="$TEST_ROOT/return-type-only"
seed_owners "$repo" || fail 'unable to seed return-type-only owners'
seed_components "$repo" || fail 'unable to seed return-type-only components'
printf 'struct Owner;\nfn own() -> WideMediaList<String> { todo!() }\n' \
    > "$repo/src/app/components/tv_workspace/mod.rs" ||
    fail 'unable to write the return-type-only fixture'
if output=$(run_checker "$repo"); then
    fail 'checker accepted a return-type-only shared owner'
fi
[[ "$output" == *'TV series and episodes: src/app/components/tv_workspace/mod.rs does not store the shared owner'* ]] ||
    fail 'checker missed the return-type-only TV owner'
[[ "$output" != *'ok: TV series and episodes'* ]] ||
    fail 'checker accepted the return-type-only TV owner as a stored field'

# A flow whose owner file no longer stores the shared owner fails.
repo="$TEST_ROOT/missing-owner"
seed_owners "$repo" || fail 'unable to seed missing-owner owners'
seed_components "$repo" || fail 'unable to seed missing-owner components'
printf 'struct Destination;\n' > "$repo/src/app/components/tv_workspace/mod.rs" ||
    fail 'unable to blank the TV owner'
if output=$(run_checker "$repo"); then
    fail 'checker accepted a flow that lost the shared owner'
fi
[[ "$output" == *'TV series and episodes: src/app/components/tv_workspace/mod.rs does not store the shared owner'* ]] ||
    fail 'checker missed the missing TV owner'

# A destination that reintroduces authoritative row state fails.
repo="$TEST_ROOT/destination-mirror"
seed_owners "$repo" || fail 'unable to seed mirror owners'
seed_components "$repo" || fail 'unable to seed mirror components'
printf 'struct Destination { track_cursor: usize }\n' \
    > "$repo/src/app/components/mirror.rs" || fail 'unable to write the mirror fixture'
if output=$(run_checker "$repo"); then
    fail 'checker accepted a destination row mirror'
fi
[[ "$output" == *'forbidden destination row-state identifier'*'track_cursor'* ]] ||
    fail 'checker missed the destination row mirror'

printf 'check-media-list-ownership-test: passed\n'
