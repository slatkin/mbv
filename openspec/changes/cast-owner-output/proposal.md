# Proposal

## Why

Cast sent the whole played queue to the receiver as one `QUEUE_LOAD` message. With about 300–400 bytes for each Emby URL, a queue of 271 items made a frame of about 100 KB. The receiver dropped the connection, and every later poll failed (#944). The interim fix (`7389aeb54`) loads only the selected item. Now next, previous, and auto-advance do nothing on Cast.

The original Cast design also broke the queue authority rule: the receiver owned its own copy of the queue, and mbv lost track of it after dispatch. The Cast attachment lives in the TUI. Because of this, the Owner process cannot see or drive Cast playback, and Cast stops when the TUI closes.

## What Changes

- **New concept: Playback output.** Each Player owner has exactly one active playback output: local playback (mpv) or one Cast receiver. The Owner holds the queue and the active slot for every output. An output plays only the active slot, and it reports its state back to the Owner.
- **The Cast attachment moves from the TUI into the Owner process.** That means the receiver connection, status poll, keep-alive, reconnect, source resolution, and progress reporting. mDNS discovery and the receiver picker stay in the TUI.
- **The Owner drives the receiver one item at a time.** On a natural end that the receiver reports, the Owner steps through its own PlayOrder and loads the next slot. Next and previous load the target slot of the Owner directly. No message ever carries the whole queue to the receiver.
- **Switching outputs hands off.** When the output changes (local to Cast, Cast to local, or Cast X to Cast Y), the active slot continues on the new output at the current position, with the same play/pause state. The old output stops.
- **The Owner presents Cast state for every Client.** The Owner translates receiver status into its normal status broadcast and extrapolates the position between polls. Every Client shows the same state.
- **Reconnect joins the running receiver session.** It does not launch the receiver app again, and it recovers a natural end that it missed during the outage.
- **Packaged `mbvd` has no Cast output.** Only the Local daemon offers it (the full `mbvd` enforcement is #946).
- **BREAKING (internal ctrl):** a new capability-gated ctrl surface selects the output and broadcasts it. No protocol version bump.
- **Removed:** the `CastClient` `dispatched_queue`/`jump` code; the `Jump`, `JumpNoQueue`, and `JumpNoAdjacent` transport kinds; the TUI `CastAttachment`, `CastPlaybackTarget`, Cast status poll, Cast progress fan-out, and the idle-load-then-dispatch play path.

## Capabilities

### New Capabilities

- `owner-playback-output`: the single active playback output of a Player owner, output selection, hand-off between outputs, and Owner-driven advance on a natural end that an output reports.

### Modified Capabilities

- `cast-session-control`: the receiver no longer owns a queue. The Owner holds the Cast attachment, drives it one slot at a time, routes transport commands to it, reports its progress, and survives the exit of a Client. Exit of the Owner leaves the current item playing. Reconnect joins the running session. Launch never reattaches.
- `cast-media-dispatch`: Uncastable items are found when the Owner loads a slot, not when a selection is dispatched. Auto-advance reports an uncastable slot and steps past it. An explicit play of an uncastable slot reports it and plays nothing.
- `unified-playback-queue`: a Cast receiver is an output projection of the Bound queue, like mpv. The populate-only load applies only to an attached Emby Session, not to Cast.

## Impact

- `crates/mbv-cast`: remove the jump code; pass `idleReason` through to `CastStatus`; map `LOADING`; join a running receiver app on connect; the transport trait and the worker become public, so that the daemon can host them.
- `crates/mbv-ctrl`: new capability, a command that selects the output, and an event that carries output state.
- `crates/mbv-daemon`, `crates/mbv-player`: output state in the Owner; the Cast branch at the three submission paths and at slot jumps; Cast source resolution (Emby device profile, ABS episode session, Feed URL); Owner-owned progress reporting for Cast; a poll and keep-alive tick; Local-only hosting through `DaemonRuntimeHooks`.
- `src/app/dispatch/cast.rs`, `cast_status.rs`, `state/types/cast.rs`, `state/playback_target/cast.rs`: most of this code is deleted. The TUI keeps discovery and the picker, and sends the output selection.
- Tests: `src/app/tests/queue/cast_dispatch.rs` is deleted. New tests on the Owner use a fake Cast transport.
- Docs: `CONTEXT.md` (new *Playback output*; *Cast attachment* and *Cast dispatch* are rewritten or retired), AGENTS.md "Playback and queue authority".
- Follow-ups: #945 (Emby remote as the second remote output) and #946 (`mbvd` local-only enforcement).
