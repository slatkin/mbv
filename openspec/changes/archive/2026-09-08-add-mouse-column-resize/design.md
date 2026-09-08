## Context

See `proposal.md` for motivation. The queue column width is shell-owned, persisted, and normalized to the existing 40-column minimum and terminal-relative maximum. Keyboard policy sends five-column resize intents through the Queue Interactive Component. Root chrome computes the Queue and Library panel geometry from the exact width before each frame.

Mouse delivery is component-based. A painted Interactive Component owns its hit geometry and private gesture state, then emits semantic messages carrying resolved values. The shared recognizer already reports press-derived drag motion and drag end. The Queue Interactive Component currently owns only the queue title and list geometry; its area is inset from the outer column edge, and its drag gesture already means queue-slot movement.

The visible boundary has no gutter: it is the full-height trailing column of the root chrome's left Queue column (`left_area.right() - 1`) next to the Library panel. It is not the edge of the inset queue title/list frame. The change must not add visible chrome or overlap either destination's interactive area.

## Goals / Non-Goals

**Goals:**

- Give the existing one-column Queue panel edge one unambiguous interaction owner.
- Recompute and paint the frame at each distinct clamped drag position.
- Preserve the final width once, when a completed drag ends.
- Cancel stale gesture state whenever the boundary is not mouse-eligible.

**Non-Goals:**

- Add a divider, gutter, hover style, pointer cursor, or wider invisible grab target.
- Change keyboard resize steps or existing width bounds.
- Resize in Mini, queue-only, or library-only Panel modes.
- Combine queue-slot dragging with column resizing.

## Decisions

### D1: A dedicated boundary Interactive Component owns the existing edge

Add a small Interactive Component whose frame area is exactly the full-height trailing column of the root chrome's Queue-side `left_area` while the Panel mode is `both`. It owns a private `MouseGestureState`, recognizes only a left press that begins inside that area, and keeps the resize gesture armed after the pointer leaves the original column.

Root chrome paints `left_area` minus that trailing column. The component is the sole painter of the reserved column and uses the same focus-dependent semantic surface role that root chrome previously painted there, so the rendered result remains visually unchanged. QueueComponent keeps its existing inset title/content geometry, does not overlap the boundary, and does not recognize resize gestures. The boundary component is mounted for stable framework identity and is mouse-eligible only when painted with no exclusive overlay or popup.

This is preferable to extending `QueueComponent`: Queue owns inset title/list geometry and already assigns drag to queue-slot movement, while the outer boundary crosses root chrome and card space. It is also preferable to the root observer or shell hit-testing, which would introduce shell-global gesture state and raw-coordinate resolution outside the mouse contract.

### D2: Drag messages carry an exact resolved width

For a pointer at terminal column `x`, the boundary component resolves the requested queue width as the distance from the frame's left edge through that column. With the current zero-origin frame this is `x + 1`; using the frame origin in the calculation keeps the component correct for a non-zero outer area. The component applies the existing width normalizer before emitting a semantic live-resize message.

The shell accepts the resolved width directly. It does not read component geometry or convert the message into repeated five-column keyboard steps. Duplicate positions produce no message, avoiding unnecessary redraws.

### D3: Live mutation and durable persistence are separate operations

A live-resize message updates only the in-memory queue-column width. A drag-end message persists the width only if that gesture changed it. This avoids filesystem writes for intermediate positions while retaining the existing preference format and startup behavior. Keyboard resizing continues to mutate and persist in one action.

A press followed by release without motion emits no resize and no persistence request.

### D4: Eligibility loss cancels the gesture

The shell synchronization pass supplies current boundary geometry and enabled state before event delivery. The enabled state requires Panel mode `both` and ordinary panel mouse eligibility. Disabling or hiding the boundary resets both its local resize anchor and the shared recognizer's drag anchor.

This prevents a drag interrupted by a Panel mode change, popup, or blocking overlay from resuming when the component later becomes eligible. Cancellation does not perform an additional preference write; only a delivered drag end completes persistence.

### D5: Verification uses one focused component check and one live-tick check

A focused component test covers the error-prone coordinate rule, exact one-column arming, clamping, click-without-motion, and cancellation. A live `Application::tick()` integration test covers the architectural property that press, drag, and release reach only the painted boundary owner through the real synchronization and overlay-arbitration path, with live mutation followed by one final persistence action.

Tests assert resolved widths and relational ownership rather than fixed absolute coordinates. A buffer ownership check pre-fills the boundary cells with a sentinel, proves root chrome leaves them untouched, then proves the boundary component restores the exact prior focused and unfocused appearance. Manual verification checks that the edge remains visually unchanged and is usable across representative terminal widths.

## Risks / Trade-offs

- [A one-column target requires precise pointing] -> This is intentional and avoids stealing panel clicks; the existing edge remains the complete target.
- [Repainting during rapid drag events can do redundant work] -> Emit only when the clamped width changes; terminal input already limits event frequency.
- [The boundary column could be painted twice during migration] -> Reserve it in root chrome and verify one-painter ownership before enabling its subscription.
- [An interrupted gesture can leave stale state] -> Reset gesture and resize anchors whenever synchronization disables the component.
