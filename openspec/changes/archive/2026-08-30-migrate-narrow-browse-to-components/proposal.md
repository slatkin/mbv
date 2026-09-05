## Why

Issue #625. The TuiRealm migration gated the legacy renderer off for each **wide** surface
as it landed, and never addressed the **narrow** breakpoint. Three regressions
are live in the app right now, all one root cause:

1. **Narrow Movies double-paints.** Legacy `render_list` and
   `BrowserComponent::view` both paint `layout.main.left_area`. Latent until
   `6cf469e1` (#618) removed the cursor mirror that kept the two in step; now
   the cursors diverge, giving doubled rows and a stale-cursor inline hero.
2. **Narrow TV navigation is dead.** No component is mounted —
   `tv_workspace_component_id` requires `is_wide_tv_active()` and
   `emby_browser_component_id` rejects `BrowserKind::TvShows`. Legacy browse
   key handling was deleted in `51bb3a16`, so keys reach the router, find no
   focused component, and fall through to nothing.
3. **Narrow grouped Music's painted cursor is frozen.**
   `MusicWorkspaceComponent` is mounted and owns the cursor, but
   `render_music_workspace_component` places it with
   `layout.main.wide_music_area`, empty at narrow — so keys move a cursor the
   legacy painter never reads.

Emby **podcast** libraries are worse: no component mounts at either width, so
narrow is dead like TV *and* wide is blank (`render_list` early-returns on the
podcast gate with nothing behind it).

`docs/architecture/interactive-surface-ledger.md:66,68,69` claims "narrow =
sole legacy renderer (D5)" for Movies, TV, and Music. That is false for Movies
and says nothing about who owns the narrow cursor.

This also unblocks `split-browse-state-interaction-fields`, whose task 4.4
stopped on exactly this: `library_list_render_ctx` cannot read a live cursor
from a component when two of the surfaces it serves have no component.

## What Changes

- Mount an owning component for every narrow browse surface: `BrowserComponent`
  gains `BrowserKind::TvShows` and podcast libraries below the wide TV
  breakpoint.
- Hoist `render_list`'s narrow composition — inline hero, letter and music pill
  rows, search box, count label, empty-state messages, grouped-album rows,
  poster prefetch — into `BrowserComponent` and `MusicWorkspaceComponent`, then
  delete `render_list` and `render_library`'s Emby paint branch.
- Land the R14 threading handed over from
  `split-browse-state-interaction-fields`: `library_list_render_ctx` takes
  cursor/scroll as parameters, sourced from the mounted component.
- Record the general invariant in the framework spec and give the surface
  ledger a per-breakpoint owner/painter column, so a surface with no owner at
  some width is a spec violation rather than a thing nobody notices.

## Non-goals

- **The Queue surface's identical defect (#623) is deferred.** `shell_run.rs:56`
  paints the legacy queue from the deliberately-stale `App::queue_cursor` and
  `:72` overlays `QueueComponent`, producing the same ghost row. The general
  rule below covers it and the ledger records it, but the fix is a different
  painter (`render_queue`) over different state, so it belongs to
  `remove-queue-legacy-underpaint`. It is likely small — `QueueComponent`
  already owns cursor, scroll, scope, rendering, and hit geometry — but the
  queue panel chrome (title row, status/playlist pills) is painted by legacy
  and must be accounted for before the legacy body paint is deleted.
- No mouse repair (D16). Hit geometry moving into the components is a
  consequence of the hoist, not a behaviour fix.
- No change to wide layouts, persistence, or the ctrl protocol.

## Capabilities

### Modified Capabilities
- `interactive-component-framework`: adds the general one-owner/one-painter
  invariant, per breakpoint, and the rule that a non-painting owner receives
  its render-derived geometry from the shell.

## Impact

- `src/app/components/browser.rs` (+ a new `browser_narrow.rs`),
  `music_workspace.rs`, `src/app/shell_browser.rs`,
  `shell_music_workspace.rs`, `shell_tv_workspace.rs`.
- `src/app/render/components/{list.rs,widgets.rs,detail.rs,detail_series_view.rs,album.rs,music.rs,list_context.rs}`,
  `src/app/render/screens/{pills.rs,detail_series.rs}`.
- `docs/architecture/interactive-surface-ledger.md`, `openspec/specs/`.

## Sequencing

Depends on `split-browse-state-interaction-fields` (phases 1–4). Blocks
`delete-browse-level-cursor-scroll`.
