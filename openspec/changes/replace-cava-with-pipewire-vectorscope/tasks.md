## 1. Dependencies And Source Types

- [x] 1.1 Add the direct `pipewire-rs` and single-cell glyph validation dependencies, plus the required PipeWire system packages in CI and release packaging.
- [x] 1.2 Replace Cava-specific core types with source-of-truth stereo sample, generation, bounded overwrite-buffer, startup, and failure types.
- [x] 1.3 Add one focused buffer check proving producer overflow retains the newest complete stereo window and never returns partial channel pairs.

## 2. PipeWire Capture Worker

- [x] 2.1 Implement a dedicated PipeWire main-loop worker that auto-connects a converted interleaved stereo capture stream to the current default sink output.
- [x] 2.2 Copy complete sample pairs into the bounded buffer without blocking the capture callback, and publish capture generations for freshness measurement.
- [x] 2.3 Implement startup reporting, stream-state failure reporting, stop, disconnect, thread join, and drop cleanup without changing PipeWire graph objects or mpv properties.
- [x] 2.4 Add the smallest useful worker lifecycle checks for bounded shutdown and unavailable-PipeWire failure, reusing or replacing existing Cava lifecycle tests rather than duplicating them.

## 3. App Lifecycle And Configuration

- [x] 3.1 Replace `CavaWorker` app state and synchronization with the PipeWire worker while preserving local, same-host Local daemon, attached-session, audio-pipe, playback-end, and toggle eligibility rules while permitting Direct remote playback forwarded into local system output.
- [x] 3.2 Add the persisted single-cell vectorscope glyph with default `●`, invalid-value fallback, and a focused config round-trip/fallback test.
- [x] 3.3 Clear vectorscope samples and isolate capture failure for the current playback without interrupting playback, input handling, or subsequent playback retries.

## 4. Vectorscope Rendering And Cadence

- [x] 4.1 Replace spectrum-bar rendering with independent stereo coordinate mapping in the existing panel, deduplicating cells and suppressing the center point for a silent window.
- [x] 4.2 Render points with the configured glyph and aqua, foam, yellow, and red amplitude bands from the existing palette while preserving empty-area and constrained-height behavior.
- [x] 4.3 Target a 16 ms active-visualizer cadence and snapshot the newest sample generation immediately before each draw rather than replaying visual frames.
- [x] 4.4 Add one non-brittle coordinate-mapping check covering channel orientation, clamping, silence, and panel bounds; verify final appearance directly in a real terminal.
- [x] 4.5 Apply the fixed internal display gain before coordinate clamping so typical PCM levels produce useful visual spread without changing captured samples or adding UI configuration.

## 5. Remove Cava

- [x] 5.1 Delete Cava configuration, FIFO/parser, child supervision, frame-channel code, tests, logs, and now-unused Unix process/resource dependencies.
- [x] 5.2 Remove Cava from Cargo package metadata, `PKGBUILD`, CI provisioning, README instructions, and current ADR/spec references; document PipeWire-only support and glyph configuration.
- [x] 5.3 Confirm no shipped source, package metadata, or current documentation still requires or invokes `cava`.

## 6. End-To-End Verification

- [x] 6.1 Verify with real PipeWire logs that bare playback captures the default sink output, unrelated output can contribute, toggle-off releases capture, and PipeWire failure leaves playback active.
- [x] 6.2 Verify same-host Local daemon playback works without daemon/protocol changes, Direct remote playback can display locally forwarded audio, and attached-session and audio-pipe playback do not start local capture.
- [x] 6.3 Measure at the terminal render boundary under steady audio and record at least 50 fresh vectorscope frames per second on a terminal capable of the 60 FPS target, including recovery to newest samples after an induced UI stall.
- [x] 6.4 Run targeted mbv and mbv-core tests, package checks, workspace clippy, formatting, and the governed source-file line check.
