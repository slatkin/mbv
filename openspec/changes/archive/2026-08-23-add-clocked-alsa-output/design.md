## Context

See `proposal.md` for motivation. In the measured packaged-`mbvd` path, a Play request reached the pipe startup boundary after about two seconds and Pause reached mpv's paused state after about 2.2 seconds; Snapserver then added about 0.5 seconds. mpv's `ao=pcm` is an untimed file writer that can block while writing to a bounded FIFO, so it is not a real-time audio-device boundary. A standalone direct-ALSA test opened the physical ALSA endpoint and acknowledged a pause command in 0.5 ms, but audible cutoff latency was not measured.

The live deployment is a Debian LXC guest. Its current kernel does not expose `snd-aloop`, so a loopback deployment requires the Proxmox host to load the module and expose the relevant `/dev/snd` nodes. That host work and the Snapserver source configuration are outside mbv. ADR 0008 also requires mbv to leave the external audio graph unchanged, and the visualizer must not change mpv output properties.

The existing `audio_pipe_enabled` setting already identifies the pipe path and is part of the active `manage-mbvd-settings` change. Existing pipe format and latency settings are persisted configuration and must remain valid.

## Goals / Non-Goals

**Goals:**
- Give packaged `mbvd` an explicit, hardware-paced ALSA output boundary by default.
- Preserve the current FIFO implementation behind one explicit legacy switch.
- Keep output selection stable for the lifetime of each Playback run.
- Produce diagnostics and tests at mbv's observable Player boundary without claiming audibility.

**Non-Goals:**
- Provisioning ALSA devices, `snd-aloop`, LXC device mappings, or permissions.
- Managing Snapserver, Snapclient, their sample format, or their buffering.
- Changing bare-mode or Local-daemon inherited audio output.
- Tuning mpv input caching, timestamps, or the downstream Snapcast buffer.

## Decisions

### Reuse the existing pipe switch

`audio_pipe_enabled = true` remains the only selector for legacy pipe output. False or absent selects the packaged daemon's clocked device path. This avoids a new output-mode enum, migration aliases, and changes to the active daemon-settings protocol work.

An `audio_device` owner-local setting carries mpv's complete ALSA device identifier. It accepts `alsa` for the default ALSA endpoint or `alsa/<device>` for an exact endpoint. The inherited packaged-daemon value is `alsa`. The setting is restart-required and deliberately not added to the `manage-mbvd-settings` allowlist: audio hardware routing is operator-owned configuration, while the existing remotely managed `audio_pipe_enabled` switch can still select legacy behavior at the next Playback run.

Alternative considered: replace all pipe fields with a generic output-mode structure. Rejected because one boolean already expresses the required compatibility boundary and persisted pipe settings have external users.

### Select ALSA through mpv's audio-device property

For clocked output, project the resolved `audio_device` to mpv and do not also force an `ao` value. The device identifier itself selects ALSA. Do not set `ao=pcm`, `ao-pcm-file`, or pipe-only startup state in this branch. Existing mpv/ALSA negotiation remains responsible for sample format; the existing `audio_pipe_samplerate` and `audio_pipe_bitdepth` remain pipe-only.

Alternative considered: reuse the pipe sample-rate and bit-depth settings for ALSA. Rejected because their persisted meaning is explicitly pipe-specific and ALSA device negotiation is a separate boundary.

### Capture output configuration per Playback run

Load the ALSA identifier with owner-local configuration at daemon startup. Resolve the current pipe switch before constructing each Playback run, then keep the resulting output choice immutable for the run. The pipe branch continues through its existing FIFO creation, mpv projection, startup guard, latency estimate, and logging. The ALSA branch bypasses all of those pipe-only operations. A configured ALSA endpoint failure is terminal for that run; automatic fallback could unexpectedly write to a physical device or revive the latency problem.

Alternative considered: hot-reload `audio_device` or change it on the active mpv instance. Rejected because hardware routing is restart-required operator configuration and output replacement has failure and buffering transitions unnecessary for this change.

### Measure the Player boundary, not audibility

Regression coverage will prove configuration projection, branch isolation, and command/event ordering without wall-clock assertions in ordinary CI. A deployment acceptance check will use synchronized monotonic logs to compare request acceptance with observed startup and pause transitions on a real writable ALSA endpoint. Downstream capture, encoding, network buffering, and hardware playout remain separate measurements.

Alternative considered: specify end-to-end audible latency. Rejected because mbv neither observes nor controls the downstream playout boundary.

### Document rather than administer the external route

Operator documentation will show the topology `mbvd/libmpv -> ALSA playback endpoint -> paired ALSA capture endpoint -> Snapserver` and state that loopback creation, container exposure, permissions, and downstream format/buffer settings are prerequisites. mbv performs no probing or lifecycle management of those components.

## Risks / Trade-offs

- [The configured ALSA device is absent, busy, or not exposed to an LXC guest] -> Fail the Playback run with the selected endpoint in the diagnostic and no silent fallback.
- [ALSA negotiation and the downstream capture format disagree] -> Keep format adaptation in operator-owned ALSA/Snapserver configuration and document a verified deployment example.
- [Users mistake a fast Player transition for audible completion] -> Preserve boundary-specific wording and report downstream latency separately.
- [The active `manage-mbvd-settings` change diverges] -> Retain its exact pipe setting semantics and keep `audio_device` outside its allowlist.
- [A wall-clock latency test is flaky in CI] -> Test deterministic branch/event behavior in CI and reserve elapsed-time acceptance for a real ALSA endpoint.

## Migration Plan

1. Add the packaged-daemon `audio_device` configuration and ship the inherited `alsa` value while retaining all existing pipe keys.
2. For a Snapcast deployment, provision and expose the ALSA playback/capture pair outside mbv, then configure Snapserver to consume the capture endpoint.
3. Set `audio_device` to the packaged daemon's ALSA playback endpoint, disable `audio_pipe_enabled`, and restart packaged `mbvd` to load the device.
4. Validate startup, pause, and resume at the Player boundary, then validate downstream audibility separately. Tune downstream buffering only after the route is stable.
5. Roll back by setting `audio_pipe_enabled = true`, restoring the downstream pipe source, and restarting packaged `mbvd`. Existing pipe path, format, and latency settings remain usable.
