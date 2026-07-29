## Why

Daemon spectrum streaming currently starts CAVA with a PulseAudio input even though daemon audio follows an mpv PCM pipe through Snapserver and is rendered later by the host's existing Snapclient. The service cannot reliably access a user PulseAudio session, while capturing before Snapcast would make the visualizer lead audible playback by the Snapcast buffering latency.

## What Changes

- Replace daemon CAVA's PulseAudio input with a dedicated second Snapclient that outputs raw PCM to a FIFO file, which CAVA reads using `method = fifo`.
- The second Snapclient connects to Snapserver independently, uses a stable identity (`--hostID puffin-balls` by default, configurable), and a distinct instance number (`--instance 2`).
- The primary Snapclient remains untouched and continues outputting to the hardware DAC.
- Restrict daemon visualization startup to configured Snapcast/audio-pipe playback (`audio_pipe_enabled = true`).
- Continue advertising `spectrum-streaming` unconditionally; validate CAVA and Snapclient only when `StartSpectrum` is received and report asynchronous failure once through `SpectrumFailed`.
- Track spectrum subscriptions per control client: the first subscriber starts the shared CAVA worker, frames are sent only to subscribers, and the last unsubscribe or subscriber disconnect stops it.
- Latch daemon spectrum failure for one activation so periodic synchronization does not retry; only an explicit off/on toggle clears the client failure state.
- Preserve the existing mpv PCM pipe, Snapserver source, primary Snapclient process, hardware playback route, control-protocol messages, and local visualization behavior.
- Document the operator's responsibility to assign the visualizer Snapclient (`puffin-balls`) to the same stream as the primary client in Snapserver.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `daemon-spectrum-streaming`: Replace daemon PulseAudio capture with post-Snapcast FIFO capture from a dedicated second Snapclient, and define subscriber-safe multi-client lifecycle and failure behavior.

## Impact

- Affects daemon CAVA configuration, spectrum subscription state, lifecycle handling, client failure latching, ADR 0008, and deployment documentation.
- Requires CAVA with FIFO input support and a second Snapclient process (lightweight, ~10-20MB RSS).
- Adds no dependency, does not supervise or restart the primary Snapclient, and does not change the existing spectrum wire protocol.
- The operator must assign the visualizer Snapclient to the same stream as the primary client via Snapserver's web UI or configuration.
