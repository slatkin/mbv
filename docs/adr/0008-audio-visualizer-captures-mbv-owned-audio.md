# Audio visualizer follows system audio

The audio visualizer runs CAVA as a supervised child process while the embedded visualizer is enabled for local playback. CAVA uses its normal Pulse input selection without a named source, so it follows the system default monitor/source. The visualizer therefore represents system audio, not mbv-owned audio only, and may include unrelated applications.

mbv does not create or modify PulseAudio/PipeWire sinks, sources, links, loopbacks, or modules. It also does not change mpv's audio output properties. Starting or stopping the visualizer must leave playback configuration and audio graph state unchanged.

## Daemon spectrum streaming

When `mbvd` runs headless, the daemon spawns CAVA and streams spectrum frames over the control protocol to connected `mbv` clients. The flow:

1. The client sends `StartSpectrum` when the user enables the visualizer during daemon playback (guarded by the `spectrum-streaming` capability advertised in the daemon's control hello).
2. The daemon spawns a `CavaWorker` and a reader thread that polls `take_latest_frame()` and sends `DaemonEvent::Spectrum` to the main loop.
3. Spectrum frames are broadcast as `CtrlEvent::Spectrum` to the connected client, which writes them to `visualizer_frame` for rendering.
4. The client sends `StopSpectrum` on toggle-off, session switch, teardown, or connection-type transition.
5. The daemon stops the CAVA worker and joins the reader thread on `StopSpectrum`, `CtrlDisconnected` (client crash/disconnect), `PlayerEvent::Stopped` (playback ends), or `DaemonEvent::Shutdown`.

CAVA is a runtime dependency for `mbvd` environments that use the visualizer. Without it, `CavaWorker::start` returns an error and the client receives `SpectrumFailed`.

**Considered Options**

- Capture an mbv-only monitor through a dedicated sink or loopback: rejected because routing adds playback and shutdown risk without being required for an embedded spectrum.
- Depend on mpv visualization video filters: rejected because they render visualization as video inside mpv rather than exposing samples for mbv's TUI.
- Implement FFT or audio capture in Rust: rejected because CAVA already provides maintained capture and spectrum analysis.
- Run CAVA on the client side for daemon playback: rejected because the daemon runs headless with audio access; streaming frames avoids requiring CAVA on every thin-client host.
