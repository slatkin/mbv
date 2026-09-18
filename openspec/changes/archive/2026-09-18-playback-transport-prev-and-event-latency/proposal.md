## Why

Two playback defects surfaced while verifying the stay-alive queue fix:

- The playback strip paints `stop / prev / next`, but only stop and next retained hit geometry, so
  the previous control was display-only in both panels that paint it.
- Navigating from mpv's own window (OSC next/prev) took up to two seconds to reach the queue
  projection. Measured against a fresh mpv, the events themselves are prompt: `playlist-next`
  produces `end-file reason=stop` in 1 ms. The delay was the run loop's own idle wait — a 2000 ms
  poll on the wakeup pipe, which a lost wakeup turned into the full deadline.

## What Changes

- Every transport glyph the strip paints retains its own hit geometry, including prev, and the
  panels resolve a prev click to the same typed transport intent the keyboard path uses. Whether
  prev is available at all is threaded through with the availability `next` already carries, and
  an unavailable control keeps its muted role and resolves nothing.
- The run loop's idle wait is bounded at 50 ms and is documented as the ceiling on event latency
  rather than an idle interval. The wakeup pipe stays a latency optimisation: libmpv may signal a
  wakeup before the event is deliverable, and that case now costs at most the ceiling instead of
  the whole deadline.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `queue-playback-panel`: pointer resolution covers every painted transport control, prev included,
  and an unavailable control resolves nothing.
- `daemon-playback-intents`: mpv-originated events are observed within a bounded latency that does
  not depend on a wakeup notification arriving for each event.

## Impact

- `crates/mbv-core/src/player/run/run.rs` — the idle-wait ceiling and its rationale.
- `src/app/render/components/chrome_player.rs` — the prev glyph's retained area and role.
- `src/app/render/components/chrome_player_context.rs`, `src/app/shell_playback.rs`,
  `src/app/components/{library,queue}_playback_panel.rs` — prev availability and click resolution.
- The OSC script is unchanged: its prev button was already bound to `playlist-prev`; mpv received
  and executed it promptly, and the observed lag was the run loop's.
