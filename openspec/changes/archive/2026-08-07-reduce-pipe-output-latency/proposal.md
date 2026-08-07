# Reduce pipe-output latency

## Problem

Playing audio through the FIFO pipe path (mbvd → mpv ao=pcm → snapfifo →
snapserver) has ~2.5 second latency on track changes and ~1 second latency on
pause/unpause. Standalone mpv writing to the same pipe has <0.5 second latency.
The gap is in mbvd's orchestration, not mpv or snapcast.

Confirmed by testing: `mpv <file> --ao=pcm --ao-pcm-file=/tmp/mbv-pipe ...`
produces near-instant response; the same operation through mbvd does not.

## Root causes

Two independent problems compound:

### 1. Polling loops add up to 750ms worst case

The player session event loop blocks for up to 500ms on `mpv.wait_event(0.5)`
before checking the command channel (`cmd_rx.try_recv()`). The daemon event loop
blocks for up to 250ms on `merged_rx.recv_timeout(250ms)`. A command from the
TUI passes through both:

```
TUI → TCP → daemon recv_timeout(250ms) → player cmd_rx → wait_event(500ms) → mpv
             avg ~125ms                    avg ~250ms
```

Average polling latency: ~375ms. Worst case: 750ms. This affects **every**
command (pause, next, prev, seek).

### 2. Blocking Emby API calls on the player thread

On every track transition (including the `LoadNew` reuse path), the player
thread runs `transition_to()` synchronously before issuing `loadfile`:

1. `ws_tx.flush(Duration::from_secs(1))` — up to 1 second websocket flush
2. `report_stopped()` — HTTP POST to Emby (stop old item)
3. `get_playback_info()` — HTTP POST to Emby (get new item session)
4. `report_start()` — HTTP POST to Emby (start new item)

Each HTTP call is 100–300ms. On error, `report_start` retries after a 500ms
sleep. All of this runs **before** `loadfile`, blocking the pipe from receiving
new PCM.

On the cold-start path (`play_queue`), `get_playback_info()` and
`report_start()` also run on the player thread between `loadfile` and the start
of `session.run()`, delaying event loop startup.

## Proposed fix

### Fix 1: Eliminate polling latency

Replace the `wait_event(0.5)` + `try_recv()` polling pattern with an
event-driven wakeup.

`libmpv2::Mpv` exposes `set_wakeup_callback()` which calls
`mpv_set_wakeup_callback` — a thread-safe hook that fires whenever mpv has a
pending event. Use this together with a signaling mechanism (eventfd or a
pipe-to-self) so the player loop can `poll(2)` or `select` on both the mpv
wakeup fd and the command channel simultaneously, waking immediately on
whichever fires first.

For the daemon loop: `merged_rx` is an `mpsc::Receiver` which doesn't support
`recv_timeout` shorter than its current 250ms without busy-waiting. Switch to
`crossbeam_channel::select!` with the merged channel, or use an eventfd/condvar
to wake the loop immediately when a ctrl command arrives. Since ctrl commands
already send on `merged_tx` directly, removing the 250ms timeout and using a
blocking `recv()` (woken by any event) may be sufficient — the timeout is only
there to poll `settle_buffering_if_due()`, which can be driven by a separate
timer channel instead.

### Fix 2: Move Emby reporting off the player thread

The Emby API calls (`report_stopped`, `get_playback_info`, `report_start`) are
bookkeeping — they don't affect whether audio can play. Move them off the
critical path:

- **`report_stopped`**: fire-and-forget on a background thread (it already is
  in the `retry_mark_played` pattern). Remove the `ws_tx.flush(1s)` from the
  synchronous path.
- **`get_playback_info` + `report_start`**: these produce a `PlaybackInfo`
  (session_id, media_source_id, ext_sub_urls) that the `SessionReporter` needs.
  Start them in a background thread and let the session begin without them. The
  reporter can use a placeholder session ID initially and swap in the real one
  when the background call completes.

On the cold-start path (`play_queue`), move the `get_playback_info()` +
`report_start()` calls after `session.run()` starts, or run them in parallel
with mpv initialization.

## Expected impact

| Source                     | Before        | After     |
|----------------------------|---------------|-----------|
| Daemon poll wait           | 0–250ms       | ~0ms      |
| Player poll wait           | 0–500ms       | ~0ms      |
| WS flush                   | 0–1000ms      | 0ms       |
| report_stopped             | 100–300ms     | 0ms (bg)  |
| get_playback_info          | 100–300ms     | 0ms (bg)  |
| report_start               | 100–300ms     | 0ms (bg)  |
| **Total removed**          | **~375–2500ms** | —       |

Remaining latency after fix: mpv audio decode + pipe write + snapcast buffer —
matching standalone mpv behavior.

## Scope

- `player_session_run.rs` — event loop wakeup
- `player_runtime.rs` — background Emby reporting in `transition_to`,
  `start_item`
- `player_runtime_controller.rs` — background reporting on cold-start path
- `daemon_run.rs` — daemon loop recv strategy
- Pipe intent / playout delay machinery (#442) is **not** in scope — it can be
  evaluated independently once the base latency is resolved.

## Non-goals

- Keeping mpv alive between sessions (helps cold start but doesn't address
  the dominant causes found here)
- Snapserver/snapclient tuning (confirmed not the bottleneck)
- Changing the pipe ownership model (snapserver `mode=read` works)
