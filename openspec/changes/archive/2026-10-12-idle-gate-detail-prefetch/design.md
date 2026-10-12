# Design

## Context

See proposal.md (Why) for the motivation.

- `App::last_nav_at` records the last cursor movement. `list_image_fetches_allowed()` (`infra/image_fetch/fetch/card_images.rs`) returns true when `last_nav_at` is at least `NAV_IMAGE_FETCH_IDLE_DELAY` (150 ms, `infra/image_fetch.rs:7`) old. Card-image prefetch and next-page loading already use this gate.
- These handlers stamp `last_nav_at` before the next sync pass: `handle_music_album_cursor` (Move), `handle_emby_library_cursor_index` (narrow TV and the flat library list), `album_cursor.rs` and `queue_keys.rs`. Wide TV movement (`TvMoveRows`, `TvJumpCursor` in `handle_tv_navigation_request`) does not stamp it.
- `finish_run_iteration` (`shell/run.rs`) calls `sync_mounted_surfaces()` on every loop iteration. The terminal poll times out after 50 ms (8 ms with the visualizer), so iterations are at most 50 ms apart. `sync_mounted_surfaces()` calls `push_music_workspace_content()` and `sync_tv_content()` (which calls `push_tv_workspace_content()`).
- `push_music_workspace_content` calls `fetch_music_workspace_album_tracks`, which calls `App::fetch_album_tracks` for the selected album. `push_tv_workspace_content` calls `App::fetch_series_detail` for the selected series. Both request functions dedupe through their loading and cache sets.
- `last_nav_at` starts one second in the past (`state/construct.rs`), so a new `App` is idle at once. Existing tests that expect a fetch from a push stay valid.

## Goals / Non-Goals

**Goals:**

- No cursor-driven detail request while the cursor moves faster than one row per 150 ms.
- The selected row's detail request starts on the first sync pass after the cursor rests.
- Music and TV use the same gate and the same rule.

**Non-Goals:**

- No change to explicit fetches: `TvTreeExpand`, `TvSeasonMove`, `enter_series_selection`, the letter-pill landing in `dispatch/library/cursor.rs`, the `fetch_series_season_episodes` cascade, and `drain_artist_album_track_fetches`.
- No gate on artist-root requests (`request_music_artist_tracks`, artist artwork). These are one-shot handlers of `MusicArtistTracks`, and the sync pass does not repeat them. A gate there would drop the request. Artist roots are also few compared to album leaves.
- No new timer and no new constant value.

## Decisions

### D1. Gate at the two sync-pass call sites, not inside the request functions

Add the gate condition to the `if` in `fetch_music_workspace_album_tracks` and before the `fetch_series_detail` call in `push_tv_workspace_content`.

The request functions also serve explicit callers. `drain_artist_album_track_fetches` pops an album from its queue and then calls `fetch_album_tracks`. A gate inside `fetch_album_tracks` would make it drop that album for good. The sync-pass sites repeat on every pass, so a skipped call only waits.

### D2. Wide TV movement stamps navigation activity

In `handle_tv_navigation_request`, the `TvMoveRows` and `TvJumpCursor` arms set `last_nav_at` and call `mark_library_navigation` with the same `Instant`, as `handle_music_album_cursor` does. The stamp happens before the `push_tv_workspace_content` call at the end of that function, so that push skips the fetch. Without this stamp the TV gate would never close in the Wide presentation.

Side effect: TV card-image prefetch now also waits during wide TV movement. Music and narrow TV already behave this way, so this removes a difference between screens.

### D3. Rename the gate

`list_image_fetches_allowed` becomes `navigation_idle`, and `NAV_IMAGE_FETCH_IDLE_DELAY` becomes `NAV_IDLE_DELAY`. The old names say "image", which is wrong after this change. The rename is mechanical: every current call site and the existing test `recent_navigation_blocks_list_card_image_fetch`.

## Risks / Trade-offs

- [The detail pane shows its loading state up to about 200 ms longer after the cursor stops] → This is the same delay that card images already have. A user who moves one row and waits sees the delay once.
- [Enter within 150 ms of a move opens the hero before the tracks arrive] → The existing pending track-focus path already handles activation that comes before the track fetch ends (`push_music_workspace_content`).
- [Tests that stamp `last_nav_at` and then expect a detail fetch] → None found. A new `App` starts idle.

## Migration Plan

One commit. Rollback is a revert. No persisted state or protocol changes.
