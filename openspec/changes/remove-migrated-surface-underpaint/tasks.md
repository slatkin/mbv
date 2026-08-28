## 1. Scout handoff (D1 — blocking, read-only)

- [ ] 1.1 Write `openspec/handoffs/scout-remove-migrated-surface-underpaint.md`.
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

- [ ] 2.1 In `src/app/render/screens/root.rs`, extract
      `App::compute_frame_layout(&mut self, area: Rect) -> ()` (keeps the
      atomic `self.layout = layout` swap and the zero-area early return) from
      `App::render` — move the geometry statements, paint nothing. Verify:
      `rtk cargo check -p mbv`.
- [ ] 2.2 Extract `App::paint_legacy_chrome(&mut self, f: &mut Frame)` that
      paints exactly what `render_main` paints today MINUS nothing yet (this
      task is behaviour-preserving: it still paints every surface body).
      `App::render` becomes `compute_frame_layout(f.area()); paint_legacy_chrome(f);`.
      Verify: `rtk cargo nextest run -p mbv` full suite green (no visual
      change).
- [ ] 2.3 Add `Model::draw_frame(&mut self, f: &mut Frame)` in
      `src/app/shell_run.rs` (or a new `src/app/shell_draw.rs` if `shell_run.rs`
      nears the 800-line cap): sets `dim_backdrop_active`, calls
      `self.app.compute_frame_layout(f.area())`, `self.app.paint_legacy_chrome(f)`,
      the resize content pushes, then the existing component painters and
      `render_overlay_stack`. Verify: `rtk cargo check -p mbv`.
- [ ] 2.4 Route all three `terminal.draw` sites (`shell_run.rs:32`, `:77`,
      `:487`) through `|f| self.draw_frame(f)`. Verify: `rtk cargo nextest run
      -p mbv` green; add a startup test asserting the first full frame contains
      the component views (Home loading affordance visible), not a chrome-only
      frame.

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
