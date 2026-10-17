# Design

## Context

See proposal.md for the reasons. Current state that shapes the approach:

- **The Cast output is the template.** `crates/mbv-daemon/src/cast.rs` holds `PlaybackOutput { Local, Cast(Box<CastOutput>) }`, `output_state()`, and the Cast output state. `cast/{advance,command,connect,status,progress,feed_events}.rs` hold the logic. The archived change `2026-10-16-cast-owner-output` (decisions D1–D14) explains it. This design reuses that shape and names only the differences.
- **The attached Emby Session lives in the TUI.** It uses `connected_session_id` and `connected_session_state` (read in about 40 files), `RemotePlaybackTarget` (`src/app/state/playback_target/remote.rs`), the whole-queue push `submit_attached_sequence` and `session_jump_track` (`src/app/dispatch/session/command.rs`), the populate-only load before a session play (`load_idle_queue_on_owner`, only for this path), a 1 s `/Sessions` poll (`src/app/shell/run/drains.rs:489`), and position extrapolation and miss counting (`src/app/dispatch/run_loop/session.rs`).
- **A connect only watches today.** `connect_to_session` (`src/app/dispatch/session/switch.rs:445`) sets `connected_session_id` and does not stop local playback. Plays then go to the device.
- **The Emby session API is plain HTTP.** `session_play` (one item, `crates/mbv-emby/src/client_sessions.rs:229`) already exists. The attached-session path before this change sent the whole queue as one `PlayNow` with `ItemIds` and `StartIndex` (`session_play_items`). The device played that list in order, and the TUI found the active slot from `NowPlayingItem`. A device that holds only one item autoplays its own next episode at the end (live test of this change), so the Owner keeps the next slot on the device too (D14). So do the transport, seek, volume, mute, audio-index, and subtitle-index commands. No `!Send` client exists here.
- **Emby gives no end reason.** `/Sessions/Playing/Stopped` has the same body as the start body. The websocket has no stop message for other clients. The Owner sees only `NowPlayingItem` disappear.
- **`SessionInfo` has no device id.** `/Sessions` returns `DeviceId`, but the parser only uses it to filter out mbv's own session (`client_sessions.rs:95`).
- **Completion recording is provider-neutral.** `record_completed_slot` (`crates/mbv-daemon/src/run.rs:82`) changes only the canonical queue (progress fields and consume rules). It sends nothing to Emby.
- **`cast.rs` has 683 lines**, which is near the 800-line limit.

## Goals / Non-Goals

**Goals:**
- Use one output enum and one step rule for all outputs. The Emby Session adds a variant and does not add a parallel path.
- The TUI holds no Emby Session connection, poll, position, or transport code. F3 lists Sessions and sends a selection.
- Cast and the Emby Session share one advance and step-over state machine.

**Non-Goals:**
- Adopting a device's own next-episode autoplay as a slot. The two-item window (D14) prevents the autoplay while the queue has a next slot.
- A slower poll while the output is disconnected.
- Removing the ctrl `UnifiedQueueLoadIdle` command. It loses its last TUI sender. Its removal is a separate change.
- Changes to *Direct remote control* or Library routes.

## Decisions

### D1. `PlaybackOutput` moves to its own module and gets an Emby Session variant

`PlaybackOutput` and `output_state()` move from `cast.rs` to a new `crates/mbv-daemon/src/output.rs`. The enum becomes `{ Local, Cast(Box<CastOutput>), EmbySession(Box<EmbySessionOutput>) }`. The new module `emby_session.rs` (with children `emby_session/{connect,poll,end,command}.rs`) holds the `EmbySessionOutput` struct. It owns:
- the device identity (`device_id`, name), the current `session_id`, and the advertised playable media types;
- the connection state, the miss count, the poll in flight (poll id and time), and the job sender;
- the last observation and its time;
- the loaded item: `QueueSlotId`, Emby item id, runtime ticks, the time of the load, `seen_playing`, `end_handled`, and `failure_reported`;
- the queued next item on the device (D14): `QueueSlotId` and Emby item id, and whether a queue edit made it stale;
- a pending pause (D9) and the shared advance state (D6).

*Alternative:* add the variant inside `cast.rs`. This was rejected because the file would go over 800 lines, and an Emby type does not belong in a Cast module.

### D2. A runtime hook enables the output; a new capability announces it

`DaemonRuntimeHooks` gets `emby_session_output: bool`. The Local daemon sets it to true, and `mbvd` sets it to false. When it is true, the Hello advertises a new capability `emby-session-output`. The Owner rejects an Emby Session selection with `CommandRejected` and a reason in three cases: the hook is off, the Owner has no `EmbyOwnerContext`, or the device is not listed.

*Alternative:* gate on the presence of the Cast connector. This was rejected because it couples two outputs that have no relation.

### D3. Ctrl vocabulary

- `OutputSelection::EmbySession { device_id, name }`. The Owner resolves the current session id itself (D4), so a session id that is no longer valid cannot reach it.
- `OutputState::EmbySession { device_id, name, connection, audio_only, now_playing }`. `now_playing: Option<SessionNowPlaying>` describes content that the Owner did not load (item id, item type, name, series id, series name, logo tag): the fields that the TUI reads from `SessionInfo` today for the card, the title overlay, and the logo. It changes only when the device item changes, so it is sent in this event and not in the hot `StatusOnly` broadcast.
- `CastConnection` is renamed to `OutputConnection`. The wire form does not change, because serde writes the variant names and not the type name.
- The Owner sends an `OutputState::EmbySession` event only to peers that negotiated `emby-session-output`. A peer that negotiated `playback-output` but not `emby-session-output` receives `OutputState::Local` in its place, on cold seed and on each change, so it does not keep a stale Cast state.

*Alternative:* put the foreign item into `PlayerStatus`. This was rejected because `PlayerStatus` is a hot, versioned shape, and this value changes rarely.

### D4. One job worker for each output; the selection resolves the device

An Emby Session output owns one worker thread with a job channel. That thread runs every HTTP call in order: the poll, play, and transport commands. Results come back as `DaemonEvent::EmbySession`. The event loop never waits on the network, and commands reach the device in the order that the user sent them. (The TUI today starts one thread for each command, so their order is not fixed.)

The worker calls a small `SessionApi` trait (`get_sessions`, `play` (a `PlayNow` with a list of item ids: one or two in D14, the rest of the queue in D15), `play_last` (a `PlayLast` with a list of item ids, D14, D15), `transport`, `seek`, `set_volume`, `set_mute`, `set_audio_index`, `set_subtitle_index`). It is implemented for `EmbyClient`. Tests use a fake (the boundary mock, the same pattern as the Cast transport). The trait has no method that reports progress, so D8 holds by construction.

The trait's `get_sessions` calls `EmbyClient::get_sessions_unfiltered`, not the filtered `get_sessions`. The filtered list (`ActiveWithinSeconds=600`) drops a device that is idle but still connected (#236), so the Owner would mark an idle TV `Disconnected`. A selection runs `get_sessions` on the worker and finds the entry whose `DeviceId` matches. If it finds none, or the call fails, the old output stays and the Owner reports the reason.

### D5. Poll, observation, and reattach

The daemon tick sends a poll job every 1 s, with one poll in flight at a time. The poll uses the same unfiltered list (D4). Each poll has a monotonic id, as the Cast poll does, so a late result from a replaced worker is ignored. For each result, the Owner finds the entry by `device_id`:
- **Entry found:** store the observation and its time, reset the miss count, and adopt its `session_id` (a reattach after the device app restarts). Then run D6 and D7.
- **Entry missing:** add one to the miss count. At 3 misses, set the connection to `Disconnected` and broadcast it. The queue, the active slot, and the loaded item do not change, and polling continues. A missing entry is never an idle Session and never an end.

### D6. End rule, load failure, and the shared advance

`EmbySessionOutput` estimates the position as `last_position + (now - observed_at)` while the Session plays, capped at the runtime. The runtime is `QueueItem::duration()` of the loaded slot. If that is absent, the Owner uses the runtime that the Session reports.

On each observation:

| Observation | Owner action |
|---|---|
| Session presents the loaded item | Set `seen_playing`, settle the transition (D7), send a pending pause (D9) |
| Session presents the queued next item, and the replaced observation presented the loaded item | The device moved on by itself. Apply the device advance of D14 |
| Session idle, `seen_playing`, not `end_handled`, estimate >= runtime − 10 s | Set `end_handled`. Record completion through `record_completed_slot`. Step with `relative_step_target(Next)` |
| Session idle, `seen_playing`, estimate < runtime − 10 s | Set `end_handled`. Present no active playback. No advance |
| Session idle, not `seen_playing`, less than 30 s after the load | Wait |
| Not `seen_playing`, 30 s or more after the load, not `failure_reported` | Report the failure by item name. Expire the transition. No advance |
| Session presents an item that the Owner did not load (not the loaded item and not the queued next item) | Put it in `now_playing` (D3). No advance and no completion |

The 30 s check also runs on the tick, so a Session that is idle and unchanged still gets the timeout.

The advance and step-over code in `cast/advance.rs` (`CastAdvance`, `begin_advance`, `step_advance`, `step_failed`, `continue_after_failed_step`) moves to `output/advance.rs` as `OutputAdvance`. `PlaybackOutput` gets an accessor that returns the advance state of the remote variant that is active. Only `apply_end_report` (which reads a `CastStatus`) stays in `cast/`. The private helpers that `begin_advance` calls move with it, except the Feed `TrackCompleted` call in `record_natural_end`, which stays on the Cast side: Feed slots never pass Emby admission, so the Emby Session needs no Feed events. For the Emby Session, the admission check of D7 is synchronous. It runs before `dispatch_step_target`, not inside it, because `step_target_advance` sets `step_request` only after the dispatch returns, and a rejection inside the dispatch would not match it in `step_failed`. A rejected step target is reported, and the next target is picked in the same loop, limited by the queue length. The user sees the same "report and step again" result as for a Cast resolve failure.

*Alternative:* copy the advance code for the Emby Session. This was rejected because two state machines would drift apart.

### D7. Admission and load at the funnels

The same four branch points as Cast D4 (`play_resolved_items`, `handle_queue_replace`, the cold path of `send_jump_or_cold_start`, and `dispatch_slot_jump`) get an `EmbySession` arm:
1. Admission: a pure function `admit(item, playable_media_types)`. Emby items pass when the Session advertises their media kind, or advertises no kinds. All other items fail, with the reason.
2. If the item passes, the Owner sets the active slot, registers the in-flight transition, and sends a `play(session_id, [item_id, next_item_id], start_ticks)` job (D14). `start_ticks` is the resume position, or the position from the hand-off.
3. An explicit play or jump of an item that fails admission reports the item and loads nothing.

There is no resolve step, because the device fetches its own stream. When an observation shows the loaded item, the transition settles. A failed `play` call or the 30 s timeout expires it. Quick repeated next presses coalesce through the existing gate.

### D8. No Owner progress reporting

The Emby Session arm builds no `SessionReporter` and opens no ABS lifecycle. Feed and ABS items never pass admission, so no Feed `PlayerEvent`s are needed. Completion and consume rules still go through `record_completed_slot`, which writes only to the canonical queue.

### D9. Transport routing

| Command | Emby Session |
|---|---|
| `TogglePause` | `transport("PlayPause")` |
| `PlaybackIntentAction::SetPaused { paused }` (the TUI pause path) | `transport("Pause")` or `transport("Unpause")`; no command when the observation already matches. The guarded intent settles at once, as on Cast |
| `Seek(delta)` | `seek(estimate + delta)`, clamped at 0 |
| `SeekAbsolute` | `seek` |
| `SetVolume`, `SetMute` | `set_volume`, `set_mute` |
| `SetAudio(index)` | `set_audio_index(index)` |
| `SetSub(id)` | `set_subtitle_index(id)`, where id 0 (off) maps to −1 |
| `SetSubtitlePrefs` | `CommandRejected` with a reason |
| Stop | `transport("Stop")`, and the loaded item is cleared |
| Next / previous on the loaded item | The Owner's relative step (spec "Next and previous load the target slot directly") |
| Next / previous on content the Owner did not load | `transport("NextTrack" / "PreviousTrack")` |

Track and volume commands act on whatever the device plays, as they do in the TUI today. Cast rejects track commands, but an Emby Session supports them.

The Next and Previous intents follow the next and previous rows. A tray or MPRIS command (`route_transport_command`) routes like the matching wire command; a local-only command is a documented no-op, as on Cast.

Most routing sites do not `match` on `PlaybackOutput` today. They use `let … else` or `matches!(…, Cast(_))`, so an Emby Session output falls through to the local player there without a compile error: `cast/command.rs` (`route_player_cmd`, `route_transport_command`, `set_receiver_paused`), `control/playback.rs:84` and `:171`, `control/queue_setup.rs:386`, and `core/transition_dispatch.rs:37`. Each of these sites gets an Emby Session branch.

**Hand-off pause:** a `play` job cannot start paused. When the hand-off state was paused, the output sets a pending pause. The first observation that shows the loaded item sends `transport("Pause")`.

### D10. Status projection

While the output is an Emby Session, the Owner fills `PlayerStatus` from the last observation:
- position = the D6 estimate; paused, volume, and muted come from the observation;
- `audio_tracks`, `sub_tracks`, `audio_id`, and `sub_id` come from `media_info` and the stream indexes (Emby −1 maps to 0, off);
- `video_height` is parsed from the video label;
- title fields come from the queue item when the device plays the loaded item. For content that the Owner did not load, they come from the observation.

The Owner publishes `StatusOnly` at the local-playback cadence and extrapolates on the tick. Clients present it as they present local playback.

### D11. Hand-off and exit

The Cast D10 sequence holds unchanged for every pair of outputs. "Connect" for an Emby Session is the device lookup of D4. "Stop the old output" for an Emby Session sends `transport("Stop")` only while the device plays the loaded item. Content that the Owner did not load is left playing. A selection of the device that is already the output does nothing. On Owner shutdown, the output hands the rest of the queue to the device (D15), then stops its worker and sends no Stop (the same rule as Cast D12).

### D12. The TUI becomes a picker

- **F3 activation of an Emby Session row:**
  - A Session with a direct endpoint keeps today's Direct remote control path. If that connect fails, the TUI reports the failure. Then, when the owner advertises `emby-session-output`, it selects the Session as the output (today's fallback to an attached session, moved to the Owner).
  - Otherwise, when the owner advertises `emby-session-output`, the TUI calls `sever_connections_without_output_change` (a Library route or Direct remote control) and sends `SelectPlaybackOutput(EmbySession)`.
  - Without the capability, it shows a warning and does nothing.
- `hand_cast_output_back_to_local` becomes `hand_remote_output_back_to_local`, for a Cast or an Emby Session output. It runs before a Direct remote or Library route connect.
- Detach of an Emby Session output sends `SelectPlaybackOutput(Local)`, as Cast detach does.
- The F3 connection mark matches a row by `device_id`. The chrome status, card, title overlay, and logo read `OutputState::EmbySession` (`now_playing` for content that the Owner did not load). `playback_eligibility` reads `audio_only` from it.
- An accepted fall-through sends `Stop`, then `SelectPlaybackOutput(Local)`, and then plays locally.
- Launch: `reconnect_direct_session` connects only when the Session found has a direct endpoint. Teardown no longer saves a plain Session, because that state is gone.
- **Deleted:**
  - the state `connected_session_id` and `connected_session_state`, and `RemoteSlotState::AttachedSession`;
  - `RemotePlaybackTarget` and the remote branch of `playback_target()`;
  - `submit_attached_sequence`, `session_jump_track`, `remote_jump_target`, `play_on_connected_session`, and `handoff_queue_play_to_session`;
  - the session-play callers of `load_idle_queue_on_owner`;
  - the 1 s poll trigger, the connected-session half of the `SessionEvent::Loaded` handling, the extrapolation, the miss counting, and `remote_tracking.rs`;
  - `session_play_items`.
- **Kept:** the F3 list load (`spawn_sessions_load`).
- **Moved to the output state:** two guards read `connected_session_id` today and must not be deleted with it. `sync_volume_from_player` does not save the volume while the output is an Emby Session, because D10 puts the device volume in `PlayerStatus`. `visualizer_should_run` does not capture while the output is an Emby Session (spec `system-audio-visualizer`), next to its `output_is_cast()` check.

### D13. Vocabulary

In `CONTEXT.md`:
- *Playback output* becomes "local playback, one Cast receiver, or one Emby Session".
- *Session watch* is retired (its avoid list moves to the new term).
- New **Session output**: the relation of a Player owner to the Emby Session that is its *Playback output*. It is keyed by the device, observed by a poll, and its end is inferred. _Avoid_: session watch, attach (bare), tracked session.
- The *Queue scope* and *Library route* entries change their references from *Session watch* to *Session output*.
- *Consume* drops its sentence that a Session watch never consumes. An Emby Session output consumes through the Owner, as every output does.
- *Cast attachment* drops "a watched session" from its exclusivity sentence. It states that a Session selection while a receiver is the output is a hand-off.

In AGENTS.md "Playback and queue authority", an Emby Session joins Cast as an output projection.

### D14. Two items on the device

The device holds the active slot and the next slot. It never holds more. Without the next slot, the device app autoplays its own next episode at the end, and the 1 s poll sees no idle between the two items, so the D6 end rule never fires.

- **Load** (every `play` job of D7, including Next, Previous, a jump, and a hand-off): `PlayNow` with `ItemIds = [active item, next item]`, `StartIndex = 0`, and the start ticks of the active item. The next item is the first slot after the active slot, in play order, that passes admission. If no slot passes, `ItemIds` holds only the active item. The output records that slot as the queued next item.
- **Device advance:** an observation presents the queued next item, and the replaced observation presented the loaded item. The device moved on by itself, so playback must not stop. The Owner:
  1. records completion of the old slot through `record_completed_slot` only when the estimate was within 10 s of its runtime (D6). A skip on the device is not a completion;
  2. makes the queued slot the active slot and the loaded item, with `seen_playing` set, and settles;
  3. finds the next admissible slot after it and sends `play_last` (`PlayCommand = PlayLast`) with its item id. That slot becomes the queued next item. If there is none, nothing is queued.

  The Owner sends no `PlayNow` here.
- **Same item in both slots:** the item id does not change, so a device advance is a position drop to less than 10 s after an estimate within 10 s of the runtime. The same three steps then apply.
- **Queue edits:** the Sessions API can add items but cannot remove or reorder them on the device. When an edit changes the next admissible slot, the Owner never interrupts the active item:
  - no item was queued and now one exists (an append at the end of the queue): send `play_last` with it;
  - otherwise: mark the queued next item stale. On the device advance to a stale item, the Owner sends `PlayNow` with the correct next slot and the slot after it. Only that boundary is interrupted.
- **End of the queue:** with nothing queued, the device holds only the last item. An idle after it follows D6 (completion, then no next slot). If the device autoplays its own next episode, that item is content the Owner did not load (D6 last row), and the queue stays on the last slot.

*Alternative:* push the whole queue as one list (the old attached-session path). This was rejected because queue edits after the push have no effect on the device (#945).

*Alternative:* hold one item and replace the device's own autoplay with a `PlayNow` of the next slot. This was rejected because the autoplayed episode starts before the Owner replaces it, at each item boundary.

### D15. Hand the rest of the queue to the device at Owner exit

When the Owner exits, it gives up queue authority. The device must then play the rest of the queue by itself, and not stop after the queued next item or autoplay its own next episode. This applies only on `DaemonEvent::Shutdown`. A stay-alive Owner that outlives its Clients keeps the output and its authority, so a Client quit then hands over nothing.

On `DaemonEvent::Shutdown` while the output is an Emby Session and the last observation presents the loaded item:
1. The Owner collects every admissible slot after the queued next item, in play order. Slots before the active slot have played, and they are not sent.
2. If the queued next item is current, the Owner sends one `play_last` with those item ids. The active item is not interrupted.
3. If the queued next item is stale (D14), the Owner sends one `play` (`PlayNow`) with the active item, the correct next slot, and the rest, with the estimated position as the start ticks. The active item reloads one time at the exit, and the device never plays the stale item.
4. The Owner waits for that one call, for at most 5 s, and then stops the worker as in D11 (no `Stop`). A failed or late call is logged, and the exit goes on.

If the device is idle or presents content the Owner did not load, the Owner sends nothing. If nothing is after the queued next item, the Owner sends nothing.

`play_last` therefore takes a list of item ids (D4).

*Alternative:* push the whole queue with `PlayNow` and `StartIndex` at the active slot. This was rejected because it reloads the active item at each exit.

## Risks / Trade-offs

- [A very long queue at exit] → One `play_last` carries every remaining item id. The old attached-session path sent the whole queue in one `PlayNow`, so the request size is known to work.

- [A device app ignores `PlayLast`] → It autoplays its own next episode after the second item, and the Owner presents that as content it did not load. The live test of this change checks `PlayLast` on the Emby web Session. The old attached-session path used only `PlayNow`.
- [A queue edit makes the queued next item stale] → The device starts the stale item, and the Owner replaces it at that boundary (D14). The active item is never interrupted.

- [A user stop in the last 10 s advances the queue] → Accepted and stated in the spec. Emby cannot separate it from an end. The credits usually play in that part of an item.
- [Both the queue item and the Session report a runtime of 0] → No natural end is possible, and the idle counts as a user stop. The queue stops there. This is rare for Emby items, and the risk is accepted.
- [The device app autoplays its own next episode after the last slot] → It shows as content that the Owner did not load, and the queue stays on the last slot (D14). Adopting it is a Non-Goal.
- [A paused hand-off plays for about one poll interval before the pause] → Accepted. Emby `PlayNow` has no start-paused flag.
- [A 1 s `/Sessions` poll loads the server] → The TUI polls at the same rate today. The Owner polls only while the output is an Emby Session.
- [A slow command delays the poll on the shared worker] → Accepted, because order matters more. The poll skips while one is in flight, and the 30 s timeout uses Owner time, not poll time.
- [An older TUI meets a newer Owner] → The `OutputState::EmbySession` event is sent only to peers with `emby-session-output`. Other peers with `playback-output` receive `OutputState::Local` (D3). An older TUI shows local state.

## Migration Plan

No persisted format changes. The output is not persisted, and the Owner starts on Local. A saved `LastRemoteConnection::DirectSession` record that names a device without a direct endpoint is ignored at launch, and the Client stays local. A TUI and an Owner from different builds negotiate `emby-session-output`: without it, the TUI does not offer Emby Session selection. Rollback is a revert. No data migration is needed.
