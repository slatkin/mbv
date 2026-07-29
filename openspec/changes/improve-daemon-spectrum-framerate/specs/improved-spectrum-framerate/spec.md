## ADDED Requirements

### Requirement: Spectrum frames arrive at near-CAVA-rate
The daemon's spectrum reader thread SHALL deliver `DaemonEvent::Spectrum` messages at a rate that approaches 60 frames per second, matching CAVA's configured output framerate, with a tolerance for system scheduling jitter.

#### Scenario: Daemon receives frames at ~60 fps under load
- **WHEN** a spectrum streaming session is active and CAVA is producing audio frames
- **THEN** the daemon's spectrum reader thread delivers `DaemonEvent::Spectrum` messages at a rate of at least 50 fps (within 83% of the configured 60 fps CAVA rate)

#### Scenario: Idle fallback does not busy-wait
- **WHEN** no spectrum frames are available from CAVA
- **THEN** the reader thread SHALL sleep briefly (no more than 16ms) before retrying, rather than spinning in a tight loop

### Requirement: Latest-wins frame delivery is preserved
The frame delivery pipeline SHALL continue to apply latest-wins semantics: when the consumer drains frames with `take_latest_frame()`, only the most recent frame is delivered and any intermediate frames accumulated since the last drain are discarded.

#### Scenario: Intermediate frames are discarded
- **WHEN** the reader thread polls and drains multiple frames in rapid succession before the consumer calls `take_latest_frame()`
- **THEN** only the single most recent frame is returned; all prior frames are silently dropped
