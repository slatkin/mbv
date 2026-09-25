# Design

## Context

The QueueColumn's layout already collapses the visual slot to zero rows while idle. It does this
at one seam: `Model::sync_queue_card_geometry` (`src/app/shell/chrome_panels.rs`) publishes
`layout.card = CardGeometry::default()` when `now_playing_status()` is `Idle`. Every downstream
consumer reads that geometry and produces the collapsed layout with no special case of its own:

- `chrome_geometry` → `queue_playback_rows` → `queue_panel_geometry` (row budget)
- `queue_playback_transport_area` (the transport rect, both breakpoints; mouse hits use it too)
- the slot paint in `Model::render_queue_playback_panel` (skipped when idle)

`last_card_height/width` are the paint checkpoint that stops `v` from moving the queue list.
Idle leaves them untouched, so playback resuming restores the previous size.

Artwork fetching is already gated in `src/app/shell/queue.rs` (`refresh_queue_card_image` runs
only while playback is active). Capture is gated by `App::visualizer_should_run`.

## Goals / Non-Goals

**Goals:**
- Hidden reuses the idle collapse seam, so geometry, paint, and mouse hits cannot drift apart.

**Non-Goals:**
- Changing Feeds' key handling (#794).
- Height-driven automatic collapse.
- A clickable on-screen indicator for the toggle (the June 2026 `[▶]` widget is not restored).

## Decisions

### D1: One flag, `visual_slot_hidden: bool`, on `App`

It is independent of `visualizer_enabled`: hiding keeps the selection, so all four combinations
are valid states and no enum is needed. It is shell-owned presentation preference (like
`queue_column_width`), not component-local state, because layout geometry and the paint path
both read it.

### D2: Collapse at the idle seam

`sync_queue_card_geometry` returns `CardGeometry::default()` when the slot is hidden, and the
slot paint site skips `render_queue_playback_slot` under the same condition. Both use one `App`
predicate: "slot shown" = not idle and not hidden. Idle keeps its existing meaning everywhere
else (the transport still renders when hidden).

Alternative rejected: threading a `hidden` input through `ChromeGeometryInput` /
`queue_playback_rows`. That duplicates what a zero `card_height` already means.

The `last_card_*` checkpoint is left untouched while hidden, as idle leaves it. Showing the slot
again restores the last painted size with no jump. A resize while hidden is corrected on the
first paint, the same as after idle.

### D3: Drop the slot/transport gap for a zero-width slot

At 100 columns and wider, `queue_playback_transport_area` offsets the transport by
`card_width + SLOT_TRANSPORT_GAP`. With a zero-width slot that leaves a 2-cell gap on the left.
The gap becomes `SLOT_TRANSPORT_GAP` only when `card_width > 0`. This also applies to the existing
zero-width cases (images off with no checkpoint yet, first frame). A gap next to nothing is a flaw
in those cases too, and the row count helper passes `card_width = 0` already, so rows are
unaffected.

### D4: `v` while hidden: guard in `toggle_visualizer`, not a router gate

`toggle_visualizer` returns immediately while hidden. Closing a policy gate would instead let `v`
fall through to the focused component, which would change routing behaviour for no benefit. The
router still consumes `v`, as before.

### D5: Capture and fetch follow the same predicate

`visualizer_should_run` adds `!visual_slot_hidden`. The toggle command calls `sync_visualizer`
so capture starts or stops right away. The artwork projection call in `shell/queue.rs` adds the
same hidden check next to its active check. When the slot is shown again, the next projection
pass fetches.

### D6: Keybinding

A new `KeybindAction { id: "hide_visual_slot", section: Playback, default_chords: ["h"],
gate: NoBlockingOverlay, rebindable: true, prefix_addressable: true }` and a matching
`KeyPolicyEntry` (`global: true`, `KeyPolicyGate::NoBlockingOverlay`), placed next to
`visualizer`. The router's existing guarantees cover text entry and blocking overlays. As a
global, `h` takes precedence over the Feeds component's local `h` (accepted; #794).

### D7: Persistence

`save_prefs` writes `visual_slot_hidden`, and construction reads it (missing → `false`), the
same pattern as `queue_column_width`. The toggle command calls `save_prefs`.

## Risks / Trade-offs

- [D3 changes the wide transport position for existing zero-width slots] → The existing geometry
  tests for the wide breakpoint are the check. Only the 2 gap cells move, and only when nothing
  is painted to their left.
- [Global `h` takes over Feeds' local `h` move-up alias] → `k`/Up still move up; #794 removes all
  vim-style aliases.
