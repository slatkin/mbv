## Context

See `proposal.md` for motivation. The queue card renderer already owns artwork selection, missing-artwork fallback, image sizing, and the `(height, width)` consumed by queue layout. The visualizer is currently rendered later in `render_main`, either below the queue list or in spare rows under the wide queue-only playback content. Its persisted `visualizer_enabled` boolean controls both display and PipeWire worker lifecycle.

The shared slot must preserve the queue card's current dimensions, remain available without playback, and retain the existing rule that PipeWire capture runs only for supported active playback on this machine. ADR 0009 currently defines `v` as enable/disable and must be amended with the implementation.

## Goals / Non-Goals

**Goals:**

- Make the queue card renderer the only owner of visualizer placement and artwork fallback.
- Keep one persisted two-state choice and the existing preference migration path.
- Reclaim every row reserved solely for the old separate visualizer.
- Preserve image-loading stability and all capture isolation guarantees.

**Non-Goals:**

- Changing vectorscope rendering, PipeWire capture, cadence, colors, or glyph configuration.
- Adding a new panel type, generalized media-slot abstraction, or additional display modes.
- Showing local system audio for playback hosted on another machine or for audio-pipe playback.

## Decisions

### Reuse the queue card renderer and its returned geometry

`render_card` will choose and paint exactly one content source inside the rectangle it already budgets: decoded artwork, the selected visualizer, a transient loading block, or the bundled placeholder. When visualization is selected, it renders into the same card rectangle and returns that rectangle's height and width so narrow, normal, and wide queue layouts remain unchanged.

The old alternative was to calculate matching geometry in `render_main` and overlay the visualizer there. That would duplicate image sizing and missing-artwork decisions, so the card renderer remains the single boundary.

### Keep the persisted boolean but treat it as content selection

The existing persisted `visualizer_enabled` value already represents the required two states. Its on-disk key remains readable so existing users retain their choice; implementation names may be clarified locally where doing so does not require compatibility scaffolding. `v` flips the selection in every context. Selecting artwork stops capture; selecting visualization asks lifecycle synchronization to start capture only when existing local/active/audio-pipe guards allow it.

An enum was rejected because there are exactly two requested states and no third state is planned.

### Render an empty selected visualizer independently of capture eligibility

Display selection and worker eligibility become explicit separate concerns. The card renderer clears and paints the visualizer background even when the sample window is empty. `sync_visualizer` continues to gate PipeWire by playback activity and locality, so stopped, audio-pipe, and remote playback show an empty selected visualizer without starting an invalid capture.

This replaces the current attached-session `v` no-op. Keeping the no-op would violate the unconditional artwork/visualizer toggle and make display behavior depend on playback ownership.

### Distinguish loading artwork from confirmed missing artwork

While artwork is selected, a pending fetch keeps the existing dim loading reservation. A fetch that resolves without artwork, a queue item with no artwork source, or an empty queue uses the existing bundled placeholder. When visualization is selected, those steady placeholder paths instead render the empty or populated vectorscope.

When terminal images are disabled, artwork selection performs no image fetch and paints no terminal image, but the card renderer keeps the fallback rectangle already used when the placeholder cannot render. Visualization selection remains available and draws into that same rectangle. Both selections return identical geometry so `v` does not move the queue list when no image protocol is available.

Immediately substituting the visualizer during every pending fetch was rejected because it would flash unrelated content before valid artwork appears and could change card geometry when the fetch completes.

### Delete both separate visualizer layout branches

`render_main` will no longer subtract `VISUALIZER_HEIGHT` from queue rows, draw the bottom rule, or fill wide playback-panel leftovers with a second visualizer. Wide playback leftovers use their existing background. This is deletion rather than adding synchronization between duplicate placements.

### Amend the existing keybinding decision

ADR 0009 will be amended in place to retain `v` as the visualizer command while changing its effect from enable/disable to queue-card artwork/visualizer selection. No new domain term is introduced; planning and code use “queue card” or “queue artwork rectangle” rather than adding another meaning for Panel.

## Risks / Trade-offs

- [A persisted `true` preference changes placement after upgrade] -> Preserve the value intentionally; users who previously enabled the visualizer see it selected in the new shared slot.
- [Visualizer rendering accidentally changes card geometry] -> Derive its rectangle from the existing placeholder/image reservation and add focused narrow and wide render assertions.
- [Missing artwork and loading artwork are confused] -> Test pending, resolved-empty, no-source, and decoded-image states separately at the card-render boundary.
- [Removing layout branches leaves stale queue-height behavior] -> Add render tests asserting no rows are subtracted below the queue and no playback-panel visualizer is drawn.

## Migration Plan

Land the preference-compatible renderer and layout deletion together with the spec and ADR updates. Rollback requires no data migration because the persisted boolean and vectorscope configuration remain unchanged.
