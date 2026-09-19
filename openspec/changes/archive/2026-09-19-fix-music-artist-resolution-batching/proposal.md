# Proposal: fix-music-artist-resolution-batching

## Why

Grouped Music resolves album artists with one HTTP request *per album*
(`spawn_album_artist_fetch`, `ParentId={album}&Limit=5`). Measured against the
user's live Emby (2026-09-19, LAN): 5.36s for a 61-album level and 20.96s for a
142-album level at concurrency 6 — versus **0.10s / 0.38s** for a single
recursive Audio request per level. The N+1 path is so slow that the 3s
`SETTLE_WINDOW` always expires on large levels, so the majority-vote artist
feature mostly never runs and grouping falls back to folder-name parsing
nondeterministically. This is a LAN client; it should warm this data at
startup instead of discovering it lazily while the user waits.

## What Changes

- Replace the per-album artist fetch with **one recursive Audio request per
  music folder level** (`ParentId={level}&IncludeItemTypes=Audio&Recursive=
  true&Fields=AlbumArtist,Artists,ParentId&SortBy=ParentIndexNumber,
  IndexNumber`), bucketed by track `ParentId` into album buckets (measured
  exactly 1:1 with albums: 61/61 and 142/142).
- Apply the **same** first-5-tracks + majority-vote rule per bucket (with the
  same `Artists[0]` per-track fallback and first-seen tie-break). Proven
  semantically identical: batched vote vs per-album vote = 61/61 identical
  artists on the Hip-Hop level.
- Handle orphan buckets: tracks whose `ParentId` is a nested disc subfolder
  rather than an album at the level (observed 1 bucket in 243, a multi-disc
  set) by attributing them to their ancestor album at the level.
- **Warm the artist map at startup in the background**, one request per music
  level, after the Emby Service connects — so opening a grouped view finds the
  cache already filled. Per-level (not one whole-library request) keeps peak
  memory at ~3.8MB instead of ~26MB, consistent with the #656 footprint work.
- Keep `SETTLE_WINDOW` as a safety net for slow or failed level fetches; it
  becomes the rare path instead of the dominant one. The per-album pending
  queue and `MAX_ALBUM_ARTIST_FETCHES` concurrency limiter become vestigial
  and are removed; the candidate/settle machinery stays.

Explicitly **not** in scope: changing the grouping layer itself, the
`music_levels` configuration, the vote, the folder-name fallback, or the
folder-browse model, and no switch to Emby's native music endpoints
(`/Artists` etc.) — the grouping layer is deliberate; this fixes the
resolution mechanism only.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `stable-music-library-grouping`: adds a requirement that grouping artist
  metadata is warmed per music level at startup so a grouped view opens
  settled without waiting on resolution.

## Impact

- `src/app/image_fetch.rs` — per-album fetch replaced by per-level batch
  fetch; pending queue and concurrency limiter removed.
- `src/app/music_grouping.rs` — candidate/settle machinery retained; fill
  source becomes level-batch arrivals.
- `src/app/app_struct.rs`, `src/app/lib_event_actions.rs`,
  `src/app/emby_service_actions.rs` — cache/loading bookkeeping, new
  level-batch event, startup warm-up trigger on Service Ready, reset paths.
- `src/app/types_events.rs` — new `LibEvent` variant(s) for level-batch
  arrivals.
- Tests: `src/app/tests_music_grouping.rs` plus new mock-based unit tests for
  bucketing, orphan attribution, and the vote (no live servers).
- No dependency, config, or protocol changes.
