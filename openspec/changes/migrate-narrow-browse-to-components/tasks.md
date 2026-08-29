## 1. Surface audit against the invariant (D1)

- [ ] 1.1 For every interactive surface in
      `docs/architecture/interactive-surface-ledger.md`, record at each
      reachable breakpoint: the mount gate, the owning component (or none), and
      every painter that runs for its rect. Method: read the `*_component_id`
      gate, the `render_*_component` placement rect, and whether
      `compose_base_frame` paints the same rect. Verify: a table with one row
      per surface × breakpoint, each row classified none / **F1** (no owner) /
      **F2** (two painters) / ok.
- [ ] 1.2 Merge the parallel audit's findings into 1.1's table as they arrive.
      New rows extend task 3's checklist; they do not change D1–D7. Verify: each
      new finding is classified F1 or F2 and has a task 3 row.
- [ ] 1.3 Record deferrals with their owning change and the reason, so no
      instance is invisible: Queue (F2) →
      `remove-queue-legacy-underpaint`. Verify: every non-ok row from 1.1 is
      either in task 3 or has a named owner elsewhere.

Known at planning time (verified against `b34ee375`):

| Surface | Breakpoint | Family | Disposition |
|---|---|---|---|
| Emby TV browse | narrow | F1 | task 3 |
| Emby podcast browse | narrow **and** wide | F1 | task 3 |
| Emby generic/Movies/home-video browse | narrow | F2 | task 3 |
| Emby grouped Music browse | narrow | F2 (degenerate) | task 3 |
| Queue | all | F2 | deferred — `remove-queue-legacy-underpaint` |

## 2. Ownership for every surface (D4, D5, D6)

Ends with every surface owned; legacy still paints. This is the state
`delete-browse-level-cursor-scroll` depends on.

- [ ] 2.1 Characterization tests (pass pre-change, still pass after): entering a
      narrow TV library restores its saved series position; entering a narrow
      grouped Music library restores its saved album position.
      Regression tests (red now): narrow TV `j`/`k` moves the painted
      selection; narrow grouped Music `j`/`k` moves the painted selection;
      narrow Movies paints each row exactly once — assert on the `TestBackend`
      buffer through `Model::draw_frame`, not `compose_base_frame` alone, since
      the double paint only exists when both painters run. Verify: restore
      green, regression red, both stated.
- [ ] 2.2 Mount gates (D4): `emby_browser_component_id` (`shell_browser.rs:133`)
      accepts `BrowserKind::TvShows` and `is_podcast_library` when
      `!self.app.layout.main.is_wide_tv_active()`, keeping
      `tv_workspace_component_id`'s gate as the wide half so the two are
      mutually exclusive at every width. Verify: a narrow TV tab and a podcast
      tab each resolve a `Some(..)` browser id, `library_child_id` routes focus
      to it, and keys reach the component (shell-routing test shape in
      `shell_browser_tests.rs`).
- [ ] 2.3 Breakpoint hand-off (D5): on an active-destination pointer flip
      between `BrowserComponent` and `TvWorkspaceComponent`, persist the
      outgoing live cursor to the resting position and set the incoming
      component's one-shot re-anchor, reusing `persist_emby_browser_scroll` and
      the `music_workspace_reanchor` shapes. Verify: resize across the wide TV
      breakpoint and back keeps the selected series.
- [ ] 2.4 Narrow Music placement (D6, first half):
      `render_music_workspace_component` (`shell_music_workspace.rs:157`) falls
      back to `layout.main.left_area` when `wide_music_area` is empty. Verify:
      the component's `view` is reached at narrow (it still paints nothing until
      3.x gives it a narrow branch — assert the call, not the pixels).
- [ ] 2.5 R14 threading (handed over from
      `split-browse-state-interaction-fields` 4.4). Give
      `library_list_render_ctx` explicit cursor/scroll parameters, then:
      - Group A (`shell_browser.rs:204`, `shell_tv_workspace.rs:109`,
        `shell_music_workspace.rs:108`) pass the mounted component's
        `cursor()`/`scroll()` / `album_cursor()`.
      - Group B: `Model::draw_frame` resolves the active library's component
        cursor/scroll once per frame and passes it to `compose_base_frame`,
        which threads it to `render_library` (`widgets.rs:507`) → `render_list`
        (`list.rs:145`), `wide_music_render_ctx` (`music_wide.rs:140`),
        `wide_tv_render_ctx` (`tv_wide.rs:100`).
      - `detail.rs:106`/`:138`/`:358` need no direct change — they consume the
        ctx the caller now builds.
      No `App` field holds the threaded value at any point. Verify:
      `rtk cargo check -p mbv` per file, commit in reviewable units; 2.1's TV
      and Music regression tests go green.
- [ ] 2.6 Gate: every surface from task 1 has exactly one owner at every
      breakpoint. Verify: `rtk cargo nextest run -p mbv`, `rtk cargo clippy
      --workspace --all-targets`, `rtk ast-grep scan`, plus a stated
      surface → owner table. **Narrow Movies' single-paint test is still red
      here and that is correct** — 2.5 makes both painters agree again (the
      latent pre-`6cf469e1` state), it does not remove the second painter.
      Task 3 does.

## 3. One painter per surface (D2, D3)

One row per surface from task 1, each following the same template. Added audit
findings become added rows; the template does not change.

**Per-surface template.** For surface S at breakpoint B:
- a. Capture a `TestBackend` snapshot of S at B before any change.
- b. Move S's composition into its owning component, reusing the existing free
     ctx functions (D3) and routing image work through the
     `HomeImagePaint` → `App::paint_home_image` seam. No effect call may end up
     inside a component.
- c. Delete the legacy branch that painted S at B.
- d. Verify: the snapshot from (a) matches; the surface's single-paint test is
     green; `rtk cargo check -p mbv`, `rtk ast-grep scan`.

- [ ] 3.1 Prerequisite: split `compact_banner_layout_with_overview`
      (`detail.rs:166`) into a pure sizing function plus the fetch it performs
      as a side effect, with `images_enabled`, the
      `right_panel_image_renders_allowed` nav-idle gate, and image-cached-ness
      as parameters. Sizing must be callable before the list flows, since
      `inline_hero_rows` determines row layout. Verify: identical
      `CompactBannerLayout` output for the same inputs (test), and the fetch
      still fires from the shell side.
- [ ] 3.2 Prerequisite: free ctx-function variants for the remaining `impl App`
      painters the narrow path reaches — `render_grouped_album_rows`
      (`album.rs:33`), `render_series_inline_detail`
      (`detail_series_view.rs:40`), `series_inline_detail_rows`
      (`screens/detail_series.rs:24`) — mirroring the existing
      `render_music_group_pills_row_with_ctx` (`music.rs:43`). Ctx fields per
      D3 kind 2; `MusicWideRenderCtx` already carries the grouped-album set.
      Verify: `rtk cargo check -p mbv`; the `impl App` originals are gone, not
      left alongside.
- [ ] 3.3 Surface: Emby generic / Movies / home video, narrow. Template above.
      The composer lands in a new `src/app/components/browser_narrow.rs` (D3
      file-size note). Closes regression 1.
- [ ] 3.4 Surface: Emby TV, narrow. Template above, same composer as 3.3 —
      series inline hero, season grid, letter pills. Record any TV-specific
      chord that is still dead as a Change D finding; do not fix it here.
- [ ] 3.5 Surface: Emby podcast, narrow **and** wide. Template above. Wide is
      currently blank, so (a)'s snapshot is the empty baseline and the
      acceptance is "paints the browse body", not "matches before".
- [ ] 3.6 Surface: Emby grouped Music, narrow (D6 second half).
      `MusicWorkspaceComponent::view` gains a narrow branch: grouped-album rows
      plus the Model A hero, **not** the wide right-rail track table.
- [ ] 3.7 Relocate `render_list`'s poster-prefetch window
      (`fetch_list_card_image_when_idle`) to the shell beside
      `paint_home_image`, keyed off the component's selection. Verify: prefetch
      still fires as the cursor moves (test asserting the fetch call, since this
      is a behaviour-preserving relocation, not a deletion).
- [ ] 3.8 Delete `render_list` (`list.rs`) and reduce `render_library`'s
      `EmbyLibrary` arm to geometry reservation, matching the Home / Feeds / ABS
      arms. Verify: `rtk cargo clippy --workspace --all-targets` reports no dead
      code; `App` has no browse painter left.

## 4. Record the invariant

- [ ] 4.1 Add the one-owner/one-painter scenarios to
      `openspec/specs/interactive-component-framework/spec.md` via this change's
      delta. Verify: the delta applies cleanly.
- [ ] 4.2 Give `docs/architecture/interactive-surface-ledger.md` a
      per-breakpoint **owner** and **painter** column, populated from task 1's
      final table — including the deferred Queue row. Correct lines 66/68/69,
      whose "narrow = sole legacy renderer (D5)" claim is false for Movies and
      silent on ownership. Verify: every ledger row states both, at every
      breakpoint.

## 5. Close out

- [ ] 5.1 Split any file over 800 lines (D3's table anticipates
      `browser.rs`). Verify: `rtk make check-code-file-lines`.
- [ ] 5.2 Full gate: `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`,
      `rtk cargo clippy --workspace --all-targets`, `rtk ast-grep scan`,
      `rtk cargo fmt`, `rtk make check-code-file-lines`.
- [ ] 5.3 Confirm all three regressions are closed by manual check at a narrow
      terminal: Movies rows paint once, TV navigates, Music's painted selection
      follows the cursor. Verify: stated per regression.
