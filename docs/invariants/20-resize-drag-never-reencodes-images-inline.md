# Invariant 20 — A resize drag never re-encodes images on the sync thread

**Scope:** every image compose/encode whose input changes while a pointer drag
moves panel geometry — the queue card's title-overlay variant
(`src/app/state/projection/card.rs::title_site_skip_reason`), the hero's
cover-fit protocol (`src/app/infra/image_fetch/protocol.rs::ensure_hero_cover_protocol`),
and any future image path keyed on panel geometry. The drag-owning components
are `QueueBoundaryComponent::is_resizing` and
`LibraryPanel::is_resizing`; the shell resolves them into one hold flag in
`src/app/shell/library_panel.rs::refresh_resize_drag_hold`, which also owns
the settle window (`RESIZE_DRAG_SETTLE`, stamped into
`App::resize_drag_activity`).

## The invariant

1. While a resize drag is live, no sync pass may compose or encode an image
   whose size or composition keys on the moving geometry. The hero reports
   not ready for a changed box, so its painter shows the loading
   placeholder — never a stretched stale crop. The queue card paints plain
   base art and carries its title site.
2. A drag's final geometry re-encodes exactly once, and only after the drag
   has been quiet for the settle window — the hold outlives the `DragEnd`
   by `RESIZE_DRAG_SETTLE`, so a pause-and-resume drag does not re-encode at
   every hesitation. A box that matches the kept encoding still paints
   during the hold.
3. The live bit is resolved from the component that owns the gesture, never
   mirrored into `App` state; only the settle timing lives on `App`. Every
   per-tick image projection takes the resolved hold value as an explicit
   input, so a new call site cannot silently omit the gate.
4. Work that must happen during the drag (keeping the picture roughly in
   step with the box) belongs to the off-thread resize worker
   (`crates/mbv-images/src/resize.rs`), never inline in the sync pass.

## Why it matters

This regression has recurred with every image feature that composes per
geometry. A landscape hero's cover-fit re-encode is a Lanczos3 resize of the
decoded source plus the logo composite — measured at 5–12 ms in release on
the sync thread. A drag delivers one mouse event per sync pass, so the drag
ran at a fraction of event rate and the panel crawl was visible whenever an
overlaid (landscape) hero was up; portrait heroes took the plain-protocol
path, skipped the re-encode, and dragged fine — which is how the user
isolated it (2026-10-09). The queue card hit the same wall first
(2026-10-03): every fitted width composed a fresh Lanczos3 title-overlay
variant on the tick thread.

## How the code maintains it today

- The queue card: `sync_queue` passes the same `refresh_resize_drag_hold`
  value to `title_site_skip_reason`, whose `ResizeDragHold` transient gate
  stops the variant compose while the hold is on; the site carries over so
  the header does not resurrect. Owned by
  `title_site_tests::column_resize_drag_builds_no_overlay_variant_until_it_ends`.
- The hero: `ensure_hero_cover_protocol` takes the shell-resolved `resizing`
  hold and reports not ready for a changed box while it is on; the settled
  box rebuilds once after the hold lifts. Owned by
  `protocol_tests::resize_drag_holds_the_placeholder_and_reencodes_once_after_it_settles`.
- The settle window: `refresh_resize_drag_hold` keeps the hold on through
  `RESIZE_DRAG_SETTLE` after the last drag event. Owned by
  `tick_integration::mouse::panels::image_hold_outlives_the_drag_by_the_settle_window`.
- The off-thread path: the card and hero painters paint through
  `ThreadProtocol`, whose `size_for` sends resize jobs to the resize worker
  and falls back to the placeholder/stale paint until the response arrives.

## Where it still fails

- During a drag the queue card's `ThreadProtocol` still posts one worker
  resize job per event (off-thread; UI stays responsive, the worker churns
  and the card shows the placeholder between responses).
- The pinned panel's width tween changes panel geometry per frame with no
  shell-visible tween-in-progress signal, so the hero re-encodes inline per
  tween frame (invariant 17 documents the tween's remaining costs). A
  pinwin-side tween-progress signal would let `refresh_resize_drag_hold`
  cover it.
