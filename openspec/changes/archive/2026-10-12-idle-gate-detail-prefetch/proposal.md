# Proposal

## Why

When the user holds a movement key in the grouped Music tree or the TV list, the shell starts one detail request for each row the cursor passes. Music fetches the album tracks and TV fetches the series detail. The cache stores each result, but the user never sees most of them. Card images already wait until the cursor rests for `NAV_IMAGE_FETCH_IDLE_DELAY` (150 ms). The detail fetches do not wait. An idle gate on these two fetches removes most of the requests during fast browsing. A worker pool only spreads the same requests over time, so we use the idle gate.

## What Changes

- The sync-pass album-track fetch for the selected Music album starts only after the cursor rests. This is the call in `fetch_music_workspace_album_tracks`.
- The sync-pass series-detail fetch for the selected TV series starts only after the cursor rests. This is the call in `push_tv_workspace_content`.
- Wide TV row movement (`TvMoveRows`, `TvJumpCursor`) records navigation activity, as Music and narrow TV movement already do. TV card-image prefetch then also waits during wide TV movement. This makes the Music and TV behavior the same.
- The gate is renamed from `list_image_fetches_allowed` to `navigation_idle`, and its constant from `NAV_IMAGE_FETCH_IDLE_DELAY` to `NAV_IDLE_DELAY`, because the gate now controls more than image fetches.
- Explicit user actions stay ungated: tree expansion, season moves, activation, landing hand-offs, and the artist album-track fallback.

No request is lost. The sync pass runs on every loop iteration (at most 50 ms apart), so the skipped fetch starts on the first pass after the cursor rests. The detail pane shows its loading state for up to about 200 ms longer after the cursor stops.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. No spec states when detail fetches start. The change affects only request timing, so `.openspec.yaml` sets `skip_specs: true`.

## Impact

- `src/app/shell/music_workspace.rs` (`fetch_music_workspace_album_tracks`).
- `src/app/shell/tv_workspace.rs` (`push_tv_workspace_content`, `handle_tv_navigation_request`).
- `src/app/infra/image_fetch.rs`, `src/app/infra/image_fetch/fetch/card_images.rs` and the other gate call sites (rename only).
- Tests in `src/app/shell/tv_workspace/tests.rs` and the `music_workspace.rs` test module.
- No new dependencies, no new timer, no change to the request functions `fetch_album_tracks` and `fetch_series_detail`.
