# Remove the Queue's legacy body underpaint

## Why

Issue #629. The Queue is the last surface where a legacy painter and its
mounted component both paint the same rows every frame, and #623 records the
symptom: arrowing through the queue leaves the previously selected row
highlighted behind the new one.

`App::compose_base_frame` runs `render_queue` (`render/screens/root.rs:506`)
before any component; `Model::draw_frame` then overlays `QueueComponent`
(`shell_run.rs:71`). There is **no suppression gate on the legacy queue paint
at all** — unlike the wide browse surfaces, which each got one as they
migrated.

The two passes disagree about which row is selected, and for a reason worth
naming: `split-queue-cursor-ownership` moved the selection cursor into
`QueueComponent`, leaving `App::player_tab.queue_cursor` as the **playback-follow
index**, written by player events (`player_event.rs:271/319`). Legacy
`render_queue` still derives its viewport and selection marker from that field
(`screens/queue.rs:293`). So the legacy pass paints the *playback* position
styled as a *selection*, while the component paints the real one — with
independently-clamped scroll offsets, so legacy rows show through around the
component's.

The ghost row is therefore not a cursor-sync bug to be patched. It is the last
unremoved underpaint, and the fix is to delete the duplicate painter.

## What Changes

- **The legacy queue body paint is deleted** — the `render_queue` call at
  `render/screens/root.rs:506` and `render_queue` itself
  (`render/screens/queue.rs:277-566`), together with its test-only helpers
  `render_queue_rows` / `render_queue_cursor_row` and the three tests that
  exercise only the deleted painter. `QueueComponent::view` →
  `render_queue_content` becomes the sole painter of queue rows, the selection
  marker, the scrollbar, and the empty state.

- **Two load-bearing geometry publications are re-homed**, because the deleted
  painter was their publication site:
  - `layout.main.queue_area` — consumed by component placement
    (`shell_queue.rs:36/47/60/73`), page-size (`actions.rs:127`), and the
    context-menu anchor (`shell_overlays_menus.rs:90/112`). Published directly
    from `render_main`, which already has the `Rect` in scope.
  - `layout.main.queue_selected_item_rect` — consumed by
    `ContextMenuAnchor::SelectedItem(PanelFocus::Queue)`
    (`shell_overlays_menus.rs:90`), a **keyboard**-triggered path, so it cannot
    be dropped under D16. Published by the component via a new
    `selected_row_rect()` accessor derived from its own `geometry.rows` and
    `cursor`, read back in `render_queue_component` — the pattern
    `render_music_workspace_component` already uses
    (`shell_music_workspace.rs:175-182`).

- **`layout.main.queue_row_map` is deleted** — write-only, no reader.
  `build_queue_rows` and the `QueueRow` enum (`ui_util.rs:236-327`) go with it;
  the deletion orphans them.

- **The panel chrome stays legacy.** `render_queue_panel_frame`, the title/status
  pills (`render_queue_title`), and the bottom playlist/autosave status row
  (`root.rs:508-539`) are not painted by `QueueComponent` and are not touched.
  Only the body is duplicated; only the body is deleted.

- **The ledger records the result** — `interactive-surface-ledger.md:63` gains
  the "no legacy underpaint" wording already used for the Playback (row 62) and
  Feeds (row 72) rows, plus a conformance test asserting that
  `compose_base_frame` reserves `queue_area` and paints no slot rows.

## Non-goals

- **Hoisting the queue title chrome is deferred.** `QueueComponent` already has
  `render_queue_title_content` (`render/components/queue.rs:125`), but it is
  gated off by a circular dependency: `shell_queue.rs:35/63` only pass a
  `title_area` when `layout.main.queue_scope_local_area.height > 0`, and that
  field is set *only* by legacy `render_queue_title`. The component's title
  renders only if the legacy title already rendered. Breaking that cycle means
  computing the title area shell-side and is its own change — the same "chrome
  trap" #625 hit, deliberately not resolved here.
- **Mouse behaviour** — accepted-broken per D16, unchanged.
- **The playback-follow index** — `App::player_tab.queue_cursor` keeps its
  current meaning and its player-event writers. This change removes the last
  reader that misinterpreted it as a selection, nothing more.

## Dependencies

**None.** Independent of #625, #626, and #614: queue geometry is published from
`render_main` directly rather than through `render_library` / `render_list` /
`library_list_render_ctx`, there is no `BrowseLevel` or `nav_stack` cursor
involved, and `QueueComponent` already owns cursor, scroll, and scope (unlike
the narrow browse surfaces #625 must first give an owner to). The only
contention with #625 is textual — both edit `render_main`, about thirty lines
apart. Land in any order.
