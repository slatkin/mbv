## Context

See `proposal.md` for motivation. The current wide layout stores a three-state `panel_mode`, while widths below 80 columns derive a two-state mini view from an ephemeral `mini_view_focus`. The wide cycle and the mini-view initializer currently choose the library panel first. Actual terminal width is refreshed from the rendered frame, and resize events currently trigger redraw cleanup rather than changing panel mode state.

## Goals / Non-Goals

**Goals:**

- Make queue-only the first single-panel destination in the wide `x` cycle.
- Make queue-only the initial mini-view state at startup and whenever the terminal crosses from wide to narrow.
- Keep narrow `x` toggling, focus-follow behavior, ephemeral mini-view state, and wide-state restoration coherent.
- Preserve the existing `both` startup mode for wide terminals and avoid persisting mini-view state.

**Non-Goals:**

- Changing the 80+ column layout geometry or queue/library rendering.
- Persisting panel mode or mini-view state across sessions.
- Changing the unrelated library list width breakpoint.
- Changing playback, visualizer, daemon, protocol, or provider behavior.

## Decisions

### Reverse the existing wide cycle

Change the wide `x` transition order to `Both -> QueueOnly -> LibraryOnly -> Both`. This keeps all three existing wide modes and the one-key cycle while making the first intentional collapse land on the queue, matching the queue-first requirement. Reordering the existing transitions is smaller and clearer than adding a separate command or a new panel mode.

### Reset mini-view focus only on entry

Initialize the ephemeral mini-view focus to `Queue`. When the rendered terminal width crosses from 80 or more columns to fewer than 80 columns, reset that ephemeral focus to `Queue` as well. Do not reset it on frames that remain narrow, so pressing `x` continues to toggle predictably without being overwritten on the next render. Do not mutate the stored wide `panel_mode` or `panel_focus` during this transition.

Using the rendered frame as the width-transition boundary keeps startup and real resize behavior on the same path, because the application learns the actual terminal size there. The resize event remains responsible for invalidating terminal-sized rendering state.

### Keep focus and restoration rules unchanged

Entering or toggling to queue-only mini view continues to move effective focus to the queue and initialize its cursor when needed. Toggling to library-only moves effective focus to the library. Widening continues to read the untouched wide mode and focus, so narrow interactions remain ephemeral.

### Update the contract and focused tests together

The delta spec changes only the two requirements whose externally visible defaults change. Tests should cover the reversed wide cycle, startup narrow rendering, crossing into narrow from each wide mode, narrow toggling, and widening restoration. Existing queue-only rendering and focus behavior remains covered by the current panel-mode tests.

## Risks / Trade-offs

- [Resetting on every narrow frame would make the narrow `x` toggle unusable] -> Detect only the wide-to-narrow threshold crossing.
- [Startup begins with the constructor's placeholder width rather than the real terminal width] -> Apply the same crossing check during the first render after the actual frame width is known.
- [Changing the wide cycle surprises users relying on the old order] -> Update the user-facing shortcut description and retain all three modes and their explicit focus rules.
- [Narrow interaction could accidentally mutate wide state] -> Continue routing narrow reads through effective state while leaving persisted wide fields untouched.

## Migration Plan

No persisted-data migration is required. The panel mode remains non-persistent, and mini-view focus remains ephemeral. Rollback is limited to restoring the previous cycle order and library-first mini-view initialization.
