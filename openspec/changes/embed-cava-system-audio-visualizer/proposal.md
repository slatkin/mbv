## Why

mbv needs an audio visualizer, but mpv does not expose decoded PCM through its supported client API. Per-player Pulse/PipeWire rerouting adds substantial complexity and has already caused playback and shutdown problems. Using CAVA with its normal system-audio input provides a small, supportable path to an embedded visualizer.

## What Changes

- Add an embedded visualizer driven by a supervised CAVA process.
- Capture the default system audio monitor/source used by CAVA.
- Display CAVA's normal spectrum output inside mbv's existing visualizer area.
- Start and stop CAVA with the local visualizer lifecycle.
- Remove all mbv-owned Pulse/PipeWire sinks, loopbacks, module cleanup, runtime routing records, and mpv audio-device overrides.
- Keep playback configuration unchanged when the visualizer is enabled or disabled.
- **BREAKING**: The visualizer represents all audio present on the system default audio path, including audio from applications other than mbv.

## Capabilities

### New Capabilities

- `system-audio-visualizer`: Provides an embedded CAVA-backed spectrum visualization sourced from the system default audio path.

### Modified Capabilities

- None.

## Impact

- Affected mbv-core player lifecycle and visualizer rendering code.
- CAVA becomes a runtime package dependency for supported local playback environments.
- No Pulse/PipeWire control API, virtual audio modules, mpv output switching, or routing-state persistence is required.
- Existing remote playback and audio-pipe modes remain outside the visualizer's scope.
