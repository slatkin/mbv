# Invariant 26 — The queue-end adoption needs the tracked item's end

**Scope:** `queue_end_continuation`, `ended_naturally`, `estimated_end`, and `ended_recently` (`crates/mbv-daemon/src/emby_session/end.rs`); `advance_from_natural_end` (`end.rs`) and `handle_sessions` (`crates/mbv-daemon/src/emby_session/poll.rs`); `handle_lookup`, `clear_continuation`, and `submit_lookup` (`crates/mbv-daemon/src/emby_session/adopt.rs`).

## The invariant

1. The Owner appends a foreign item to the queue only when the device started it within `EMBY_SESSION_LOAD_TIMEOUT` (30 s) of the tracked item's estimated end. A foreign item presented later stays present-only.
2. The window runs from the estimated end (the observed time plus the runtime that observation had left), not from the poll that detects the natural end. An idle observation can arrive minutes after the end, after the device left `/Sessions`.
3. A lookup result adopts only while three things hold: the lookup is still pending, the output still tracks the ended item, and the Session still presents the looked-up item. Every other exit of the lookup clears the pending continuation through `clear_continuation`.
4. An adoption keeps a queued next record that a queue edit re-established during the lookup flight. Dropping it adopts the same device item a second time at its end.

## Why it matters

Adoption changes the queue on the user's behalf. A window that runs from the detecting poll lets an item the user starts minutes after the end join the queue. A missing gate on the tracked item lets a stopped output adopt content. Dropping a queued next record duplicates the item the device plays.

## How the code maintains it today

- `ended_naturally` returns the estimated end. `advance_from_natural_end` opens `ContinuationWatch::Window` at that end plus 30 s, only at the queue's edge.
- `queue_end_continuation` has two paths. The direct path needs an unhandled sighting, an estimate within the end margin, and `ended_recently`. The after-gap path needs an open window and a handled end. An expired window is cleared on the next observation.
- `handle_lookup` checks pending, tracks-ended, and still-presented before it appends. The three non-adopting exits call `clear_continuation`.
- Guard tests in `crates/mbv-daemon/src/tests/emby_session_end/continuation.rs`: `the_queue_end_autoplay_is_adopted`, `a_foreign_item_presented_after_a_long_absence_is_not_adopted`, `a_natural_end_revealed_after_a_long_absence_opens_no_window`, `a_lookup_result_after_the_output_was_stopped_adopts_nothing`, and `an_append_during_the_lookup_flight_keeps_the_queued_next_item`.

## Where it currently fails / how it could regress

- The after-gap path trusts the window's `until` value. A future code path that opens a window from another anchor, such as the detecting poll, reopens the late-adoption gap, and no test fails unless it sends an idle observation after a long absence.
- `ended_recently` bounds only the direct path. The after-gap path relies on the window's anchor, so the two paths guard the same contract in different ways.
- `submit_lookup` clears the pending continuation on a dead worker without the daemon, so it keeps its own assignment rather than the shared helper.
