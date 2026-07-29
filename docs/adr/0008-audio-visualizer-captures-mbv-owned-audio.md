# Audio visualizer follows the audible Snapcast playout

> **Superseded (2026-07-29):** The daemon spectrum streaming path described below
> has been removed by the `remove-daemon-spectrum-streaming` change. The local
> CAVA visualizer for local playback is retained; daemon-connected clients no
> longer offer a daemon-backed visualizer. See the `remove-daemon-spectrum-streaming`
> change's `specs/local-audio-visualizer/spec.md` for the current behavior.

The local audio visualizer runs CAVA as a supervised child process and keeps its existing system-audio input. Daemon playback uses a separate post-Snapcast capture path so the visualizer follows the audible playout timeline rather than leading it from mpv's pre-Snapcast PCM pipe.

mbv does not create or modify PulseAudio/PipeWire sinks, sources, links, loopbacks, or modules. It also does not change mpv's audio output properties. Starting or stopping the visualizer must leave playback configuration and audio graph state unchanged.

## Daemon spectrum streaming (removed)

**This section is preserved for historical context only.** The daemon spectrum feature was removed by `remove-daemon-spectrum-streaming` because it produced visibly different cadence from CAVA's direct local-audio input, added runtime dependencies and lifecycle complexity, and was not worth maintaining for a visual-only feature.

When `mbvd` ran headless with `audio_pipe_enabled = true`, the daemon started a dedicated second Snapclient and CAVA and streamed spectrum frames over the control protocol. The flow was:

1. The client sent `StartSpectrum` when the user enabled the visualizer during daemon playback.
2. The daemon created a FIFO, started Snapclient and CAVA, then forwarded frames.
3. Spectrum frames were broadcast as `CtrlEvent::Spectrum` to the connected client.
4. The client sent `StopSpectrum` on toggle-off, session switch, teardown, or connection-type transition.
5. The daemon stopped the CAVA worker and joined the reader thread on stop, disconnect, playback end, or shutdown.

**Considered Options**

- Capture through an ALSA loopback or multi device: rejected because a slow or failed visualization path can propagate XRUNs into the primary DAC.
- Read the mpv PCM pipe directly: rejected because FIFO readers split bytes and because the source precedes Snapcast buffering, making bars lead audible output.
- PulseAudio/PipeWire capture from the daemon service: rejected because a root system service has no reliable user audio session.
- Depend on mpv visualization video filters: rejected because they render visualization as video inside mpv rather than exposing samples for mbv's TUI.
- Implement FFT or audio capture in Rust: rejected because CAVA already provides maintained capture and spectrum analysis.
- Run CAVA on the client side for daemon playback: rejected because the daemon runs headless with audio access; streaming frames avoids requiring CAVA on every thin-client host.
