## 1. Remove the abandoned routing design

- [ ] 1.1 Remove dedicated Pulse/PipeWire route creation, module loading/unloading, runtime routing records, and stale-route recovery from the visualizer implementation.
- [ ] 1.2 Remove mpv `ao` and `audio-device` override, rollback, and restoration logic from visualizer lifecycle code.
- [ ] 1.3 Update package metadata, ADRs, and change documentation to remove routing requirements and document intentional system-audio capture.

## 2. Implement the CAVA worker

- [ ] 2.1 Add a supervised CAVA worker that uses CAVA's default system-audio input without naming or creating a Pulse/PipeWire source.
- [ ] 2.2 Add a private bounded raw-frame transport and parser that publishes only complete, normalized spectrum frames for rendering.
- [ ] 2.3 Implement startup readiness, unexpected-exit diagnostics, bounded shutdown, child reaping, and temporary-resource cleanup.
- [ ] 2.4 Add the `cava` runtime dependency to supported package and CI metadata.

## 3. Integrate and render

- [ ] 3.1 Start and stop the CAVA worker with supported local visualizer lifecycle events while leaving mpv playback properties unchanged.
- [ ] 3.2 Keep remote-player and audio-pipe paths from starting the local CAVA worker.
- [ ] 3.3 Render the newest CAVA spectrum frame in the existing embedded Ratatui visualizer area and clear bars when inactive.
- [ ] 3.4 Preserve normal playback and input handling when CAVA is unavailable or fails.

## 4. Verify behavior

- [ ] 4.1 Add focused tests for default-input CAVA configuration, bounded frame parsing, normalization, malformed frames, and worker cleanup.
- [ ] 4.2 Run formatting, focused tests, Clippy, and the relevant build/package checks in the isolated worktree.
- [ ] 4.3 Manually verify visualizer startup and shutdown with local playback, concurrent unrelated system audio, CAVA failure, and unchanged mpv audio configuration.
