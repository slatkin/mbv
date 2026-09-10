#!/usr/bin/env bash

# Self-test for scripts/check-media-list-ownership.sh: proves the inventory
# check fails against representative forbidden fixtures and passes on a
# conforming production-like tree (openspec/changes/
# complete-shared-media-list-ownership row 8.3).

set -u

readonly CHECKER="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/check-media-list-ownership.sh"
readonly TEST_ROOT="$(mktemp -d)"

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

# owner file|required shared-owner token (mirrors the checker's inventory)
readonly -a OWNERS=(
    'src/app/components/queue.rs|MediaListCarrier<'
    'src/app/components/home.rs|MediaListCarrier<'
    'src/app/components/browser/mod.rs|MediaListCarrier<'
    'src/app/components/music_workspace.rs|MediaListCarrier<'
    'src/app/components/music_workspace.rs|WideMediaList<'
    'src/app/components/tv_workspace/mod.rs|WideMediaList<'
    'src/app/components/feeds.rs|MediaListCarrier<'
    'src/app/components/audiobookshelf_podcast.rs|MediaListCarrier<'
    'src/app/components/audiobookshelf_podcast.rs|WideMediaList<'
    'src/app/components/audiobookshelf_book.rs|MediaListCarrier<'
    'src/app/components/audiobookshelf_book.rs|WideMediaList<'
)

seed_owners() {
    local root=$1
    local entry file token

    for entry in "${OWNERS[@]}"; do
        IFS='|' read -r file token <<<"$entry"
        mkdir -p "$root/$(dirname "$file")" || return 1
        printf 'struct Owner;\nfn own() -> %sString> { todo!() }\n' "$token" \
            >> "$root/$file" || return 1
    done
}

seed_components() {
    local root=$1

    mkdir -p "$root/src/app/components" || return 1
    printf '// legitimate parent chrome: season_cursor, pane focus, queue scope\n' \
        > "$root/src/app/components/benign.rs" || return 1
}

# A conforming production-like tree passes and reports every in-scope flow.
repo="$TEST_ROOT/pass"
seed_owners "$repo" || fail 'unable to seed conforming owners'
seed_components "$repo" || fail 'unable to seed conforming components'
if ! output=$(run_checker "$repo"); then
    fail "checker rejected a conforming inventory: $output"
fi
[[ "$output" == *'14 in-scope flows store the shared owner'* ]] ||
    fail 'checker did not report the full in-scope inventory'

# A flow whose owner file no longer stores the shared owner fails.
repo="$TEST_ROOT/missing-owner"
seed_owners "$repo" || fail 'unable to seed missing-owner owners'
seed_components "$repo" || fail 'unable to seed missing-owner components'
printf 'struct Destination;\n' > "$repo/src/app/components/tv_workspace/mod.rs" ||
    fail 'unable to blank the TV owner'
if output=$(run_checker "$repo"); then
    fail 'checker accepted a flow that lost the shared owner'
fi
[[ "$output" == *'TV series: src/app/components/tv_workspace/mod.rs does not store the shared owner'* ]] ||
    fail 'checker missed the missing TV series owner'

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
