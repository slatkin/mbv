## Context

The daemon spectrum feature starts a second Snapclient, writes its PCM output to a FIFO, runs CAVA on the daemon, and forwards normalized bars to a remote mbv client over the ctrl socket. The local visualizer instead runs CAVA against the local system-audio input. The remote path has materially different timing and lifecycle behavior, while adding daemon configuration, processes, protocol surface, and runtime dependencies for a non-essential display feature.

The attempted frame-rate tuning demonstrated that this is not a worthwhile path to optimize. The product will return to a single local-audio visualizer rather than introduce another audio transport or client-side reconstruction protocol.

## Goals / Non-Goals

**Goals:**
- Remove all daemon spectrum capture, CAVA, Snapclient, FIFO, and ctrl-streaming behavior.
- Keep the local CAVA visualizer and its existing appearance/lifecycle intact for local playback.
- Make daemon-connected clients clearly unavailable for visualization instead of silently starting remote audio plumbing.
- Remove only spectrum-specific configuration, dependencies, docs, tests, and protocol types; retain unrelated audio-pipe behavior.

**Non-Goals:**
- Streaming PCM or spectrum data from mbvd for reconstruction on the client.
- Replacing Snapcast or changing normal audible Snapclient deployment.
- Changing local CAVA rendering, bars, or visualizer appearance.
- Reworking unrelated daemon control protocol behavior.

## Decisions

### Remove the remote feature end-to-end

Delete the daemon-only `CavaInput::Fifo` path, `SpectrumSnapclient`, prerequisite probe, reader state, daemon events, and lifecycle handling. Keep the generic CAVA worker's system-input path because the local client still uses it.

This is preferable to retaining dormant process/configuration code: retaining it would leave an unsupported audio pipeline and invite future accidental activation.

### Remove the spectrum ctrl protocol surface

Remove the spectrum capability, `StartSpectrum`/`StopSpectrum` commands, spectrum/failure ctrl events, remote-player event conversion, and compatibility state. A daemon-connected UI will not make a visualizer request; local playback remains the only path that starts CAVA.

The alternative—leaving unused protocol variants for compatibility—would preserve public behavior that no longer has an implementation. Older peers that send removed commands must not cause daemon playback/control failure; normal unknown-command handling remains sufficient.

### Remove dedicated configuration and dependency requirements only

Remove the spectrum Snapserver host/port/client/FIFO settings and spectrum-specific docs. Remove `snapclient` as an mbvd spectrum runtime prerequisite and package recommendation where it exists solely for this feature. Preserve generic audio-pipe settings and any normal Snapclient deployment because they are outside the visualizer feature.

### Treat the previous frame-rate change as superseded

Remove the temporary frame-rate instrumentation and the reduced CAVA FIFO poll interval along with the daemon FIFO path. The incomplete `improve-daemon-spectrum-framerate` change is superseded by this removal and must not be implemented or deployed independently.

## Risks / Trade-offs

- **[Remote users lose a visualizer toggle]** → The daemon path was unreliable and unsupported; local playback retains the visualizer and the UI will make remote unavailability explicit.
- **[Existing daemon configuration contains spectrum keys]** → Verify config parsing tolerates obsolete keys or provide an actionable migration note; do not let them block daemon startup.
- **[A running daemon leaves child processes during upgrade]** → A normal service restart terminates the daemon and its children; verify no dedicated CAVA/Snapclient remains after deployment.
- **[Removing a shared audio setting by mistake]** → Scope deletion to dedicated spectrum fields and prove unrelated audio-pipe tests/configuration remain intact.

## Migration Plan

1. Deploy the new mbvd binary with a normal service restart, which terminates any active daemon spectrum children.
2. Remove obsolete dedicated spectrum configuration and `snapclient` installation requirements from daemon hosts when no other service uses them.
3. On remote mbv connections, the visualizer is unavailable; local playback continues to use local CAVA.
4. Roll back by reinstalling the prior binary only if remote spectrum must be temporarily restored; no state/data migration is required.

## Open Questions

- None. A future client-side visualization transport, if ever needed, requires a separate proposal with explicit audio synchronization and latency requirements.
