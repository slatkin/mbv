# Design: fix-music-artist-resolution-batching

## Context

See proposal.md for motivation and measurements. Current state:
`spawn_album_artist_fetch` (src/app/image_fetch.rs) issues one HTTP request
per album (`ParentId={album}&IncludeItemTypes=Audio&Limit=5&SortBy=
ParentIndexNumber,IndexNumber&Fields=AlbumArtist,Artists`) from
`music_grouping.rs`'s candidate creation, throttled by
`MAX_ALBUM_ARTIST_FETCHES = 6` with a `pending_album_artist_fetches` queue;
results arrive as `LibEvent::AlbumArtistFetched { album_id, artist }` and
fill `album_artist_cache` (in-memory, keyed by album folder id, cleared on
Service reset). `MusicGroupCandidate` waits for all albums to reach a
terminal identity, forced by `SETTLE_WINDOW = 3s`.

Verified constraints (measured against the live server, recorded in
handoff.md): album-level items are plain `Folder` with no `AlbumArtist`
(folder-view music library), so track-derived artists are genuinely required;
`ParentId` rejects comma lists (HTTP 500), so the batch unit must be an
ancestor folder; bucketing a level's Audio rows by `ParentId` is exactly 1:1
with albums; the batched vote is semantically identical to the per-album
vote (61/61); nested disc subfolders produce rare orphan buckets (1 in 243).

## Goals / Non-Goals

**Goals:**

- Fill the album-artist map for a music level with one recursive Audio
  request per level instead of one per album.
- Warm the map at startup in the background, per level, before any grouped
  view opens (spec: `stable-music-library-grouping` — *Music grouping
  metadata is warmed at startup*).
- Preserve the exact vote semantics and the settle-and-fallback contract.

**Non-Goals:**

- Changing the grouping layer, `music_levels` config, the vote rule, the
  folder-name fallback, or the folder-browse model.
- Persisting the artist cache (remains in-memory).
- Emby-native music endpoints (`/Artists` etc.) — raised and rejected; the
  grouping layer is deliberate.
- Removing `SETTLE_WINDOW` (kept as safety net per user decision).

## Decisions

### D1: Batch unit is the folder level, bucketed by track `ParentId`

One request per album-bearing level:
`/Users/{uid}/Items?ParentId={level}&IncludeItemTypes=Audio&Recursive=true&
Fields=AlbumArtist,Artists,ParentId,Path&SortBy=ParentIndexNumber,IndexNumber&
Limit=100000`. Tracks are bucketed by `ParentId`; each bucket whose key is an
album folder at the level yields that album's artist via the vote.

Why: `ParentId` rejects comma lists (500), so arbitrary album sets are
impossible; an ancestor folder is the only batch unit. Per-level rather than
whole-library keeps peak memory at ~3.8MB vs ~26MB (measured), consistent
with the #656 footprint work, and matches the level-granular cache lifecycle.

Alternatives considered: whole-library single request (2.88s / 26MB — too
much peak memory, wrong cache granularity); Emby `MusicAlbum` identification
(unavailable — these are plain `Folder`s); per-album fetch with higher
concurrency (still N+1; 6→N concurrency just hammers the server).

### D2: The vote is extracted, not changed

The majority vote over the first ≤5 tracks per bucket (`AlbumArtist`, falling
back to per-track `Artists[0]`, first-seen wins ties) moves into a pure,
unit-testable function shared by nothing else — it is applied per bucket
after grouping. `SortBy=ParentIndexNumber,IndexNumber` is retained so
"first 5 tracks" per bucket means the same thing it does today. Proven
identical on the live library (61/61).

### D3: Orphan buckets are attributed by `Path` prefix, else left to fallback

A bucket whose `ParentId` is not an album folder at the level (a nested disc
subfolder) is attributed to the album whose `Path` is a prefix of the
tracks' `Path` (album `Path`s come from the level listing already in hand).
If no album prefix matches, the bucket is dropped and the album resolves via
the existing settle/fallback path — the same terminal outcome as a failed
lookup today.

Why prefix-match over walking up: walking the parent chain costs extra
requests for a 1-in-243 case; `Path` is already on the track payload (one
extra field) and on the album items.

### D4: Level-fill is keyed by level id, cache stays keyed by album id

New state: `album_artist_levels: HashMap<level_id, LevelFillState>` where
`LevelFillState` is `Loading | Filled | Failed`, replacing
`album_artist_loading` / `pending_album_artist_fetches` /
`MAX_ALBUM_ARTIST_FETCHES` (per-album concurrency limiting is meaningless
when a level is one request). `album_artist_cache` (album id → artist) is
unchanged, as are its Service-reset clear paths. Candidate creation requests
the level fill once; concurrent candidates and startup warm-up dedupe on the
level state. Arrival is a single `LibEvent::AlbumArtistLevelFetched {
level_id, artists: Vec<(album_id, artist)> }` (empty on HTTP failure →
`Failed`) that bulk-fills the cache; the existing settle observation then
resolves every waiting album in that level at once. On `Failed`, albums stay
unresolved until `SETTLE_WINDOW` forces the deterministic fallback — the
window's kept role as safety net.

### D5: Warm-up runs after Service Ready, discovered from the root listing

After the Emby Service transitions to Ready, warm-up fetches the configured
music library's group-level listing (root children) and spawns one level
fill per group-level child — 15 requests, ~3s total measured — on the
existing worker-thread + `lib_tx` pattern. Warm-up shares the D4 level-fill
state, so a later candidate for an already-`Filled` level does no work, and
one in `Loading` just waits. Failure of any warm-up request marks that level
`Failed` and is otherwise silent; browsing is unaffected (spec scenario:
*Warm-up does not gate startup*).

Alternative considered: warm lazily when each level is first listed (status
quo timing) — rejected by the user; a LAN client should not wait for the
user to browse before fetching.

## Risks / Trade-offs

- [A level with pathological track counts inflates peak memory] → Measured
  worst level is 3.8MB; per-level batching caps this naturally. Whole-library
  was the 26MB alternative and was rejected.
- [`Path` prefix attribution fails on servers that omit or rewrite `Path`]
  → Orphan bucket is dropped; album gets the same fallback it would get
  today on lookup failure. No regression.
- [Warm-up fires for libraries the user never opens] → 15 small background
  requests on a LAN; deliberate per user direction ("it's a client and it's
  on a LAN").
- [Warm-up races candidate creation for the same level] → Both dedupe on
  the level-fill state; the single arrival resolves both waiters.
- [A `Failed` level never retries while cached state lives] → `Failed`
  levels retry on the next candidate creation for that level (state is not
  cached as terminal across candidates); the settle window bounds the wait.

## Migration Plan

Pure client-side behavior change; no data, config, or protocol migration.
Rollback is reverting the change; the per-album path it replaces carries no
persistent state.
