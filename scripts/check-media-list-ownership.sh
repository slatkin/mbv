#!/usr/bin/env bash

# Architecture inventory for the canonical media-list change
# (openspec/changes/complete-shared-media-list-ownership, row 8.3 / design.md
# D7). It is deliberately an *explicit* inventory of every in-scope logical
# media-row flow, not a scan for arbitrary field names, because destinations
# legitimately keep chrome state (section/pane focus, season cursors, queue
# scope) that must not be mistaken for a row mirror.
#
# Two assertions:
#   1. Each in-scope flow's production owner file stores the shared owner.
#   2. No destination component stores the legacy authoritative row-state
#      identifiers removed with the destination mirrors.
#
# Usage: check-media-list-ownership.sh [ROOT]
#   ROOT defaults to the repository root (the parent of this script's dir).

set -u

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="${1:-$(cd "$script_dir/.." && pwd)}"

# flow|owner file|required shared-owner token
readonly -a INVENTORY=(
    'queue slots|src/app/components/queue.rs|MediaListCarrier<'
    'home rows|src/app/components/home.rs|MediaListCarrier<'
    'generic Emby catalog rows|src/app/components/browser/mod.rs|MediaListCarrier<'
    'Movies|src/app/components/browser/mod.rs|MediaListCarrier<'
    'Emby homevideos feed view|src/app/components/browser/mod.rs|MediaListCarrier<'
    'grouped Music albums|src/app/components/music_workspace.rs|MediaListCarrier<'
    'grouped Music tracks|src/app/components/music_workspace.rs|WideMediaList<'
    'TV series|src/app/components/tv_workspace/mod.rs|WideMediaList<'
    'TV episodes|src/app/components/tv_workspace/mod.rs|WideMediaList<'
    'Feeds entries|src/app/components/feeds.rs|MediaListCarrier<'
    'Podcast shows|src/app/components/audiobookshelf_podcast.rs|MediaListCarrier<'
    'Podcast filtered episodes|src/app/components/audiobookshelf_podcast.rs|WideMediaList<'
    'Book titles|src/app/components/audiobookshelf_book.rs|MediaListCarrier<'
    'Book chapter/audio-part rows|src/app/components/audiobookshelf_book.rs|WideMediaList<'
)

# Authoritative media-row state identifiers that the migration removed. A
# destination that reintroduces one is storing a second row authority.
readonly -a FORBIDDEN=(
    'owns_canonical_position'
    'track_cursor'
    'episode_selection'
    'chapter_selection'
    'browser_offset'
    'painted_row_offset'
    'content_cursor'
)

failures=0

for entry in "${INVENTORY[@]}"; do
    IFS='|' read -r flow file token <<<"$entry"
    path="$root/$file"
    if [[ ! -f "$path" ]]; then
        printf 'media-list-ownership: %s: missing owner file %s\n' "$flow" "$file" >&2
        failures=1
        continue
    fi
    if ! grep -Fq -- "$token" "$path"; then
        printf 'media-list-ownership: %s: %s does not store the shared owner (%s)\n' \
            "$flow" "$file" "$token" >&2
        failures=1
    fi
done

components="$root/src/app/components"
if [[ ! -d "$components" ]]; then
    printf 'media-list-ownership: missing destination component directory %s\n' \
        "$components" >&2
    exit 1
fi

for token in "${FORBIDDEN[@]}"; do
    while IFS= read -r hit; do
        [[ -z "$hit" ]] && continue
        printf 'media-list-ownership: forbidden destination row-state identifier: %s\n' \
            "$hit" >&2
        failures=1
    done < <(grep -RFn --exclude='*tests*.rs' --exclude='*_test_support.rs' \
        --exclude='*test_helpers*.rs' -- "$token" "$components" 2>/dev/null || true)
done

if (( failures != 0 )); then
    exit 1
fi

printf 'media-list-ownership: %d in-scope flows store the shared owner and no destination row mirror remains\n' \
    "${#INVENTORY[@]}"
