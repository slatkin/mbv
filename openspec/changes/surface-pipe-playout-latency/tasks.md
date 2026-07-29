## 1. Prerequisite and Configuration

- [x] 1.1 Confirm `reliable-daemon-playback-intents` is applied and use its request lifecycle and generation model.
- [x] 1.2 Add an optional nonnegative generic downstream playout-delay setting beside existing audio-pipe configuration, preserving behavior when absent.
- [x] 1.3 Extend existing configuration fixtures and user-facing documentation for the estimate and ownership limits.

## 2. Pipe Startup Phase Model

- [x] 2.1 Add request-correlated Resolving, PlayerOpening, OutputStarted, and OutputBuffering status data without downstream-specific types or dependencies.
- [x] 2.2 Derive phases from existing daemon/player transitions and document the concrete event used for OutputStarted.
- [x] 2.3 When configured, create a generation-bound buffering deadline and report approximate remaining duration.
- [x] 2.4 When absent, settle at OutputStarted and report downstream delay as unknown without an indefinite guard.
- [x] 2.5 Re-check connection identity and generation before a deadline settles.
- [x] 2.6 Extend same-target coalescing through buffering while preserving target supersession, Stop priority, and post-settlement restart.

## 3. Visibility and Diagnostics

- [x] 3.1 Render direct-daemon pipe phases and estimated remaining delay using explicitly estimated wording.
- [x] 3.2 Keep phase UI absent for local playback, attached Emby sessions, and non-pipe daemon output.
- [x] 3.3 Log phase transitions and terminal outcomes with request ID, generation, and monotonic elapsed milliseconds without contacting the pipe consumer.

## 4. Documentation and Verification

- [x] 4.1 Document manual calibration, estimate drift, observed-versus-estimated boundaries, and explicit exclusion of downstream control.
- [x] 4.2 Update existing coverage for configured, unconfigured, superseded, stopped, disconnected, and stale-deadline scenarios.
- [x] 4.3 Exercise direct pipe output with and without an estimate, confirming honest wording, duplicate guarding, Stop priority, target supersession, and normal same-item restart.
- [x] 4.4 Run formatting, relevant existing tests, and clippy for touched crates; record unrelated failures separately.
