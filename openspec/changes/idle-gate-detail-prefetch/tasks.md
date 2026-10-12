# Tasks

## 1. Idle gate

- [x] 1.1 Rename `App::list_image_fetches_allowed` to `navigation_idle` and `NAV_IMAGE_FETCH_IDLE_DELAY` to `NAV_IDLE_DELAY` at every use (design D3). Find them with `rg "list_image_fetches_allowed|NAV_IMAGE_FETCH_IDLE_DELAY" src`. Verify: `cargo check -p mbv` passes and the `rg` matches nothing.
- [x] 1.2 In `src/app/shell/music_workspace.rs` `fetch_music_workspace_album_tracks`, add `self.app.navigation_idle()` to the `if` condition before `fetch_album_tracks` (design D1). Do not change `App::fetch_album_tracks`.
- [x] 1.3 In `src/app/shell/tv_workspace.rs` `push_tv_workspace_content`, call `self.app.fetch_series_detail(&item.id)` only when `self.app.navigation_idle()` is true (design D1). Do not change `App::fetch_series_detail` or the `TvTreeExpand` arm.
- [x] 1.4 In `handle_tv_navigation_request`, make the `TvMoveRows` and `TvJumpCursor` requests set `self.app.last_nav_at` and call `self.app.mark_library_navigation` with one `Instant::now()` before the final `push_tv_workspace_content` (design D2). Copy the pattern in `handle_music_album_cursor` (`shell/messages/music.rs`). Update the "cursor moves need no App effect" comment to match. Verify: `cargo nextest run -p mbv` passes.

## 2. Tests

- [ ] 2.1 In `src/app/shell/tv_workspace/tests.rs`, add `wide_tv_row_move_defers_series_detail_fetch`. Contract: a wide TV row move closes the idle gate, so the push in that handler starts no series-detail request. Set up the model as `push_tv_workspace_content_fetches_uncached_selected_series_once` does (ready Emby runtime), call `model.handle_tv_request(ShellRequest::TvMoveRows { rows: 1 })`, and assert `model.app.series_detail_loading` is empty. The existing test `push_tv_workspace_content_fetches_uncached_selected_series_once` already covers the idle case.
- [ ] 2.2 In the `music_workspace.rs` test module, add `music_push_defers_album_track_fetch_while_navigating`. Contract: a sync-pass push with a recent `last_nav_at` starts no album-track request. Use `make_music_group_app()` with `ready_emby_runtime()`, set `app.last_nav_at = Instant::now()`, call `push_music_workspace_content()`, and assert `album_tracks_loading` is empty. Use no sleeps.
- [ ] 2.3 Run `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings` and verify that both are clean. Verify: `cargo nextest run -p mbv` passes. Commit.

## Workflow follow-up

- Archive this change after review.
