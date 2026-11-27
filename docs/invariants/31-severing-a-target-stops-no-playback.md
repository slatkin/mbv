# Invariant 31 — Severing a target stops no playback

**Scope:** `sever_connections_without_output_change` and `hand_remote_output_back_to_local` (`mbv_client_core::state::remote_slot`), every attach/switch path in `mbv_client_core::dispatch::session::switch` (`connect_to_session`, `activate_session_target`, `switch_to_library_route`, `connect_direct_session`), `App::select_panel_target` (`mbv_client_core::dispatch::cast`), and the output-switch answer path in `mbv_client_core::dispatch::session::output_handoff` (which sends `App::switch_playback_output`, `mbv_client_core::state::remote_slot`).

## The invariant

1. Severing a connection is a control-link operation only. It detaches the Client from the outgoing target's owner and sends that owner no playback commands: no `Stop`, no `SelectPlaybackOutput(Local)`.
2. The outgoing owner keeps playing whatever it plays — a Cast receiver, an Emby Session output, a suspended home link.
3. An output selection is always the owner's own, asked for separately, and the user answers it first. Two user-answered output sends exist: `SelectPlaybackOutput` (the hand-off fork's answer, which moves the active slot) and `SwitchPlaybackOutput` (the switch answer, which hands nothing off). Both go out after the sever. No attach path moves the outgoing owner's output back to Local as a side effect.
4. The switch answer releases a remote old output: the Owner sends a Cast receiver or an Emby Session device no stop, tears down no receiver app, and stops tracking it, so the device keeps playing. Only the device stop is gone; the Owner's bookkeeping (the Emby stop report for the presented position, the Feed `Stopped`) still goes out when it releases. A switch away from Local still stops the local run, because the Owner hosts it. A hand-off (`SelectPlaybackOutput`) still stops the old output, because the slot moves to the target.
5. A `Stop`-then-`Local` send happens only where the user's own answer hands the slot to local playback: the explicit "play locally" fall-through (`SelectPlaybackOutput(Local)`). The switch answer to Local (`SwitchPlaybackOutput(Local)`) hands nothing off: it releases the old remote output with no stop and makes Local the output with no active slot. Neither is a side effect of a sever.

## Why it matters

Before the session-switch playback-continuity decision, every switch path called `sever_active_connection`, which handed the remote output back to Local — so picking another Session killed the prior session's playback. When the decision reversed that, `switch_to_library_route` kept the old hand-back anyway, because nothing names "sever means sever" as a contract: each attach path chose its own pre-attach send, and one chose wrong. The spec scenario "A session or direct remote is connected while a cast target is attached" describes the rule. The switch answer keeps that rule and goes one step further: a switch hands nothing off, so the Owner releases the old remote device instead of stopping it — a stop would end playback there as a side effect of a selection.

## How the code maintains it today

- `sever_connections_without_output_change` is the shared sever: it restores a library route and disconnects a remote, and sends nothing to the outgoing owner.
- `attach_remote_owner` suspends the home link (which keeps playing) or disconnects the previous remote itself; attach paths do not pre-send output commands.
- A Session activation sends an output selection only where the user answers a prompt: the hand-off fork (`ConfirmAction::HandOffPlaybackToSession`) sends `SelectPlaybackOutput`, and, when the local Owner advertises the output-switch capability, the switch answer on the same prompt sends `SwitchPlaybackOutput` after the same `sever_connections_without_output_change`. Neither send rides on the sever.
- The Owner-side handling of `SwitchPlaybackOutput` performs the swap in release mode (`StopScope::Release`, selected by `SelectionIntent::SwitchOnly` in `stop_old_output_for_handoff`, `mbv-daemon`): the old Cast receiver or Emby Session device receives no stop and no receiver app teardown, while the stop report and Feed `Stopped` bookkeeping still go out. A `Continue` selection keeps the stop (`LoadedItemOnly`) and the teardown.
- Guard tests: `connecting_to_a_session_leaves_the_local_owners_cast_output_playing`, `connecting_to_a_session_leaves_the_local_owners_emby_session_output_playing` (`mbv_client_core::app::tests::session_connect`), and `switching_to_a_library_route_leaves_the_local_owners_remote_output_playing` (`mbv_client_core::app::tests::route_state`) each assert the owner channel receives nothing.

## Where it currently fails / how it could regress

- A new attach path that sends `Stop` or `SelectPlaybackOutput(Local)` to `self.player` before `attach_remote_owner` reintroduces the kill-the-prior-playback bug. Nothing type-level prevents it; the guard tests cover only the three paths above.
- `hand_remote_output_back_to_local` still exists for the play-locally fall-through. Reusing it in a sever path is the exact regression shape: its name reads like teardown, but it sends playback commands.
- Moving the switch send into `sever_connections_without_output_change` is the same regression shape for the new path: a sever that carries a playback command. The switch send stays in the answer paths, after the sever returns.
- Handling a `SwitchOnly` swap with the hand-off stop (`LoadedItemOnly` and the receiver app teardown) instead of `Release` reintroduces the stop-on-switch: the released device's playback ends as a side effect of a selection.
