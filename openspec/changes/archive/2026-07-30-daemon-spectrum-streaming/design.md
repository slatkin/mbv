## Context

The phase 1 visualizer (`embed-cava-system-audio-visualizer`) runs CAVA inside the mbv process for local playback. The CAVA worker (`CavaWorker`) is self-contained: it spawns the cava child, manages a private FIFO, parses raw frames, and exposes `start()`, `take_latest_frame()`, and `stop()`. It currently lives in `src/app/visualizer.rs` alongside `App` orchestration methods.

For mbv-to-mbvd connections, audio plays on the daemon's machine. The client has no system audio to capture. The daemon already runs the mpv player and has access to the system audio output, so it is the natural host for CAVA. The existing ctrl socket protocol already carries bidirectional commands and events between mbv and mbvd; spectrum frames fit naturally into this channel.

## Goals / Non-Goals

**Goals:**

- Extract the CAVA worker to mbv-core so both mbv and mbvd can use it.
- Add a `spectrum-streaming` capability to the ctrl protocol handshake for feature advertisement.
- Add ctrl protocol messages for spectrum start/stop/streaming/failure.
- mbvd starts CAVA on `StartSpectrum`, streams frames via a dedicated reader thread, auto-stops on full playback stop (not pause).
- mbvd stops CAVA on client disconnect to prevent orphaned workers.
- mbv sends `StopSpectrum` on session switch and teardown to cleanly stop daemon-side CAVA.
- mbv receives daemon spectrum frames and feeds them into the existing renderer.
- The visualizer toggle branches on connection type (local vs daemon).
- Keep render code, render cadence, and visualizer UI unchanged.

**Non-Goals:**

- Broadcasting spectrum to multiple concurrent clients (pending #395).
- Supporting Emby Sessions remote playback.
- Handling headless daemons with no audio output.
- Changing the visualizer appearance.

## Decisions

- **Extract CavaWorker to mbv-core.** The worker is already self-contained (no App dependencies). Moving it to `crates/mbv-core/src/visualizer.rs` lets mbvd link against it without duplication. Visibility modifiers change from `pub(super)` to `pub` to expose the API across crates.
- **Reuse the ctrl socket for spectrum frames.** The CavaWorker's FIFO reader produces ~6 frames/sec (bounded by `POLL_INTERVAL_MS = 100ms` and `FRAME_QUEUE_CAPACITY = 2`). Each `CtrlEvent::Spectrum` JSON message is ~400–500 bytes (64 floats × ~7 chars each + framing). Total bandwidth is ~2–3KB/s — negligible compared to the existing state broadcasts. No separate channel needed.
- **JSON encoding for spectrum frames.** Consistent with all other ctrl protocol messages. Binary encoding would save bandwidth but adds complexity for negligible gain at this scale.
- **Client enables spectrum explicitly.** mbv sends `StartSpectrum` when the user toggles the visualizer on while connected to a daemon. mbvd does not auto-start CAVA on every playback.
- **Daemon auto-stops on full playback stop (not pause).** The trigger is `PlayerStatus.active` becoming `false` (i.e., a `Stopped` event), not a pause. Paused playback keeps CAVA running — audio is still on the system bus, and the bars show silence. A full stop (track ended, queue exhausted, user stopped) kills CAVA.
- **Daemon stops CAVA on client disconnect.** When the ctrl client disconnects (crash, network loss, session switch), the daemon stops the CAVA worker. CAVA without a consumer is wasted CPU and holds FIFO resources. This cleanup happens in the existing `CtrlDisconnected` handler.
- **Client sends StopSpectrum on connection-type transition.** When mbv switches from daemon mode to local mode (or to an Emby session) while the visualizer is active, `stop_visualizer_worker()` sends `StopSpectrum` to the daemon before tearing down the local state. This prevents orphaned CAVA workers. `PlayerProxy::send_ctrl_cmd` returns `false` for local players, so the call is safe without an explicit remote check.
- **Spectrum capability advertisement.** A new `CTRL_CAP_SPECTRUM = "spectrum-streaming"` capability is advertised by the daemon. The daemon appends this to capabilities after constructing the hello (not via `CtrlHello::current()`, which is shared with the client). The client stores this in a new `supports_spectrum: bool` field on `CtrlCompatibility`, populated during `perform_handshake`. The client checks this via `PlayerProxy::supports_spectrum()` before enabling the V key for daemon mode. The capability is always advertised by the daemon; if `cava` is not installed, the daemon still advertises it and relies on `SpectrumFailed` as the fallback.
- **Daemon spectrum thread model.** The daemon spawns a dedicated spectrum reader thread (similar to the existing CavaWorker thread model in mbv). This thread reads frames from the CAVA FIFO and sends them as `DaemonEvent::Spectrum(Vec<f32>)` back to the main event loop via a new `DaemonEvent` variant. The main loop then dispatches `CtrlEvent::Spectrum` to the connected client. This decouples spectrum frame rate from the main loop's 250ms poll timeout. Stale frames are acceptable — the client renders the latest frame only.
- **Daemon spectrum lifecycle state.** The daemon main loop holds a `SpectrumState { worker: Option<CavaWorker>, reader: Option<JoinHandle<()>> }` local variable. The `stop()` method is idempotent — it handles being called multiple times safely (e.g., from `StopSpectrum`, `CtrlDisconnected`, playback stop, and shutdown paths).
- **Auto-stop on playback stop.** The daemon's `DaemonEvent::Player(pe)` match arm includes an explicit check: when `pe` is `PlayerEvent::Stopped` (player `active` becomes `false`) and spectrum is active, stop CAVA. This is a specific check before the catch-all broadcast, not an implicit side effect.
- **Source-agnostic renderer.** The renderer reads `visualizer_frame: Vec<f32>`. Both local CAVA and daemon spectrum write to this same field via the same data path. No render code changes.
- **Spectrum frame data path on client.** Spectrum frames arrive via `CtrlEvent::Spectrum` in `apply_ctrl_event`, which cannot access `App`'s `visualizer_frame` field. Instead, `apply_ctrl_event` converts spectrum events into `PlayerEvent::Spectrum(Vec<f32>)` and `PlayerEvent::SpectrumFailed(String)` variants (consistent with existing `CommandRejected`/`RemoteDisconnected` pattern). The root crate's event loop handles these `PlayerEvent` variants and writes to `visualizer_frame`.
- **Protocol backward compatibility via deserialization failure.** New `CtrlCmd` and `CtrlEvent` variants are additive. With serde's default externally-tagged enum representation, unknown variants cause deserialization errors. On the daemon side, `daemon_core.rs` already wraps command parsing in `if let Ok(cmd) = serde_json::from_str::<CtrlCmd>(&line)`, so an older daemon silently drops unknown commands. On the client side, `remote_player.rs` logs a warning for unrecognized events and continues. The *effect* is benign — unknown messages are dropped — but the mechanism is deserialization failure, not silent ignoring. No protocol version bump required for additive changes.

## Architecture

```
mbv (client)                              mbvd (daemon)
┌──────────────┐                          ┌──────────────────────────────────┐
│              │                          │                                  │
│  visualizer  │                          │  mbv-core/visualizer             │
│  toggle (V)  │                          │  ┌──────────────┐                │
│      │       │                          │  │  CavaWorker   │               │
│      ▼       │                          │  │  (extracted)  │               │
│  sync_       │  ctrl socket             │  └──────┬───────┘                │
│  visualizer  │◀────────────────────────▶│         │                        │
│      │       │                          │         │ FIFO                   │
│      ├─ local?──▶ local CavaWorker      │         ▼                        │
│      │       │                          │      cava ──▶ system audio       │
│      └─ daemon?─▶ StartSpectrum ───────▶│         │                        │
│              │    capability check      │         │                        │
│              │    (spectrum-streaming)  │  ┌──────┴────────┐               │
│              │    Spectrum frames ◀─────┤  │ spectrum       │               │
│              │    StopSpectrum ─────────▶│  │ reader thread  │               │
│              │                          │  │                │               │
│  render      │                          │  └───────┬────────┘               │
│  visualizer_ │                          │          │ DaemonEvent::Spectrum   │
│  frame       │                          │          ▼                        │
│  (unchanged) │                          │  ┌──────────────┐                 │
│              │                          │  │ main event   │                 │
│  disconnect/ │                          │  │ loop          │                 │
│  switch ─────┼──▶ StopSpectrum ────────▶│  │              │                 │
│  session     │                          │  └──────┬───────┘                 │
│              │                          │         │                         │
│              │                          │  ┌──────┴────────┐                │
│              │                          │  │ CtrlDisconnected│               │
│              │                          │  │ → stop CAVA    │                │
│              │                          │  └────────────────┘                │
└──────────────┘                          └──────────────────────────────────┘
```

## Protocol Messages

### Capability Negotiation

- `CTRL_CAP_SPECTRUM = "spectrum-streaming"`: Added to `CtrlHello.capabilities`. Advertises that the daemon supports spectrum streaming. The client checks this before enabling the visualizer toggle for daemon connections.

### Client → Daemon (CtrlCmd)

- `StartSpectrum`: Request the daemon to start CAVA and stream spectrum frames.
- `StopSpectrum`: Request the daemon to stop CAVA.

### Daemon → Client (CtrlEvent)

- `Spectrum { bars: Vec<f32> }`: A normalized spectrum frame (64 values, 0.0–1.0).
- `SpectrumFailed { reason: String }`: CAVA failed to start or stopped unexpectedly.

### Internal (DaemonEvent — new variant)

- `Spectrum(Vec<f32>)`: Sent from the spectrum reader thread to the main event loop, carrying a frame from `CavaWorker::take_latest_frame()`.

## Risks / Trade-offs

- [CAVA not installed on daemon host] → Daemon sends `SpectrumFailed`, client renders inactive bars. Non-fatal to playback. Capability advertisement lets the client preemptively disable the toggle.
- [Spectrum frames add load to ctrl socket] → ~2–3KB/s at the actual ~6fps reader rate is negligible. If higher frame rates are desired later, `POLL_INTERVAL_MS` can be reduced.
- [Extracting CavaWorker to mbv-core adds complexity] → The worker is already self-contained; extraction is a file move with visibility changes and a `libc` dependency addition to mbv-core (already used elsewhere in the crate).
- [Protocol additions are additive but unversioned] → Unknown variants cause deserialization failures that are silently dropped by existing error-handling code. This is benign for additive changes but would need re-evaluation for breaking protocol changes.
- [Spectrum reader thread adds a new concurrent component to mbvd] → The thread is bounded: it's spawned on `StartSpectrum` and joined on `StopSpectrum`/disconnect/stop. Its only output is `DaemonEvent::Spectrum` messages on a bounded channel. Failure modes are the same as the existing CavaWorker in mbv.

## Migration Plan

1. Extract `CavaWorker` and related code from `src/app/visualizer.rs` to `crates/mbv-core/src/visualizer.rs`. Change visibility from `pub(super)` to `pub`. Gate the module with `#[cfg(unix)]` in mbv-core's `lib.rs`. Update mbv to import from mbv-core.
2. Add `CTRL_CAP_SPECTRUM` capability constant to `crates/mbv-core/src/ctrl.rs`. Add `supports_spectrum: bool` field to `CtrlCompatibility`. Add `CtrlCmd::StartSpectrum`/`StopSpectrum` and `CtrlEvent::Spectrum`/`SpectrumFailed` variants. Add `PlayerEvent::Spectrum(Vec<f32>)` and `PlayerEvent::SpectrumFailed(String)` variants. Add wire-stability serialization tests.
3. Add `send_ctrl_cmd(CtrlCmd) -> bool` method to `PlayerProxy` (returns `false` for local, forwards to `RemotePlayer` for remote). Daemon appends `spectrum-streaming` to capabilities after constructing `CtrlHello` (not via `current()`). Populate `CtrlCompatibility::supports_spectrum` during handshake.
4. Add `DaemonEvent::Spectrum` variant and spectrum reader thread to mbvd. Define `SpectrumState { worker: Option<CavaWorker>, reader: Option<JoinHandle<()>> }` in main loop. Handle `StartSpectrum`/`StopSpectrum`, stream frames, auto-stop on playback stop (explicit `Stopped` check), stop on client disconnect.
5. Update mbv's `sync_visualizer` to branch on connection type (check `supports_spectrum()`). Send `StopSpectrum` via `PlayerProxy::send_ctrl_cmd` on session switch, teardown, and connection-type transition.
6. Update `apply_ctrl_event` to handle `CtrlEvent::Spectrum`/`SpectrumFailed` by emitting `PlayerEvent::Spectrum`/`SpectrumFailed`. Handle these `PlayerEvent` variants in root crate's event loop to write to `visualizer_frame`.
7. Add focused tests for CavaWorker extraction, protocol serialization, daemon spectrum lifecycle, and client-side branching.
