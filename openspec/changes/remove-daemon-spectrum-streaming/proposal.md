## Why

Daemon spectrum streaming routes audio through Snapserver, a managed Snapclient, a FIFO, CAVA, and the control socket before it reaches the UI. That path produces visibly different cadence from CAVA's direct local-audio input, adds runtime dependencies and lifecycle complexity, and is not worth maintaining for a visual-only feature.

## What Changes

- **BREAKING** Remove daemon-managed spectrum capture, CAVA processing, and spectrum-frame delivery over the ctrl protocol.
- Remove the dedicated spectrum Snapclient/FIFO configuration, prerequisite checks, lifecycle state, and daemon runtime dependency on `snapclient`.
- Remove remote spectrum capability negotiation, commands, events, and client-side remote-spectrum handling.
- Preserve the existing local CAVA visualizer for local playback; daemon-connected clients will not offer a daemon spectrum visualizer.
- Remove daemon-spectrum setup documentation while retaining local CAVA runtime documentation.

## Capabilities

### New Capabilities
- `local-audio-visualizer`: Provides CAVA visualization only from the local application's system-audio input and explicitly excludes daemon audio streaming.

### Modified Capabilities
- None.

## Impact

- Affects `mbv-core` visualizer, daemon, ctrl protocol, remote-player compatibility, configuration, UI visualizer branching, tests, and README/package metadata.
- Removes the daemon's `cava`/`snapclient` spectrum prerequisites and dedicated spectrum configuration fields; local CAVA remains a runtime dependency.
