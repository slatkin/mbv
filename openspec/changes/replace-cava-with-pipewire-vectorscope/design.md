## Context

See `proposal.md` for motivation and issue [#587](https://github.com/slatkin/mbv/issues/587) for tracking. The current core worker runs Cava against the default Pulse-compatible system input, reads 64 normalized spectrum bars from a private FIFO, and passes frames through a bounded channel. The app drains that channel and redraws the existing visualizer panel from the newest queued frame.

The replacement must retain system-output semantics because a same-host Local daemon owns playback in a separate process. mpv does not expose a passive PCM tap through mbv's supported integration, and changing mpv output or any daemon/control protocol is outside this change. The existing panel is at most 11 rows high, making a stereo vectorscope a useful single-mode visualization without FFT analysis.

ncmpcpp is GPL-2.0-or-later. It is a behavioral reference only; this change must use independently written Rust based on standard PCM coordinate mapping and the requirements in the delta spec.

## Goals / Non-Goals

**Goals:**

- Own capture, buffering, freshness, and rendering inside mbv.
- Keep the capture callback bounded and independent from TUI timing.
- Preserve the existing visualizer lifecycle conditions and panel placement, except that Direct remote playback may capture audio forwarded into the local system output.
- Make failure observable in logs but harmless to playback.
- Leave a raw stereo PCM boundary that later modes can consume without designing those modes now.

**Non-Goals:**

- Abstract audio backends or support PulseAudio, ALSA, macOS, or Windows.
- Add FFT, spectra, waveform modes, mode cycling, trails, decay, user-facing gain controls, or channel selection.
- Capture only mbv-owned playback or synchronize against downstream device latency; the monitor may include any locally audible forwarded audio.
- Change Local daemon startup, shared data, Stay-Alive behavior, or the ctrl protocol.
- Translate ncmpcpp source or reproduce its configuration surface.

## Decisions

### Use `pipewire-rs` directly on one worker thread

The core visualizer worker will own a PipeWire main loop and input stream on a dedicated thread. The stream will request interleaved stereo floating-point PCM and set PipeWire's capture-sink property so automatic connection selects the current default audio sink output. PipeWire may convert the sink's native format; mbv will not request `NO_CONVERT`, create graph objects, or name a particular device.

The process callback will only validate complete stereo pairs and copy them into the bounded sample buffer. It will not log, allocate per sample, render, or block on the UI. Stream state changes will communicate startup or terminal failure to the worker lifecycle. Stopping the worker will quit the main loop, disconnect the stream, join the thread, and release PipeWire objects.

This replaces Cava with a library/system dependency rather than another executable. Spawning `pw-cat` was rejected because it would retain child supervision and stream parsing. PulseAudio compatibility was rejected because this change intentionally has one PipeWire-native boundary.

### Capture the default sink selected at worker startup

The stream will auto-connect to the default system-output sink when visualization starts. A default-device change while the worker is active will not add metadata tracking or graph rebinding in this first version; toggling the visualizer or beginning the next playback creates a fresh stream against the then-current default.

Explicit node selection was rejected because it would introduce device configuration and persistence unrelated to the single-mode replacement.

Direct remote Player-owner playback does not imply that audio is inaudible locally: external forwarding such as Snapcast may feed the same machine's default sink. The app therefore permits capture for Direct remote playback while retaining the attached Emby Session and audio-pipe exclusions. No forwarding detection or configuration is added; an unforwarded remote playback path produces a silent local monitor.

### Share a bounded overwrite buffer, not frame messages

Capture and rendering will share a fixed-capacity circular buffer of interleaved stereo samples. Capture overwrites the oldest samples when full. Rendering snapshots the newest complete window and never consumes a queue of precomputed visual frames. Lock acquisition on the capture path must be non-blocking; if the renderer momentarily owns the buffer, the incoming block may be dropped rather than delaying PipeWire.

The first window will represent approximately 33 ms of audio. This supplies enough points for a stable figure while ensuring that successive 60 Hz renders substantially change the sample set. The exact capacity will be derived from the negotiated sample rate and bounded by a documented maximum rather than assuming 44.1 kHz.

A frame channel was rejected because the current `try_send` behavior retains old frames when full. An unbounded PCM queue was rejected because visualization may lose data without affecting playback.

### Render one independently defined stereo vectorscope

For every complete sample pair in the newest window, the renderer will apply a fixed internal 4x display gain, clamp each channel to `[-1.0, 1.0]`, map left amplitude around the panel's horizontal center, and map right amplitude around its vertical center. The gain is a rendering adjustment only: captured samples remain raw and no user-facing setting is added. Duplicate terminal coordinates need only be written once. Silence maps to the center; a fully silent window is treated as inactive so it does not display a permanent center point.

The renderer will clear the panel interior each frame and preserve the existing background palette. Point color is selected from the existing bright palette by amplified distance from center: aqua, foam, yellow, then red across four amplitude bands. This stable mapping avoids sample-order flicker while making signal intensity visible. It will not add persistence trails, interpolation, antialiasing, user-facing gain controls, or ncmpcpp's color-ring calculation.

### Persist one validated glyph

The existing UI configuration will gain one vectorscope glyph with default `●`. Parsing accepts exactly one non-control Unicode glyph occupying one terminal cell; invalid values fall back to the default and do not fail config loading. Only file-based configuration is required in this change; a text editor inside F2 settings would add a general text-input interaction for one value.

The two-character ncmpcpp `visualizer_look` contract was rejected because only the point glyph has meaning for this mode. A future filled mode may add its own setting without changing this one.

### Measure freshness at the render boundary

The active visualizer will target a 16 ms render interval. Each render snapshots current PCM immediately before drawing, so delayed iterations skip history rather than replay it.

Configured timers alone are not acceptance evidence. The old assumption that a `poll` timeout caps reads was incorrect because readiness wakes `poll` immediately; measurement must cover capture callback publication, app-loop wakeup, and terminal draw completion.

## Risks / Trade-offs

- [PipeWire development libraries increase build requirements] -> Add the binding and system packages explicitly to CI and packaging, and remove Cava from the same surfaces.
- [PipeWire or the default sink is unavailable] -> Fail the visualizer worker once for that playback, log the stream diagnostic, and leave playback active.
- [The default sink changes while capture is active] -> Keep the current stream; restart capture on the next toggle or playback rather than adding metadata tracking.
- [A capture callback races the renderer] -> Use bounded non-blocking publication and drop visualization samples instead of blocking either side.
- [A wide, shallow terminal distorts the vectorscope] -> Scale independently to the existing panel dimensions; the ellipse is intentionally terminal-cell shaped rather than geometrically circular.
- [Whole-TUI drawing cannot sustain 50 fresh FPS on a terminal] -> Keep newest-sample semantics so degradation drops frames without accumulating latency; acceptance applies only where the terminal can sustain the target.
- [GPL implementation details leak into the port] -> Implement only behavior specified here, use project-native names and structure, and do not copy or translate ncmpcpp code, comments, constants, or control flow.

## Migration Plan

1. Introduce the PipeWire capture and vectorscope path while retaining the current enable preference and panel lifecycle.
2. Replace Cava-specific state, tests, logs, and documentation after the new path satisfies lifecycle and freshness checks.
3. Remove Cava from package recommendations and CI; add the required PipeWire build/runtime packages.
4. Verify bare playback, same-host Local daemon playback, remote/attached playback, audio-pipe exclusion, capture failure, toggle-off, and application shutdown.
5. Roll back by reinstalling the previous mbv release and its optional Cava dependency. No persisted data migration is required; older versions ignore the new UI glyph key.
