## 1. Scout handoff (D1 — blocking, read-only)

- [x] 1.1 Write `openspec/handoffs/scout-remove-migrated-surface-underpaint.md`.
      Per surface that `App::render` / `render_main` paints (tab bar, status
      bar, player chrome, Home, Emby generic/Movies/HomeVideo browser, wide TV
      workspace, narrow TV, wide Music workspace, narrow Music, album-track,
      ABS book, ABS podcast, Feeds, inline search, hero), record:
      (a) current painter — legacy body, component view, or both this frame;
      (b) every `AppLayout` field the legacy renderer for that surface
      produces and each reader of it (component or other surface);
      (c) loading-state, image-prefetch, image-handoff, scroll-reconciliation,
      and responsive-variant logic that lives only in the legacy path;
      (d) at which breakpoints a component is the active painter vs. a legacy
      variant;
      (e) the smallest compile-complete suppression units and their dependency
      order.
      Verify: the handoff answers every open question in `design.md` including
      whether `PlaybackComponent` is already the sole player-chrome painter.
      No code changes in this task group.

## 2. Geometry / paint split (D2, D3)

The original 2.1 was a one-shot extraction. The scout found 18 render modules
and 110 layout assignments, so this is now a dependency-first progressive
checkpoint queue dispatched by `render_main` at each owning natural position.
Each pure arrangement result is computed and published before its corresponding
paint; load/paint-coupled geometry is published immediately after its
authoritative producer. The base frame does not imply that every checkpoint
precedes legacy paint: geometry-only is checkpoint-local, not a promise that the
whole frame is paint-free before `render_main`. Every row is bounded to at most
six production files and preserves behaviour.

- [x] 2.1-prep Preserve the preparatory seam in `root.rs` from commit
      `94002d25b75e0f34df200c3def57939c6cffd156`; do not treat it as the
      completed geometry extraction.
- [x] 2.1a Root/chrome foundation (landed): introduce the plain-data frame
      context/result seam and fresh draft `AppLayout`; migrate root/chrome
      geometry into the partial typed subresult and preserve zero-area
      no-mutation plus atomic installation. Publish at the root/chrome natural
      checkpoint, before its pure chrome paint. Boundary: `src/app/layout.rs`,
      `src/app/render/screens/root.rs`, `src/app/render/components/chrome.rs`.
      Verify focused root/chrome tests, `cargo check -p mbv`, and fmt.
- [x] 2.1b CARD CHECKPOINT: publish card geometry at the card owner's natural
      checkpoint, preserving cache/`size_for`/`fetch` as one path across every
      rendering state (loading, missing, cached, fetched, and responsive).
      Boundary: `src/app/layout.rs`, `src/app/render/screens/root.rs`,
      `src/app/render/components/card.rs`, `src/app/images.rs`. Depends on
      2.1a. Verify all card rendering states, cache/image handoff tests,
      check, and fmt.
      Existing implementation already fulfilled this checkpoint; this docs-only
      commit records it (`60342cd8`).
- [x] 2.1c SHARED HERO ARRANGEMENT: publish pure hero arrangement geometry
      before hero paint and retain authoritative image/load handoff timing.
      Boundary: `src/app/layout.rs`, `src/app/render/arrangements/hero_left.rs`,
      `src/app/render/arrangements/library.rs`,
      `src/app/render/components/hero.rs`. Depends on 2.1b. Verify narrow and
      wide arrangement/hero states, check, and fmt.
      Existing implementation already fulfilled this checkpoint; this docs-only
      commit records it (`136c5388`).
- [x] 2.1d FLAT/LETTER LIST CHECKPOINT: publish flat and letter-group list
      geometry at the list owner's natural checkpoint without changing rows,
      loading, empty, selection, or responsive output. Boundary:
      `src/app/layout.rs`, `src/app/render/components/widgets.rs`,
      `src/app/render/components/list.rs`,
      `src/app/render/components/list_rows.rs`,
      `src/app/render/components/list_plain.rs`,
      `src/app/render/components/list_letter_groups.rs`. Depends on 2.1c.
      Verify all list states and characterization tests, check, and fmt.
      Existing implementation already fulfilled this checkpoint; this docs-only
      commit records it (`9050d832`).
- [x] 2.1e GROUPED ALBUM CHECKPOINT: publish grouped-album and album-detail
      geometry at each owning natural checkpoint, retaining album-art loading
      and handoff after their authoritative operations. Boundary:
      `src/app/layout.rs`, `src/app/render/components/album.rs`,
      `src/app/render/components/album_inline.rs`,
      `src/app/render/components/album_detail.rs`,
      `src/app/render/components/album_art.rs`,
      `src/app/render/screens/album_plan.rs`. Depends on 2.1d. Verify grouped,
      inline, detail, loading, and responsive album states, check, and fmt.
      Existing implementation already fulfilled this checkpoint; this docs-only
      commit records it (`0cf9e873`).
- [x] 2.1f DOWNSTREAM QUEUE+PILLS: consume the card and pills checkpoints
      without recomputation, and publish queue/pill geometry at the owning
      natural checkpoint while preserving queue selection and responsive
      behaviour. Boundary: `src/app/layout.rs`,
      `src/app/render/screens/root.rs`, `src/app/render/screens/queue.rs`,
      `src/app/render/screens/pills.rs`,
      `src/app/render/components/widgets.rs`,
      `src/app/render/components/music.rs`. Depends on 2.1e. Verify queue,
      pills, card-consumption, and all rendering-state tests, check, and fmt.
      Existing implementation already fulfilled this checkpoint; this docs-only
      commit records it (`dd46fc6c`).
- [x] 2.1g Feeds/home: publish feed and home geometry at their owning natural
      checkpoints, before pure paint and after image/load operations where
      coupled; preserve loading and image handoff. Boundary:
      `src/app/layout.rs`, `src/app/render/components/feeds.rs`,
      `src/app/render/components/home.rs`,
      `src/app/render/components/home_hero.rs`,
      `src/app/render/arrangements/home.rs`. Depends on 2.1f. Verify feeds/home
      loading, empty, populated, image, and responsive states, check, and fmt.
      Real writer: feeds selector computation collapsed to one paint per region
      and feeds/home geometry published at natural checkpoints (`1a4fb6cf`).
- [x] 2.1h Music: publish ordinary, wide, browser, and track geometry at each
      owning natural checkpoint, preserving grouped-album and image handoff
      operations. Boundary: `src/app/layout.rs`,
      `src/app/render/components/music.rs`,
      `src/app/render/components/music_wide.rs`,
      `src/app/render/components/music_wide_browser.rs`,
      `src/app/render/components/music_wide_tracks.rs`,
      `src/app/render/arrangements/music.rs`. Depends on 2.1g. Verify narrow,
      wide, grouped, track, loading, and responsive states, check, and fmt.
      Real writer: wide music paint now consumes the arrangement computed
      once in `publish_geometry` (no recomputation), commits `324386f2` +
      `b7e97d5b`.
- [x] 2.1i TV/widgets: publish TV-wide and shared widget geometry at their
      owning natural checkpoints, preserving breakpoint and loading behaviour.
      Boundary: `src/app/layout.rs`, `src/app/render/components/tv_wide.rs`,
      `src/app/render/components/widgets.rs`,
      `src/app/render/components/detail.rs`. Depends on 2.1h. Verify wide and
      narrow TV/widget states, check, and fmt.
      Docs-only: TV-wide geometry is published at its natural checkpoint
      before `render_list` (gated by the shared breakpoint, loading
      component-side), shared widget geometry already published pre-paint by
      rows 2.1a–2.1f, and component-local episode rows/season tabs stay
      paint-coupled. Boundary note added in `layout.rs`.
- [x] 2.1j Aggregate consolidation: merge the progressive checkpoint results
      into the complete fresh `AppLayout`, retire deferred legacy computation,
      and verify one authoritative computation for every aggregate field.
      Publish the aggregate at its natural final checkpoint and atomically
      install it; preserve zero-area no-mutation. Boundary:
      `src/app/layout.rs`, `src/app/render/screens/root.rs`,
      `src/app/shell_audiobookshelf_book.rs`,
      `src/app/render/components/audiobookshelf_book.rs`,
      `src/app/audiobookshelf_book_modal_actions.rs` (Book surface has no
      AppLayout producer after the legacy renderer removal; the shell mirror
      restores it). Depends on 2.1a–i.
      Verify full render characterization, aggregate zero-area tests, check, and
      fmt.
      Real writer: restored the Audiobookshelf Book AppLayout projection via a
      shell geometry mirror (left_area/hero/selected/selector_tabs + wide flag
      driving `is_wide_book_active`), re-homed the narrow modal branch off the
      component-reported flag, and added aggregate zero-area + per-surface
      single-producer tests.
- [x] 2.2 After 2.1j, extract `paint_legacy_chrome` from the progressive
      geometry orchestration, preserving all legacy painting initially. Depends
      on 2.1j. Verify full nextest and fmt.
      Real writer: `App::paint_legacy_chrome` extracts the four pre-body chrome
      paints (left/right column backgrounds, tab bar, right-column player panel)
      and is called from `render_main` at the root/chrome checkpoint; `render_tabs`
      reads the mid-`render_main` `self.tab` normalization, so 2.3's `draw_frame`
      hoists the call out. Output byte-identical (no new test failures).
- [x] 2.3 Add `Model::draw_frame` in `shell_run.rs` (or `shell_draw.rs` if
      needed), preserving resize pushes and component paint order. Depends on
      2.2. Verify check and fmt.
      Real writer: `Model::draw_frame(&mut self, f, music_resize, tv_resize)`
      holds the verbatim steady-state closure body (dim-backdrop, `App::render`,
      resize pushes, component views, overlay stack) and the one steady-state
      `terminal.draw` site now calls it; the two startup sites are unchanged
      (2.4). `shell_run.rs` stays at 546 lines, so no `shell_draw.rs` needed.
- [x] 2.4 Route all three terminal draws through `draw_frame`, then add the
      startup-frame characterization. Depends on 2.3. Verify focused startup,
      full nextest, and fmt.
      Real writer: the two startup `terminal.draw` sites in `Model::run`
      (`shell_run.rs`) now call `self.draw_frame(f, false, false)` — there are
      no startup resize locals — so all three terminal draws route through the
      one `draw_frame` implementation. Per D3, startup now paints the full
      first frame (mounted component views + loading affordances) instead of
      the old chrome-only flash. New characterization
      `startup_frame_paints_loading_affordances_not_blank_panes`
      (`tests_home_characterization.rs`) renders the startup frame via
      `draw_frame` and asserts the mounted `HomeComponent` paints its pill bar
      and empty-state placeholder rather than a blank pane. No existing test
      expectation needed updating; the three known pre-existing failures
      (`browser_local_navigation_mirrors_legacy_flat_movement`,
      `feeds_pill_row_and_targets_are_characterized_end_to_end`,
      `matrix_all_surfaces_paint_one_pill_bar_with_one_parent_spacer`) still
      fail identically and are not caused by this change.

## 3. Per-surface suppression (D4 — one unit per surface × breakpoint, scout order)

> Each sub-task below is a template; the scout handoff (1.1) fixes the exact
> list and order. For every unit: (i) make `render_main`'s dispatch arm for
> that surface+breakpoint return before painting the body, still computing
> geometry; (ii) add a debug assertion / test counter that fires if the legacy
> arm paints while the component is the active target; (iii) run that surface's
> render characterization tests + `rtk cargo nextest run -p mbv <surface>`.

- [x] 3.1 Home body — suppress legacy paint when `HomeComponent` is active.
      Already geometry-only: the Home dispatch arm is `layout.home_area = area`
      with no width branch (`src/app/render/components/widgets.rs:528`).
      Characterized by
      `tests_home_characterization.rs::legacy_base_frame_does_not_paint_home_content_before_the_component`
      (extended to also cover the wide 120x40 breakpoint).
- [x] 3.2 Emby generic/Movies/HomeVideo browser body — suppress when
      `BrowserComponent` is active; confirm the #611 browser change already
      removed the `wide_movies` / `movies_wide_right_area` residue, else fold
      D18 step 2 in here.
      Already suppressed: `render_list` returns after publishing
      `movies_wide_*` geometry (`src/app/render/components/list.rs:98`). No
      legacy `wide_movies` renderer residue in `src/app/render/` — only the
      geometry publication, `is_wide_movies_library` predicate, and the
      `is_wide_movies_active()` layout accessor remain (the component's own
      `render_wide_movies` lives in `src/app/components/browser.rs`).
      Characterized by
      `tests_non_music.rs::wide_movies_legacy_base_frame_publishes_geometry_but_paints_no_rows`.
- [x] 3.3 Wide TV workspace body — suppress when `TvWorkspaceComponent` is
      active and wide.
      Already suppressed: `render_list` returns for
      `is_wide_tv_library || is_podcast_library` under a wide hero presentation
      (`src/app/render/components/list.rs:113`), after `render_library`
      publishes `tv_wide_*`. Characterized by
      `tv_wide_tests.rs::wide_tv_legacy_base_frame_publishes_geometry_but_paints_no_workspace`.
- [x] 3.4 Wide Music workspace body + album-track — suppress when
      `MusicWorkspaceComponent` is active and wide.
      Real writer: restored the wide-grouped-Music early return in
      `render_list` (`src/app/render/components/list.rs:116`), dropped in
      `dce4389d` during the component migration (#613). It fires on the same
      predicate that `render_library` (`widgets.rs:545`) uses to publish
      `wide_music_*` via `publish_geometry` — `is_music_group_view &&
      is_viewing_album_folders && shared_hero_presentation(area).is_some()` —
      so geometry (`wide_music_area`, `wide_music_right_area`, `left_area`,
      `hero_area`, `wide_music_art_area`) is still published by `render_library`
      before the return; `ensure_lib_loaded_for` also runs before it. The
      mounted `MusicWorkspaceComponent` (`render_wide_music_group_with_ctx`) is
      the sole wide-music painter and owns its own pill/track hit geometry;
      `App.layout.main.selector_tabs` has no non-test reader for wide music.
      Narrow Music and album-track keep the legacy painter (D5): `shared_hero_presentation`
      needs width ≥ 82, and album-track is a deeper nav level where
      `is_viewing_album_folders` is false. Tests updated: rewrote
      `music_buffer_characterization_covers_wide_unfocused_narrow_and_selected_states`
      (wide legacy frame now paints no "First Album"; narrow still does), added
      D4 proof `wide_music_legacy_base_frame_publishes_geometry_but_paints_no_rows`,
      and routed the Music case of `matrix_all_surfaces_paint_one_pill_bar_with_one_parent_spacer`
      through the mounted component (new `render_music_component` helper),
      mirroring the Home/Book/Podcast precedent.
- [x] 3.5 ABS book body — suppress when `AudiobookshelfBookComponent` is active.
      Already geometry-only: the Book case of `render_audiobookshelf_library`
      sets `layout.audiobookshelf_book_area = area` and returns with no paint
      (`src/app/render/components/widgets.rs:599`-`600`; the legacy Book
      renderer was removed in 5.3d.13). Characterized by
      `tests_conformance_matrix.rs::abs_book_legacy_base_frame_publishes_geometry_but_paints_no_books`
      (narrow 60x20 and wide 120x40).
- [x] 3.6 ABS podcast body — suppress when `AudiobookshelfPodcastComponent` is
      active.
      Already geometry-only: the podcast case of `render_audiobookshelf_library`
      only assigns `layout.audiobookshelf_podcast_area = area`
      (`src/app/render/components/widgets.rs:605`) and nothing else runs in the
      function — the scout's "no explicit return" note is moot, the assignment
      is the last statement. Characterized by
      `tests_conformance_matrix.rs::abs_podcast_legacy_base_frame_publishes_geometry_but_paints_no_shows`
      (narrow 60x20 and wide 120x40).
- [x] 3.7 Feeds body — suppress when `FeedsComponent` is active.
      Already geometry-only: the Feeds arm of `render_library` is
      `layout.feeds_area = area` with no `render_list` delegation
      (`src/app/render/components/widgets.rs:531`), so the legacy base frame
      paints no feed entry or pill. (The `feeds.rs` component double-pill-bar
      fix in `33782e1e` was a separate, component-side bug, not this arm.)
      Characterized by
      `tests_conformance_matrix.rs::feeds_legacy_base_frame_publishes_geometry_but_paints_no_entries`
      (narrow 60x20 and wide 140x30).
- [x] 3.8 Inline search body — suppress when an `InlineSearchComponent` is
      active.
      Real writer: the recon's `if search_active { return; }` was a no-op —
      `render_list`'s `search_active` (`list.rs:141`) has been permanently
      `false` since commit `d006e3fa` removed `LibraryTab.search`, so
      `library_list_render_ctx` hard-codes `search_query: None` and every
      `search_active` block in `render_list` (incl. the `render_search_box`
      call) is dead. The real underpaint: while the mounted
      `InlineSearchComponent` overlays `left_area`, legacy `render_list` still
      paints the ordinary browse list into the same rect. Mount state is the
      only source of truth (no `App` search state remains), so — mirroring
      `dim_backdrop_active` — added transient `App::inline_search_active`
      (`app_struct.rs:211`), projected once per frame in `Model::draw_frame`
      (`shell_run.rs:46`) from
      `matches!(self.app.tab, EmbyLibrary(i) if self.inline_search_component_id(i).is_some())`,
      and gate `if self.inline_search_active { return; }` in `render_list`
      immediately after `layout.left_area = list_area;` (just before the
      `n == 0` branch). Wide Movies/home-video already returns earlier
      (3.2 gate, `list.rs:98`), so this only gates the narrow breakpoint.
      Layout-field audit: clean. `left_area` is the only `App.layout.main`
      field any inline-search reader needs — `shell_inline_search.rs`
      `inline_search_area()` reads `left_area` (then wide fallbacks) and
      `shell_browser.rs:235` reads `left_area`/`movies_wide_area`; both are
      published before the gate. `router.rs`/`key_policy.rs` read no layout.
      `hero_area`/`inline_hero_area`/`selected_item_rect`/row maps have no
      non-render `App.layout.main` reader for the Emby-browser surface
      (`browser.rs` mouse hit-tests its own component `LayoutMain`, not
      `App`'s; the ABS-book/podcast `hero_area` readers are a different
      destination), and per-frame `AppLayout::default()` keeps them zeroed.
      Tests: added D4 proof
      `inline_search_active_legacy_base_frame_publishes_geometry_but_paints_no_rows`
      (`tests_non_music.rs`); no existing test asserted the narrow
      inline-search underpaint, and
      `inline_library_search_renders_plain_candidates_without_app` renders the
      component directly and is unaffected. The dead `search_active` blocks
      in `render_list` are left in place — possible follow-up cleanup, out of
      scope for 3.8.
- [x] 3.9 Player chrome — suppress the legacy player-chrome paint if the scout
      confirms `PlaybackComponent` is the sole painter; otherwise move the
      chrome paint into `paint_legacy_chrome` and record it as sole-legacy.
      Real writer: the right-column player panel was a true redundant
      underpaint — `PlaybackComponent::view` (`src/app/components/playback.rs`)
      and `paint_legacy_chrome` (`src/app/render/screens/root.rs`) both called
      `render_player_panel` into the same `player_area` every right-column
      frame. Deleted the `render_player_panel` block from `paint_legacy_chrome`
      and dropped its now-unused `playback`/`player_h`/`show_controls`/
      `now_playing_title` params (and the `player_area` destructure); the
      queue-only-mode panels in `render_main` (~lines 411/429) stay untouched
      as the sole legacy renderer (D5) because `player_area` is
      `Rect::default()` in queue-only mode. Background match confirmed: legacy
      `queue_focused` (`root.rs:107`) and component `focused`
      (`shell_playback.rs`) are the identical
      `matches!(effective_panel_focus(), PanelFocus::Queue)` expression, both
      resolving `SURFACE_FOCUSED` / `SURFACE_PLAYBACK`. Field audit: the only
      production reader of `App.layout.playback.{seekbar,play_pause,stop,next}_area`
      is `seek_to_col` (`mouse_gestures.rs`), reached solely via
      `PlaybackRequest::SeekTo` from `PlaybackComponent`'s own seekbar
      left-click — a D16 accepted-broken mouse path, not a hit-test/layout/
      persistence reader; after this change that field stays `Rect::default()`
      in right-column mode so seekbar click-seek is a no-op there (mouse,
      out of scope). Parity fix: `sync_playback` derived the now-playing title
      from the local queue slot even when `effective_playback_state` reports
      `active` for a cast target with an empty local queue, so the cast title
      (previously painted by the legacy chrome) vanished; added a
      `cast_now_playing_title` fallback in that branch. Tests: added D4 proof
      `player_chrome_legacy_base_frame_publishes_geometry_but_paints_no_panel`
      (`tests.rs`), and rerouted `rendered_text` (feeding the two
      `cast_now_playing_title_*` tests) through the shell playback-component
      path since the legacy frame no longer paints the title. Component-direct
      `render_player_panel`/`playback_panel_context` tests unaffected;
      `startup_frame_paints_loading_affordances_not_blank_panes` still passes.
- [x] 3.10 After each unit: `rtk cargo nextest run -p mbv` full suite,
      `rtk ast-grep scan`, `rtk cargo clippy --workspace --all-targets` green.
      All Section 3 units pass the gate: full `rtk cargo nextest run -p mbv
      --no-fail-fast` is green except the pre-existing allowed failure
      `browser_local_navigation_mirrors_legacy_flat_movement`; `rtk ast-grep
      scan` holds at 66 pre-existing errors (no new); `rtk cargo clippy
      --workspace --all-targets` holds at 97 warnings (no new); `rtk cargo fmt`
      accepted.

## 4. Dead renderer deletion (D6)

- [x] 4.1 For each suppressed body whose legacy renderer now runs only to
      publish an `AppLayout` field: move that derivation into
      `compute_frame_layout` (or the owning component per D18 step 2), then
      delete the renderer. Verify: `rtk cargo check -p mbv` — no remaining
      caller; `rtk ast-grep scan` clean.
      Real writer: `render_audiobookshelf_library`
      (`src/app/render/components/widgets.rs`) was the one pure-geometry
      renderer — an unused `_f`/`_focused` and nothing but
      `audiobookshelf_book_area`/`audiobookshelf_podcast_area` assignments.
      Inlined the Book-vs-Podcast area reservation into `render_library`'s ABS
      dispatch arm and deleted the function (grep-confirmed sole caller). No
      other renderer is pure-geometry; Section 3 suppressed the rest in place
      with geometry already published upstream.
- [x] 4.2 Confirm `App::render` (the old entry point) has no remaining callers
      and delete it, or reduce it to the two-call shim if a test still uses it
      — prefer deletion, update tests to `compute_frame_layout` +
      `paint_legacy_chrome`. Verify: `rtk cargo nextest run -p mbv`.
      Real writer: `App::render`'s body is entirely `App`-scoped (chrome
      checkpoint, fresh draft `AppLayout`, toast expiry, now-playing-title
      derivation, `render_main`, atomic install), so a `Model` wrapper or a
      `compute_frame_layout` + `paint_legacy_chrome` shim would only add
      indirection. Renamed the method to `App::compose_base_frame` (not
      `render` — #607: no parallel legacy render path, only the base-frame
      composer beneath the mounted component views). `Model::draw_frame` and
      all 15 test call sites route to the new name; the zero-area
      `compute_frame_layout` → `None` no-mutation guard and the single atomic
      `self.layout = layout` install are byte-for-byte unchanged. Full suite:
      1142 passed, 1 failed (only the pre-existing allowed
      `browser_local_navigation_mirrors_legacy_flat_movement`).
- [x] 4.3 `rtk make check-code-file-lines` — split any file the extraction
      pushed over 800 lines in the same change.
      No split needed: `shell_run.rs` 559→559, `root.rs` 597→604 (+7 doc
      comment), `list.rs` 596→596. All governed files at or below 800.

## 5. Ledger + gate (D5)

- [ ] 5.1 Update each affected row's Notes cell in
      `docs/architecture/interactive-surface-ledger.md`: single-painter
      ownership for component-owned surfaces; "wide: component; narrow: sole
      legacy renderer" for split surfaces (TV, Music); note that
      `self.app.render(f)` underpaint is removed and the draw path is one
      `Model::draw_frame` entry point. Verify: no row still describes a
      legacy painter running beneath a component.
- [ ] 5.2 `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`,
      `rtk cargo clippy --workspace --all-targets`,
      `rtk cargo fmt --all -- --check`, `rtk ast-grep scan`,
      `rtk make check-code-file-lines` — all green.
- [ ] 5.3 `openspec validate remove-migrated-surface-underpaint --strict` passes.
- [ ] 5.4 Confirm issue #607's acceptance criterion "Migrated surfaces are not
      painted by a parallel legacy path" is now satisfied, and note in the PR
      that #614 can proceed on the ledger/ADR/source reconciliation for these
      rows.
