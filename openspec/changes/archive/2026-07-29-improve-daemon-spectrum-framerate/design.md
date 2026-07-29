## Context

The daemon spectrum visualization pipeline has three stages operating at different rates:

1. **CAVA process**: Configured at 60 fps via `framerate = 60` in the generated config. Writes ASCII frames to a private FIFO.
2. **CavaWorker poll loop** (in `visualizer.rs`): Uses `libc::poll()` with `POLL_INTERVAL_MS = 100ms` timeout. If no data is available within that timeout, the loop spins again. This effectively caps the read-out rate at ~10 fps worst case (1 / 0.1s). The poll timeout also serves as the idle-backoff mechanism.
3. **Daemon reader thread** (in `daemon_core.rs`): Calls `worker.take_latest_frame()` in a loop, sends `DaemonEvent::Spectrum` messages through a channel. When no frame is available (`Ok(None)`), it sleeps 16ms.
4. **Client render loop**: Renders at ~83 fps (12ms interval), but is starved for fresh frames because of the bottleneck at stage 2.

The design doc for the daemon spectrum feature explicitly noted: "If higher frame rates are desired later, `POLL_INTERVAL_MS` can be reduced."

## Goals / Non-Goals

**Goals:**
- Remove the 100ms polling bottleneck so the CavaWorker reads frames from the FIFO at CAVA's native ~60 fps rate.
- Maintain the existing latest-wins frame semantics with the bounded `sync_channel(2)`.
- Keep the change minimal — a single constant change, no new dependencies, no architectural restructuring.

**Non-Goals:**
- Replacing the polling model with async I/O.
- Increasing frame queue capacity (latest-wins is the desired behavior).
- Changing CAVA configuration or input method.
- Guaranteeing exactly 60 fps (system scheduling is beyond our control).
- Adding formal frame-rate telemetry (can be validated through manual testing).

## Decisions

### Decision 1: Change `POLL_INTERVAL_MS` from 100 to 16

**Rationale**: 16ms corresponds to ~62.5 Hz, slightly above CAVA's 60 fps output. This ensures the poll loop wakes up frequently enough to drain frames as CAVA produces them, while still blocking in the kernel when the FIFO is empty (no busy-waiting).

**Alternatives considered**:
- *Set to 0 (non-blocking poll)*: Would turn the loop into a busy-wait when the FIFO is empty, wasting CPU. Rejected.
- *Set to 8ms (120 Hz)*: Overkill; 2x CAVA's rate gains nothing while doubling wake-ups. Rejected.
- *Use epoll/kqueue*: Too heavy for a single-FIFO use case. No measurable benefit over `poll()`. Rejected.
- *Use `tokio`/async*: Would require significant refactoring of the worker thread. Out of scope.

### Decision 2: Keep `FRAME_QUEUE_CAPACITY = 2`

**Rationale**: The capacity of 2 already supports latest-wins behavior: CAVA can produce into one slot while the consumer drains the other. `take_latest_frame()` drains ALL available frames and returns only the last one. Increasing capacity would only waste memory without improving frame freshness.

### Decision 3: Keep the 16ms idle sleep in the daemon reader thread

**Rationale**: The `thread::sleep(16ms)` at `daemon_core.rs:114` only fires when `take_latest_frame()` returns `Ok(None)` — i.e., when NO frame is available. This is the idle case, not the busy case. When frames are flowing at 60 fps, the reader loop will call `take_latest_frame()` frequently and the sleep will almost never be hit. Removing it would cause busy-waiting when CAVA is not producing data (e.g., no audio playing).

### Decision 4: No frame-rate telemetry instrumentation needed

**Rationale**: The improvement can be validated subjectively (smoother visualization) and with simple timestamp logging during development. Formal telemetry is out of scope for this minimal change.

## Risks / Trade-offs

- **[Risk] Increased CPU usage from more frequent poll wake-ups** → Mitigation: `poll()` with a non-zero timeout is still a blocking syscall. When the FIFO is empty, the kernel puts the thread to sleep. The incremental cost is one additional wake-up every 100ms vs. every 16ms (6.25x more wake-ups, but from a baseline of ~10 wake-ups/second to ~62.5 — negligible in absolute terms on any modern system).
- **[Risk] Frame bursts could still cause drops with capacity=2** → Mitigation: This is by design — latest-wins means older frames are intentionally discarded. A burst that fills the channel will only keep the most recent frame.
