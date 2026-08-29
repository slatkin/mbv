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
- [ ] 2.3 Add `Model::draw_frame` in `shell_run.rs` (or `shell_draw.rs` if
      needed), preserving resize pushes and component paint order. Depends on
      2.2. Verify check and fmt.
- [ ] 2.4 Route all three terminal draws through `draw_frame`, then add the
      startup-frame characterization. Depends on 2.3. Verify focused startup,
      full nextest, and fmt.

## 3. Per-surface suppression (D4 — one unit per surface × breakpoint, scout order)

> Each sub-task below is a template; the scout handoff (1.1) fixes the exact
> list and order. For every unit: (i) make `render_main`'s dispatch arm for
> that surface+breakpoint return before painting the body, still computing
> geometry; (ii) add a debug assertion / test counter that fires if the legacy
> arm paints while the component is the active target; (iii) run that surface's
> render characterization tests + `rtk cargo nextest run -p mbv <surface>`.

- [ ] 3.1 Home body — suppress legacy paint when `HomeComponent` is active.
- [ ] 3.2 Emby generic/Movies/HomeVideo browser body — suppress when
      `BrowserComponent` is active; confirm the #611 browser change already
      removed the `wide_movies` / `movies_wide_right_area` residue, else fold
      D18 step 2 in here.
- [ ] 3.3 Wide TV workspace body — suppress when `TvWorkspaceComponent` is
      active and wide.
- [ ] 3.4 Wide Music workspace body + album-track — suppress when
      `MusicWorkspaceComponent` is active and wide.
- [ ] 3.5 ABS book body — suppress when `AudiobookshelfBookComponent` is active.
- [ ] 3.6 ABS podcast body — suppress when `AudiobookshelfPodcastComponent` is
      active.
- [ ] 3.7 Feeds body — suppress when `FeedsComponent` is active.
- [ ] 3.8 Inline search body — suppress when an `InlineSearchComponent` is
      active.
- [ ] 3.9 Player chrome — suppress the legacy player-chrome paint if the scout
      confirms `PlaybackComponent` is the sole painter; otherwise move the
      chrome paint into `paint_legacy_chrome` and record it as sole-legacy.
- [ ] 3.10 After each unit: `rtk cargo nextest run -p mbv` full suite,
      `rtk ast-grep scan`, `rtk cargo clippy --workspace --all-targets` green.

## 4. Dead renderer deletion (D6)

- [ ] 4.1 For each suppressed body whose legacy renderer now runs only to
      publish an `AppLayout` field: move that derivation into
      `compute_frame_layout` (or the owning component per D18 step 2), then
      delete the renderer. Verify: `rtk cargo check -p mbv` — no remaining
      caller; `rtk ast-grep scan` clean.
- [ ] 4.2 Confirm `App::render` (the old entry point) has no remaining callers
      and delete it, or reduce it to the two-call shim if a test still uses it
      — prefer deletion, update tests to `compute_frame_layout` +
      `paint_legacy_chrome`. Verify: `rtk cargo nextest run -p mbv`.
- [ ] 4.3 `rtk make check-code-file-lines` — split any file the extraction
      pushed over 800 lines in the same change.

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
