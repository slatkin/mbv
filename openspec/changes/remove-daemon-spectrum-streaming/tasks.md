## 1. Remove daemon spectrum runtime

- [ ] 1.1 Remove FIFO CAVA input, `SpectrumSnapclient`, daemon spectrum prerequisite checks, and their focused tests while retaining the local system-input CAVA worker.
- [ ] 1.2 Remove `SpectrumState`, spectrum daemon events, daemon control handlers, capability advertisement, and playback/disconnect/shutdown spectrum lifecycle handling.
- [ ] 1.3 Remove spectrum ctrl commands/events, compatibility state, remote-player conversion, player events, and related serialization/daemon tests.

## 2. Make visualization local-only

- [ ] 2.1 Simplify the application visualizer flow so only local playback can start CAVA and daemon-connected UI does not offer remote spectrum control.
- [ ] 2.2 Remove dedicated spectrum configuration fields, defaults, validation/save/login propagation, and tests; preserve unrelated audio-pipe settings and behavior.
- [ ] 2.3 Add or update focused tests proving local visualization remains available and daemon mode neither starts nor requests spectrum processing.

## 3. Remove operational surface

- [ ] 3.1 Remove headless/daemon spectrum setup guidance from README and retain the local CAVA runtime guidance.
- [ ] 3.2 Supersede the daemon-spectrum ADR with a local-only visualizer decision, preserving the historical record rather than leaving obsolete operational guidance as current.
- [ ] 3.3 Record that `improve-daemon-spectrum-framerate` is superseded and remove its temporary instrumentation as part of the deleted daemon path.

## 4. Verify removal

- [ ] 4.1 Run formatting, build, and the project test suite.
- [ ] 4.2 Verify a daemon starts without `snapclient` and does not spawn dedicated visualization CAVA/Snapclient/FIFO processes during playback.
- [ ] 4.3 Manually verify local playback still renders the CAVA visualizer and daemon-connected playback leaves the visualizer unavailable.
