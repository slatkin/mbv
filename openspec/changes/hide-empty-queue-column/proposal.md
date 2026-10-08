# Proposal

## Why

In the two-panel layout, an empty queue still takes a full column that shows only a `¯\_(ツ)_/¯` placeholder. That space is wasted. The library-only layout already exists and fits this case, so the two-panel layout should show it while the queue is empty.

## What Changes

- In the two-panel (`both`) layout at 80+ columns, while the displayed queue has no slots, the window SHALL render the existing library-only layout: full-width library, Library playback panel, no queue column and no queue boundary.
- The stored Panel mode stays `both`. The `x` cycle is unchanged: the next press goes to queue-only. When the displayed queue gains a slot, the queue column comes back without user action.
- If the queue held panel focus when it became empty, focus moves to the library and stays there when the queue refills.
- Column resize and Alt+Left are inactive while the column is hidden. This follows from the existing "not `both`" gate, which reads the effective layout.
- Queue-only and the mini-view queue panel keep the empty placeholder, because hiding the only displayed panel would leave a blank window.
- The pinned panel's layouts are unchanged: they set `library-only` or `queue-only` directly, never `both`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `panel-mode`: new requirement that the two-panel layout renders as library-only while the displayed queue is empty, with the focus and refill rules above.

## Impact

- `src/app/state/panel_focus.rs`: `effective_panel_mode` and `effective_panel_focus` derive library-only while `both` holds an empty displayed queue.
- `src/app/shell/queue.rs`: the queue sync moves the stored `panel_focus` to the library when the column is hidden.
- No change to `chrome_geometry`, the queue component, or the empty placeholder. Every layout, input, and hit-test reader already goes through `effective_panel_mode`.
