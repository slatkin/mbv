# Proposal

## Why

The seekbar lost its gesture when the TUI moved to TuiRealm mouse subscriptions.

Legacy behaviour (`src/app/input_mouse_dispatch.rs`, deleted in `b74cb22ef`, 2026-08-25):

- a seek needed a **double-click**: two left presses within 400 ms at the same cell. A single
  press fell through to cursor focus and never moved the position;
- a left **drag** on the bar scrubbed, throttled to one seek per 150 ms.

The replacement handler (`PlaybackComponent` at `ac5626b82`, live since `b5e45f301`, 2026-09-04)
matched a raw left press. The double-click window and the drag arm were never carried over, and the
capability spec was then written to match the new code (`queue-playback-panel`: "a click on its
seekbar SHALL seek"). Today a stray single click on the bar jumps playback, and dragging the bar
does nothing.

## What Changes

- Both transport panels (`QueuePlaybackPanel`, `LibraryPlaybackPanel`) recognize pointer gestures
  through their own `MouseGestureState` (the shared recognizer, ADR 0024).
- The seekbar resolves a seek only on a recognized **double-click**. A single press on the bar
  resolves nothing.
- A left **drag** that anchored inside the bar scrubs to the pointer column, at most one seek per
  150 ms.
- The transport controls keep their single-press behaviour exactly: play/pause, stop, previous and
  next still resolve on the first press, and the added recognition changes neither their intents
  nor their availability gates.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `queue-playback-panel`: the pointer requirement now separates a transport glyph's single press
  from the seekbar's double-click, and adds the throttled drag scrub with its in-bar anchor. The
  partial-cell scenario of "Fractional Queue fill preserves the seek row and interaction
  boundaries" states the double-click.
- `mouse-input`: the "Chrome that never holds focus is still clickable" scenario states the
  transport click and the seekbar double-click, so it no longer reads as a single-click seek.

## Impact

- `crates/mbv-components/src/queue_playback_panel.rs` and its `tests.rs`
- `crates/mbv-components/src/library_playback_panel.rs` and its test module
- `src/app/tests/tick_integration/queue_playback.rs`, `src/app/tests/tick_integration.rs` (the
  seekbar click fixtures become double-click fixtures)
- No public API change outside `mbv-components`. No new dependency.
- `App::seek_to_fraction` and `PlaybackRequest::SeekTo` are unchanged: only the gesture that
  produces the request changes.
