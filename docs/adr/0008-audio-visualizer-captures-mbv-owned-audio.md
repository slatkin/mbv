# Audio visualizer follows the audible Snapcast playout

The local audio visualizer runs CAVA as a supervised child process and keeps its existing system-audio input. Daemon playback uses a separate post-Snapcast capture path so the visualizer follows the audible playout timeline rather than leading it from mpv's pre-Snapcast PCM pipe.

mbv does not create or modify PulseAudio/PipeWire sinks, sources, links, loopbacks, or modules. It also does not change mpv's audio output properties. Starting or stopping the visualizer must leave playback configuration and audio graph state unchanged.

## Daemon spectrum streaming

When `mbvd` runs headless with `audio_pipe_enabled = true`, the daemon starts a dedicated second Snapclient and CAVA and streams spectrum frames over the control protocol. The flow is:

1. The client sends `StartSpectrum` when the user enables the visualizer during daemon playback (guarded by the `spectrum-streaming` capability advertised in the daemon's control hello).
2. The daemon creates `/tmp/mbv-spectrum.fifo` (configurable), starts Snapclient with host ID `puffin-balls` and instance `2` in `-o file` mode, then starts CAVA with `method = fifo` reading that path. The primary Snapclient and mpv/Snapserver pipe are not touched.
3. Spectrum frames are broadcast as `CtrlEvent::Spectrum` to the connected client, which writes them to `visualizer_frame` for rendering.
4. The client sends `StopSpectrum` on toggle-off, session switch, teardown, or connection-type transition.
5. The daemon stops the CAVA worker and joins the reader thread on `StopSpectrum`, `CtrlDisconnected` (client crash/disconnect), `PlayerEvent::Stopped` (playback ends), or `DaemonEvent::Shutdown`.

CAVA and Snapclient are runtime dependencies for daemon visualization. The daemon advertises `spectrum-streaming` only when the audio-pipe mode and both executables are available, and repeats the check at `StartSpectrum`. Child stderr is captured for actionable startup failures; either child can fail without interrupting primary playback.

The dedicated Snapclient identity must be assigned to the same Snapserver stream as the audible client. Configure `spectrum_snapserver_host`, `spectrum_snapserver_port`, `spectrum_snapclient_host_id`, `spectrum_snapclient_instance`, and `spectrum_fifo_path` under `[mbvd]`. In Snapserver's web UI, verify that `puffin-balls` appears and assign it to the audible stream (or make the equivalent group assignment in Snapserver configuration). This assignment is intentionally operator-managed and persists by client identity.

**Considered Options**

- Capture through an ALSA loopback or multi device: rejected because a slow or failed visualization path can propagate XRUNs into the primary DAC.
- Read the mpv PCM pipe directly: rejected because FIFO readers split bytes and because the source precedes Snapcast buffering, making bars lead audible output.
- PulseAudio/PipeWire capture from the daemon service: rejected because a root system service has no reliable user audio session.
- Depend on mpv visualization video filters: rejected because they render visualization as video inside mpv rather than exposing samples for mbv's TUI.
- Implement FFT or audio capture in Rust: rejected because CAVA already provides maintained capture and spectrum analysis.
- Run CAVA on the client side for daemon playback: rejected because the daemon runs headless with audio access; streaming frames avoids requiring CAVA on every thin-client host.
