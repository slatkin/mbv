# Tasks: fix-music-artist-resolution-batching

## 1. Batch fill core

- [ ] 1.1 Extract the majority vote from `spawn_album_artist_fetch` (src/app/image_fetch.rs) into a pure function (`AlbumArtist` per track, per-track `Artists[0]` fallback, first ≤5 tracks, first-seen wins ties) and verify with unit tests covering: majority win, `Artists[0]` fallback, empty-candidate skipping, first-seen tie-break, ≤5 cap.
- [ ] 1.2 Add pure bucketing: group a level's Audio rows by `ParentId`; attribute a bucket whose `ParentId` is no album at the level to the album whose `Path` prefixes the tracks' `Path` (drop the bucket if none matches). Verify with unit tests covering: 1:1 buckets, multi-disc orphan attributed via `Path`, unmatched orphan dropped.
- [ ] 1.3 Add level-fill state `album_artist_levels: HashMap<String, LevelFillState>` (`Loading | Filled | Failed`) to `App` (src/app/app_struct.rs), replacing `album_artist_loading`; add `LibEvent::AlbumArtistLevelFetched { level_id, artists }` (src/app/types_events.rs) and its handler (src/app/lib_event_actions.rs): bulk-fill `album_artist_cache`, mark `Filled` (or `Failed` on empty/failure). Verify `cargo check -p mbv` passes and handler unit tests cover fill, failure, and Service-reset clearing (src/app/emby_service_actions.rs).

## 2. Level fetch + candidate integration

- [ ] 2.1 Implement `spawn_level_artist_fetch(level_id)` in src/app/image_fetch.rs: one recursive Audio request (`Recursive=true&Fields=AlbumArtist,Artists,ParentId,Path&SortBy=ParentIndexNumber,IndexNumber&Limit=100000`) on the existing worker-thread/`lib_tx` pattern, applying 1.1's vote per 1.2's buckets. Verify with a mock-JSON unit test of the parse+bucket+vote pipeline (no live server).
- [ ] 2.2 Change candidate creation in src/app/music_grouping.rs to request the level fill once (deduped on `Filled`/`Loading`) instead of per-album `fetch_album_artist`; keep `SETTLE_WINDOW` and the settle/fallback contract unchanged. Verify `src/app/tests_music_grouping.rs` passes, updated so arrivals come from level events.
- [ ] 2.3 Delete the per-album machinery: `fetch_album_artist`, `drain_album_artist_fetches`, `spawn_album_artist_fetch`, `pending_album_artist_fetches`, `MAX_ALBUM_ARTIST_FETCHES`, and `LibEvent::AlbumArtistFetched`; update remaining `album_artist_cache` readers untouched. Verify `cargo clippy --workspace --all-targets -- -D warnings` is clean and no references remain (`rg fetch_album_artist src/`).

## 3. Startup warm-up

- [ ] 3.1 After the Emby Service transitions to Ready, fetch the configured music library's group-level listing and spawn one level fill per group-level child (deduped through the level-fill state); failures mark the level `Failed` silently. Verify with unit tests (mock client): fills are requested per group child without the view being opened, duplicate warm-up/candidate requests dedupe, and failure leaves browsing state untouched.

## 4. Verification

- [ ] 4.1 Run `cargo nextest run -p mbv`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --all -- --check`; all pass.
- [ ] 4.2 Run `openspec validate fix-music-artist-resolution-batching` and confirm the delta applies cleanly against `stable-music-library-grouping`.
