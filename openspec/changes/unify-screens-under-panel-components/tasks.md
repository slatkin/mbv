Every section removes a named cross-screen difference (design D0, D14). If a task finds that a
destination needs a slot, arm, surface or policy the panel types lack, stop: change the type for every
destination, never add a destination-only arm. Before any TUI edit, follow
`.agents/skills/mbv-frontend/SKILL.md`. Tests are ordinary buffer and `Application::tick()` integration
tests; no test, rule or script checks one destination against another.

## 1. Root frame and draw-time state (S0)

*Unification:* removes the shell as a painter-with-side-effects, so every later panel is composed from
one paint-free placement.

- [ ] 1.1 Move `render_main`'s state mutations into the sync pass: the `library_tab_pending` resolution
  and `normalize_stale_browse_destination` run in `sync_mounted_surfaces` before any draw, and
  `render_main` no longer writes `self.tab`. Verify: the existing tab-restore and stale-destination tests
  pass unchanged, and a new `shell_run` unit test drives one sync + draw with a pending tab and asserts
  the tab is resolved before `draw_frame` is entered.
- [ ] 1.2 Move `compute_frame_layout`'s resize side effects (clearing `card_image_states`, queue-column
  clamp + `save_prefs`, forcing `mini_view_focus`) into the resize handling in the sync pass; the draw
  path only reads geometry. Verify: resize tests in `src/app/tests_tick_integration.rs` pass unchanged,
  plus a tick test that crosses the mini-view threshold and asserts focus moves before the frame paints.
- [ ] 1.3 Introduce `RootFrame` in `src/app/render/arrangements/chrome.rs`, extending
  `chrome_geometry`/`FrameChromeGeometry` with one placement per panel (Tab, Library, Library playback,
  Queue, Queue playback, Status bar, Queue boundary) for each Panel mode, and make `draw_frame` iterate
  its placements in the order `RootFrame` records, calling the existing painters for not-yet-migrated
  panels. Verify: relational unit tests in `chrome.rs` (panels partition the terminal area per mode,
  no overlap), and every existing render/tick test passes unchanged.

## 2. Tab panel and Status bar panel (S1)

*Unification:* removes shell-painted chrome and the `tabs_hitmap`/status-pill side channels.

- [ ] 2.1 Create `TabPanel` (`src/app/components/tab_panel.rs`) that paints the tab bar (moved from
  `chrome_tabs.rs::render_tabs`, keeping `visible_tab_range` overflow arrows), retains its tab hit
  regions, and emits a tab-select `Msg` for clicks; mount it at the root; delete `render_tabs`' call in
  `paint_legacy_chrome`, `LayoutMain.tabs_hitmap`, and the shell tab-click path in `shell.rs`. Verify:
  tab buffer tests move to the component and pass; a tick integration test clicks a tab and asserts the
  destination changes; the right-column backdrop is painted by the panel, not `render_legacy_backdrops`.
- [ ] 2.2 Create `StatusBarPanel` that paints the status row (moved from
  `chrome_status.rs::render_status_bar`) and retains its volume/mute/remote pill regions; delete
  `LayoutPlayback.{ind_vol, ind_mu, ind_rc}` and the `render_status_bar` call in `render_main`. Verify:
  status-row buffer tests pass from the component; a tick test scrolls on the volume pill and asserts the
  volume intent; the status row paints only where `RootFrame` places it.

## 3. Queue panel and Queue playback panel (S2, folds `add-now-playing-sidebar`)

*Unification:* removes the base-frame queue painting, the queue-only-only transport, and the
`narrow_player` flag; one Queue playback panel in every queue-visible layout.

- [ ] 3.1 Move the queue frame, title row and status pill row into `QueuePanel` (today's
  `QueueComponent`): delete `render_queue_panel_frame`/`render_queue_status` calls from `render_main`,
  paint the left backdrop from the panel, and move `LayoutMain.queue_{area,title_area,selected_item_rect}`
  into the component. Verify: `src/app/render/tests_queue.rs` and `queue_component_tests.rs` pass; the
  context-menu keyboard anchor for a queue row still opens at the selected row (existing test).
- [ ] 3.2 Add `header_height` to `QueuePanelInputs` and offset `queue_panel_geometry` by it alongside
  the visual-slot and transport heights. Verify: unit test in `arrangements/queue.rs` that the queue panel
  starts one row below the header and the 1-row gap appears only when rows sit above it; at 24 rows
  `short_window_keeps_queue_in_left_column` and `short_queue_panel_drops_padding_before_rows` pass with
  re-derived (not loosened) thresholds.
- [ ] 3.3 Derive `NowPlayingStatus { Playing, Paused, Idle }` once per frame next to
  `effective_playback_state()`, and extract `playback_host_label()` from `queue_title_model` (queue title
  uses it; no tracking suffix, no uppercasing). Verify: unit test of the three states (unreachable
  `!active && paused` → `Idle`); `queue_title_characterization_tests.rs` green unchanged; the label for an
  attached session has no ` · TRACKING`.
- [ ] 3.4 Move the visual slot's image fetch out of `render_card`: the queue projection issues
  `fetch_card_image` for the now-playing item and projects image state; painting reads it only.
  Verify: a push test asserts one fetch per new now-playing key and none on repaint; `render_card`'s
  replacement paints from projected state in a buffer test with no `App` access.
- [ ] 3.5 Create `QueuePlaybackPanel` owning the header row (status left, `on <host>` right, on
  `SURFACE_CHROME`), the visual slot (artwork/placeholder/visualizer) and the queue-column transport
  presentation, with placement: below 100 columns stacked, 100+ side by side (slot left, 2-cell gap,
  panel height = max); mount it at the root's Queue playback placement; delete `render_card` and both
  queue-only `render_player_panel` calls from `render_main` and remove `narrow_player`. Verify: buffer
  tests at 80 and 100+ columns showing `PLAYING`/`PAUSED`/`IDLE` and the target; a remote-attached frame
  with the Local scope selected names the remote target; no header row in library-only.
- [ ] 3.6 Idle collapse in every queue-visible layout: no visual slot or transport while idle (the
  connected-idle exception is deleted); paused keeps both; playback start restores both. Verify:
  `connected_idle_queue_only_keeps_panel_but_collapses_card` becomes the inverse assertion;
  `idle_queue_only_hides_card_and_panel_at_both_widths`, `idle_both_hides_card_and_reclaims_queue_rows`,
  `idle_queue_only_reclaims_card_and_panel_rows_until_playback_starts` and
  `paused_queue_only_keeps_card_and_panel` (plus a `both` counterpart) pass at narrow, 80 and 100+.
- [ ] 3.7 Pointer input on the Queue playback panel transport from its own retained geometry. Verify:
  tick integration test clicks play/pause and the seekbar in `both` and mini-view `queue-only`
  (`TogglePlayPause`, `SeekTo`); a click in a collapsed panel's rows emits nothing.
- [ ] 3.8 Re-point the idle-feed open-link gate at the playback panel's presence instead of the panel
  mode (`action.rs:104-118`, `shell.rs:284`). Verify: `src/app/action_tests.rs` and
  `src/app/tests_routing_matrix_playback.rs` rows: suppressed in idle `both` and `queue-only`, fires in
  idle `library-only`.

## 4. Library playback panel (S3)

*Unification:* one transport painter per frame; the strip reserves rows only when it paints.

- [ ] 4.1 Turn `PlaybackComponent` into `LibraryPlaybackPanel`, mounted only when the queue column is
  hidden; `RootFrame` reserves `PLAYER_BOX_HEIGHT` in the library column only then. Verify: buffer
  proof in `tests_queue.rs` that `both` reserves no strip rows, `library-only` exactly
  `PLAYER_BOX_HEIGHT`, `queue-only` none; a tick integration test draws `both`, `queue-only` and
  `library-only` and finds exactly one transport per frame, painted by the expected panel; strip clicks
  resolve against the strip's painted geometry.

## 5. Library panel types, skeletons and Home (S4)

*Unification:* creates the only paint path for library screens; no destination can add a pill bar,
choose a focus kind, pick a surface or lay out a pane.

- [ ] 5.1 Create the panel module (`src/app/components/library_panel/`) with the content types of
  design D3 (`LibraryPanelContent`, `SelectorRow`, `ListControls`, `ListSlot`, `HeroContent`,
  `HeroHeader`, `Workspace`) and the slot Render Components for Selector row (one pill bar + spacer,
  retained `HitRegions`) and List controls row (optional pills + optional label). Make
  `wide_hero_presentation`, `pill_bar_areas`, `wide_hero_browser_pane`, `wide_hero_hero_content_box`,
  `place_media_list_below` private to the panel's arrangement. Verify: buffer tests for each slot
  component (pill active state, label, empty row absent), and a hit test resolving a painted pill.
- [ ] 5.2 Implement the Wide skeleton: Browser pane (Selector row, List controls row, list box fill +
  `wide_hero_browser_border`, list presentation or empty placeholder or Inline Search), gap, Hero pane
  (resting surface; focused only when a Workspace is present and focused). Verify: buffer tests for a
  read-only hero and a focused-workspace hero; role-rect containment tests only (no coordinates).
- [ ] 5.3 Implement `HeroHeader` Landscape / Portrait / Square with one title/meta painter
  (`paint_hero_content`), artwork placeholder at full size while loading, artwork shrinking before a
  Workspace viewport drops, and the overview Main content box rendered only when overview text exists.
  Add `Hero::header_kind()` from item kind (Landscape: Movie, home video, Series, Emby Home items;
  Portrait: ABS book; Square: MusicAlbum, ABS podcast, feed entry), replacing `HeroArtworkAspect`.
  Verify: buffer tests per arm (art above vs art right), overview present/absent, placeholder size.
- [ ] 5.4 Implement the Workspace: optional Selector row over one Main content box holding a
  `WideMediaList`, accent-soft surface while focused, owning-surface selected row. Verify: buffer tests
  for focused/unfocused box surface and selected-row surface; `WideHeroContentBoxSurface` deleted.
- [ ] 5.5 Implement the Narrow skeleton and the one `InlineHero` form (right-aligned image sized from
  its aspect, wrap-around text, full width below), with no selector, controls or constituent rows inside
  it. Verify: buffer tests with a 2:3 poster, a 16:9 thumbnail and no image — all right-aligned
  wrap-around.
- [ ] 5.6 Mount `LibraryPanel` as `ComponentId::Library`: owner map keyed by `LibraryKey`, focus and
  mouse subscription for the library area, slot events (`SelectorPicked`, `ControlPicked`,
  `WorkspaceSelectorPicked`, list delegation) routed to the active owner, the Wide split-boundary drag
  moved in from `WideHeroBoundaryComponent`, owner retention while the library is in the catalog (moved
  from `reconcile_destination_mounts`). `RootFrame` gives the library rect to `LibraryPanel` when the
  active library's owner has migrated, otherwise to the old component. Verify: tick integration tests
  for focus following the active library, mouse eligibility, split drag
  (`tests_tick_integration_wide_split_resize.rs`), and an inactive owner keeping cursor/scroll across a
  tab change.
- [ ] 5.7 Move image requests for Hero headers and inline heroes into the owners' shell projection
  (`push_*`), generalizing TV's `push_tv_workspace_content` pattern; painting reads projected image
  state. Verify: push tests assert one fetch per new key and none on repaint.
- [ ] 5.8 Convert Home into `HomeContent` (embedded owner): section pills → Selector row, rows → list
  slot, selected item → Landscape header (or its own kind), Narrow inline hero; delete
  `render_home_content`, `HomeCarrier`, Home's `pill_regions`/spacer paint and `ComponentId::Home`.
  Verify: `tests_home_characterization.rs`/`tests_home_inline.rs` rewritten to the panel output where
  the overview box and header changed; `tests_tick_integration_home.rs` passes; pill clicks select
  sections.

## 6. Movies, home videos and generic Emby libraries (S5)

*Unification:* removes `render_wide_movies`, the Narrow Movie banner model, the count-label row and the
Grid catalog.

- [ ] 6.1 Convert `BrowserComponent` for Movies, HomeVideos and Generic into `BrowserContent`: letter
  pills → Selector row, home-video count → List controls label, Landscape header, Inline Search as list
  slot state; generic libraries use the Wide/Inline presentations (no Grid). Delete
  `browser/paint.rs::render_wide_movies`, the Movies/HomeVideos/Generic arms of
  `render_narrow_browse_with_ctx`, `CompactBannerLayout`, `render_compact_detail_with_ctx`, and the
  `ComponentId::Browser` arm for these kinds. Verify: `tests_library_characterization.rs` and
  `tests_non_music.rs` rewritten where output changed; `tests_tick_integration_browser.rs` passes;
  `browser_inline_search_tests.rs` passes.

## 7. Feeds (S6)

*Unification:* removes the second pill bar and the Feeds-only hero box — the most divergent screen
proves the slot types early.

- [ ] 7.1 Convert `FeedsComponent` into `FeedsContent`: feed-group pills → Selector row, Watched filter
  → List controls pills, entries → list slot, selected entry → Square header + overview box, Narrow
  inline hero. Delete `render_feeds_content`, its `render_selector_content` closure, `paint_feed_hero`,
  `LayoutMain.feeds_area`, and `ComponentId::Feeds`. Verify: `tests_feeds.rs` rewritten (one pill bar,
  controls row, Square header); `feeds_component_tests.rs` and `tests_tick_integration_feeds.rs` pass;
  the `w` key and a click on a Watched pill both change the filter.

## 8. TV (S7)

*Unification:* removes the two-component TV split and TV's private header and poster painters.

- [ ] 8.1 Merge `TvWorkspaceComponent` and `BrowserComponent(TvShows)` into one `TvContent` owner:
  one series list owner (Wide + Inline presentations), episode list, season cursor, Inline Search
  session; delete `hand_off_tv_breakpoint`, the TV part of `apply_pending_inline_search_transfer`, and
  `ComponentId::TvWorkspace`. Verify: `tv_workspace_component_tests.rs` and the TV re-anchor tests pass
  (stable target across Wide↔Narrow); `tests_tick_integration_tv.rs` passes.
- [ ] 8.2 Paint TV through the panel: letter pills → Selector row, Landscape header, overview box,
  Workspace = season pills (Workspace Selector row) + episodes; Narrow inline hero with the series
  poster; Narrow episodes only via the selection modal. Delete `tv_wide.rs::render_wide_tv_with_ctx`,
  `render_series_inline_detail`, `SERIES_IMAGE_COLS/ROWS`, `NarrowInlineHero`, and
  `LayoutMain.tv_wide_*`. Verify: `tv_wide_tests.rs`/`detail_series_tests.rs` rewritten to panel output;
  season-pill and episode clicks resolve via tick tests.

## 9. Music (S8)

*Unification:* removes Music's private header layout, its focused-box surface arm and dead search
branches.

- [ ] 9.1 Convert `MusicWorkspaceComponent` into `MusicContent`: group pills → Selector row, albums →
  list slot, Square header, no overview box when the album has none, Workspace = tracks (accent-soft on
  focus, now shared); Narrow inline hero with album art, tracks via the modal. Delete
  `music_wide.rs::render_{wide,narrow}_music_group_with_ctx`, `render_wide_left_hero`,
  `wide_music_left_layout`, `music_wide_browser.rs`, the `is_search_active` search-box/`render_plain_rows`
  branches, and `LayoutMain.wide_music_*`. Verify: `tests_music_wide.rs`, `tests_music_narrow.rs`,
  `tests_music_groups.rs`, `tests_album_focus.rs` rewritten where output changed;
  `tests_tick_integration_music_mouse.rs` and the Music re-anchor characterization pass.

## 10. Audiobookshelf Books (S9)

*Unification:* removes the second content box, the list-backdrop workspace row and inline chapters —
Books conforms to Emby.

- [ ] 10.1 Convert `AudiobookshelfBookComponent` into `BookContent`: surname buckets → Selector row,
  Portrait header with progress in meta, overview box, Workspace = chapters (owning-surface row); Narrow
  inline hero without chapters, Enter opens the selection modal. Delete
  `render_audiobookshelf_book_content`, `render_narrow_book`, `render_book_rows`' inline path and the
  private geometry struct. Verify: `tests_audiobookshelf_books.rs` and
  `audiobookshelf_book_component_tests.rs` rewritten; `tests_tick_integration_book.rs` passes; Narrow
  Enter opens the chapter modal.

## 11. Audiobookshelf Podcasts (S10)

*Unification:* removes the episode table and the Narrow inline filter pills and episodes — Podcasts
conforms to Emby.

- [ ] 11.1 Convert `AudiobookshelfPodcastComponent` into `PodcastContent`: alphabetical buckets →
  Selector row, Square header, overview box, Workspace = filter pills (Workspace Selector row) +
  downloaded episodes; Narrow inline hero without pills or episodes, Enter opens the modal. Delete
  `render_audiobookshelf_podcast_content`, `render_narrow_podcast`, `paint_bucket_pills` and the private
  geometry struct. Verify: `tests_audiobookshelf_podcasts.rs`, the podcast component/geometry tests
  rewritten; `tests_tick_integration_podcast.rs` passes.

## 12. Delete the base frame and legacy structures (S11)

*Unification:* removes every remaining way to paint outside a panel.

- [ ] 12.1 Delete `compose_base_frame`, `render_main`, `paint_legacy_chrome`,
  `render_legacy_backdrops`, `render_library`, every shell `render_*_component` method, the transitional
  old-component branch in `RootFrame`, and the remaining legacy `ComponentId` arms
  (`Browser`, `WideHeroBoundary`). Verify: `cargo check -p mbv`; `rg` finds none of these symbols;
  a tick integration test asserts a full frame's cells are painted with no base frame (each panel's
  surface present in its placement in every Panel mode).
- [ ] 12.2 Delete `LayoutMain`, `LayoutPlayback` and `FrameChromeGeometry`'s paint-to-input use; answer
  context-menu keyboard anchors from the owning component through the existing request path. Verify:
  `rg LayoutMain src` returns nothing; context-menu keyboard-anchor tests pass for a library row and a
  queue row.
- [ ] 12.3 Delete the dead paths of design D13 (`GridMediaList`, `NarrowBrowseControl`,
  `GridPaintPolicy`, `LeftPaneFocus`, `SelectedRowSurface` as a caller argument, `HeroArtworkAspect`) and
  the tests that only exercised deleted structures, including `src/app/render/tests_conformance_matrix.rs`.
  Verify: `cargo clippy --workspace --all-targets` reports no dead code; `cargo nextest run -p mbv` green.

## 13. Docs and glossary (S12)

- [ ] 13.1 `CONTEXT.md`: broaden **Panel** to every root-composed region (Panel focus still means
  Library vs Queue); rename Panel mode **Normal** to **Narrow**; add **Tab panel**, **Status bar
  panel**, **Library panel**, **Library playback panel**, **Queue playback panel**, **Selector row**,
  **List controls row**, **Hero header** (Landscape/Portrait/Square), **Workspace**; rewrite
  **Variant**/**Policy**/**Bespoke surface** so a caller-selected arm with one user is named a defect.
  Verify: every term used in this change's specs appears once; no *Avoid* term is used in the specs.
- [ ] 13.2 Amend ADRs 0022–0024 with composition ownership (root composes panels; destinations supply
  slot content; no base frame), or add one ADR recording it that the three reference. Verify: ADR text
  names composition, not only state, ownership.
- [ ] 13.3 Update `docs/architecture/interactive-surface-ledger.md` (rows per panel; the 2026-08-27
  `migrated` note re-scoped to state ownership), `docs/architecture/interactive-tui-component-map.md`,
  `.agents/skills/mbv-frontend/SKILL.md` (remove "migration complete", "Central variant" and Grid
  guidance; add the panel/slot workflow), and the `AGENTS.md` repository map and embedded-list section.
  Verify: none of these documents claims the migration complete or endorses caller-selected variants.

## 14. Whole-change verification

- [ ] 14.1 Manual visual sweep of every library destination at Wide and Narrow, and of `both`,
  `queue-only`, `library-only` and mini view with playback idle, paused and playing. Verify: every
  destination shows the same skeleton (one Selector row, optional List controls row, same panes, same
  surfaces), exactly one transport per frame, correct header status and target, no cell painted twice.
- [ ] 14.2 Gates: `cargo nextest run -p mbv`, `cargo clippy --workspace --all-targets`,
  `cargo fmt --all -- --check`, `openspec validate unify-screens-under-panel-components --strict`, and no
  governed file over 800 lines. Verify: all clean.
- [ ] 14.3 At archive, sync the deltas and confirm `openspec/specs/queue-only-playback/` is deleted and
  `openspec/specs/library-panel/` and `queue-playback-panel/` exist. Verify: `openspec validate --strict`
  clean after sync.
