## Why

mbv's Cava integration adds child-process, FIFO, parsing, and frame-queue machinery without matching standalone Cava's smooth visual cadence. Replacing that pipeline with a focused in-process vectorscope gives mbv direct control over capture, freshness, and rendering while keeping visualization isolated from playback.

GitHub issue: [#587](https://github.com/slatkin/mbv/issues/587)

## What Changes

- Replace the supervised Cava worker and normalized spectrum frames with in-process stereo PCM capture from the default PipeWire system-output monitor.
- Render a stereo vectorscope, modeled on ncmpcpp's ellipse behavior but implemented independently under mbv's MIT license, in the existing visualizer panel.
- Add one persisted configurable Unicode glyph for vectorscope points, defaulting to `●`.
- Preserve the `v` toggle, playback isolation, and system-audio semantics, including playback heard locally through a same-host Local daemon or local forwarding from a Direct remote Player owner.
- Remove Cava configuration generation, private FIFO transport, ASCII frame parsing, lifecycle handling, documentation, and package recommendations.
- **BREAKING**: visualizer support becomes PipeWire-only and no longer falls back to the `cava` executable or PulseAudio compatibility input.
- Exclude FFT/spectrum analysis, additional visualization modes, mode cycling, mpv audio routing, and daemon/control-protocol changes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `system-audio-visualizer`: Replace Cava spectrum behavior and failure/resource requirements with native PipeWire capture, latest-sample stereo vectorscope rendering, and configurable point glyph behavior.

## Impact

- Core visualizer source-of-truth types and runtime ownership in `crates/mbv-core/`.
- App visualizer lifecycle, frame synchronization, preferences, and Ratatui rendering in `src/app/`.
- Linux build dependencies and release packaging: remove Cava and add the selected PipeWire development/runtime requirements.
- Existing visualizer specifications, ADR references, README text, tests, and CI provisioning.
- No changes to mpv output, the system audio graph, Local daemon behavior, shared data, or the ctrl protocol.
