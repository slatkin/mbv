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
and 110 layout assignments, so user approval widened it into the staged family
queue below. Commit `94002d25b75e0f34df200c3def57939c6cffd156` is preparatory
only: its helper seam exists, but it does not satisfy the paint-free geometry
contract and is not proof that this section is complete.

- [x] 2.1-prep Preserve the preparatory seam in `root.rs` from commit 94002d25;
      do not treat it as the completed geometry extraction.
- [ ] 2.1a Root/chrome family: make `compute_frame_layout` zero-area-safe before
      any mutation and move root/chrome-owned `AppLayout` initialization and
      publication into the paint-free pass. Production boundary:
      `src/app/render/screens/root.rs`, `src/app/render/components/chrome*.rs`,
      and directly required root/chrome tests. Verify focused root/chrome tests,
      `cargo check -p mbv`, and fmt.
- [ ] 2.1b Queue/pills family: move queue and pill geometry publication into the
      paint-free seam without changing paint. Boundary:
      `render/screens/queue.rs`, `render/screens/pills.rs`, queue/pill component
      modules, and focused tests. Depends on 2.1a. Verify queue/pill tests and
      check/fmt.
- [ ] 2.1c Lists/albums family: migrate list, grouped-list, album, and inline
      album geometry producers while retaining exact painter output. Boundary:
      `list*.rs`, `album*.rs`, and focused characterization tests. Depends on
      2.1b. Verify library/album tests and check/fmt.
- [ ] 2.1d Feeds/home family: migrate Feeds and Home geometry publication and
      preserve image handoff/loading behavior. Boundary: `feeds.rs`, `home*.rs`,
      corresponding shell seam tests, and focused characterization tests.
      Depends on 2.1c. Verify feeds/home tests and check/fmt.
- [ ] 2.1e Music family: migrate ordinary, wide, browser, and track geometry
      producers. Boundary: `music*.rs`, focused music tests, and required shell
      seam helpers. Depends on 2.1d. Verify music/group tests and check/fmt.
- [ ] 2.1f TV/widgets family: migrate TV-wide and shared widget geometry
      producers, preserving breakpoint behavior. Boundary: `tv_wide.rs`,
      `widgets.rs`, and focused TV/widget tests. Depends on 2.1e. Verify TV
      tests and check/fmt.
- [ ] 2.2 After families 2.1a–f, extract `paint_legacy_chrome` from the now
      paint-free geometry pass, preserving all legacy painting initially.
      Depends on 2.1f. Verify full nextest and fmt.
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
