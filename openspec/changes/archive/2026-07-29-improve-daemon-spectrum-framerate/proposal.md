> **Superseded (2026-07-29):** This change is superseded by `remove-daemon-spectrum-streaming`,
> which removes the entire daemon spectrum streaming path. The temporary frame-rate
> instrumentation is deleted along with the daemon FIFO path.

## Why

The daemon's spectrum visualization is throttled to ~10 fps by the `POLL_INTERVAL_MS = 100ms` poll timeout in `CavaWorker`, despite CAVA being configured to produce 60 fps and the client render loop running at ~83 fps. This causes jerky, low-resolution visualizations that waste the CAVA pipeline's capacity and the client's rendering budget.

## What Changes

- Reduce `POLL_INTERVAL_MS` from 100ms to 16ms (matching CAVA's ~60 fps output rate) so the poll loop responds to data availability without unnecessary artificial delay.
- Keep `FRAME_QUEUE_CAPACITY = 2` — the existing latest-wins drain in `take_latest_frame()` already discards stale frames; increasing queue capacity is not needed.
- The idle `thread::sleep(16ms)` in the daemon spectrum reader thread only fires when no frames are available; it stays as-is since it does not throttle busy reads.
- Validate that observed frame rate in the daemon's spectrum stream approaches 60 fps under load.

## Capabilities

### New Capabilities
- `improved-spectrum-framerate`: The daemon spectrum stream SHALL deliver frames at a rate approaching 60 fps (matching CAVA's configured output rate), subject to system scheduling.

### Modified Capabilities
<!-- No existing spec-level requirements are changing — this is a change within an existing capability boundary. -->

## Impact

- **Code**: `crates/mbv-core/src/visualizer.rs` — change `POLL_INTERVAL_MS` constant from `100` to `16`.
- **No API changes**, no dependency changes, no config changes.
- **Risk**: Polling more frequently increases CPU wake-ups. Mitigation: `libc::poll` with a 16ms timeout is still a blocking syscall; the worker thread does not busy-wait. On a system where CAVA runs anyway, the incremental cost is negligible.
