Every section removes a named cross-screen difference (design D0, D14). If a task finds that a
destination needs a slot, arm, surface or policy the panel types lack, stop: change the type for every
destination, never add a destination-only arm. Before any TUI edit, follow
`.agents/skills/mbv-frontend/SKILL.md`. Tests are ordinary buffer and `Application::tick()` integration
tests; no test, rule or script checks one destination against another.

Every subtask is one implementer session: it ends with `cargo nextest run -p mbv` green, one painter per
surface, and no half-applied surface flip. A destination slice is therefore split into its content step
(the owner and its mapping, still painted by the legacy painter), its painting step (the panel skeleton
replaces that painter) and its composition step (the mounted component collapses into the panel's owner
map); the painting step lands with the deletion of the painter it replaces (D16).

## 0. Base

- [x] 0.1 Confirm the base is `main` at or after `9e59a29a` (not the discarded
  `refactor/unify-wide-hero-content-box-frame` branch) and re-verify every `file:line` citation in
  `design.md` Context and D17 against it; correct any that moved in `design.md` before starting 1.1.
  Verify: `git merge-base --is-ancestor 9e59a29a HEAD` succeeds and each cited symbol is found at (or
  re-cited to) its current line (manual check).

## 1. Root frame and draw-time state (S0)

*Unification:* removes the shell as a painter-with-side-effects, so every later panel is composed from
one paint-free placement.

- [x] 1.1 Move `render_main`'s state mutations into the sync pass: the `library_tab_pending` resolution
  and `normalize_stale_browse_destination` run in `sync_mounted_surfaces` before any draw, and
  `render_main` no longer writes `self.tab`. Verify: the existing tab-restore and stale-destination tests
  pass unchanged, and a tick integration test with a pending tab asserts, after one `tick()` + sync pass
  and without drawing, that the tab is resolved.
- [x] 1.2 Move `compute_frame_layout`'s resize side effects (clearing `card_image_states`, queue-column
  clamp + `save_prefs`, forcing `mini_view_focus`) into the resize handling in the sync pass; the draw
  path only reads geometry. Verify: resize tests in `src/app/tests_tick_integration.rs` pass unchanged,
  plus a tick test that crosses the mini-view threshold and asserts, after `tick()` + sync and without
  drawing, that focus has moved.
- [x] 1.3 Introduce `RootFrame` in `src/app/render/arrangements/chrome.rs` as **data only**, extending
  `chrome_geometry`/`FrameChromeGeometry` with the placement of each panel (Tab, Library, Library
  playback, Queue, Queue playback, Status bar, Queue boundary) present in the current Panel mode, per the
  mount rule in design D1 (a panel absent from a mode has no placement, never an empty rect).
  `draw_frame` still delegates to `compose_base_frame`. Verify: relational unit tests in `chrome.rs`
  (present placements partition the terminal per mode, no overlap, no empty placement, the absent set per
  mode matches D1), and every existing render/tick test passes unchanged.
- [x] 1.4 Place `QueueBoundaryComponent` from `RootFrame` (two-panel layout only, unmounted otherwise)
  instead of `LayoutMain.queue_boundary_area`, and delete that field. Verify: queue-boundary drag tick
  tests pass; a tick test in `queue-only` and `library-only` finds the boundary unmounted.

## 2. Tab panel and Status bar panel (S1)

*Unification:* removes shell-painted chrome and the `tabs_hitmap`/status-pill side channels.

- [x] 2.1 Create `TabPanel` (`src/app/components/tab_panel.rs`) that paints the tab bar (moved from
  `chrome_tabs.rs::render_tabs`, keeping `visible_tab_range` overflow arrows), retains its tab hit
  regions, and emits a tab-select `Msg` for clicks; mount it at the root; delete `render_tabs`' call in
  `paint_legacy_chrome`, `LayoutMain.tabs_hitmap`, and the shell tab-click path in `shell.rs`. Verify:
  tab buffer tests move to the component and pass; a tick integration test clicks a tab and asserts the
  destination changes; the tab panel fills its own placement with its surface (the full-column
  backdrop underneath is removed in 12.1, once every right-column panel fills itself).
- [x] 2.2 Create `StatusBarPanel` that paints the status row (moved from
  `chrome_status.rs::render_status_bar`) and retains its volume/mute/remote pill regions; delete
  `LayoutPlayback.{ind_vol, ind_mu, ind_rc}` and the `render_status_bar` call in `render_main`. Verify:
  status-row buffer tests pass from the component; a tick test scrolls on the volume pill and asserts the
  volume intent; the status row paints only where `RootFrame` places it.

## 3. Queue panel and Queue playback panel (S2, folds `add-now-playing-sidebar`)

*Unification:* removes the base-frame queue painting, the queue-only-only transport, and the
`narrow_player` flag; one Queue playback panel in every queue-visible layout.

- [x] 3.1 Move the queue frame, title row and status pill row into `QueuePanel` (today's
  `QueueComponent`): delete `render_queue_panel_frame`/`render_queue_status` calls from `render_main`,
  paint the left backdrop from the panel, and move `LayoutMain.queue_{area,title_area,selected_item_rect}`
  into the component. Verify: `src/app/render/tests_queue.rs` and `queue_component_tests.rs` pass; the
  context-menu keyboard anchor for a queue row still opens at the selected row (existing test).
- [x] 3.2 Add `header_height` (always 1 in queue-visible layouts, idle included) to `QueuePanelInputs`
  and offset `queue_panel_geometry` by it alongside the visual-slot and transport heights, so the Queue
  panel always starts below the Queue playback panel's placement plus its separator row. Until 3.5 lands,
  `render_main` leaves that header row blank. Verify: unit test in `arrangements/queue.rs` that the queue
  panel starts below the header row and its separator in idle, paused and playing states; at 24 rows
  `short_window_keeps_queue_in_left_column` and `short_queue_panel_drops_padding_before_rows` pass with
  re-derived (not loosened) thresholds.
- [x] 3.3 Derive `NowPlayingStatus { Playing, Paused, Idle }` once per frame next to
  `effective_playback_state()`, and extract `playback_host_label()` from `queue_title_model` (queue title
  uses it; no tracking suffix, no uppercasing). Verify: unit test of the three states (unreachable
  `!active && paused` → `Idle`); `queue_title_characterization_tests.rs` green unchanged; the label for an
  attached session has no ` · TRACKING`.
- [x] 3.4 Move the visual slot's image fetch out of `render_card`: the queue projection issues
  `fetch_card_image` for the now-playing item and projects image state; painting reads it only.
  Verify: a push test asserts one fetch per new now-playing key and none on repaint; `render_card`'s
  replacement paints from projected state in a buffer test with no `App` access.
- [x] 3.5 Create `QueuePlaybackPanel`, mounted in every queue-visible layout (idle included), owning the
  header row (status left, `on <host>` right, on `SURFACE_CHROME`), the visual slot
  (artwork/placeholder/visualizer) and the queue-column transport presentation, with placement: below
  100 columns stacked, 100+ side by side (slot left, 2-cell gap, panel height = max); while idle it paints
  only the header row. Extract one width-driven transport arrangement (which rows and indicators show
  at a given width) that both this panel and the Library playback panel (4.1) call. Mount it at the
  root's Queue playback placement; delete `render_card` and both
  queue-only `render_player_panel` calls from `render_main` and remove `narrow_player`. Verify: buffer
  tests at 80 and 100+ columns showing `PLAYING`/`PAUSED`/`IDLE` and the target; an idle frame paints the
  header row and nothing else of the panel; a remote-attached frame with the Local scope selected names
  the remote target; the panel is unmounted in library-only.
- [x] 3.6 Idle collapse in every queue-visible layout: no visual slot or transport while idle (the
  connected-idle exception is deleted); paused keeps both; playback start restores both. Verify:
  `connected_idle_queue_only_keeps_panel_but_collapses_card` becomes the inverse assertion;
  `idle_queue_only_hides_card_and_panel_at_both_widths`, `idle_both_hides_card_and_reclaims_queue_rows`,
  `idle_queue_only_reclaims_card_and_panel_rows_until_playback_starts` and
  `paused_queue_only_keeps_card_and_panel` (plus a `both` counterpart) pass at narrow, 80 and 100+.
- [x] 3.7 Pointer input on the Queue playback panel transport from its own retained geometry. Verify:
  tick integration test clicks play/pause and the seekbar in `both` and mini-view `queue-only`
  (`TogglePlayPause`, `SeekTo`); a click in a collapsed panel's rows emits nothing.
- [x] 3.8 Re-point the idle-feed open-link gate at the playback panel's presence instead of the panel
  mode (`action.rs:104-118`, `shell.rs:284`). Verify: `src/app/action_tests.rs` and
  `src/app/tests_routing_matrix_playback.rs` rows: suppressed in idle `both` and `queue-only`, fires in
  idle `library-only`.

## 4. Library playback panel (S3)

*Unification:* one transport painter per frame; the strip reserves rows only when it paints.

- [x] 4.1 Turn `PlaybackComponent` into `LibraryPlaybackPanel`, mounted only when the queue column is
  hidden and laying out its transport through the shared transport arrangement from 3.5; `RootFrame` reserves `PLAYER_BOX_HEIGHT` in the library column only then. Verify: buffer
  proof in `tests_queue.rs` that `both` reserves no strip rows, `library-only` exactly
  `PLAYER_BOX_HEIGHT`, `queue-only` none; a tick integration test draws `both`, `queue-only` and
  `library-only` and finds exactly one transport per frame, painted by the expected panel; strip clicks
  resolve against the strip's painted geometry.

## 5. Library panel types, skeletons and Home (S4)

*Unification:* creates the only paint path for library screens; no destination can add a pill bar,
choose a focus kind, pick a surface or lay out a pane.

- [x] 5.1 Create the panel module (`src/app/components/library_panel/`) with the content types of
  design D3 (`LibraryPanelContent`, `SelectorRow`, `ListControls`, `ListSlot`, `HeroContent`,
  `HeroHeader`, `Workspace`) and the slot Render Components for Selector row (one pill bar + spacer,
  retained `HitRegions`) and List controls row (optional pills + optional label). The shared
  arrangement primitives (`wide_hero_presentation`, `pill_bar_areas`, `wide_hero_browser_pane`,
  `wide_hero_hero_content_box`, `place_media_list_below`) stay public until 12.3, because un-migrated
  destinations still call them. Verify: buffer tests for each slot component (pill active state, label,
  empty row absent), and a hit test resolving a painted pill; `cargo check -p mbv` green.
- [x] 5.2 Implement the Wide skeleton: Browser pane (Selector row, List controls row, list box fill +
  `wide_hero_browser_border`, list presentation or empty placeholder), gap, Hero pane (resting surface;
  focused only when a Workspace is present and focused). For `ListSlot::Search` the panel places the
  Inline Search box in the Selector row's rect and its results in the list box, calling the existing
  Inline Search painter with those rects. Verify: buffer tests for a read-only hero, a focused-workspace
  hero, and an active search (box in the Selector row's place, results in the list box, Hero pane
  unchanged); role-rect containment only (no coordinates).
- [x] 5.3 Parse Emby image availability: add `ImageTags` (`Thumb`, `Primary`) and `BackdropImageTags`
  (and, for episodes, the series' thumb/backdrop tags) to `EmbyItem` (`crates/mbv-core/src/api_types.rs`),
  requesting them where the item `Fields` lists need it. Verify: `cargo nextest run -p mbv-core` with
  parser tests on recorded item JSON with and without each tag.
- [x] 5.4 Implement the artwork policy and hero producers (design D5): `artwork_policy(&item) ->
  HeroArtwork` (Music and podcasts Square; else first declared of Landscape > Square > Portrait; none →
  Landscape, or Square for Music/podcasts), returning shape, Emby image-type chain and cache key; one
  `hero_content(&item)` producer per content type (`EmbyItem`, `QueueItem`, ABS book, ABS podcast show
  and episode, feed entry) filling `HeroFacts` with plain `meta_rows: Vec<String>`. Delete
  `HeroArtworkAspect`. Verify: unit tests of the policy table (movie with backdrop + poster →
  Landscape; poster only → Portrait; album with a thumb → Square; podcast episode → Square; book →
  Portrait; feed entry → Landscape placeholder; podcast feed entry → Square), and that the Home path
  and the Books tab produce identical `HeroContent` for the same ABS book.
- [x] 5.5 Implement `HeroHeader` Landscape / Portrait / Square, constructed only from the policy's
  shape, with one title/meta painter that colours meta row *n* with `HERO_META_ROLES[n % 3]` (three
  roles added to `render/theme`) and owns truncation/wrapping; placeholder at full box size while
  loading; artwork shrinking before a Workspace viewport drops; the overview Main content box only when
  overview text exists. Add the paint-free `hero_artwork_box(area, &HeroContent)` and cover fit (the
  image worker `resize_to_fill`s to that box's pixel size, keyed by box size, then `Resize::Scale`).
  Verify: buffer tests per arm (art above vs art right), four meta rows cycling colours 1-2-3-1,
  overview present/absent, placeholder size; a unit test that a 4:3 source filled into a 16:9 box is
  cropped top and bottom; `HeroHeader` has no public constructor.
- [x] 5.6 Implement the Workspace: optional Selector row over one Main content box holding a
  `&mut dyn PanelList`, accent-soft surface while focused, owning-surface selected row. Verify: buffer
  tests for focused/unfocused box surface and selected-row surface; `WideHeroContentBoxSurface` deleted.
- [x] 5.7 Implement the Narrow skeleton and the inline hero derived from the same `HeroContent` (policy
  image right-aligned, sized from its aspect, wrap-around text in the three meta colours, full width
  below), with no selector, controls or constituent rows inside it; the panel computes the Inline
  `desired_detail_rows`. Verify: buffer tests with a portrait, a landscape and no image — all
  right-aligned wrap-around — and a test that Wide and Narrow render the same title, meta rows and image
  for one `HeroContent`.
- [x] 5.8 Add the object-safe `PanelList` trait (design D3) implemented once by the shared media-list
  carrier for every `Target`; the panel calls `set_presentation(Wide | Inline, anchor)` from its own
  breakpoint choice and sets the paint policy (focus, `SelectedRowSurface` by slot, throbber from
  `ListSlot` loading state); remove the destination-side `MediaListCarrier::active` selection and
  `Presentation::Grid`. Verify: the canonical-list re-anchor tests pass with the panel driving the
  transition; `cargo check -p mbv` shows no `ListSlot`/`Workspace` arm per destination.
- [x] 5.9 Mount `LibraryPanel` as `ComponentId::Library`: owner map keyed by `LibraryKey`, focus and
  mouse subscription for the library area, slot events (`SelectorPicked`, `ControlPicked`,
  `WorkspaceSelectorPicked`, list delegation) routed to the active owner, the Wide split-boundary drag
  moved in from `WideHeroBoundaryComponent`, owner retention while the library is in the catalog (moved
  from `reconcile_destination_mounts`). `RootFrame` gives the library rect to `LibraryPanel` when the
  active library's owner has migrated, otherwise to the old component. Verify: tick integration tests
  for focus following the active library, mouse eligibility, split drag
  (`tests_tick_integration_wide_split_resize.rs`), and an inactive owner keeping cursor/scroll across a
  tab change.
- [x] 5.10 Move image requests for Hero headers and inline heroes into the shell projection: for each
  projected hero run `artwork_policy` + `hero_artwork_box` (with the Library panel's `RootFrame` area)
  and issue `fetch_card_image` there, generalizing TV's `push_tv_workspace_content`; painting reads
  projected image state. Verify: push tests assert one fetch per new key and none on repaint; a tick
  test resizes the terminal and asserts one re-encode request at the new box size with the placeholder
  shown for at most one frame.
- [x] 5.11 Convert Home into `HomeContent` (embedded owner): section pills → Selector row, rows → list
  slot, selected item → the shared `hero_content` producer for its content type (Emby, ABS or Feeds),
  Narrow inline hero from the same content; delete `render_home_content`, `HomeCarrier`,
  `keep_watching_hero_image_types` and the `"{id}:pwr_kw"` key, Home's `pill_regions`/spacer paint and
  `ComponentId::Home`. Verify: `tests_home_characterization.rs`/`tests_home_inline.rs` rewritten to the
  panel output where header, artwork and overview changed; `tests_tick_integration_home.rs` passes; pill
  clicks select sections.

## 6. Movies, home videos and generic Emby libraries (S5)

*Unification:* removes `render_wide_movies`, the Narrow Movie banner model, the count-label row and the
Grid catalog.

- [x] 6.1 Convert `BrowserComponent` for Movies, HomeVideos and Generic into `BrowserContent`: letter
  pills → Selector row, home-video count → List controls label, hero from the shared `EmbyItem` producer, Inline Search as list
  slot state; generic libraries use the Wide/Inline presentations (no Grid). Delete
  `browser/paint.rs::render_wide_movies`, the Movies/HomeVideos/Generic arms of
  `render_narrow_browse_with_ctx`, `CompactBannerLayout`, `render_compact_detail_with_ctx`, and the
  `ComponentId::Browser` arm for these kinds. Verify: `tests_library_characterization.rs` and
  `tests_non_music.rs` rewritten where output changed; `tests_tick_integration_browser.rs` passes;
  `browser_inline_search_tests.rs` passes.

## 7. Feeds (S6)

*Unification:* removes the second pill bar and the Feeds-only hero box — the most divergent screen
proves the slot types early.

- [x] 7.1 `FeedsContent`: move `FeedsComponent`'s content state into a plain `LibraryContentOwner`
  (feed-group pills → the one `SelectorRow`; the Watched filter → `ListControls` pills; entries → the
  list slot; selected entry → the shared feed-entry producer — Square for a podcast feed, Landscape
  placeholder otherwise — plus its overview), filled by the shell's existing Feeds push. The component
  still paints its legacy painter this step, so no output moves. Verify: unit tests of `content()` (one
  pill bar; the controls row only with a filter; Square/Landscape per feed kind); `tests_feeds.rs`,
  `feeds_component_tests.rs` and `tests_tick_integration_feeds.rs` pass unchanged.
- [x] 7.2 Paint Feeds through the panel: `FeedsComponent::view` becomes the shared Wide/Narrow skeleton
  over `content()`, keeping only its own pill/row hit store, and `render_feeds_content`, its
  `render_selector_content` closure and `paint_feed_hero` are deleted with `tests_feeds.rs` rewritten to
  the new output (the hero image is the projected `HeroImageState`, never a paint-time fetch). Verify:
  one pill bar, one controls row and the policy header in the buffer; `w` and a click on a Watched pill
  both change the filter; `feeds_component_tests.rs` and `tests_tick_integration_feeds.rs` still pass.
- [x] 7.3 Register Feeds as `LibraryKey::Feeds`: delete `ComponentId::Feeds`, the mounted component, its
  hit store and `LayoutMain.feeds_area`, with slot-event translation and hit resolution moving to the
  panel, and re-point `feeds_component_tests.rs` at the owner type. Verify: tick integration tests for
  focus, mouse eligibility and a pill/row click through the panel, and owner retention across a tab
  change; `rg FeedsComponent src` finds nothing.

## 8. TV (S7)

*Unification:* removes the two-component TV split and TV's private header and poster painters (Narrow TV now shows the policy's landscape art).

- [x] 8.1 Merge `TvWorkspaceComponent` and `BrowserComponent(TvShows)` into one `TvContent` owner:
  one series list owner (Wide + Inline presentations), episode list, season cursor, Inline Search
  session; delete `hand_off_tv_breakpoint`, the TV part of `apply_pending_inline_search_transfer`, and
  the second mounted id (Narrow keeps painting `render_narrow_browse_with_ctx` from the merged
  component this step). Verify: `tv_workspace_component_tests.rs` and the TV re-anchor tests pass
  (stable target across Wide↔Narrow); `tests_tick_integration_tv.rs` passes.
- [ ] 8.2 TV Wide through the panel: the Wide view becomes the shared `render_wide_skeleton` (letter
  pills → Selector row, hero from the shared `EmbyItem` producer, overview box, Workspace = season
  pills + episodes), and `tv_wide.rs::render_wide_tv_with_ctx` with `LayoutMain.tv_wide_*` is deleted,
  `tv_wide_tests.rs` rewritten to panel output. Narrow keeps `render_series_inline_detail` this step.
  Verify: season-pill and episode clicks resolve through tick tests; the Narrow tests pass unchanged.
- [ ] 8.3 TV Narrow through the panel: delete `render_series_inline_detail`, `SERIES_IMAGE_COLS/ROWS`,
  `NarrowInlineHero` and their tests; the inline hero derives from the same `HeroContent` (the policy's
  landscape art, no longer the poster) and episodes open only through `SelectionModal`. Verify:
  `detail_series_tests.rs` and the Narrow TV characterization rewritten to panel output; a Wide↔Narrow
  resize keeps the selected series and its viewport.
- [ ] 8.4 Register TV as `LibraryKey::Service(TvShows)`: delete the remaining mounted component, its
  `ComponentId` arm and its hit stores, moving season-pill/episode hit resolution to the panel, and
  re-point `tv_workspace_component_tests.rs` and the TV rows of `tests_library_characterization.rs` at
  the owner. Verify: tick integration tests for focus, mouse eligibility and both lists; an inactive TV
  owner keeps cursor/scroll across a tab change.

## 9. Music (S8)

*Unification:* removes Music's private header layout, its focused-box surface arm and dead search
branches.

- [ ] 9.1 `MusicContent`: move `MusicWorkspaceComponent`'s content state (group pills, albums, tracks,
  cursor, inline search) into a `LibraryContentOwner` (group pills → Selector row, albums → list slot,
  Square header, no overview box when the album has none, Workspace = tracks), filled by the shell's
  existing Music push; the component keeps painting its legacy painters this step. Verify: unit tests of
  `content()` (Square artwork for an album, overview omitted when absent, tracks as the workspace);
  `tests_music_wide.rs`, `tests_music_narrow.rs`, `tests_music_groups.rs` and the Music component tests
  pass unchanged.
- [ ] 9.2 Music Wide through the panel: delete `music_wide.rs`'s wide path
  (`render_wide_music_group_with_ctx`, `render_wide_left_hero`), `music_wide_browser.rs`,
  `wide_music_left_layout` and `LayoutMain.wide_music_*`, painting `render_wide_skeleton` instead;
  Narrow keeps its legacy painter this step. Verify: `tests_music_wide.rs`, `tests_music_groups.rs` and
  `tests_music_wide_reanchor_characterization.rs` rewritten to panel output; the re-anchor
  characterization still passes.
- [ ] 9.3 Music Narrow through the panel: delete `render_narrow_music_group_with_ctx` and the
  `is_search_active` search-box/`render_plain_rows` branches, rewrite `tests_music_narrow.rs` and
  `tests_music_characterization.rs`, with the inline hero built from the same `HeroContent` and tracks
  opening only through `SelectionModal`. Verify: `tests_tick_integration_music_mouse.rs` passes at both
  breakpoints; album art is the projected image, never a paint-time fetch.
- [ ] 9.4 Register Music as `LibraryKey::Service(Music)`: delete the mounted component, its `ComponentId`
  arm and the per-component pushes in `shell_music_workspace*.rs`, moving hit resolution to the panel,
  and re-point the Music component tests at the owner type. Verify: tick integration tests for focus,
  mouse eligibility and album/track clicks; an inactive Music owner keeps cursor/scroll across a tab
  change.

## 10. Audiobookshelf Books (S9)

*Unification:* removes the second content box, the list-backdrop workspace row and inline chapters —
Books conforms to Emby.

- [ ] 10.1 `BookContent`: move `AudiobookshelfBookComponent`'s content state into a
  `LibraryContentOwner` (surname buckets → Selector row, Portrait header with progress in meta,
  overview box, Workspace = chapters), filled by the shell's existing Books push; the component keeps
  painting its legacy painter this step. Verify: unit tests of `content()` (one pill bar, Portrait
  artwork, progress as a plain meta row, chapters as the workspace); `tests_audiobookshelf_books.rs`,
  `audiobookshelf_book_component_tests.rs` and `tests_tick_integration_book.rs` pass unchanged.
- [ ] 10.2 Paint Books through the panel: delete `render_audiobookshelf_book_content`,
  `render_narrow_book`, `render_book_rows`' inline path and the private geometry struct, painting the
  shared skeleton instead, with `tests_audiobookshelf_books.rs` rewritten to panel output (Narrow
  inline hero without chapters, Enter opens the selection modal). Verify: buffer tests for the Wide
  Workspace box and the Narrow hero; Narrow Enter opens the chapter modal;
  `audiobookshelf_book_component_tests.rs` still passes.
- [ ] 10.3 Register Books as `LibraryKey::Service(AudiobookshelfBook)`: delete the mounted component, its
  `ComponentId` arm, its hit store and `LayoutMain.audiobookshelf_book_area`, moving slot-event
  translation to the panel, and re-point the book component tests at the owner type. Verify: tick
  integration tests for focus, mouse eligibility and chapter-row clicks; the book owner keeps
  cursor/scroll across a tab change.

## 11. Audiobookshelf Podcasts (S10)

*Unification:* removes the episode table and the Narrow inline filter pills and episodes — Podcasts
conforms to Emby.

- [ ] 11.1 `PodcastContent`: move `AudiobookshelfPodcastComponent`'s content state into a
  `LibraryContentOwner` (alphabetical buckets → Selector row, Square header, overview box, Workspace =
  filter pills + downloaded episodes), filled by the shell's existing Podcasts push; the component
  keeps painting its legacy painter this step. Verify: unit tests of `content()` (one pill bar, Square
  artwork, the workspace carrying both the filter Selector and the episode list);
  `tests_audiobookshelf_podcasts.rs` and the podcast component tests pass unchanged.
- [ ] 11.2 Paint Podcasts through the panel: delete `render_audiobookshelf_podcast_content`,
  `render_narrow_podcast`, `paint_bucket_pills` and the private geometry struct — the episode table,
  the Narrow inline filter pills and the Narrow inline episodes go with them — painting the shared
  skeleton instead. Verify: `tests_audiobookshelf_podcasts.rs` and
  `audiobookshelf_podcast_geometry_tests.rs` rewritten to panel output; Narrow Enter opens the episode
  modal; the Wide filter pills are the Workspace Selector row.
- [ ] 11.3 Register Podcasts as `LibraryKey::Service(AudiobookshelfPodcast)`: delete the mounted
  component, its `ComponentId` arm, its hit store and `LayoutMain.audiobookshelf_podcast_area`, moving
  slot-event translation to the panel, and re-point the podcast component tests at the owner type.
  Verify: `tests_tick_integration_podcast.rs` passes through the panel; tick tests for focus and filter
  pill/episode clicks; the podcast owner keeps cursor/scroll across a tab change.

## 12. Delete the base frame and legacy structures (S11)

*Unification:* removes every remaining way to paint outside a panel.

- [ ] 12.1 Delete the base frame composition: `compose_base_frame`, `render_main`,
  `paint_legacy_chrome`, `render_legacy_backdrops`, `render_library`, every shell `render_*_component`
  method, the transitional old-component branch in `RootFrame`, and the remaining legacy `ComponentId`
  arms (`Browser`, `WideHeroBoundary`), so the draw path composes the `RootFrame` placements plus the
  overlay stack only. Verify: `cargo check -p mbv`; `rg` finds none of these symbols; no
  `fetch_card_image`/`fetch_*` call is reachable from any component `view` or Render Component (manual
  check); one tick integration test per Panel mode asserts a non-empty frame.
- [ ] 12.2 Pass the sentinel test: one tick integration test per Panel mode (`both`, `queue-only`,
  `library-only`, mini view) pre-fills the test buffer with a sentinel symbol, draws one frame, and
  asserts no sentinel cell remains inside any mounted panel's `RootFrame` placement — fixing the panels
  that do not yet fill their own surface (what is left of the full-column backdrops). Verify: the four
  tests pass and each fails when a panel's fill is removed.
- [ ] 12.3 Delete the list/cursor half of the remaining `LayoutMain` (`left_item_rows`, `hero_area`,
  `inline_hero_area`, `selector_tabs`, `breadcrumbs`, `selected_item_rect` — whatever the destination
  slices have not already removed), answering context-menu keyboard anchors from the owning component
  through the existing request path. Verify: column-aware cursor and mouse-hit tests pass; `rg` finds no
  removed field.
- [ ] 12.4 Delete `LayoutPlayback` and the remaining chrome `LayoutMain` fields (`panel_area`,
  `panel_content_area`, `home_area`, `card` — whatever the slices have not already removed) with `FrameChromeGeometry`'s paint-to-input use, so
  `RootFrame` is the only root geometry. Verify: `rg LayoutMain src` and `rg LayoutPlayback src` return
  nothing; context-menu keyboard-anchor tests pass for a library row and a queue row.
- [ ] 12.5 Delete the dead paths of design D13 (`GridMediaList`, `NarrowBrowseControl`, `GridPaintPolicy`,
  `LeftPaneFocus`, `SelectedRowSurface` as a caller argument) and the tests that only exercised them.
  Verify: `cargo check -p mbv`; `rg GridMediaList|NarrowBrowseControl|GridPaintPolicy|LeftPaneFocus src`
  finds nothing.
- [ ] 12.6 Delete the last dead branches (Music/TV list-level search painters, `HomeCarrier`, leftover
  per-destination paint helpers) with `src/app/render/tests_conformance_matrix.rs`, then make
  `wide_hero_presentation`, `pill_bar_areas`, `wide_hero_browser_pane`, `wide_hero_hero_content_box` and
  `place_media_list_below` private to the Library panel's arrangement module (no caller outside it
  remains). Verify: `cargo clippy --workspace --all-targets` reports no dead code; `cargo nextest run -p
  mbv` green.

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
- [ ] 14.3 At archive, sync the deltas and confirm `openspec/specs/queue-only-playback/` and
  `openspec/specs/hero-big-text-title/` are deleted and
  `openspec/specs/library-panel/` and `queue-playback-panel/` exist. Verify: `openspec validate --strict`
  clean after sync.
