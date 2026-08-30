## 1. Surface audit against the invariant (D1)

- [x] 1.1 For every interactive surface in
      `docs/architecture/interactive-surface-ledger.md`, record at each
      reachable breakpoint: the mount gate, the owning component (or none), and
      every painter that runs for its rect. Method: read the `*_component_id`
      gate, the `render_*_component` placement rect, and whether
      `compose_base_frame` paints the same rect. Verify: a table with one row
      per surface × breakpoint, each row classified none / **F1** (no owner) /
      **F2** (two painters) / ok.
- [x] 1.2 Merge the parallel audit's findings into 1.1's table as they arrive.
      New rows extend task 3's checklist; they do not change D1–D7. Verify: each
      new finding is classified F1 or F2 and has a task 3 row.
- [x] 1.3 Record deferrals with their owning change and the reason, so no
      instance is invisible: Queue (F2) →
      `remove-queue-legacy-underpaint`. Verify: every non-ok row from 1.1 is
      either in task 3 or has a named owner elsewhere.

Known at planning time (verified against `b34ee375`):

| Surface | Breakpoint | Family | Disposition |
|---|---|---|---|
| Emby TV browse | narrow | F1 | task 3 |
| Emby podcast browse | narrow **and** wide | F1 | task 3 |
| Emby generic/Movies/home-video browse | narrow | F2 | task 3 |
| Emby grouped Music browse | narrow | **F1** — mounted but never focused (`shell_library.rs:64-68`) *and* never painted (component early-returns on empty `wide_music_area`, so legacy `render_list` is sole painter — no F2 at HEAD) | task 3 |
| Feed / home-video group picker | all | F1 — nothing mounts (`shell_browser.rs:130`), all nav dead | task 3 |
| Queue | all | F2 | deferred — #629 `remove-queue-legacy-underpaint` |

### Audit results (task 1 — read-only sweep, verified against `730220fa`)

Full surface × breakpoint table lives in the campaign record. All `ok` /
overlay / modal surfaces confirmed single-owner single-painter. Non-ok rows and
their disposition:

| Surface × breakpoint | Class | Disposition |
|---|---|---|
| Emby generic/Movies/home-video browse, narrow | F2 | task 3.3 |
| Emby **generic-collection** browse, **wide** (`is_wide_movies_library` false, `movies_wide_area` empty) | F2 | same root as narrow; closed by the `render_list` deletion in **task 3.8**. Scope note added to 3.3. |
| Emby TV browse, narrow | F1 | task 3.4 (dead TV-specific chords → Change D finding, per 3.4) |
| Emby podcast browse, narrow + wide | F1 | task 3.5 |
| Emby grouped Music browse, narrow | F1 | tasks 2.2 / 2.4 / 2.5 (focus + placement + ctx) then 3.6 (narrow paint branch) |
| Emby inline album-track interaction, narrow | none (by design) | D6: narrow deliberately has no right-rail track table; component's explicit unfocused-narrow mode is correct, not a defect. No row. |
| Feed / home-video group picker, all widths | F1 | **Same surface as Emby podcast browse** — the predicate `is_feed_home_video_group_view` is true for every podcast library *and* for configured home-video feed-view libraries; same rect, same `feed_home_video.video_cursor` state, same legacy painter family. Owner = `BrowserComponent` (D4 precedent). Folded into tasks **2.2** (mount/focus, widened predicate + `BrowserCycleGroup`) and **3.5** (paint the group pill bar + rows, delete `render_feed_home_video_group_view`). Maintainer decision 2026-08-30: in scope, no standalone rows. |
| Queue, all widths | F2 | deferred — #629 `remove-queue-legacy-underpaint` |
| `PanelMode::QueueOnly` legacy player panels | — | deliberate D5 split (component cannot paint an empty `player_area`); not F1/F2. |

## 2. Ownership for every surface (D4, D5, D6)

Ends with every surface owned; legacy still paints. This is the state
`delete-browse-level-cursor-scroll` depends on.

- [x] 2.1 Characterization tests (pass pre-change, still pass after): entering a
      narrow TV library restores its saved series position; entering a narrow
      grouped Music library restores its saved album position.
      Regression tests (red now): narrow TV `j`/`k` moves the painted
      selection; narrow grouped Music `j`/`k` moves the painted selection;
      narrow Movies paints each row exactly once — assert on the `TestBackend`
      buffer through `Model::draw_frame`, not `compose_base_frame` alone, since
      the double paint only exists when both painters run. Verify: restore
      green, regression red, both stated.
- [x] 2.2 Mount gates (D4): `emby_browser_component_id` (`shell_browser.rs:135`)
      accepts `BrowserKind::TvShows` when `!self.app.layout.main.is_wide_tv_active()`
      (keeping `tv_workspace_component_id`'s gate as the wide half so the two are
      mutually exclusive at every width), and accepts every
      `is_feed_home_video_group_view(index)` library **at every width** (this
      predicate covers all Emby podcast libraries *and* configured home-video
      feed-view libraries — one surface, `feed_home_video.video_cursor` state;
      audit-results table). Drop the `is_podcast_library(index) ||
      is_feed_home_video_group_view(index)` early `return None` from both
      `emby_browser_component_id` (`shell_browser.rs:135`) and the matching focus
      gate in `emby_library_child_id` (`shell_library.rs:57`). These libraries are
      already `BrowserKind::Generic`/`HomeVideos`, both in the accept-list.
      Also add `ShellRequest::BrowserCycleGroup { delta }` (mirror
      `BrowserCycleLetterPill`: `browser.rs` chord → `msg.rs` →
      `shell_messages.rs` browser-request group → `shell_browser.rs` calls the
      existing-but-currently-dead `App::switch_feed_folder_group`). Content
      projection for these libraries feeds the component
      `feed_home_video_selected_items` + `video_cursor` + `video_scroll` + group
      labels, and the component's cursor projects back through
      `apply_lib_cursor_index`'s existing `is_feed_home_video_group_view` branch.
      Verify: a narrow TV tab, an Emby podcast tab, and a home-video feed-view
      tab each resolve a `Some(..)` browser id, `library_child_id` routes focus
      to it, keys reach the component, `[`/`]` cycles the group (shell-routing
      test shape in `shell_browser_tests.rs`); `switch_feed_folder_group`'s
      dead-code warning is gone.
- [x] 2.3 Breakpoint hand-off (D5): on an active-destination pointer flip
      between `BrowserComponent` and `TvWorkspaceComponent`, persist the
      outgoing live cursor to the resting position and set the incoming
      component's one-shot re-anchor, reusing `persist_emby_browser_scroll` and
      the `music_workspace_reanchor` shapes. Verify: resize across the wide TV
      breakpoint and back keeps the selected series.
- [x] 2.4 Narrow Music placement (D6, first half):
      `render_music_workspace_component` (`shell_music_workspace.rs:157`) falls
      back to `layout.main.left_area` when `wide_music_area` is empty. Verify:
      the component's `view` is reached at narrow (it still paints nothing until
      3.x gives it a narrow branch — assert the call, not the pixels).
- [x] 2.5 R14 threading (handed over from
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
      Note: the feed/home-video group-picker surface (folded into 2.2) does
      **not** flow through `library_list_render_ctx` — every cursor path branches
      on `is_feed_home_video_group_view` first and reads/writes
      `feed_home_video.video_cursor`/`video_scroll` directly. Its
      component-cursor→state threading is the `apply_lib_cursor_index` branch,
      handled in 2.2, not here.
- [x] 2.6 Gate: every surface from task 1 has exactly one owner at every
      breakpoint. Verify: `rtk cargo nextest run -p mbv`, `rtk cargo clippy
      --workspace --all-targets`, `rtk ast-grep scan`, plus a stated
      surface → owner table. **Narrow Movies' single-paint test is still red
      here and that is correct** — 2.5 makes both painters agree again (the
      latent pre-`6cf469e1` state), it does not remove the second painter.
      Task 3 does.

### Change D findings (keyboard-routing gaps — recorded, not fixed here)

Per design Risks / D coupling. These surface during task 2/3 and belong to the
keyboard-routing family, not this change:

- **Narrow TV Enter on a Series** now emits `BrowserActivate` → `select_item`
  (drill into the series folder) instead of the legacy
  `open_series_selection_modal`. Matches D4's "activation `BrowserComponent`
  already implements" and task 3.4's drill-in direction. The narrow arm of
  `activate_selected_series` / `activate_selected_series_item` becomes
  unreachable for TV. (Unit C, `56e5cfb0`.)
- **Narrow TV season/episode chords** have no `BrowserComponent` translation and
  stay dead under the router (ADR 0023). (Unit C.)

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

- [x] 3.1 Prerequisite: split `compact_banner_layout_with_overview`
      (`detail.rs:166`) into a pure sizing function plus the fetch it performs
      as a side effect, with `images_enabled`, the
      `right_panel_image_renders_allowed` nav-idle gate, and image-cached-ness
      as parameters. Sizing must be callable before the list flows, since
      `inline_hero_rows` determines row layout. Verify: identical
      `CompactBannerLayout` output for the same inputs (test), and the fetch
      still fires from the shell side.
- [x] 3.2a Prerequisite — series inline detail: make
      `series_inline_detail_rows` and `render_series_inline_detail` explicit
      context functions. Shell-side series detail/image/cache effects become
      typed outputs consumed by the shell. Verify: targeted series-detail tests
      execute; the component-callable path has no `App` parameter or access.
- [x] 3.2b Prerequisite — grouped Music image seam: add the narrow typed
      `MusicImagePaint` → shell executor seam, mirroring `HomeImagePaint`.
      Album-art helpers consume only context/cache state and emit an instruction;
      they do not fetch or access cache. Verify: a targeted shell-execution test.
- [x] 3.2c Prerequisite — grouped Music planning and inline rows: make the
      grouped-album display plan pure from an explicit context and make inline
      row composition consume that plan/context. Reuse existing
      `MusicWideRenderCtx`, `LibraryListRenderCtx`, and cursor context data.
      Verify: targeted pure-plan and inline-row tests execute with no App-backed
      component-callable path.
- [x] 3.2d Prerequisite — grouped Music detail composition: make grouped album
      rows, hero/detail/art/action-hint composition explicit-context functions
      whose image work is typed `MusicImagePaint` output for the shell. Preserve
      existing hero/detail/art behavior. Verify: targeted grouped-Music hero,
      art, and row tests execute; no component-callable grouped-album renderer
      accepts or reaches `App`.
- [x] 3.2e Gate: the three named legacy painters and every helper reachable by
      their component-callable paths are context/pure functions; no App-backed
      original or parallel component-callable path remains. Verify:
      `rtk cargo check -p mbv --all-targets`, targeted series and grouped-Music
      tests, `rtk ast-grep scan`, and a stated dependency check.
- [x] 3.3 Surface: Emby generic / Movies / home video, narrow. Template above.
      The composer lands in a new `src/app/components/browser_narrow.rs` (D3
      file-size note). Closes regression 1. Scope note: a generic-collection
      Emby library also double-paints at **wide** (`is_wide_movies_library`
      false → `render_list` not suppressed; `movies_wide_area` empty →
      `BrowserComponent` paints `left_area` too). No distinct wide layout for
      generic; the `render_list` deletion in 3.8 closes the wide case with no
      extra composer work. Verify the wide generic snapshot in 3.8.
- [x] 3.4 Surface: Emby TV, narrow. Template above, same composer as 3.3 —
      series inline hero, season grid, letter pills. Record any TV-specific
      chord that is still dead as a Change D finding; do not fix it here.
- [x] 3.5 Surface: Emby podcast **and feed/home-video group picker**, narrow
      **and** wide. One surface (`is_feed_home_video_group_view`; audit-results
      table). Template above. Wide podcast is currently blank, so (a)'s snapshot
      is the empty baseline and the acceptance is "paints the browse body", not
      "matches before"; the home-video feed-view case has a populated legacy
      baseline (`render_feed_home_video_group_view`) that must match after.
      Depends on 3.1 (this surface calls `compact_banner_layout_with_overview`
      at `home_feed.rs:123` and via `render_selected_home_video_detail`).
      Composer gains a group pill bar above the home-video rows (reuse
      `render_generic_movies_home_video_rows_with_ctx`; add a `_with_ctx`
      group-pill row mirroring `render_music_group_pills_row_with_ctx`). Delete
      `render_feed_home_video_group_view` (`home_feed.rs`) and its
      `is_feed_home_video_group_view` dispatch branch in `render_library`
      (`widgets.rs:554-559`) — converges with 3.8. `ensure_lib_loaded_for`
      (an effect, `home_feed.rs:21`) moves to the shell side.
- [x] 3.6 Surface: Emby grouped Music, narrow (D6 second half).
      `MusicWorkspaceComponent::view` gains a narrow branch: grouped-album rows
      plus the Model A hero, **not** the wide right-rail track table.
- [x] 3.7 Relocate `render_list`'s poster-prefetch window
      (`fetch_list_card_image_when_idle`) to the shell beside
      `paint_home_image`, keyed off the component's selection. Verify: prefetch
      still fires as the cursor moves (test asserting the fetch call, since this
      is a behaviour-preserving relocation, not a deletion).
- [x] 3.8 Delete `render_list` (`list.rs`) and reduce `render_library`'s
      `EmbyLibrary` arm to geometry reservation, matching the Home / Feeds / ABS
      arms. Verify: `rtk cargo clippy --workspace --all-targets` reports no dead
      code; `App` has no browse painter left.
      Wide-podcast follow-ups from 3.5b (pre-existing, deferred here): (a)
      `widgets.rs:580` still ORs `is_podcast_library` into the
      `wide_tv_render_ctx().publish_geometry` guard, so `is_wide_tv_active()`
      reports `true` for wide podcast and `input_browse_dispatch.rs:33` /
      `lib_cursor_actions.rs:77` / `shell_overlays_menus.rs:100` treat it as
      wide TV (1-col stride, Series-Enter interception, context-menu anchor) —
      drop that disjunct, mirroring the `list.rs` change 3.5b made; (b)
      re-purpose `wide_emby_podcast_uses_the_series_workspace_and_right_rail`
      (`tests_non_music.rs:134`) to expect zeroed `tv_wide_*` once (a) lands;
      (c) the shared narrow composer run wide leaves an empty placeholder hero
      frame with the first rows behind the hero reservation (same as wide
      generic) — the 3.8 wide-generic cleanup covers this.

## 4. Record the invariant

- [x] 4.1 Add the one-owner/one-painter scenarios to
      `openspec/specs/interactive-component-framework/spec.md` via this change's
      delta. Verify: the delta applies cleanly.
- [x] 4.2 Give `docs/architecture/interactive-surface-ledger.md` a
      per-breakpoint **owner** and **painter** column, populated from task 1's
      final table — including the deferred Queue row. Correct lines 66/68/69,
      whose "narrow = sole legacy renderer (D5)" claim is false for Movies and
      silent on ownership. Verify: every ledger row states both, at every
      breakpoint.

## 5. Close out

- [x] 5.1 Split any file over 800 lines (D3's table anticipates
      `browser.rs`). Verify: `rtk make check-code-file-lines`.
- [x] 5.2 Full gate: `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`,
      `rtk cargo clippy --workspace --all-targets`, `rtk ast-grep scan`,
      `rtk cargo fmt`, `rtk make check-code-file-lines`.
- [ ] 5.3 Confirm all three regressions are closed by manual check at a narrow
      terminal: Movies rows paint once, TV navigates, Music's painted selection
      follows the cursor. Verify: stated per regression.
