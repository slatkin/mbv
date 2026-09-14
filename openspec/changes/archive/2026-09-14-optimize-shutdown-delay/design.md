## Context

See proposal.md. The shutdown path is:

1. `App::teardown` → `stop_visualizer_worker()` (blocking, up to 500ms)
2. `player.stop_for_shutdown(quit_timeout)` → signals the player thread
3. Player run loop: sends `mpv quit`, waits up to **2s** for `Shutdown` event
   (`player_run_run.rs:97`), then runs `report_stop_now_or_background`
4. `report_stop_now_or_background` during shutdown:
   - `ProgressGuard::stop_and_join` (budget: quit_timeout/2 = 2.5s)
   - `report_stopped_for_shutdown`: WS flush (up to 1s) + HTTP POST
5. `App::teardown` → `player.join_or_timeout(outer_bound)`

Steps 1 and 3 are the dominant contributors to the ~2s wall time.

## Goals / Non-Goals

**Goals:**
- Reduce quit-to-exit latency to roughly one HTTP round-trip (~200ms–1s)
- Preserve the Emby Stopped report (position saved, item no longer "playing")

**Non-Goals:**
- Changing `quit_timeout_secs` config semantics
- Altering the non-shutdown (track transition) path

## Decisions

### D1: Reduce the mpv quit fallback from 2s to 200ms

The 2s fallback at `player_run_run.rs:97` exists because mpv may not deliver
`EndFile`/`Shutdown` promptly after `quit`. But 2s is generous — mpv's own quit
processing is sub-100ms when not blocked on output (the FIFO case already uses
async quit). 200ms gives mpv adequate time for the common case while cutting
the worst-case fallback by 90%.

Alternative: 0ms (skip the wait entirely). Rejected because the clean
`on_shutdown` path handles `EndFile` → `Shutdown` event ordering correctly and
is worth giving a brief window to fire.

### D2: Skip ProgressGuard::stop_and_join during shutdown

`report_stop_now_or_background` currently calls `progress.stop_and_join()`
synchronously during shutdown before sending the Stopped HTTP report. This
waits for the progress-reporting thread to finish its last tick.

The join exists to prevent a stale progress ping from arriving at Emby *after*
the Stopped report. But Emby discards progress reports for sessions that have
already stopped — the Stopped report is authoritative. So the ordering
guarantee is unnecessary.

Change: during shutdown (`is_quit_shutdown()`), send the stop signal
(`stop_tx.send(())`) but don't join. The progress thread will drain on its own
or be abandoned when the process exits moments later.

### D3: Drop WS flush before Stopped report

`report_stopped_for_shutdown` calls `tx.flush(timeout.min(1s))` to drain
pending WebSocket messages before the HTTP Stopped call. These messages are
progress pings — purely informational, not authoritative. The HTTP Stopped
report supersedes them.

Change: remove the `ws_tx.flush()` call from `report_stopped_for_shutdown`.

### D4: Parallelize visualizer worker join with player shutdown

`stop_visualizer_worker()` blocks in `teardown` before
`player.stop_for_shutdown()` starts. The visualizer and player have no
dependency — they can shut down concurrently.

Approach: extract the visualizer worker handle from `self.visualizer` before
teardown, start the player shutdown, then join the visualizer handle while the
player thread is running. Since `stop_visualizer_worker` takes `&mut self`
(mutating `self.visualizer` and `self.visualizer_window`), extract the worker
via `self.visualizer.take()`, send its stop signal immediately, then join it
after `player.stop_for_shutdown()` but before (or during)
`player.join_or_timeout()`.

### D5: Update outer_bound formula

With D1–D3, the player thread's internal shutdown budget shrinks:
- No progress join wait
- No WS flush wait
- Stopped HTTP call: still bounded by `quit_timeout`
- mpv quit fallback: 200ms instead of 2s

The `outer_bound` in `teardown` should be updated to
`quit_timeout + Duration::from_millis(200) + Duration::from_secs(1)`
(HTTP budget + mpv fallback + cushion). The existing test must be updated
to match.

## Risks / Trade-offs

- **Stale progress ping after Stopped (D2, D3):** Emby ignores progress for
  stopped sessions, so this is harmless. If a future Emby version changes this
  behavior, the worst case is a brief "still playing" ghost that clears on the
  next session poll. → Acceptable; revisit if Emby behavior changes.

- **mpv quit not delivered within 200ms (D1):** Falls through to the forced
  stop path, which already handles this correctly (reports stopped, emits
  `PlayerEvent::Stopped`). The only difference is that `on_shutdown`'s
  `EndFile`-based near-end detection won't fire — but the fallback has its own
  near-end logic (`quit_timeout_stop_flags`). → No regression.

- **Visualizer join racing player teardown (D4):** No shared state between
  visualizer and player — the visualizer reads PipeWire, the player reads mpv.
  → No race.
