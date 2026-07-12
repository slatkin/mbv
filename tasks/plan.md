# Plan: #145 — Power View inline album detail + track-selection mode

Scope: **Power View music album browsing only.** Do not touch the classic
tab-based library view or any other album-browsing surface (the issue's
scope-correction comment broadened this to "all album browsing surfaces";
per explicit user direction that broadening is rejected — Power View only).

## Background (from investigation)

- `App::is_viewing_album_folders(lib_idx)` (`src/app/actions.rs`) is true
  exactly when the current `nav_stack` top level is the album-folder listing
  (works for both the flat `music_levels = ["album"]` config and the grouped
  `["group","album"]` config — both are real, supported configs per
  `crates/mbv-core/src/config.rs` tests). This is the condition for "album
  list is visible, no track drilldown yet."
- `App::is_album_level(lib_idx)` is true one level *deeper*: the user has
  already drilled into an album's track list (today via `Enter` pushing a
  `BrowseLevel` in `select()`, `src/app/actions.rs:2341`). This is the
  existing drilldown this issue replaces with inline rendering + a track
  mode flag instead of a nav_stack push.
- `render_power_library` (`src/app/render/power/mod.rs:420`) currently picks
  exactly one renderer for the whole `area`: detail XOR list. `is_album` ->
  `render_power_album_detail`. Task 2 changes this to split `area` and show
  both simultaneously when `is_viewing_album_folders(lib_idx)`.
- `render_power_album_detail` (`src/app/render/power/detail.rs`... actually
  `src/app/render/power/album.rs`? — verify at implementation time) reads
  `items`/`cursor` directly from `lib.nav_stack.last()`. It needs to be
  callable with explicit `items`/`cursor` params so it can render either the
  legacy pushed-level data (until Task 3 removes the push) or the new
  proactively-fetched inline cache.
- Precedent for "proactively fetch data for the render-visible item, cache
  it, no user action required" already exists: `album_artist_cache` /
  `album_artist_loading` / `fetch_album_artist` / `spawn_album_artist_fetch`
  / `LibEvent::AlbumArtistFetched` (`src/app/images.rs`, `src/app/mod.rs`,
  `src/app/actions.rs`). Mirror this exactly for album *tracks* rather than
  inventing a new pattern.
- `is_album_level` is **HIGH risk** upstream per GitNexus impact analysis
  (touches `handle_lib_event`, `execute_context_action`, `select` — 3
  execution flows). Changes here need care and full regression testing of
  existing music/movie/series Power View behavior.

## Tasks

1. **Data model + fetch plumbing.** Add `album_tracks_cache:
   HashMap<String, Vec<MediaItem>>` and `album_tracks_loading:
   HashSet<String>` to `App` (mirrors `album_artist_cache`/
   `album_artist_loading`). Add `fetch_album_tracks`/
   `spawn_album_tracks_fetch` (mirrors `fetch_album_artist`/
   `spawn_album_artist_fetch`, but fetches the full track list, not a
   5-track artist sample) and a new `LibEvent::AlbumTracksFetched { album_id,
   tracks }` variant handled in `actions.rs`. Refactor
   `render_power_album_detail` to take `items: &[MediaItem], cursor: usize`
   explicitly instead of reading `nav_stack` internally, updating its one
   existing call site to pass the same data it reads today (no behavior
   change at this call site). Unit tests: cache hit / cache miss triggers
   fetch once / loading flag prevents duplicate fetch / event handler
   populates cache and clears loading flag.

2. **Render album detail inline.** In `render_power_library`, when
   `is_viewing_album_folders(lib_idx)` is true, split `area` into an upper
   sub-rect (album list) and lower sub-rect (album detail) instead of
   picking one exclusively. Resolve the selected album's id from the
   album-folder-listing cursor, look it up in `album_tracks_cache` (calling
   `fetch_album_tracks` on miss, showing a loading state otherwise), and
   render it via the refactored `render_power_album_detail`. `Enter`/`Escape`
   keep their **current** (pre-issue) push/pop drilldown behavior in this
   task — deliberately incomplete but safe and independently revertable.
   Test: moving the album cursor updates which album's tracks render inline,
   without a nav_stack push.

3. **Track-selection mode.** Add `LibraryTab.album_track_focus:
   Option<usize>` (`Some(idx)` = track-selection mode, focused track index
   into the inline cache; `None` = normal album-list navigation). `Enter` at
   the album-folder-listing level sets `Some(0)` instead of pushing
   nav_stack. `Escape` while `Some(_)` clears it back to `None` (same album
   still shown inline) instead of calling `go_back()`. Up/Down: route to
   moving `album_track_focus` within track-cache bounds when `Some(_)`,
   otherwise move the album cursor as today. Tests: Enter enters mode at
   idx 0; Up/Down inside mode move only the track focus and leave the album
   cursor untouched; Escape exits back to album-list mode with the same
   album still visible; Up/Down outside the mode still move albums exactly
   as before.

4. **Scope-correct actions.** Album-list mode: keyboard shortcuts and the
   context menu (`open_context_menu`/`context_menu_power_lib_idx` in
   `input.rs`) offer play-all/shuffle/add-all-to-queue against the
   *selected album* via the existing `play_folder`/`shuffle_folder`/
   `do_enqueue_folder`. Track-selection mode: `Enter` plays the *focused
   track* (existing per-track play path); per-track context-menu actions
   (enqueue, etc.) target the focused track (existing per-track path).
   Tests: album-scope action fires folder-level call with the selected
   album's id in list mode; track-scope action fires track-level call with
   the focused track's id in track mode; context menu offers the right verb
   set in each mode.

5. **Regression coverage.** Full pass confirming existing non-music Power
   View behavior (movies, series, home videos) and other Power View
   keyboard/queue shortcuts are unchanged; `detect_changes()` compared
   against `main` before commit/PR.

Each task = one RED/GREEN/regression/build/commit cycle per
`incremental-implementation`. Default `/build` mode: implement the next
pending task, stop.

## Status

- [x] Task 1 — data model + fetch plumbing
- [x] Task 2 — render album detail inline
- [x] Task 3 — track-selection mode
- [ ] Task 4 — scope-correct actions
- [ ] Task 5 — regression coverage
