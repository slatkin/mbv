# per-destination-item-navigation — Tasks

## 1. Reveal-item resolution (worker side)

- [x] 1.1 In `spawn_navigate_to_item` (`src/app/library_browse_actions.rs`), resolve
  the reveal item per design D1 before building state: Episode/Season → owning
  Series via `series_id` (`get_ancestors` fallback), Audio track → `album_id`
  album, Series → itself, Movie/generic → itself. Verify with a unit table test
  over item_type → reveal kind resolution (mocked Emby boundary).
- [x] 1.2 Carry the reveal item in the `LibEvent::NavigateTo` payload (design D2):
  the Movie/generic arm keeps the built nav stack; show/album arms send the
  reveal item. Verify the exhaustive dispatch compiles with the new field and
  `cargo check -p mbv` is clean.

## 2. Per-kind landing (App side)

- [x] 2.1 Implement the Movie/generic arm unchanged (root level + cursor,
  `save_default_library_position` before `switch_tab`), keeping the
  `navigate_to_item_keeps_navigated_cursor_across_tab_switch` regression green
  (`cargo nextest run -p mbv navigate_to_item_keeps`).
- [x] 2.2 Implement the show arm via `activate_searched_series` (letter pill,
  cursor on the series, `fetch_series_detail`) applied to the target library's
  current root level; a miss (series absent from the corpus) flashes the
  library-error path and leaves the tab unchanged. Verify with a unit test
  asserting root-level-only landing, pill applied, and cursor on the show.
- [x] 2.3 Implement the Music album arm via `activate_recursive_album` on the
  resolved album; verify the landed stack matches that flow's existing shape
  for one grouped and one flat library (unit tests, mocked fetch).

## 3. Shell hand-off (Model drain)

- [x] 3.1 On a navigated show, run the Inline Search series hand-off in the Model
  drain: `reanchor_tv_owner_selection` + `push_tv_workspace_content`, then
  `activate_selected_series_item` (Wide) or `open_library_hero_overlay`
  (Narrow), re-push. Verify with a tick-integration test through
  `Application::tick()` for both Wide and Narrow (`tests_tick_integration_tv.rs`
  family): workspace visible in Wide, overlay open in Narrow.
- [x] 3.2 On a navigated album, set `music_workspace_reanchor` and push; verify
  the retained Music owner's workspace shows the album's track list
  (`shell_music_workspace_owner_tests.rs` pattern).
- [x] 3.3 Keep the Movie/generic re-anchor from 34dbbd55 (owner identity change
  re-seeds via `apply_position`); verify the retained browser lands with the
  cursor on the movie.

## 4. Fences and failure paths

- [x] 4.1 Extend the NavigateTo arm's save-and-fence to the per-kind landings
  (landing replaces the saved Library position; stale restore discarded).
  Verify by extending `navigate_to_item_keeps_navigated_cursor_across_tab_switch`
  with a late in-flight `RestoreLibraryPosition` for the pre-navigation state.
- [x] 4.2 Resolve-failure path: reveal-item resolution failure sends
  `LibEvent::Error`, flashes, and leaves the active tab unchanged. Verify with a
  unit test on the drained error event.

## 5. Gates and sync

- [x] 5.1 Full gates: `cargo nextest run -p mbv` and `-p mbv-core`, clippy
  `-D warnings`, `cargo fmt --check`, `openspec validate --all`.
- [x] 5.2 Manual check (single-user system): queue → right-click an episode →
  "Go to Library" lands on the show with its Workspace/overlay open; a queued
  track lands on its album; Delete on the first queue item still removes it.

## 6. Deep selection (the chosen episode/track is selected)

- [x] 6.1 TV: an Episode reveal carries the episode through
  `NavigateLanding::Series` and the pending/hand-off chain; after the series
  hand-off opens the Wide workspace, the workspace resolves the episode's
  season + episode (fetching season episodes when uncached) and selects the
  episode with episode focus. A Season reveal keeps the show-level landing
  with default selection. When the episode is absent from the fetched detail,
  the show landing stands with default selection (no error — the navigation
  target was reached). Verify with a tick-integration test through
  `Application::tick()` (Wide: workspace open, episode selected; Narrow:
  overlay opens for the show).
- [x] 6.2 Music: an Audio track reveal carries the track through
  `NavigateLanding::Album`; after the album activation the workspace track
  list selects the track (grouped; flat has no workspace track list — the
  landing stands at the root cursor). When the track is absent, the
  album landing stands with default selection (no error). Verify with
  unit/tick test on the retained Music owner pattern.
- [x] 6.3 Gates + live re-check: `cargo nextest run -p mbv` and `-p mbv-core`,
  clippy `-D warnings`, `cargo fmt --check`, `openspec validate --all`; user
  live check of episode → show + episode selected and track → album + track
  selected.
