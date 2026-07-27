# Audio visualizer captures system audio, not mbv-only audio

The audio visualizer runs CAVA as a supervised child process while the embedded visualizer is enabled for local playback. CAVA uses its normal Pulse input selection without a named source, so it follows the system default monitor/source. The visualizer therefore represents system audio, not mbv-owned audio only, and may include unrelated applications.

mbv does not create or modify PulseAudio/PipeWire sinks, sources, links, loopbacks, or modules. It also does not change mpv's audio output properties. Starting or stopping the visualizer must leave playback configuration and audio graph state unchanged.

**Considered Options**

- Capture an mbv-only monitor through a dedicated sink or loopback: rejected because routing adds playback and shutdown risk without being required for an embedded spectrum.
- Depend on mpv visualization video filters: rejected because they render visualization as video inside mpv rather than exposing samples for mbv's TUI.
- Implement FFT or audio capture in Rust: rejected because CAVA already provides maintained capture and spectrum analysis.
