# Invariant 18 — Sync-projected paint geometry is clamped to the frame buffer at paint time

**Scope:** every rect a sync pass projects into a mounted component that the component later
paints (`set_transport_area` into `QueuePlaybackPanel`, the pattern case), and the painters that
index the frame buffer without clipping — `ratatui-widgets`' `Gauge` (`buf[(x, y)]` in
`gauge.rs`) and `tui-scrollbar`'s `ScrollBar` (`buf[(x, y)]` in `scrollbar/render.rs`). Landed
by the 2026-10-05 pinned-view-toggle crash fix.

## The invariant

1. A painter SHALL NOT index a frame cell outside the buffer it is painting into. Clipped
   writers (`Paragraph`, `Block`, `cell_mut` callers) uphold this on their own; unclipped
   writers (`Gauge`, `ScrollBar`) uphold it only if every rect handed to them is inside the
   buffer.
2. A rect projected during the sync pass can be stale by one frame against the buffer: the
   pinned width tween resizes the pty between the sync pass's size read and the draw's buffer
   build, so a collapse can retain a wider rect than the buffer holds. A component that paints
   from retained geometry SHALL clamp that geometry to the area its `view` is handed (the
   current draw-time placement) before painting — `QueuePlaybackPanel::view`'s
   `transport_area.intersection(area)`.
3. Hit geometry follows the paint: it is republished from the render pass every frame, so a
   clamped (or emptied) paint leaves hits that match what is on screen. A stale-wide hit rect
   left unpainted would be unreachable by pointer events in the narrower buffer, but the
   republication removes it anyway.

## Why it matters

Pressing `x` in a pinned panel toggles the two pinned views through `apply_layout_animated`
(`ANIM_DEFAULT_MS`): the pty resize lands over the tween, not inside the dispatch. The first
frame after the tween snaps runs `sync_mounted_surfaces` with the pre-snap `terminal_width`
(`sync_queue_playback_panel` projects the transport band from it), then `draw_frame` builds its
buffer from the snapped pty. The queue band's seek row rendered `Gauge` into the stale-wide
seek rect and panicked: `index outside of buffer: the area is Rect { x: 0, y: 0, width: 40,
height: 51 } but index is (102, 3)` — the stale seek row's first fill cell. The crash was
intermittent because it needs the snap to land between the sync pass and the draw of the same
iteration.

## How the code maintains it today

`QueuePlaybackPanel::view` clamps the retained transport rect to the `view` placement before
deriving the band rows, so every rect reaching the gauge is inside the buffer; when the stale
band no longer overlaps the placement the frame paints nothing and the hit geometry zeroes.
Every other unclipped-writer call site takes its rect from the current frame's compose
(`media_list`, `overview_box`, the sidebar scrollbars), not from retained state.

## Where it still fails

The clamp lives at one component's paint boundary, not in a type: a future component that
paints retained sync-projected geometry through an unclipped writer must repeat the clamp by
hand. A `Frame`-level paint guard (clipping every `render_widget` area against
`frame.area()`) would enforce it structurally, at the cost of an intersection on every widget
render.
