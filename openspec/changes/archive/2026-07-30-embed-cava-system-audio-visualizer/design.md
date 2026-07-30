## Context

mbv's supported mpv integration does not expose decoded PCM frames to the application. CAVA already provides maintained audio capture and spectrum analysis, while mbv owns the Ratatui surface where the visualization is shown. The previous design attempted to create an mbv-specific Pulse route and loopback; that added playback risk without being necessary once system-wide audio is accepted.

## Goals / Non-Goals

**Goals:**

- Run CAVA only while the local embedded visualizer is active.
- Let CAVA select its normal default system audio input.
- Transfer bounded spectrum frames from CAVA to mbv and render the newest frame.
- Preserve mpv's existing output, device, volume, and playback configuration.
- Keep startup failure and child termination non-fatal to playback.

**Non-Goals:**

- Per-application or mbv-only audio isolation.
- Creating, changing, or destroying PulseAudio/PipeWire sinks, sources, links, or loopbacks.
- Setting or restoring mpv `ao` or `audio-device` properties.
- Implementing FFT or audio capture in Rust.
- Supporting remote players or audio-pipe playback through this visualizer.

## Decisions

- **Use CAVA as a child process.** CAVA owns audio capture and spectrum analysis; mbv supervises its lifecycle and consumes a machine-readable frame stream.
- **Use CAVA's default input selection.** The generated configuration selects the Pulse input method without naming a source, allowing CAVA to follow the system default audio monitor/source. This avoids all graph manipulation and makes the accepted system-wide behavior explicit.
- **Keep the existing mbv renderer.** CAVA supplies normalized bar levels; mbv draws them inside its existing embedded visualizer area so the result remains compatible with Ratatui layout and input handling.
- **Use a private raw-output transport.** CAVA's terminal output is intended for a standalone terminal and contains control sequences. A private FIFO or equivalent bounded transport carries only fixed-width raw frames to mbv; no CAVA terminal UI is written directly into the Ratatui buffer.
- **Make enablement playback-neutral.** Starting or stopping CAVA does not modify mpv properties or load audio modules. If CAVA is unavailable or fails, mbv leaves playback running and renders inactive bars.
- **Scope lifecycle to local playback.** The player creates the worker only for supported local playback with the visualizer enabled. Existing audio-pipe and remote-player paths remain unchanged.

## Risks / Trade-offs

- [System audio includes unrelated applications] -> Document this as intentional behavior and do not claim per-player visualization.
- [CAVA is unavailable or its default source cannot be opened] -> Log the failure, keep playback unchanged, and leave the visualizer inactive.
- [CAVA terminal output is not safe to embed directly] -> Consume only bounded raw frames and keep rendering in mbv.
- [CAVA child or reader hangs during shutdown] -> Use supervised termination, bounded joins, and cleanup of the private transport.
- [System default audio changes while CAVA is active] -> Follow CAVA's normal default-source behavior; device-following policy is delegated to CAVA rather than reimplemented in mbv.

## Migration Plan

1. Remove the abandoned dedicated-route implementation and its runtime state, Pulse control, mpv routing, and cleanup paths.
2. Add the CAVA worker and embedded rendering using the default system input.
3. Add the `cava` runtime dependency and focused lifecycle/frame tests.
4. Verify playback is unchanged with the visualizer disabled and enabled, including concurrent unrelated system audio.

Rollback is limited to disabling the visualizer feature or removing the CAVA worker; because no playback routing is changed, rollback does not require restoring audio graph state.

## Open Questions

- Which CAVA raw frame size and refresh rate best match the existing embedded panel without unnecessary CPU use?
- Should CAVA's default input follow runtime default-device changes automatically on all supported Pulse-compatible hosts, or should the first release document setup-time behavior?
