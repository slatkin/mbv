# Invariant 28 — A guarded playback intent always settles, and a stop says what it stops

**Scope:** `PlaybackIntentState` (`crates/mbv-daemon/src/core.rs`), `handle_playback_intent` (`crates/mbv-daemon/src/control/playback.rs`), `stop_active_output` and `StopScope` (`crates/mbv-daemon/src/cast.rs`).

## The invariant

1. A guarded playback intent (`Play`, `SetPaused`, `Stop`) leaves `Accepted` for `Applied` or `Rejected` before the same Client can send the next intent that matters. `Stop` has no resolve/open lifecycle, so it settles in the handler that performs it, exactly as `SetPaused` does on a remote output.
2. A stop names what it stops. A user stop (`CtrlCmd::Stop`, the `Stop` intent, the MPRIS/tray transport stop) acts on the active output's device, whatever it plays. Only the stop an output change issues (`stop_old_output_for_handoff`) is limited to the item the Owner loaded.

## Why it matters

`PlaybackIntentState::accept` coalesces an intent that matches an unresolved intent from the same connection. An unsettled `Stop` therefore swallows every later `Stop` from that Client: the user's next press runs nothing at all, with no error. The symptom is "the stop key stopped working", not "a command failed".

Conflating the two stops produced the same kind of silent no-op the other way: an Emby Session that was already playing when the user connected kept playing through the user's stop, because the hand-off rule ("leave content the Owner did not load playing", emby-session-owner-output D11) was applied to the user's explicit stop too. The spec says a playback command from any Client acts on the active output; the Cast arm already stopped foreign media.

## How the code maintains it today

- `PlaybackIntentAction::Stop` calls `handle_stop`, then `intents.applied_if_current` and sends the `Applied` event to the Client — the same settle the `SetPaused` remote arm performs.
- `stop_active_output` takes a `StopScope`. The explicit callers (`handle_stop` for `CtrlCmd::Stop` and the intent, `DaemonLoop::handle_transport` for a transport stop) pass `StopScope::Everything`, which sends `transport("Stop")` to a Session and clears the loaded record. `stop_old_output_for_handoff` passes `StopScope::LoadedItemOnly`, which keeps `stop_loaded_item`'s guard.
- Guard tests: `a_client_stop_intent_stops_the_loaded_item_on_every_press` (two presses, two `Applied` events, two device stops) and `a_user_stop_while_foreign_content_plays_stops_the_device`, both in `crates/mbv-daemon/src/tests/`.

## Where it currently fails / how it could regress

- A new intent action with no resolve/open lifecycle that forgets to settle latches as `Accepted` and coalesces later presses from its connection. `PlaybackIntentState::accept` says in its comment that only `Play`, `SetPaused`, and `Stop` settle, but nothing enforces that a new action keeps the promise.
- A new explicit-stop call site that passes `StopScope::LoadedItemOnly` reintroduces the "connected to a playing device, cannot stop it" no-op. The hand-off site is the only one that should.
