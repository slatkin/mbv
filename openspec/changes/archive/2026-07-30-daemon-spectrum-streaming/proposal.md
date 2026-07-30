## Why

The embedded CAVA visualizer currently only works for local playback (mbv + mpv on the same machine). When mbv connects to mbvd for remote playback, the audio plays on the daemon's machine, so the client has no system audio to capture. Extending the visualizer to mbv-to-mbvd connections requires the daemon to run CAVA and stream spectrum frames to the client over the existing ctrl socket protocol.

## What Changes

- Extract the CAVA worker from `src/app/visualizer.rs` into `crates/mbv-core/src/visualizer.rs` so both mbv and mbvd can use it.
- Add a `spectrum-streaming` capability to the ctrl protocol handshake for feature advertisement.
- Add protocol support for spectrum streaming: `CtrlCmd::StartSpectrum`, `CtrlCmd::StopSpectrum`, `CtrlEvent::Spectrum`, and `CtrlEvent::SpectrumFailed`.
- mbvd starts CAVA when it receives `StartSpectrum` and streams spectrum frames to the connected client via a dedicated reader thread.
- mbvd auto-stops CAVA when playback fully stops (player becomes inactive — not pause). mbvd also stops CAVA on client disconnect.
- mbv receives spectrum frames via `RemotePlayer` and renders them using the existing renderer (source-agnostic).
- The visualizer toggle (V key) branches on connection type: local CAVA for standalone playback, `StartSpectrum`/`StopSpectrum` for daemon playback. `StopSpectrum` is also sent on session switch and teardown.

## Capabilities

### New Capabilities

- `daemon-spectrum-streaming`: mbvd captures system audio via CAVA and streams normalized spectrum frames to connected mbv clients over the ctrl protocol.

### Modified Capabilities

- `system-audio-visualizer`: Extended to accept spectrum frames from either a local CAVA worker (existing) or a remote daemon (new). The renderer is unchanged.

## Impact

- mbv-core gains a shared CAVA worker module; mbv and mbvd both depend on it.
- The ctrl protocol gains a new capability (`spectrum-streaming`) and four new message variants.
- mbvd's event loop gains a spectrum reader thread and lifecycle management tied to playback state and client connections.
- mbv's visualizer orchestration branches on connection type (local vs daemon).
- `RemotePlayer` gains a `send_ctrl_cmd` method for non-player commands.
- CAVA becomes a runtime dependency for mbvd environments where the visualizer is desired.
- No changes to the render code, render cadence, or visualizer UI.

## Non-Goals

- Broadcasting spectrum to multiple concurrent clients (pending #395: multi-connection support).
- Supporting Emby Sessions remote playback (different architecture, no ctrl socket).
- Running CAVA on headless daemons with no audio output (garbage in, garbage out).
- Changing the visualizer appearance or render pipeline.
- Auto-stopping CAVA on pause (only on full stop — paused audio is still on the system bus).
