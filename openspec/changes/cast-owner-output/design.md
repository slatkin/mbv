# Design

## Context

See proposal.md for the reasons. Current state that shapes the approach:

- **Cast is TUI-only.** `src/app/state/types/cast.rs` (`CastTransport`, `spawn_cast_worker`, `CastAttachment`), `src/app/dispatch/cast.rs` (dispatch, resolve, connect), `src/app/dispatch/cast_status.rs` (7s poll, 3s keep-alive, 20s watchdog, extrapolation, progress fan-out to Emby, Feed, and ABS), `src/app/state/playback_target/cast.rs` (transport routing). `mbv-daemon` has no reference to Cast.
- **`CastClient` is `!Send`.** One worker thread owns it for the life of an attachment. Commands go in as boxed jobs, and results come back on a channel.
- **The Owner already has what Cast needs.** It has its own Emby client and ABS context (`EmbyOwnerContext`, `AudiobookshelfOwnerContext`). It resolves `QueueItem`s by itself (`prepare_source`, `crates/mbv-player/src/sources.rs:179`). It has a cursor and an active slot with no run (`UnifiedQueueLoadIdle`, `control/queue_load.rs`), a relative-step resolver (`PlayerOwnerState::relative_step_target`), and a transition gate (`OwnerTransitionState`) that the run observations settle and expire.
- **Funnels.** Every playback start passes through three submission funnels plus the live-run jump: `control/playback.rs` `play_resolved_items`, `control/queue_setup.rs` `handle_queue_replace`, `core.rs` `send_jump_or_cold_start` (cold path), and `dispatch_slot_jump` (live `JumpTo`).
- **Role difference.** `mbvd` and the Local daemon share `run_with_options`. They differ only in `DaemonRuntimeHooks` (the tray is the precedent for Local-only features).
- **Receiver status.** `rust_cast` 0.21 already parses `StatusEntry.idle_reason` (`Finished`/`Cancelled`/`Interrupted`/`Error`). It also exposes `receiver.get_status()` with the running applications and their `transport_id`. mbv uses neither today.

## Goals / Non-Goals

**Goals:**
- The type system enforces a single output in the Owner. It cannot be represented as two outputs.
- One step rule (`relative_step_target`) for next, previous, and natural advance on every output.
- The TUI holds no Cast connection, status, or progress code. Only discovery and the picker stay.

**Non-Goals:**
- Emby remote sessions as an output (#945). The output enum is shaped so that they can be added later, but this change adds no variant for them.
- A faster status poll near the end of an item. The gap between items can be up to one poll interval (7s). Tune it later if it is a problem.
- Text subtitles or track selection on Cast (unchanged v1 gap).
- Full Client-side `mbvd` enforcement beyond Cast (#946).

## Decisions

### D1. `PlaybackOutput` is an enum held by the Owner

`DaemonPlayerOwner` gets `output: PlaybackOutput`, where `enum PlaybackOutput { Local, Cast(CastOutput) }`. `CastOutput` owns:
- the worker sender and the receiver identity (id, name, address);
- the connection state;
- the loaded item: `QueueSlotId`, content id, progress target, ABS lifecycle, and an end-handled flag;
- the last status and its timestamp.

A single enum makes "two active outputs" impossible to represent. *Alternative:* an `Option<CastOutput>` beside the run. This was rejected because it permits a live run and a Cast output at the same time.

### D2. Cast is hosted in `mbv-daemon` and enabled through `DaemonRuntimeHooks`

`mbv-daemon` depends on `mbv-cast`. `DaemonRuntimeHooks` gets a Cast connector. The Local daemon supplies it, and `mbvd` passes `None`. Without the connector, the Owner does not advance the capability and rejects a Cast selection with a reason. This is the same pattern as the tray. *Alternative:* a cargo feature. This was rejected because it adds a build matrix for one runtime difference. The `mbv-cast` code is linked into `mbvd` but never runs there.

### D3. Ctrl surface: one command, one event, one capability

- A new capability `playback-output`. No version bump (additive, per the `ctrl-protocol` capability rule).
- `CtrlCmd::SelectPlaybackOutput(OutputSelection)` with `OutputSelection::{Local, Cast { receiver_id, name, host, port }}`. The TUI supplies the address from its own discovery.
- `CtrlEvent::PlaybackOutput(OutputState)` with `OutputState::{Local, Cast { receiver_id, name, connection }}`. It is sent on cold seed and on every change, only to peers that negotiated the capability.
- Slot, position, and pause state use the existing `UnifiedQueueState` and `StatusOnly`, which have no output-specific shape. Failures use `CommandRejected`. *Alternative:* an `output` field in `UnifiedQueueStateData`. This was rejected because it would change a hot, versioned shape for a value that changes rarely.

### D4. The output branch sits at the funnels

The three submission funnels and `dispatch_slot_jump` match on `PlaybackOutput`:
- `Local` keeps today's path.
- `Cast` sets the active slot, starts the Cast source resolve for that slot on a worker thread, and re-enters through a `DaemonEvent`, as `PlaybackResolved` does today. Then it sends `load(item, start_position)` to the Cast worker.

Transport `WireCommand`s:
- `TogglePause`, `Seek`, `SeekAbsolute`, `SetVolume`, and `SetMute` are routed to the receiver.
- `SetAudio`, `SetSub`, and `SetSubtitlePrefs` are rejected with a reason.
- Stop stops the receiver media.

*Alternative:* a trait object for the output. This was rejected because there is one remote variant now, and an enum match keeps every arm exhaustive.

### D5. The transition gate settles Cast loads

A Cast load is an in-flight transition in `OwnerTransitionState`:
- A receiver status with the loaded content id in Playing, Buffering, or Paused state settles it, as `TrackChanged` does.
- A failed load or a resolve failure expires it.

So fast repeated next presses coalesce with no code that is specific to Cast. *Alternative:* a Cast-only pending-load flag. This was rejected because it would duplicate the gate.

### D6. A separate Cast source resolver in the Owner

`prepare_cast_source(item, ctx) -> Result<CastSource, Uncastable>` moves from the TUI into `mbv-daemon`, with `CastSource { media: CastMediaItem, progress: CastProgressTarget }`:
- **Emby:** `build_cast_device_profile`, then `get_playback_info_for_cast`.
- **ABS episode:** a bounded session open. The session stays open for progress.
- **Feed:** the enclosure URL.
- **ABS book:** uncastable.

It does not extend `prepare_source`, because the mpv path (bearer-header options, HLS wait, close-on-drop lifecycle) and the Cast path (credential in the URL, a server-chosen rendition, a lifecycle that the output owns) share almost nothing. *Alternative:* a target parameter on `prepare_source`. This was rejected because every arm would branch twice.

### D7. Advance: `idleReason` plus a content guard

`CastStatus` gets `idle_reason: Option<CastIdleReason>`, and `LOADING` is mapped to a buffering state. On each status:

| Status | Owner action |
|---|---|
| `Idle(Finished)` for the loaded content id, end not yet handled | Mark handled, record completion through the same observation path as `TrackCompleted` (progress, consume rules), step with `relative_step_target(Next)` |
| `Idle(Cancelled)` | Present no active playback. No advance |
| `Idle(Error)` | Report the failure. No advance |
| `Idle(Interrupted)` | Ignore (this is the Owner's own replacing load) |
| Idle for content that the Owner did not load | Present it. No advance, no progress |

The end-handled flag stops a second poll of the same `Finished` entry from advancing two times. If the step target is uncastable, the Owner reports the item and steps again. The number of steps is limited by the queue length. At an edge, the output stays idle. Natural advance and the next key use the same `relative_step_target(Next)`. In Shuffle, this reads the pre-picked upcoming slot, the same as mpv.

### D8. The Owner polls, extrapolates, and publishes

The daemon tick runs the keep-alive (3s) and the status poll (7s) as jobs on the Cast worker. Results come back as `DaemonEvent::Cast`. While Playing, the Owner extrapolates the position on its tick and publishes `StatusOnly` at the same cadence as local playback. The 20s worker watchdog moves with the poll. Clients do not extrapolate Cast. They present the status as they do for local playback.

### D9. The Cast output owns progress reporting

- **Emby:** a `SessionReporter` built from the ids in the `CastProgressTarget` (item, media source, play session), fed with the status positions.
- **ABS episode:** the `AudiobookshelfLifecycle` from the resolve, held in the loaded item. It is observed and synced on each status, and closed on advance, stop, or output change.
- **Feed:** entry state follows the path that local playback uses for Feed progress. The Cast-only Feed write in `cast_status.rs` is deleted. The implementer verifies this path before deleting (task 6.4).

On an output change, the old output reports stop before the new output reports start.

### D10. Hand-off sequence

1. Capture `(active slot, position, paused)` from the current output.
2. Stop the old output: Local stops the run; Cast stops the media and tears down the app.
3. Swap `PlaybackOutput`.
4. If a slot was active, load it on the new output at the position. Cast: `load(item, position)`, then pause if it was paused. Local: cold start at the resume position, then pause if it was paused.

If the slot is uncastable on the new output, the Owner reports it and the output stays idle. A Cast connect failure keeps the old output and reports the failure. In that case the stop in step 2 has not happened yet, because the connect is attempted first.

### D11. Connect joins and does not hijack

`CastClient::connect` reads `receiver.get_status()` first:
- If `DefaultMediaReceiver` is running, it connects to that app's `transport_id` and adopts the `media_session_id` from `media.get_status`.
- If no app is running, it launches the app as today.
- If another app is running (another sender took over the TV), a reconnect does not launch, and the output stays `Cast` in the `Disconnected` state. A first selection by the user still launches.

With the D7 guard, a reconnect picks up a `Finished` that it missed during the outage.

### D12. Owner exit drops the connection only

When the Owner shuts down, it closes the socket and does not call `stop_app`. Only a deliberate output change or detach tears down the receiver app.

### D13. The TUI becomes a thin picker

- F3 Cast selection and detach send `SelectPlaybackOutput`.
- The output indicator, the F3 connection mark, and the idle "keep panel" rule read `CtrlEvent::PlaybackOutput`.
- `playback_target()` loses its Cast branch. While a receiver is the output, the TUI target is its Local daemon owner.
- Cast rows are offered only when the owner advertises `playback-output`.

Deleted:
- `CastAttachment`, `CastPlaybackTarget`, the TUI poll, keep-alive, watchdog, extrapolation, and progress;
- `play_selection_on_cast`, `dispatch_selection_to_cast`, `dispatch_queue_play_cursor_to_cast`, and `cast_active_queue_index`.

Discovery (`spawn_cast_discovery`) stays.

### D14. Vocabulary

`CONTEXT.md` gets **Playback output**: the one place where a Player owner plays its active slot, local playback or one Cast receiver. It avoids "output target", "active player", and "sink". The entry states that it differs from *Clocked audio output*, which is the audio device of a Playback run. *Cast attachment* is redefined as the Owner's relation to the receiver that is its output. *Cast dispatch* is retired (its avoid-list moves to *Playback output*).

## Risks / Trade-offs

- [The gap between items can be up to 7s] → Accepted for now (Non-Goal). The poll interval is one constant if a faster near-end poll is needed later.
- [A second poll sees the same `Finished` entry and advances two times] → The end-handled flag on the loaded item (D7). A test owns this contract.
- [A blocking Cast or HTTP call stalls the Owner event loop] → Every receiver call runs on the Cast worker, and every resolve runs on a resolve thread. Results come back as `DaemonEvent`s. The event loop never waits on the network.
- [A reconnect takes over a TV that someone else now uses] → D11: a reconnect never launches when another app is running.
- [Two Clients select different receivers at nearly the same time] → The last selection wins. Both see the broadcast result. This is the same as the latest-play-wins rule.
- [The Emby play session for the Cast rendition differs from the session of the local path] → The reporter uses the `PlaySessionId` that `get_playback_info_for_cast` returns, carried in `CastProgressTarget`, as the TUI does today.
- [Local cold start may not support "start paused"] → D10 falls back to start then pause. Task 7.2 checks which one the run supports.
- [`mbv-cast` (and `rust_cast`) are now linked into `mbvd`] → They are inert without the hook. This was accepted to avoid a cargo feature (D2).

## Migration Plan

No persisted format changes. The output is not persisted, and the Owner always starts on Local. A TUI and an Owner from different builds negotiate the capability. A TUI without it keeps showing local state, and an Owner without it hides Cast rows. Rollback means a revert of the change. No data migration is needed.
