## Why

Stay-alive daemon mode has three independent queue-tracking bugs that make Next/Previous unreliable and cause brief wrong-track flashes on queue load. The `observed_active_slot` feedback loop — the mechanism by which the daemon learns what track the Playback run is actually playing — is broken in active_file mode (never emits TrackChanged) and fragile in normal mode (settle requires exact dual match). These aren't edge cases; they affect every stay-alive session.

## What Changes

- **Emit TrackChanged from active_file JumpTo path**: `select_active_slot` (or its JumpTo call-site in `commands.rs`) will emit TrackChanged with the transition tag so the daemon's `observed_active_slot` advances. This closes the feedback loop that is currently open in active_file mode.
- **Reset `observed_active_slot` on queue replacement**: When `UnifiedQueueReplace` installs a new queue, the daemon will clear `observed_active_slot` (and the owner's copy) so stale slot IDs from the previous queue don't pollute Next/Previous resolution.
- **Fix mpv loading-order race in `queue_load_indices`**: Restructure so that items before `start_idx` are loaded before the "replace" loadfile, or defer playback start until the full playlist is loaded and `playlist-pos` is set correctly. Eliminates the window where `playlist-pos=0` points at the wrong track.
- **Relax settle matching**: Investigate whether settle should match on request_id alone (not request_id AND slot), so that a natural track advance during an in-flight transition doesn't stall the pipeline until the 5s timeout.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `daemon-playback-intents`: The requirement "Next and Previous SHALL each remain single-flight until the requested track change reaches Applied" is violated because the TrackChanged event that would settle the transition is never emitted in active_file mode, leaving every Next in perpetual flight.
- `unified-playback-queue`: The requirement "A Player owner SHALL change the observed active slot only from a Playback-run observation naming an owner-assigned slot" is correct in intent but the implementation has a gap — the active_file JumpTo path changes the active slot without producing the observation that updates `observed_active_slot`. The requirement "A request to play a Queue slot SHALL create desired transition state and SHALL NOT itself change the observed active slot" is also affected: the transition is created but never settled.

## Impact

- `crates/mbv-core/src/player/run/commands.rs` — JumpTo handler active_file branch
- `crates/mbv-core/src/player/run/queue.rs` — `select_active_slot`
- `crates/mbv-core/src/player/run/events.rs` — `on_end_file` / TrackChanged emission
- `crates/mbv-core/src/player/mod.rs` — `queue_load_indices`, `reassert_queue_layout`
- `crates/mbv-core/src/daemon_control.rs` — `UnifiedQueueReplace` handler, Next/Previous resolution
- `crates/mbv-core/src/daemon_run.rs` — TrackChanged handler, `observed_active_slot` lifecycle
- `crates/mbv-core/src/playback/transition.rs` — `settle` matching logic
