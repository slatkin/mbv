## 1. Configuration And Terminology

- [ ] 1.1 Add the canonical `Clocked audio output` term to `CONTEXT.md`, distinguishing device-paced output from legacy PCM pipe output without renaming an existing term.
- [ ] 1.2 Add validated owner-local `audio_device` configuration at the source-of-truth config type, accepting only `alsa` or `alsa/...`, with packaged `mbvd` inheriting `alsa` while bare mode and the Local daemon retain their current defaults.
- [ ] 1.3 Load restart-required `audio_device` with owner-local daemon configuration, capture the resolved output choice with `audio_pipe_enabled` at the existing next-Playback-run boundary, and avoid daemon-settings or protocol changes.

## 2. Player Output Selection

- [ ] 2.1 Project the captured ALSA identifier through mpv's `audio-device` property for packaged-daemon clocked output without also forcing `ao`.
- [ ] 2.2 Keep the existing `ao=pcm`, FIFO creation, PCM format, startup guard, latency estimate, and diagnostics together behind `audio_pipe_enabled = true`; ensure pipe-only state is absent from the ALSA branch.
- [ ] 2.3 Propagate failure to open the selected ALSA endpoint as the Playback run's output error without falling back to another device or the legacy pipe.

## 3. Regression Checks

- [ ] 3.1 Extend the closest config tests with one table covering the packaged-daemon ALSA default, exact ALSA device selection, invalid identifiers, and unchanged bare-mode/Local-daemon defaults.
- [ ] 3.2 Extend the closest player-runtime test to prove mutually exclusive mpv projection: ALSA mode sets only the selected device and bypasses pipe startup state, while explicit pipe mode preserves its current properties and ignores `audio_device`.
- [ ] 3.3 Add one deterministic command/event regression check showing that ALSA-mode startup, pause, and resume do not enter the pipe guard; do not add wall-clock timing assertions to CI.

## 4. Operations And Verification

- [ ] 4.1 Document `mbvd/libmpv -> ALSA playback endpoint -> paired capture endpoint -> Snapserver`, including host/LXC device provisioning, permissions, sample-format agreement, downstream-buffer ownership, migration, and rollback.
- [ ] 4.2 Update packaged configuration examples to show inherited `audio_device = "alsa"`, explicit loopback selection, and `audio_pipe_enabled = true` as the legacy path without removing existing pipe keys.
- [ ] 4.3 Run `cargo nextest run -p mbv-core` and the affected `mbvd` package tests, `cargo check` for both packages, workspace Clippy, and the code-file line-limit check through the repository's `rtk` commands.
- [ ] 4.4 On a host with a writable ALSA endpoint, obtain explicit listener readiness before sending controls, then record synchronized startup, pause, and resume Player-boundary timings separately from audible and downstream Snapcast timing.
