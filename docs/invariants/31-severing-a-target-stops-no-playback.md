# Invariant 31 — Severing a target stops no playback

**Scope:** `sever_connections_without_output_change` and `hand_remote_output_back_to_local` (`src/app/state/remote_slot.rs`), every attach/switch path in `src/app/dispatch/session/switch.rs` (`connect_to_session`, `activate_session_target`, `switch_to_library_route`, `connect_direct_session`), and `App::select_panel_target` (`src/app/dispatch/cast.rs`).

## The invariant

1. Severing a connection is a control-link operation only. It detaches the Client from the outgoing target's owner and sends that owner no playback commands: no `Stop`, no `SelectPlaybackOutput(Local)`.
2. The outgoing owner keeps playing whatever it plays — a Cast receiver, an Emby Session output, a suspended home link.
3. A hand-off is always the owner's own output selection (`SelectPlaybackOutput`), asked for separately. No attach path moves the outgoing owner's output back to Local as a side effect.
4. The only remaining `Stop`-then-`Local` send is the explicit "play locally" fall-through, where the user asked for local playback and the remote output must end first.

## Why it matters

Before the session-switch playback-continuity decision, every switch path called `sever_active_connection`, which handed the remote output back to Local — so picking another Session killed the prior session's playback. When the decision reversed that, `switch_to_library_route` kept the old hand-back anyway, because nothing names "sever means sever" as a contract: each attach path chose its own pre-attach send, and one chose wrong. The spec scenario "A session or direct remote is connected while a cast target is attached" describes the rule.

## How the code maintains it today

- `sever_connections_without_output_change` is the shared sever: it restores a library route and disconnects a remote, and sends nothing to the outgoing owner.
- `attach_remote_owner` suspends the home link (which keeps playing) or disconnects the previous remote itself; attach paths do not pre-send output commands.
- The hand-off fork (`ConfirmAction::HandOffPlaybackToSession`) is the only place a Session activation sends an output selection, and the user answers it first.
- Guard tests: `connecting_to_a_session_leaves_the_local_owners_cast_output_playing`, `connecting_to_a_session_leaves_the_local_owners_emby_session_output_playing` (`src/app/tests/session_connect.rs`), and `switching_to_a_library_route_leaves_the_local_owners_remote_output_playing` (`src/app/tests/route_state.rs`) each assert the owner channel receives nothing.

## Where it currently fails / how it could regress

- A new attach path that sends `Stop` or `SelectPlaybackOutput(Local)` to `self.player` before `attach_remote_owner` reintroduces the kill-the-prior-playback bug. Nothing type-level prevents it; the guard tests cover only the three paths above.
- `hand_remote_output_back_to_local` still exists for the play-locally fall-through. Reusing it in a sever path is the exact regression shape: its name reads like teardown, but it sends playback commands.
